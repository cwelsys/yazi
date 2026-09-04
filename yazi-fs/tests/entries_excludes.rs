use std::{str::FromStr, sync::Arc};

use hashbrown::HashMap;
use yazi_fs::{Entries, ExcludeFilter, file::File};
use yazi_shared::url::{UrlBuf, UrlLike};

fn init() {
	static ONCE: std::sync::Once = std::sync::Once::new();
	ONCE.call_once(yazi_shared::init);
}

fn file(name: &str) -> File {
	File::from_dummy(UrlBuf::from_str(&format!("/p/{name}")).expect("url"), None)
}

/// Hides anything whose last path segment is `secret`.
fn hiding_secret() -> ExcludeFilter {
	ExcludeFilter::new(Arc::new(|url, _is_dir| {
		Some(url.loc().to_string_lossy().ends_with("secret")).filter(|&b| b)
	}))
}

fn visible(entries: &Entries) -> Vec<String> {
	entries.iter().map(|f| f.url.loc().to_string_lossy().into_owned()).collect()
}

fn loaded() -> Entries {
	let mut entries = Entries::new(true);
	entries.set_excludes(Some(hiding_secret()));
	entries.update_full(vec![file("a"), file("secret")]);
	entries
}

#[test]
fn a_full_load_conceals_the_excluded_entry() {
	init();
	assert_eq!(visible(&loaded()), ["/p/a"]);
}

#[test]
fn an_in_place_update_does_not_resurface_it() {
	init();
	let mut entries = loaded();
	let mut files = HashMap::new();
	files.insert(file("secret").key().to_owned(), file("secret"));
	entries.update_upserting(files);

	assert_eq!(visible(&entries), ["/p/a"]);
}

#[test]
fn an_in_place_update_still_lands_for_a_visible_entry() {
	init();
	let mut entries = loaded();

	let mut files = HashMap::new();
	files.insert(file("b").key().to_owned(), file("b"));
	entries.update_upserting(files);

	assert_eq!(visible(&entries), ["/p/a", "/p/b"]);
}

#[test]
fn toggling_show_excluded_reveals_and_conceals_again() {
	init();
	let mut entries = loaded();

	entries.set_show_excluded(true);
	let mut shown = visible(&entries);
	shown.sort();
	assert_eq!(shown, ["/p/a", "/p/secret"]);

	entries.set_show_excluded(false);
	assert_eq!(visible(&entries), ["/p/a"]);
}

#[test]
fn an_update_while_revealed_keeps_the_entry_visible() {
	init();
	let mut entries = loaded();
	entries.set_show_excluded(true);

	let mut files = HashMap::new();
	files.insert(file("secret").key().to_owned(), file("secret"));
	entries.update_upserting(files);

	let mut shown = visible(&entries);
	shown.sort();
	assert_eq!(shown, ["/p/a", "/p/secret"]);
}
