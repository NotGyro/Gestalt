#[macro_use] extern crate crossbeam_channel;
extern crate hashbrown;
extern crate kiss3d;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
extern crate nalgebra as na;
extern crate num;
extern crate parking_lot;
extern crate rand;
extern crate serde;
extern crate serde_json;
extern crate ustr;
extern crate uuid;


use kiss3d::light::Light;
use kiss3d::window::Window;
use na::{UnitQuaternion, Vector3};

pub mod world;
#[macro_use] pub mod util;

fn main() {
    println!("Hello, world!");
    let mut window = Window::new("Kiss3d: cube");
    let mut c = window.add_cube(1.0, 1.0, 1.0);

    c.set_color(1.0, 0.0, 0.0);

    window.set_light(Light::StickToCamera);

    let rot = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.014);

    while window.render() {
        c.prepend_to_local_rotation(&rot);
    }
}