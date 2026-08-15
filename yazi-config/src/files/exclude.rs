use std::str::FromStr;

use anyhow::Result;
use serde::{Deserialize, Deserializer};
use yazi_shared::url::{AsUrl, Url};

use crate::Pattern;

#[derive(Debug, Clone)]
pub struct Exclude {
	/// The folders this rule applies in, matched against the folder's own URL.
	pub r#in: Pattern,

	ignores:            Vec<Pattern>,
	whitelists:         Vec<Pattern>,
	/// `<glob>/**` counterparts of the above. Kept apart because they may only
	/// match in a flattened listing: applying them while browsing inside an
	/// excluded directory would hide every entry and leave the folder empty.
	ignore_contents:    Vec<Pattern>,
	whitelist_contents: Vec<Pattern>,
}

impl<'de> Deserialize<'de> for Exclude {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[derive(Deserialize)]
		struct Shadow {
			url:  Globs,
			r#in: String,
		}

		#[derive(Deserialize)]
		#[serde(untagged)]
		enum Globs {
			One(String),
			Many(Vec<String>),
		}

		use serde::de::Error;
		let shadow = Shadow::deserialize(deserializer)?;
		let globs = match shadow.url {
			Globs::One(s) => vec![s],
			Globs::Many(v) => v,
		};

		let mut me = Self {
			r#in:               Pattern::from_str(&shadow.r#in).map_err(D::Error::custom)?,
			ignores:            vec![],
			whitelists:         vec![],
			ignore_contents:    vec![],
			whitelist_contents: vec![],
		};

		for glob in globs {
			let (negated, raw) = match glob.strip_prefix('!') {
				Some(rest) => (true, rest),
				None => (false, glob.as_str()),
			};

			let (selves, contents) = if negated {
				(&mut me.whitelists, &mut me.whitelist_contents)
			} else {
				(&mut me.ignores, &mut me.ignore_contents)
			};

			selves.push(Pattern::from_str(raw).map_err(D::Error::custom)?);
			if let Some(c) = contents_glob(raw) {
				contents.push(Pattern::from_str(&c).map_err(D::Error::custom)?);
			}
		}

		Ok(me)
	}
}

/// The `<glob>/**` counterpart of `raw`, or `None` if it already reaches
/// descendants. `Pattern` wants the case-sensitivity marker to stay at the very
/// front, so it's lifted off before the suffix goes on.
fn contents_glob(raw: &str) -> Option<String> {
	let (mark, body) = match raw.strip_prefix(r"\s") {
		Some(rest) => (r"\s", rest),
		None => ("", raw),
	};

	let body = body.trim_end_matches('/');
	if body.ends_with("/**") { None } else { Some(format!("{mark}{body}/**")) }
}

/// Whether `pattern` covers `url`, disregarding the trailing-slash marker.
///
/// The marker distinguishes a file from a directory, which only means something
/// for a pattern aimed at an entry. A folder scope is a directory by definition
/// and a generated `/**` glob matches descendants of either kind, so for those
/// the marker is fed back to take the check out of play.
fn covers(pattern: &Pattern, url: Url<'_>) -> bool { pattern.match_url(url, pattern.is_dir) }

impl Exclude {
	/// `Some(true)` to exclude, `Some(false)` to whitelist, `None` for no match.
	/// Rules apply in order and the last match wins, so `rules` must already be
	/// narrowed to the folder being listed and still be in declaration order.
	pub fn verdict(rules: &[&Self], url: impl AsUrl, is_dir: bool, recursive: bool) -> Option<bool> {
		let url = url.as_url();
		rules.iter().fold(None, |acc, r| r.matches(url, is_dir, recursive).or(acc))
	}

	pub fn matches_context(&self, url: impl AsUrl) -> bool { covers(&self.r#in, url.as_url()) }

	/// `recursive` admits the contents globs, and belongs only to flattened
	/// listings — the sole kind holding entries from below the folder itself.
	pub fn matches(&self, url: impl AsUrl, is_dir: bool, recursive: bool) -> Option<bool> {
		let url = url.as_url();
		let any = |ps: &[Pattern]| ps.iter().any(|p| p.match_url(url, is_dir));
		let any_below = |ps: &[Pattern]| recursive && ps.iter().any(|p| covers(p, url));

		if any(&self.whitelists) || any_below(&self.whitelist_contents) {
			return Some(false);
		}
		if any(&self.ignores) || any_below(&self.ignore_contents) {
			return Some(true);
		}
		None
	}
}
