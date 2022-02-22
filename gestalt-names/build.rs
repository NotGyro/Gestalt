extern crate string_cache_codegen;

use std::path::Path;

fn main() {
    string_cache_codegen::AtomType::new("msgtype::MsgType", "msg_type!")
        .atoms(&["foo", "bar"])
        .write_to_file(&Path::new("src/generated").join("msg_type.rs"))
        .unwrap()
}