use std::{path::Path, sync::Arc};

use anyhow::Result;
use yazi_config::YAZI;
use yazi_core::tab::Folder;
use yazi_fs::{FolderStage, IgnoreFilter};
use yazi_macro::{act, render, render_and, succ};
use yazi_parser::VoidOpt;
use yazi_shared::{data::Data, url::UrlLike};

use crate::{Actor, Ctx};

pub struct Ignore;

impl Actor for Ignore {
	type Options = VoidOpt;

	const NAME: &str = "ignore";

	fn act(cx: &mut Ctx, _: Self::Options) -> Result<Data> {
		let cwd = cx.cwd();
		let cwd_str = if cwd.is_search() {
			"search://**".to_string()
		} else {
			cwd.loc().as_os().ok().map(|p| p.display().to_string()).unwrap_or_default()
		};

		let exclude_patterns = YAZI.files.excludes_for_context(&cwd_str);

		// Check if we're inside an excluded directory
		// If so, don't apply filters to allow viewing excluded directory contents
		if let Some(cwd_path) = cwd.loc().as_os().ok() {
			for pattern in &exclude_patterns {
				if pattern.starts_with('!') {
					continue;
				}
				if let Some(name) = cwd_path.file_name().and_then(|n| n.to_str()) {
					let clean_pattern = pattern.trim_end_matches("/**").trim_start_matches("**/");
					if name == clean_pattern || pattern == name {
						succ!();
					}
				}
			}
		}

		let hovered = cx.hovered().map(|f| f.urn().to_owned());
		let apply = |f: &mut Folder, context: &str| {
			let changed = Ignore::apply_filter_for_context(f, context);
			if f.stage == FolderStage::Loading {
				render!();
				false
			} else {
				render_and!(changed && f.files.catchup_revision())
			}
		};

		let cwd_changed = apply(cx.current_mut(), &cwd_str);

		let parent_changed = if let Some(p) = cx.parent_mut() {
			let parent_str = if p.url.is_search() {
				"search://**".to_string()
			} else {
				p.url.loc().as_os().ok().map(|p| p.display().to_string()).unwrap_or_default()
			};
			apply(p, &parent_str)
		} else {
			false
		};

		if cwd_changed || parent_changed {
			act!(mgr:hover, cx)?;
			act!(mgr:update_paged, cx)?;
		}

		if let Some(h) = cx.hovered_folder_mut() {
			let hovered_str = if h.url.is_search() {
				"search://**".to_string()
			} else {
				h.url.loc().as_os().ok().map(|p| p.display().to_string()).unwrap_or_default()
			};
			if apply(h, &hovered_str) {
				render!(h.repos(None));
				act!(mgr:peek, cx, true)?;
			} else if cx.hovered().map(|f| f.urn()) != hovered.as_ref().map(Into::into) {
				act!(mgr:peek, cx)?;
				act!(mgr:watch, cx)?;
			}
		}

		succ!();
	}
}

impl Ignore {
	/// Build and apply an IgnoreFilter for a folder using the given context string.
	/// Returns true if the filter changed and files need a revision catchup.
	pub(super) fn apply_filter_for_context(folder: &mut Folder, context: &str) -> bool {
		let excludes = YAZI.files.excludes_for_context(context);
		let matcher: Option<Arc<dyn Fn(&Path) -> Option<bool> + Send + Sync>> =
			if !excludes.is_empty() {
				let ctx = context.to_string();
				Some(Arc::new(move |path: &Path| YAZI.files.matches_path(path, &ctx)))
			} else {
				None
			};
		folder.files.set_ignore_filter(IgnoreFilter::from_patterns(matcher))
	}
}
