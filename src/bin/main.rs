extern crate gestalt;

/// Nonsense shim so gestalt can build as a library so rustdoc will properly run doc tests.
#[allow(dead_code)]
fn main() {
    gestalt::main();
}