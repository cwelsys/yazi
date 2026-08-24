use anyhow::Result;

use super::{Dependency, Git};

impl Dependency {
	/// Moves the package onto the latest revision and deploys it. The repository
	/// must already have been brought up to date by a [`super::Fetcher`].
	///
	/// Returns whether the package was newly deployed, rather than already in
	/// place.
	pub(super) async fn add(&mut self, discard: bool) -> Result<bool> {
		let path = self.local();
		Git::checkout(&path, "origin/HEAD").await?;

		let fresh = self.deploy(discard).await?;
		self.rev = Git::revision(&path).await?;
		Ok(fresh)
	}
}
