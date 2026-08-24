use std::{num::NonZeroUsize, path::PathBuf};

use anyhow::{Result, bail};
use futures::{StreamExt, stream};
use hashbrown::HashMap;

use super::{Dependency, Git};
use crate::shared::must_exists;

/// Brings every repository behind a batch of dependencies up to date, several
/// at a time, so that the round trips overlap instead of queueing up behind the
/// package-by-package work that follows.
///
/// Dependencies sharing a repository, such as the children of a monorepo, fetch
/// it once. Whatever fails is remembered rather than raised, since a repository
/// nobody could reach only concerns the dependencies that wanted it.
pub(super) struct Fetcher(HashMap<PathBuf, String>);

impl Fetcher {
	pub(super) async fn run<'a>(
		deps: impl IntoIterator<Item = &'a Dependency>,
		jobs: NonZeroUsize,
	) -> Self {
		let repos: HashMap<_, _> = deps.into_iter().map(|d| (d.local(), d.remote())).collect();

		Self(
			stream::iter(repos)
				.map(async |(path, remote)| {
					let r = if must_exists(&path).await {
						Git::fetch(&path).await
					} else {
						Git::clone(&remote, &path).await
					};
					r.err().map(|e| (path, format!("{e:#}")))
				})
				.buffer_unordered(jobs.get())
				.filter_map(async |e| e)
				.collect()
				.await,
		)
	}

	/// Fails with whatever kept this dependency's repository from being fetched.
	pub(super) fn check(&self, dep: &Dependency) -> Result<()> {
		match self.0.get(&dep.local()) {
			Some(e) => bail!("{e}"),
			None => Ok(()),
		}
	}
}
