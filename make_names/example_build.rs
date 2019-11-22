use std::collections::HashSet;

pub fn main() {
    // collect names
    let mut names = HashSet::new();
    let re = regex::Regex::new(r"name!\(([a-zA-Z0-9_]*)\)").unwrap();

    for entry in walkdir::WalkDir::new("src")
                        .into_iter()
                        .filter_map(Result::ok)
                        .filter(|e| !e.file_type().is_dir()) {

        let text = std::fs::read_to_string(entry.path()).unwrap();
        for caps in re.captures_iter(&text) {
            names.insert(caps.get(1).map(|m| m.as_str()).unwrap().to_string());
        }
    }

    // update names.rs
    let mut namestr = String::new();
    for name in names {
        namestr += &name;
        namestr += ",\n    ";
    }

    let namestr = format!("#[macro_export] macro_rules! name {{ ($i:ident) => {{ crate::names::$i }} }}\n\nmake_names::make_names! {{\n    {}\n}}", namestr);
    std::fs::write("src/names.rs", namestr).unwrap();
}