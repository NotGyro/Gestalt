use std::time::Duration;

use glam::{Mat4, Vec3};
use winit::event::VirtualKeyCode;

//TODO - here for testing, better input system needed.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Directions {
    Left,
    Right,
    Up,
    Down,
    Forward,
    Backward,
}
impl Directions {
    pub fn from_key(value: VirtualKeyCode) -> Option<Directions> {
        match value {
            VirtualKeyCode::W => Some(Directions::Forward),
            VirtualKeyCode::A => Some(Directions::Left),
            VirtualKeyCode::S => Some(Directions::Backward),
            VirtualKeyCode::D => Some(Directions::Right),
            VirtualKeyCode::R => Some(Directions::Up),
            VirtualKeyCode::C => Some(Directions::Down),
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Camera {
    position: Vec3,
    front: Vec3,
    up: Vec3,
    right: Vec3,
    world_up: Vec3,

    yaw: f32,
    pitch: f32,
    pub speed: f32,
    pub zoom: f32,
}

impl Camera {
    pub fn new(pos: Vec3) -> Self {
        let yaw = -90.0;
        let pitch = 0.0;
        let world_up = Vec3::new(0.0, 1.0, 0.0);
        let front = Camera::calc_front(yaw, pitch);
        let right = Camera::calc_right(&front, &world_up);
        let up = Camera::calc_up(&right, &front);

        Self {
            position: pos,
            front,
            up,
            right,
            world_up,
            yaw,
            pitch,
            speed: 2.5,
            zoom: 45.0,
        }
    }

    pub fn get_zoom(&self) -> f32 {
        self.zoom
    }
    pub fn get_position(&self) -> &Vec3 {
        &self.position
    }
    pub fn get_front(&self) -> &Vec3 {
        &self.front
    }

    pub fn get_view_matrix(&self) -> Mat4 {
        glam::Mat4::look_at_lh(
            self.position,
            /*center*/ self.position + self.front,
            self.up,
        )
    }

    pub fn key_interact(&mut self, direction: Directions, time_elapsed: Duration) {
        match direction {
            Directions::Forward => {
                self.position += self.front * self.speed * (time_elapsed.as_secs_f64() as f32);
            }
            Directions::Left => {
                self.position += self.right * self.speed * (time_elapsed.as_secs_f64() as f32);
            }
            Directions::Right => {
                self.position -= self.right * self.speed * (time_elapsed.as_secs_f64() as f32);
            }
            Directions::Up => {
                self.position += self.up    * self.speed * (time_elapsed.as_secs_f64() as f32);
            }
            Directions::Down => {
                self.position -= self.up    * self.speed * (time_elapsed.as_secs_f64() as f32);
            }
            Directions::Backward => {
                self.position -= self.front * self.speed * (time_elapsed.as_secs_f64() as f32);
            }
        }
    }

    pub fn mouse_interact(&mut self, dx: f32, dy: f32) {
        self.yaw = self.yaw - dx;
        self.pitch = (self.pitch - dy).max(-89.0).min(89.0);

        self.front = Camera::calc_front(self.yaw, self.pitch);
        self.right = Camera::calc_right(&self.front, &self.world_up);
        self.up = Camera::calc_up(&self.right, &self.front);
    }

    pub fn scroll_wheel_interact(&mut self, delta: f32) {
        let new_zoom = (self.zoom + delta).max(1.0).min(55.0);
        //println!("Scroll {} + delta {} => {}", self.zoom, delta, new_zoom );
        self.zoom = new_zoom;
    }

    fn calc_front(yaw: f32, pitch: f32) -> Vec3 {
        let ya = yaw.to_radians();
        let pa = pitch.to_radians();

        Vec3::new(ya.cos() * pa.cos(), pa.sin(), ya.sin() * pa.cos()).normalize()
    }

    fn calc_right(front: &Vec3, world_up: &Vec3) -> Vec3 {
        front.cross(*world_up).normalize()
    }

    fn calc_up(right: &Vec3, front: &Vec3) -> Vec3 {
        right.cross(*front).normalize()
    }
}
