use std::{env, fmt::Display, io::{self, IsTerminal, Write}};

use ratatui_core::style::Color;
use yazi_macro::writef;
use yazi_tty::sequence::{If, ResetAttrs, SetFg, SetSgr};

/// Width of the status column, which right-aligns every verb against it.
const VERB: usize = 12;

pub(super) struct Report;

impl Report {
	/// Something changed.
	pub(super) fn done(verb: &str, body: impl Display) -> io::Result<()> {
		Self::line(io::stdout(), SetSgr::Bold, Color::Green, verb, body)
	}

	/// Nothing to do.
	pub(super) fn noop(verb: &str, body: impl Display) -> io::Result<()> {
		Self::line(io::stdout(), SetSgr::Dim, Color::Reset, verb, body)
	}

	/// Something went wrong, so report it on stderr.
	pub(super) fn fail(verb: &str, body: impl Display) -> io::Result<()> {
		Self::line(io::stderr(), SetSgr::Bold, Color::Red, verb, body)
	}

	fn line(
		mut out: impl Write + IsTerminal,
		sgr: SetSgr,
		color: Color,
		verb: &str,
		body: impl Display,
	) -> io::Result<()> {
		// Keep anything spanning several lines, such as captured `git` output, within
		// the body column.
		let body = body.to_string();
		let body = body.trim_end().replace('\n', &format!("\n{:VERB$} ", ""));

		let ansi = env::var_os("YA_FORCE_ANSI").is_some_and(|v| v == "1") || out.is_terminal();
		writef!(
			out,
			"{}{}{verb:>VERB$}{} {body}\n",
			If(ansi, sgr),
			If(ansi && color != Color::Reset, SetFg(color)),
			If(ansi, ResetAttrs),
		)
	}
}
