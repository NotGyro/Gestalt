
extern crate glium;

use voxel::voxelstorage::VoxelStorage;
use std::vec::Vec;
use util::cubeaxis::CubeAxis;

#[derive(Copy, Clone)]
pub struct Vertex {
    position: [f32; 3],
}

implement_vertex!(Vertex, position);

const POSX_POSY_POSZ_VERT : Vertex  = Vertex{ position : [1.0,1.0,1.0]};
const POSX_POSY_NEGZ_VERT : Vertex  = Vertex{ position : [1.0,1.0,0.0]};
const POSX_NEGY_NEGZ_VERT : Vertex  = Vertex{ position : [1.0,0.0,0.0]};
const POSX_NEGY_POSZ_VERT : Vertex  = Vertex{ position : [1.0,0.0,1.0]};
const NEGX_POSY_NEGZ_VERT : Vertex  = Vertex{ position : [0.0,1.0,0.0]};
const NEGX_POSY_POSZ_VERT : Vertex  = Vertex{ position : [0.0,1.0,1.0]};
const NEGX_NEGY_POSZ_VERT : Vertex  = Vertex{ position : [0.0,0.0,1.0]};
const NEGX_NEGY_NEGZ_VERT : Vertex  = Vertex{ position : [0.0,0.0,0.0]};

const POSITIVE_X_FACE : [Vertex; 6] = [
		POSX_POSY_NEGZ_VERT,
		POSX_POSY_POSZ_VERT,
		POSX_NEGY_POSZ_VERT,
		//-Second triangle:
		POSX_NEGY_POSZ_VERT,
		POSX_NEGY_NEGZ_VERT,
		POSX_POSY_NEGZ_VERT ];

const NEGATIVE_X_FACE : [Vertex; 6] = [
		//-First triangle:
		NEGX_POSY_POSZ_VERT,
		NEGX_POSY_NEGZ_VERT,
		NEGX_NEGY_NEGZ_VERT,
		//-Second triangle
		NEGX_NEGY_NEGZ_VERT,
		NEGX_NEGY_POSZ_VERT,
		NEGX_POSY_POSZ_VERT ];

const POSITIVE_Y_FACE : [Vertex; 6] = [
		//-First triangle:
		NEGX_POSY_NEGZ_VERT,
		NEGX_POSY_POSZ_VERT,
		POSX_POSY_POSZ_VERT,
		//-Second triangle
		POSX_POSY_POSZ_VERT,
		POSX_POSY_NEGZ_VERT,
		NEGX_POSY_NEGZ_VERT ];
		
const NEGATIVE_Y_FACE : [Vertex; 6] = [
		//-First triangle:
		POSX_NEGY_NEGZ_VERT,
		POSX_NEGY_POSZ_VERT,
		NEGX_NEGY_POSZ_VERT,
		//-Second triangle
		NEGX_NEGY_POSZ_VERT,
		NEGX_NEGY_NEGZ_VERT,
		POSX_NEGY_NEGZ_VERT ];

const POSITIVE_Z_FACE : [Vertex; 6] = [
		//-First triangle:
		POSX_POSY_POSZ_VERT,
		NEGX_POSY_POSZ_VERT,
		NEGX_NEGY_POSZ_VERT,
		//-Second triangle
		NEGX_NEGY_POSZ_VERT,
		POSX_NEGY_POSZ_VERT,
        POSX_POSY_POSZ_VERT ];

const NEGATIVE_Z_FACE : [Vertex; 6] = [
		//-First triangle:
		NEGX_POSY_NEGZ_VERT,
		POSX_POSY_NEGZ_VERT,
		POSX_NEGY_NEGZ_VERT,
		//-Second triangle
		POSX_NEGY_NEGZ_VERT,
		NEGX_NEGY_NEGZ_VERT,
		NEGX_POSY_NEGZ_VERT ];
        
const FULL_CUBE : [[Vertex; 6]; 6] = [
    POSITIVE_X_FACE,
    NEGATIVE_X_FACE,
    POSITIVE_Y_FACE,
    NEGATIVE_Y_FACE,
    POSITIVE_Z_FACE,
    NEGATIVE_Z_FACE
];

pub fn mesh_voxels(vs : &VoxelStorage<bool, u32>, context : &glium::backend::glutin_backend::GlutinFacade) -> glium::VertexBuffer<Vertex> {
    let mut localbuffer : Vec<Vertex> = Vec::new();
    //Unwrap is okay here since you cannot mesh something infinite in size.
    for x in vs.get_x_lower().unwrap() .. vs.get_x_upper().unwrap() as u32 {
        for y in vs.get_y_lower().unwrap() .. vs.get_y_upper().unwrap() as u32 {
            for z in vs.get_z_lower().unwrap() .. vs.get_z_upper().unwrap() as u32 {
                let result = vs.get(x, y, z);
                if result.is_some() {
                    if result.unwrap() {
                        //We have a filled cube.
                        //Iterate over faces:
                        for ii in 0..6 {
                            for jj in 0..6 {
                                let mut temp_vert = FULL_CUBE[ii][jj];
                                temp_vert.position[0] += x as f32;
                                temp_vert.position[1] += y as f32;
                                temp_vert.position[2] += z as f32;
                                localbuffer.push(temp_vert);
                            }
                        }
                    }
                }
            }
        }
    }
    let vertexbuffer = glium::vertex::VertexBuffer::new(context, localbuffer.as_slice()).unwrap();
    return vertexbuffer;
}