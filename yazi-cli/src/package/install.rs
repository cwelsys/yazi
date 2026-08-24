use anyhow::Result;

use super::{Dependency, Git};

impl Dependency {
	/// Deploys the package at its pinned revision, or at whatever the repository
	/// already sits on when unpinned. The repository must already have been
	/// brought up to date by a [`super::Fetcher`].
	///
	/// Returns whether the package was newly deployed, rather than already in
	/// place.
	pub(super) async fn install(&mut self, discard: bool) -> Result<bool> {
		let path = self.local();
		if !self.rev.is_empty() {
			Git::checkout(&path, self.rev.trim_start_matches('=')).await?;
		}

		let fresh = self.deploy(discard).await?;
		if self.rev.is_empty() {
			self.rev = Git::revision(&path).await?;
		}

		Ok(fresh)
	}
}
