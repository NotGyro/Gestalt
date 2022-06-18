extern crate string_cache_codegen;

use std::path::Path;

fn main() {
    let generated_path = Path::new("src/generated");
    if !generated_path.exists() { 
        std::fs::create_dir_all(generated_path).unwrap();
    }
    println!("cargo:rerun-if-changed=build.rs");
    
    string_cache_codegen::AtomType::new("msgtype::MsgType", "msg_type!")
        .atoms(&["foo", "bar"])
        .write_to_file(&generated_path.join("msg_type.rs"))
        .unwrap()
}