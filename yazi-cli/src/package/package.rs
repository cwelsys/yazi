use std::{fmt::Display, io, num::NonZeroUsize, path::PathBuf, str::FromStr};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use yazi_fs::{Xdg, engine::{Engine, local::Local}};
use yazi_macro::{ok_or_not_found, outln};

use super::{Dependency, Fetcher, Report};

#[derive(Default)]
pub(crate) struct Package {
	pub(crate) plugins: Vec<Dependency>,
	pub(crate) flavors: Vec<Dependency>,
}

impl Package {
	pub(crate) async fn load() -> Result<Self> {
		let s = ok_or_not_found!(Local::regular(&Self::toml()).read_to_string().await);
		Ok(toml::from_str(&s)?)
	}

	pub(crate) async fn add_many(&mut self, uses: &[String], jobs: NonZeroUsize) -> Result<()> {
		let mut tally = Tally::default();

		// Resolve the whole batch up front, so that every repository it needs can be
		// fetched at once; each package is still reported in the order it was given.
		let mut deps = Vec::with_capacity(uses.len());
		for u in uses {
			deps.push(self.resolve(&deps, u));
		}

		let fetcher = Fetcher::run(deps.iter().flatten(), jobs).await;
		for (u, dep) in uses.iter().zip(deps) {
			let r = match dep {
				Ok(mut d) => match fetcher.check(&d) {
					Ok(()) => d.add(false).await.map(|_| d),
					Err(e) => Err(e),
				},
				Err(e) => Err(e),
			};

			match r {
				Ok(d) => {
					tally.change("Added", format_args!("{} {}", d.name, d.rev))?;
					if d.is_flavor {
						self.flavors.push(d);
					} else {
						self.plugins.push(d);
					}
				}
				Err(e) => tally.fail(u, &e)?,
			}
			self.save().await?;
		}

		tally.finish("added")
	}

	pub(crate) async fn delete_many(&mut self, uses: &[String], discard: bool) -> Result<()> {
		let mut tally = Tally::default();
		for u in uses {
			let r = self.delete(&mut tally, u, discard).await;
			self.save().await?;
			if let Err(e) = r {
				tally.fail(u, &e)?;
			}
		}
		tally.finish("deleted")
	}

	pub(crate) async fn install(&mut self, discard: bool, jobs: NonZeroUsize) -> Result<()> {
		let mut tally = Tally::default();
		Tally::heading("Installing", self.plugins.len() + self.flavors.len())?;

		let fetcher = Fetcher::run(self.plugins.iter().chain(&self.flavors), jobs).await;

		macro_rules! go {
			($dep:expr) => {
				let r = match fetcher.check(&$dep) {
					Ok(()) => $dep.install(discard).await,
					Err(e) => Err(e),
				};
				self.save().await?;
				match r {
					Ok(true) => tally.change("Installed", format_args!("{} {}", $dep.name, $dep.rev))?,
					Ok(false) => tally.keep("Unchanged", format_args!("{} {}", $dep.name, $dep.rev))?,
					Err(e) => tally.fail(&$dep.name, &e)?,
				}
			};
		}

		for i in 0..self.plugins.len() {
			go!(self.plugins[i]);
		}
		for i in 0..self.flavors.len() {
			go!(self.flavors[i]);
		}
		tally.finish("installed")
	}

