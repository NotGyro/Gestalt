extern crate std;
extern crate num;
extern crate rgb;
extern crate parking_lot;

use voxel::voxelmath::*;
use voxel::subdivmath::*;
use std::fmt::Debug;
use std::error::Error;
use std::result::Result;
use cgmath::{Vector3, Point3, InnerSpace, Rotation, Rotation3, Quaternion, Rad};

use rgb::*;
use rgb::ComponentBytes;

use world::tile::TileID;

use hashbrown::HashMap;

use voxel::subdivstorage::*;
use voxel::subdivmath::*;

use std::io::Cursor;
use std::io::Write;
use std::io::Seek;
use std::io::SeekFrom;

use self::parking_lot::RwLock;

pub struct TileArt {
    pub color : Color,
    //Is this utterly see-through?
    pub air: bool,
}

lazy_static! {
    pub static ref TILE_TO_ART: RwLock<HashMap<TileID, TileArt>> = RwLock::new(HashMap::new());
}

pub type Color = RGB<u8>;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SimpleLOD {
    pub color : Color,
    pub filled : u8,
}

pub type World = NaiveVoxelOctree<TileID, SimpleLOD>;

impl LODData<TileID> for SimpleLOD  {
    
    fn represent(voxel: &TileID) -> Self {
        let artmap_lock = TILE_TO_ART.read();
        let art = artmap_lock.get(voxel).unwrap();
        let mut filled = 255;
        if art.air == true { filled = 0 };
        SimpleLOD{ color: art.color.clone(), filled:filled  }
    }
    fn downsample_from(&mut self, child_values: &[Self; 8]){
        self.color.r = 0;
        self.color.g = 0;
        self.color.b = 0;
        self.filled = 0; 

        for child in child_values{ 
            if child.filled != 0 {
                self.color.r += child.color.r/8;
                self.color.g += child.color.g/8;
                self.color.b += child.color.b/8;
                self.filled += child.filled/8;
            }
        }
    }
}

pub struct PixelRay { 
    pub ray: VoxelRaycast,
    pub done: bool, //This goes "True" once the ray encounters a valid voxel to render.
    //pub result: Option<Color>, //This is what we encountered. This will be a "Some" once we have something to render.
}
impl PixelRay { 
    fn new(ray: VoxelRaycast, starting_scale : Scale) -> Self {
        PixelRay {
            ray: ray,
            done: false, 
        }
    }
}

fn construct_ray_normals(fov : Rad<f64>, resolution_x : u32, resolution_y : u32, aspect_ratio: f64) -> Vec<Vec<Vector3<f64>>> {
    let mut result : Vec<Vec<Vector3<f64>>> = Vec::new();
    let angle_per_pixel_x = fov / (resolution_x as f64); 
    let angle_per_pixel_y = (fov/aspect_ratio) / (resolution_y as f64); 
    for screen_y in 0..resolution_y {
        result.push(Vec::new());
        for screen_x in 0..resolution_x {
            // Furthest edge counter-clockwise 
            let x_ccw_bound = -fov / 2.0;
            let y_ccw_bound = -(fov/aspect_ratio) / 2.0;

            let yaw_offset = angle_per_pixel_x * (screen_x as f64); //fov * ( (screen_x as f64) / (resolution_x as f64));
            let pitch_offset = angle_per_pixel_y * (screen_y as f64);  //(fov/aspect_ratio) * ( (screen_y as f64) / (resolution_y as f64) );

            let ray_yaw : Quaternion<f64> = Quaternion::from_angle_y(x_ccw_bound + yaw_offset);
            let ray_pitch : Quaternion<f64> = Quaternion::from_angle_x(y_ccw_bound + pitch_offset);
            let rotation = (ray_yaw * ray_pitch).normalize();
            
            let mut forward : Vector3<f64> = Vector3::new(0.0, 0.0, -1.0);

            forward = rotation.rotate_vector(forward);

            result[screen_y as usize].push(forward);
        }
    }
    result
}

