use std::time::Duration;

use glam::{Mat4, Vec3, EulerRot, Quat};
use winit::event::VirtualKeyCode;

use crate::common::{DegreeAngle, Angle, RadianAngle};

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
pub struct Perspective {
    pub aspect_ratio: f32,
	/// fov_y in radians
    pub fov_y: RadianAngle,
    pub near_clip_z: f32,
    pub far_clip_z: f32,
}
impl Perspective { 
	pub fn new(aspect_ratio: f32) -> Self { 
		Self { 
			aspect_ratio,
			..Default::default()
		}
	}
	pub fn set_fov_y_degrees(&mut self, fov_y_degrees: f32) { 
		self.fov_y = RadianAngle::from_degrees(fov_y_degrees)
	}
	/// Make a right-handed coordinate system perspective matrix
	pub fn make_matrix(&self) -> Mat4 {
		glam::Mat4::perspective_rh(
			self.fov_y.get_radians(),
			self.aspect_ratio,
			self.near_clip_z,
			self.far_clip_z)
	}
}

impl Default for Perspective {
    fn default() -> Self {
        Self { 
			aspect_ratio: 16.0 / 9.0,
			fov_y: RadianAngle::from_degrees(80.0),
			near_clip_z: 0.001,
			far_clip_z: 1000.0 }
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
	pub perspective: Perspective,
}

impl Camera {
	pub fn new(pos: Vec3, aspect_ratio: f32) -> Self {
		let yaw = 0.0;
		let pitch = 0.0;
		let world_up = Vec3::new(0.0, 1.0, 0.0);
		let front = Camera::calc_front(DegreeAngle(0.0), DegreeAngle(0.0));
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
			zoom: 1.0,
			perspective: Perspective::new(aspect_ratio),
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
		glam::Mat4::look_at_rh(self.position, /*center*/ self.position + self.front, Vec3::Y)
	}

	pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) { 
		self.perspective.aspect_ratio = aspect_ratio;
	}

	pub fn key_interact(&mut self, direction: Directions, time_elapsed: Duration) {
		match direction {
			Directions::Forward => {
				self.position += self.front * self.speed * (time_elapsed.as_secs_f64() as f32);
			}
			Directions::Left => {
				self.position -= self.right * self.speed * (time_elapsed.as_secs_f64() as f32);
			}
			Directions::Right => {
				self.position += self.right * self.speed * (time_elapsed.as_secs_f64() as f32);
			}
			Directions::Up => {
				self.position += self.up * self.speed * (time_elapsed.as_secs_f64() as f32);
			}
			Directions::Down => {
				self.position -= self.up * self.speed * (time_elapsed.as_secs_f64() as f32);
			}
			Directions::Backward => {
				self.position -= self.front * self.speed * (time_elapsed.as_secs_f64() as f32);
			}
		}
	}

	pub fn update_orientation(&mut self) {
		self.front = Camera::calc_front(self.get_yaw(), self.get_pitch());
		self.right = Camera::calc_right(&self.front, &self.world_up);
		self.up = Camera::calc_up(&self.right, &self.front);
	}
	pub fn mouse_interact(&mut self, dx: f32, dy: f32) {
		self.yaw = self.yaw - dx;
		self.pitch = (self.pitch - dy).max(-89.0).min(89.0);
		self.update_orientation();
	}

	pub fn get_yaw(&self) -> DegreeAngle { 
		DegreeAngle(self.yaw)
	}
	pub fn get_pitch(&self) -> DegreeAngle { 
		DegreeAngle(self.pitch)
	}
	pub fn get_roll(&self) -> DegreeAngle { 
		DegreeAngle(0.0)
	}

	pub fn scroll_wheel_interact(&mut self, delta: f32) {
		let new_zoom = (self.zoom + delta).max(1.0).min(55.0);
		//println!("Scroll {} + delta {} => {}", self.zoom, delta, new_zoom );
		self.zoom = new_zoom;
	}

	fn calc_front<A: Angle>(yaw: A, pitch: A) -> Vec3 {
		const COORDINATE_SYSTEM_FORWARD: Vec3 = Vec3::new(0.0, 0.0, -1.0);
		let yaw = yaw.get_radians() % (2.0 * std::f32::consts::PI);
		let pitch = pitch.get_radians() % (2.0 * std::f32::consts::PI);
		let quat = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0); 
		quat.mul_vec3(COORDINATE_SYSTEM_FORWARD).normalize()
	}

	fn calc_right(front: &Vec3, world_up: &Vec3) -> Vec3 {
		front.cross(*world_up).normalize()
	}

	fn calc_up(right: &Vec3, front: &Vec3) -> Vec3 {
		right.cross(*front).normalize()
		//Vec3::new(0.0, 1.0, 0.0)
	}

    pub fn build_view_projection_matrix(&self) -> glam::Mat4 {
        let view = self.get_view_matrix();
        let proj = self.perspective.make_matrix();

        return proj * view;
    }
}
