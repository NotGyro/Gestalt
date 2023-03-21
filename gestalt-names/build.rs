use std::path::Path;

fn main() {
	println!("cargo:rerun-if-changed=build.rs");

	let generated_path = Path::new("src/generated");
	if !generated_path.exists() {
		std::fs::create_dir_all(generated_path).unwrap();
	}

	string_cache_codegen::AtomType::new("gestalt_atom::GestaltAtom", "gestalt_atom!")
		.atoms(&["foo", "bar"])
		.write_to_file(&generated_path.join("gestalt_atom.rs"))
		.unwrap()
}
