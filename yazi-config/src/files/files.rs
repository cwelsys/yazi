use serde::Deserialize;
use yazi_codegen::DeserializeOver2;
use yazi_shared::url::AsUrl;
use yazi_shim::toml::DeserializeOverHook;

use super::Exclude;

#[derive(Debug, Deserialize, DeserializeOver2, Default)]
pub struct Files {
	#[serde(default)]
	pub excludes: Vec<Exclude>,
}

impl DeserializeOverHook for Files {}

impl Files {
	/// The rules that apply in the folder at `url`, in declaration order.
	pub fn excludes_in(&self, url: impl AsUrl) -> Vec<&Exclude> {
		let url = url.as_url();
		self.excludes.iter().filter(|e| e.matches_context(url)).collect()
	}

	/// Resolves the applicable rules on every call; a caller walking a whole
	/// folder should hoist [`Self::excludes_in`] out of its loop instead.
	pub fn matches(&self, url: impl AsUrl, is_dir: bool, context: impl AsUrl) -> Option<bool> {
		let context = context.as_url();
		Exclude::verdict(&self.excludes_in(context), url, is_dir, context.is_search())
	}
}
