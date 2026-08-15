use std::sync::Arc;

use yazi_shared::url::{AsUrl, Url};

use crate::file::File;

/// The rules live in `yazi-config`, which this crate can't depend on, so the
/// folder they were resolved for hands the lookup over as a closure.
pub type ExcludeMatcher = Arc<dyn for<'a> Fn(Url<'a>, bool) -> Option<bool> + Send + Sync>;

#[derive(Clone)]
pub struct ExcludeFilter {
	matcher: ExcludeMatcher,
}

impl std::fmt::Debug for ExcludeFilter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ExcludeFilter").finish_non_exhaustive()
	}
}

impl ExcludeFilter {
	pub fn new(matcher: ExcludeMatcher) -> Self { Self { matcher } }

	pub fn matches(&self, file: &File) -> bool {
		(self.matcher)(file.url.as_url(), file.is_dir()).unwrap_or(false)
	}
}
