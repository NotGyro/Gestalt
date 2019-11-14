extern crate rerun_except;

pub fn main() {
    rerun_except::rerun_except(&["nonexistent file"]).unwrap();
}