use std::str::FromStr;

use mlua::{FromLua, IntoLua, Lua, LuaSerdeExt, Value};
use serde::{Deserialize, Serialize};
use yazi_binding::SER_OPT;
use yazi_shared::event::ActionCow;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ExcludedOpt {
	pub state: ExcludedOptState,
}

impl TryFrom<ActionCow> for ExcludedOpt {
	type Error = anyhow::Error;

	fn try_from(a: ActionCow) -> Result<Self, Self::Error> {
		Ok(Self { state: a.str(0).parse().unwrap_or_default() })
	}
}

impl FromLua for ExcludedOpt {
	fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> { lua.from_value(value) }
}

impl IntoLua for ExcludedOpt {
	fn into_lua(self, lua: &Lua) -> mlua::Result<Value> { lua.to_value_with(&self, SER_OPT) }
}

// --- State
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExcludedOptState {
	#[default]
	None,
	Show,
	Hide,
	Toggle,
}

impl FromStr for ExcludedOptState {
	type Err = serde::de::value::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::deserialize(serde::de::value::StrDeserializer::new(s))
	}
}

impl ExcludedOptState {
	pub fn bool(self, old: bool) -> bool {
		match self {
			Self::None => old,
			Self::Show => true,
			Self::Hide => false,
			Self::Toggle => !old,
		}
	}
}
