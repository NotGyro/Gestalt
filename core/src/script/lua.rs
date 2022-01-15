use super::SupportedLanguages;
use mlua::Lua;
use crate::common::resource::ResourceDescriptor;

/*
// A ScriptProvider loads and builds `ScriptContext`s, including any build tasks required to compile, 
// transpile, lint, or otherwise examine the code before we attempt to run it.
pub struct LuaProvider {}

impl ScriptProvider<{SupportedLanguages::Lua}> for LuaProvider { 

}

pub struct LuaContext {
    pub vm: Lua,
}*/