//! Spike verification for the `Pattern`-based `Exclude`. Covers every case the
//! old hand-rolled implementation had a test for, plus the semantics that only
//! `Pattern` brings (dir marker, case-insensitivity, schemes).

use std::str::FromStr;

use yazi_config::files::Files;
use yazi_shared::url::UrlBuf;

fn files(toml: &str) -> Files { toml::from_str(toml).expect("parse") }

fn url(s: &str) -> UrlBuf { UrlBuf::from_str(s).expect("url") }

/// `Some(true)` excluded, `Some(false)` whitelisted, `None` no match.
fn check(f: &Files, entry: &str, is_dir: bool, context: &str) -> Option<bool> {
	f.matches(&url(entry), is_dir, &url(context))
}

fn init() {
	static ONCE: std::sync::Once = std::sync::Once::new();
	ONCE.call_once(yazi_shared::init);
}

#[test]
fn dir_marker_distinguishes_files_from_dirs() {
	init();
	let f = files(r#"excludes = [{ url = "**/.git/", in = "*" }]"#);

	// The `.git` directory is excluded in its parent...
	assert_eq!(check(&f, "/home/u/proj/.git", true, "/home/u/proj"), Some(true));
	// ...but a *file* named `.git` (a worktree pointer) is not.
	assert_eq!(check(&f, "/home/u/proj/.git", false, "/home/u/proj"), None);
}

#[test]
fn browsing_inside_an_excluded_dir_still_shows_its_contents() {
	init();
	let f = files(r#"excludes = [{ url = "**/.git/", in = "*" }]"#);

	// This is the regression that motivated the contents/self split.
	assert_eq!(check(&f, "/home/u/proj/.git/config", false, "/home/u/proj/.git"), None);
}

#[test]
fn flattened_listings_hide_descendants() {
	init();
	let f = files(r#"excludes = [{ url = "**/.git/", in = "*" }]"#);

	// A search listing holds entries below the folder, so the generated
	// `**/.git/**` counterpart applies there and only there.
	assert_eq!(check(&f, "/home/u/proj/.git/config", false, "search://q//home/u/proj"), Some(true));
	// Descendant directories too, not just files.
	assert_eq!(check(&f, "/home/u/proj/.git/hooks", true, "search://q//home/u/proj"), Some(true));
}

#[test]
fn a_glob_matches_the_whole_path() {
	init();
	// A pattern with no separator has `*` span them, which is what lets `*.pyc`
	// reach into any directory. A bare literal has nothing to span with, so it
	// only ever matches a path that *is* that literal.
	let bare = files(r#"excludes = [{ url = "node_modules", in = "*" }]"#);
	assert_eq!(check(&bare, "/home/u/node_modules", false, "/home/u"), None);

	let anchored = files(r#"excludes = [{ url = "**/node_modules", in = "*" }]"#);
	assert_eq!(check(&anchored, "/home/u/node_modules", false, "/home/u"), Some(true));

	let ext = files(r#"excludes = [{ url = "*.pyc", in = "*" }]"#);
	assert_eq!(check(&ext, "/home/u/deep/x.pyc", false, "/home/u/deep"), Some(true));
}

#[test]
fn the_trailing_slash_selects_directories() {
	init();
	// Same convention as every other pattern in yazi.toml: no slash means files,
	// a trailing slash means directories. Covering both takes two entries, the way
	// `prepend_fetchers` writes `url = "*"` alongside `url = "*/"`.
	let f = files(r#"excludes = [{ url = ["**/node_modules", "**/node_modules/"], in = "*" }]"#);

	assert_eq!(check(&f, "/home/u/node_modules", false, "/home/u"), Some(true));
	assert_eq!(check(&f, "/home/u/node_modules", true, "/home/u"), Some(true));

	let dirs = files(r#"excludes = [{ url = "**/node_modules/", in = "*" }]"#);
	assert_eq!(check(&dirs, "/home/u/node_modules", true, "/home/u"), Some(true));
	assert_eq!(check(&dirs, "/home/u/node_modules", false, "/home/u"), None);
}

#[test]
fn prefix_of_a_longer_name_does_not_match() {
	init();
	let f = files(r#"excludes = [{ url = "**/node_modules/", in = "*" }]"#);
	assert_eq!(check(&f, "/home/u/mynode_modules", true, "/home/u"), None);
}

#[test]
fn in_scopes_a_rule_to_a_subtree() {
	init();
	let f = files(r#"excludes = [{ url = "**/target/", in = "/code/**" }]"#);

	assert_eq!(check(&f, "/code/proj/target", true, "/code/proj"), Some(true));
	assert_eq!(check(&f, "/elsewhere/target", true, "/elsewhere"), None);
}

#[test]
fn in_star_applies_everywhere_including_search() {
	init();
	let f = files(r#"excludes = [{ url = "**/*.tmp", in = "*" }]"#);

	assert_eq!(check(&f, "/anywhere/x.tmp", false, "/anywhere"), Some(true));
	assert_eq!(check(&f, "/anywhere/x.tmp", false, "search://q//anywhere"), Some(true));
}

#[test]
fn in_can_target_a_scheme() {
	init();
	let f = files(r#"excludes = [{ url = "**/*.log", in = "search://**" }]"#);

	// Only in flattened listings, not while browsing.
	assert_eq!(check(&f, "/p/a.log", false, "search://q//p"), Some(true));
	assert_eq!(check(&f, "/p/a.log", false, "/p"), None);
}

#[test]
fn negation_whitelists_and_order_decides() {
	init();
	let f = files(r#"excludes = [{ url = ["**/*.log", "!**/keep.log"], in = "*" }]"#);

	assert_eq!(check(&f, "/p/a.log", false, "/p"), Some(true));
	assert_eq!(check(&f, "/p/keep.log", false, "/p"), Some(false));
}

#[test]
fn later_rules_win() {
	init();
	let f = files(
		r#"excludes = [
			{ url = "**/*.log", in = "*" },
			{ url = "!**/*.log", in = "/keep{,/**}" },
		]"#,
	);

	assert_eq!(check(&f, "/p/a.log", false, "/p"), Some(true));
	assert_eq!(check(&f, "/keep/a.log", false, "/keep"), Some(false));
	assert_eq!(check(&f, "/keep/sub/a.log", false, "/keep/sub"), Some(false));
}

#[test]
fn a_subtree_glob_excludes_its_own_root() {
	init();
	// `/x/**` covers everything below `/x` but not `/x` itself, so a rule meant to
	// cover both is spelled with an empty alternate.
	let narrow = files(r#"excludes = [{ url = "**/a", in = "/x/**" }]"#);
	assert_eq!(narrow.excludes_in(&url("/x")).len(), 0);
	assert_eq!(narrow.excludes_in(&url("/x/sub")).len(), 1);

	let wide = files(r#"excludes = [{ url = "**/a", in = "/x{,/**}" }]"#);
	assert_eq!(wide.excludes_in(&url("/x")).len(), 1);
	assert_eq!(wide.excludes_in(&url("/x/sub")).len(), 1);
	assert_eq!(wide.excludes_in(&url("/xylophone")).len(), 0);
}

#[test]
fn matching_is_case_insensitive_unless_opted_out() {
	init();
	let f = files(r#"excludes = [{ url = "**/NODE_MODULES/", in = "*" }]"#);
	assert_eq!(check(&f, "/home/u/node_modules", true, "/home/u"), Some(true));

	let f = files(r#"excludes = [{ url = "\\s**/NODE_MODULES/", in = "*" }]"#);
	assert_eq!(check(&f, "/home/u/node_modules", true, "/home/u"), None);
}

#[test]
fn urn_accepts_a_single_string_or_a_list() {
	init();
	let one = files(r#"excludes = [{ url = "**/a", in = "*" }]"#);
	let many = files(r#"excludes = [{ url = ["**/a", "**/b"], in = "*" }]"#);

	assert_eq!(check(&one, "/p/a", false, "/p"), Some(true));
	assert_eq!(check(&many, "/p/b", false, "/p"), Some(true));
}

#[test]
fn excludes_in_selects_applicable_rules() {
	init();
	let f = files(
		r#"excludes = [
			{ url = "**/a", in = "/x/**" },
			{ url = "**/b", in = "*" },
		]"#,
	);

	assert_eq!(f.excludes_in(&url("/x/proj")).len(), 2);
	assert_eq!(f.excludes_in(&url("/y/proj")).len(), 1);
}

#[test]
fn a_bad_glob_is_a_config_error() {
	init();
	assert!(toml::from_str::<Files>(r#"excludes = [{ url = "**/[", in = "*" }]"#).is_err());
}
