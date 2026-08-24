use anyhow::{Context, Result};
use yazi_fs::{engine::{Engine, local::Local}, ok_or_not_found};

use super::{Dependency, Report};
use crate::shared::{maybe_exists, remove_sealed};

impl Dependency {
	/// Returns whether the package was there to delete.
	pub(super) async fn delete(&self, discard: bool) -> Result<bool> {
		let dir = self.target();
		if !maybe_exists(&dir).await {
			return Ok(false);
		} else if !discard {
			self.hash_check().await?;
		}

		self.delete_assets().await?;
		if !self.delete_sources().await? {
			Report::noop(
				"Preserved",
				format_args!("user data in {}, delete it manually", dir.display()),
			)?;
		}

		Ok(true)
	}

	pub(super) async fn delete_assets(&self) -> Result<()> {
		let assets = self.target().join("assets");
		match tokio::fs::read_dir(&assets).await {
			Ok(mut it) => {
				while let Some(dent) = it.next_entry().await? {
					remove_sealed(&dent.path())
						.await
						.with_context(|| format!("failed to remove `{}`", dent.path().display()))?;
				}
			}
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
			Err(e) => Err(e).context(format!("failed to read `{}`", assets.display()))?,
		};

		Local::regular(&assets).remove_dir_clean().await;
		Ok(())
	}

	pub(super) async fn delete_sources(&self) -> Result<bool> {
		let dir = self.target();
		let files =
			if self.is_flavor { Self::flavor_files() } else { Self::plugin_files(&dir).await? };

		for path in files.iter().map(|s| dir.join(s)) {
			ok_or_not_found(remove_sealed(&path).await)
				.with_context(|| format!("failed to delete `{}`", path.display()))?;
		}

		Ok(ok_or_not_found(Local::regular(&dir).remove_dir().await).is_ok())
	}
}
