use anyhow::Result;

use super::Dependency;

impl Dependency {
	/// Returns whether the package was newly deployed, rather than already in
	/// place.
	pub(super) async fn upgrade(&mut self, discard: bool) -> Result<bool> {
		if self.rev.starts_with('=') { Ok(false) } else { self.add(discard).await }
	}
}
