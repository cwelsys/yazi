use anyhow::Result;

use super::{Dependency, Git};
use crate::shared::must_exists;

impl Dependency {
	/// Returns whether the package was newly deployed, rather than already in
	/// place.
	pub(super) async fn add(&mut self, discard: bool) -> Result<bool> {
		let path = self.local();
		if must_exists(&path).await {
			Git::pull(&path).await?;
		} else {
			Git::clone(&self.remote(), &path).await?;
		};

		let fresh = self.deploy(discard).await?;
		self.rev = Git::revision(&path).await?;
		Ok(fresh)
	}
}