fn create_rays(origin : Point3<f64>, yaw: Rad<f64>, pitch: Rad<f64>, normals: &Vec<Vec<Vector3<f64>>>, starting_scale: Scale) -> Vec<PixelRay> {
    let yaw_quat : Quaternion<f64> = Quaternion::from_angle_z(yaw);
    let pitch_quat : Quaternion<f64> = Quaternion::from_angle_y(pitch);
    let rotation = (yaw_quat * pitch_quat).normalize();
    
    let mut result : Vec<PixelRay> = Vec::new();
    for row in normals {
        for normal in row {
            result.push( PixelRay::new(VoxelRaycast::new(origin, rotation.rotate_vector(*normal)), starting_scale));
        }
    }
    result
}

pub struct SoftwareRenderer {
    // Reusable struct of where the ray for each pixel on your raster should be pointing. 
    pub ray_normals: Vec<Vec<Vector3<f64>>>,
    pub rays: Vec<PixelRay>,
    pub resolution_x : u32,
    pub resolution_y : u32,
    pub aspect_ratio: f64,
    pub fov: Rad<f64>,
    pub clear_color : Color,
}

impl SoftwareRenderer {
    pub fn init(&mut self) {
        self.ray_normals = construct_ray_normals(self.fov, self.resolution_x, self.resolution_y, self.aspect_ratio);
    }
    pub fn new(resolution_x : u32, resolution_y : u32, fov: Rad<f64>) -> Self {
        let aspect_ratio = (resolution_x as f64) / (resolution_y as f64);
        let mut result = SoftwareRenderer {
            ray_normals: Vec::with_capacity((resolution_x*resolution_y) as usize),
            rays: Vec::with_capacity((resolution_x*resolution_y) as usize),
            resolution_x: resolution_x,
            resolution_y: resolution_y,
            aspect_ratio: aspect_ratio,
            fov : fov,
            clear_color : RGB{r: 168, g: 220, b: 255},
        };
        result.init();
        result
    }
    //To be called every frame with player information.  
    pub fn draw_frame(&mut self, origin : Point3<f64>, yaw: Rad<f64>, pitch: Rad<f64>, world: &World, surface: &mut [u8], surface_ty: swsurface::ImageInfo) -> Result<(), Box<dyn Error>> {
        let starting_scale = -1;
        //let max_scale = 6;
        let max_steps : usize = 128;
        let current_scale = starting_scale;
        //let falloff_period = 16;
        //let falloff_counter = 0;
        //Number of tiles each ray will have gone through.
        let mut step : usize = 0;
        self.rays = create_rays(origin, yaw, pitch, &self.ray_normals, current_scale);
        let total_rays = self.resolution_x * self.resolution_y;
        let mut finished_count = 0; 
        //Start drawing a frame. 
        while step < max_steps {
            /*if falloff_counter >= falloff_period {
                current_scale += 1; 
            }
            if current_scale >= max_scale {
                break;
            }*/
            if finished_count >= total_rays { 
                break;
            }
            //let mut i = 0;
            for (i, ref mut ray) in self.rays.iter_mut().enumerate() {
                if !ray.done {
                    let oct_pos = opos!( (ray.ray.pos.x as u32, ray.ray.pos.y as u32, ray.ray.pos.z as u32) @ current_scale); 
                    
                    let voxel_result = world.get(oct_pos);
                    match voxel_result {
                        Ok(cell) => {
                            let to_draw = match cell { 
                                SubNode::Branch(lod_data) => lod_data,
                                SubNode::Leaf(tile) => SimpleLOD::represent(&tile),
                            };
                            if to_draw.filled != 0 {
                                // Treat this as a hit.
                                // Let's do the fancy thing where it gets darker the further away from the camera it is, for this test. 
                                let mut color = to_draw.color;
                                let distance = ( ((step as f32) / (max_steps as f32)) * 255.0) as u8;
                                if color.r >= distance {
                                    color.r -= distance;
                                } 
                                else { 
                                    color.r = 0;
                                }
                                if color.g >= distance {
                                    color.g -= distance;
                                } 
                                else { 
                                    color.g = 0;
                                }
                                if color.b >= distance {
                                    color.b -= distance;
                                } 
                                else { 
                                    color.b = 0;
                                }
                                let y = i / self.resolution_x as usize;
                                let x = i % self.resolution_x as usize;
                                surface[(y*surface_ty.stride + x*4) as usize] = color.b;
                                surface[(y*surface_ty.stride + x*4+1) as usize] = color.g;
                                surface[(y*surface_ty.stride + x*4+2) as usize] = color.r;
                                surface[(y*surface_ty.stride + x*4+3) as usize] = 255u8;
                                ray.done = true;
                            }
                        },
                        //This is not within bounds - skip it for this test,
                        //the player is most likely flying out of our test chunk.
                        Err(SubdivError::OutOfBounds) => {},
                        Err(other_error) => return Err(Box::new(other_error)),
                    }
                    //If we didn't hit a valid voxel just now...
                    if !ray.done {
                        //Step our voxel raycast.
                        ray.ray.step();
                    }
                    else {
                        finished_count += 1;
                    }
                }
                //i += 1;
            }
            step += 1;
        }
        //Clean up whichever didn't hit a voxel.
        for (i, ref mut ray) in self.rays.iter_mut().enumerate() {
            if !ray.done {
                            let y = i / self.resolution_x as usize;
                            let x = i % self.resolution_x as usize;
                            surface[(y*surface_ty.stride + x*4) as usize] = self.clear_color.b;
                            surface[(y*surface_ty.stride + x*4+1) as usize] = self.clear_color.g;
                            surface[(y*surface_ty.stride + x*4+2) as usize] = self.clear_color.r;
                            surface[(y*surface_ty.stride + x*4+3) as usize] = 255u8;
            }
        }

        /*
        //Blit to our surface. 
        for y in 0..(self.resolution_y as usize) {
            for x in 0..(self.resolution_x as usize) {
                //This will be in BGRA format and we're ignoring alpha for now.
                let ray = self.rays.get( (y* (self.resolution_x as usize)) + x).unwrap();
                if ray.done {
                    let color = ray.result.unwrap();
                    surface[(y*surface_ty.stride + x*4) as usize] = color.b;
                    surface[(y*surface_ty.stride + x*4+1) as usize] = color.g;
                    surface[(y*surface_ty.stride + x*4+2) as usize] = color.r;
                    surface[(y*surface_ty.stride + x*4+3) as usize] = 255u8;
                }
                else { 
                    
                    surface[(y*surface_ty.stride + x*4) as usize] = self.clear_color.b;
                    surface[(y*surface_ty.stride + x*4+1) as usize] = self.clear_color.g;
                    surface[(y*surface_ty.stride + x*4+2) as usize] = self.clear_color.r;
                    surface[(y*surface_ty.stride + x*4+3) as usize] = 255u8;
                }
            }
        }*/
        //Note this pixel format is little-endian - it's BGRA!
        /*
        let [size_w, size_h] = surface_ty.extent;
        for y in 0..(size_h as usize) {
            for x in 0..(size_w as usize) {
                if x % 16 == 0  {
                    //cursor.write(&[255u8])?;
                    surface[(y*surface_ty.stride + x*4) as usize] = 255u8;
                    surface[(y*surface_ty.stride + x*4+1) as usize] = 255u8;
                    surface[(y*surface_ty.stride + x*4+2) as usize] = 0u8;
                    surface[(y*surface_ty.stride + x*4+3) as usize] = 255u8;
                }
                else if y % 16 == 0 {
                    //cursor.write(&[255u8])?;
                    surface[(y*surface_ty.stride + x*4) as usize] = 0u8;
                    surface[(y*surface_ty.stride + x*4+1) as usize] = 0u8;
                    surface[(y*surface_ty.stride + x*4+2) as usize] = 0u8;
                    surface[(y*surface_ty.stride + x*4+3) as usize] = 255u8;
                }
                else { 
                    surface[(y*surface_ty.stride + x*4) as usize] = 0u8;
                    surface[(y*surface_ty.stride + x*4+1) as usize] = 255u8;
                    surface[(y*surface_ty.stride + x*4+2) as usize] = 0u8;
                    surface[(y*surface_ty.stride + x*4+3) as usize] = 255u8;
                }
            }
            //End of row, lets get to the next one. 
            //cursor.seek(SeekFrom::Current(
            //    (surface_ty.stride as i64) - (size_w*4) as i64 ));
        };*/
        //Drawing process has completed.
        self.rays.clear();
        Ok(())
    }
}