	pub(crate) async fn upgrade_many(
		&mut self,
		uses: &[String],
		discard: bool,
		jobs: NonZeroUsize,
	) -> Result<()> {
		let mut tally = Tally::default();
		let selected = |d: &Dependency| uses.is_empty() || uses.contains(&d.r#use);
		Tally::heading(
			"Upgrading",
			self.plugins.iter().chain(&self.flavors).filter(|d| selected(d)).count(),
		)?;

		// Pinned packages stay where they are, so their repositories are left alone.
		let fetcher = Fetcher::run(
			self.plugins.iter().chain(&self.flavors).filter(|d| selected(d) && !d.pinned()),
			jobs,
		)
		.await;

		macro_rules! go {
			($dep:expr) => {
				if selected(&$dep) {
					let old = $dep.rev.clone();
					let r = if $dep.pinned() {
						Ok(false)
					} else {
						match fetcher.check(&$dep) {
							Ok(()) => $dep.add(discard).await,
							Err(e) => Err(e),
						}
					};
					self.save().await?;
					match r {
						Ok(fresh) => tally.upgraded(&$dep, &old, fresh)?,
						Err(e) => tally.fail(&$dep.name, &e)?,
					}
				}
			};
		}

		for i in 0..self.plugins.len() {
			go!(self.plugins[i]);
		}
		for i in 0..self.flavors.len() {
			go!(self.flavors[i]);
		}
		tally.finish("upgraded")
	}

	pub(crate) fn print(&self) -> Result<()> {
		outln!("Plugins:")?;
		for d in &self.plugins {
			if d.rev.is_empty() {
				outln!("\t{}", d.r#use)?;
			} else {
				outln!("\t{} ({})", d.r#use, d.rev)?;
			}
		}

		outln!("Flavors:")?;
		for d in &self.flavors {
			if d.rev.is_empty() {
				outln!("\t{}", d.r#use)?;
			} else {
				outln!("\t{} ({})", d.r#use, d.rev)?;
			}
		}

		Ok(())
	}

	/// Parses a package URL, rejecting it if package.toml or the rest of the same
	/// batch already covers it.
	fn resolve(&self, pending: &[Result<Dependency>], r#use: &str) -> Result<Dependency> {
		let dep = Dependency::from_str(r#use)?;
		if let Some(d) =
			self.identical(&dep).or_else(|| pending.iter().flatten().find(|d| d.identical(&dep)))
		{
			bail!(
				"{} `{}` already exists in package.toml",
				if d.is_flavor { "Flavor" } else { "Plugin" },
				dep.name
			)
		}
		Ok(dep)
	}

	async fn delete(&mut self, tally: &mut Tally, r#use: &str, discard: bool) -> Result<()> {
		let Some(dep) = self.identical(&Dependency::from_str(r#use)?).cloned() else {
			bail!("`{}` was not found in package.toml", r#use)
		};

		if dep.delete(discard).await? {
			tally.change("Deleted", &dep.name)?;
		} else {
			tally.keep("Missing", &dep.name)?;
		}

		if dep.is_flavor {
			self.flavors.retain(|d| !d.identical(&dep));
		} else {
			self.plugins.retain(|d| !d.identical(&dep));
		}
		Ok(())
	}

	async fn save(&self) -> Result<()> {
		let s = toml::to_string_pretty(self)?;
		Local::regular(&Self::toml()).write(s).await.context("Failed to write package.toml")
	}

	fn toml() -> PathBuf { Xdg::config_dir().join("package.toml") }

	fn identical(&self, other: &Dependency) -> Option<&Dependency> {
		self.plugins.iter().chain(&self.flavors).find(|d| d.identical(other))
	}
}

/// Reports the outcome of each package as it lands, and totals them up at the
/// end.
#[derive(Default)]
struct Tally {
	changed:   usize,
	unchanged: usize,
	failed:    usize,
}

impl Tally {
	fn heading(verb: &str, total: usize) -> io::Result<()> {
		Report::done(verb, format_args!("{total} package{}", if total == 1 { "" } else { "s" }))
	}

	fn change(&mut self, verb: &str, body: impl Display) -> io::Result<()> {
		self.changed += 1;
		Report::done(verb, body)
	}

	fn keep(&mut self, verb: &str, body: impl Display) -> io::Result<()> {
		self.unchanged += 1;
		Report::noop(verb, body)
	}

	fn fail(&mut self, id: &str, e: &anyhow::Error) -> io::Result<()> {
		self.failed += 1;
		Report::fail("Failed", format_args!("{id}: {e:#}"))
	}

	fn upgraded(&mut self, dep: &Dependency, old: &str, fresh: bool) -> io::Result<()> {
		if old.starts_with('=') {
			self.keep("Pinned", format_args!("{} {old}", dep.name))
		} else if fresh || old.is_empty() {
			self.change("Installed", format_args!("{} {}", dep.name, dep.rev))
		} else if old == dep.rev {
			self.keep("Unchanged", format_args!("{} {old}", dep.name))
		} else {
			self.change("Upgraded", format_args!("{} {old} -> {}", dep.name, dep.rev))
		}
	}

	fn finish(&self, changed: &str) -> Result<()> {
		let mut parts = vec![];
		if self.changed > 0 {
			parts.push(format!("{} {changed}", self.changed));
		}
		if self.unchanged > 0 {
			parts.push(format!("{} unchanged", self.unchanged));
		}
		if self.failed > 0 {
			parts.push(format!("{} failed", self.failed));
		}

		Report::done("Finished", parts.join(", "))?;
		if self.failed > 0 {
			// Each failure was reported in full as it happened, so only the exit code is
			// left to set.
			std::process::exit(1);
		}

		Ok(())
	}
}

impl<'de> Deserialize<'de> for Package {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[derive(Deserialize)]
		struct Outer {
			#[serde(default)]
			plugin: Shadow,
			#[serde(default)]
			flavor: Shadow,
		}
		#[derive(Default, Deserialize)]
		struct Shadow {
			deps: Vec<Dependency>,
		}

		let mut outer = Outer::deserialize(deserializer)?;
		outer.flavor.deps.iter_mut().for_each(|d| d.is_flavor = true);

		Ok(Self { plugins: outer.plugin.deps, flavors: outer.flavor.deps })
	}
}

impl Serialize for Package {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		#[derive(Serialize)]
		struct Outer<'a> {
			plugin: Shadow<'a>,
			flavor: Shadow<'a>,
		}
		#[derive(Serialize)]
		struct Shadow<'a> {
			deps: &'a [Dependency],
		}

		Outer { plugin: Shadow { deps: &self.plugins }, flavor: Shadow { deps: &self.flavors } }
			.serialize(serializer)
	}
}
