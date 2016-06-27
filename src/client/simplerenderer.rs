
extern crate glium;

#[allow(unused_parens)]

use voxel::voxelstorage::VoxelStorage;
use std::vec::Vec;
use util::cubeaxis::CubeAxis;
use std::collections::HashMap;
use voxel::material::MaterialID;
use voxel::material::Material;
use glium::texture::RawImage2d;
use glium::texture::Texture2dArray;
use glium::backend::glutin_backend::GlutinFacade;
use client::materialart::MaterialArt;
use client::materialart::MatArtSimple;

#[derive(Copy, Clone)]
pub struct PackedVertex {
    vertexdata : u32,
}

implement_vertex!(PackedVertex, vertexdata);

#[derive(Copy, Clone)]
pub struct Vertex {
    position: [u32; 3],
}

//Bitmask
//V. U, tex, Z, Y, X
//b0_0_000000000000_000000_000000_000000
impl PackedVertex {
    pub fn set_x(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_000000000000_000000_000000_111111;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        let mut val = value;
        val = val & bitmask;
        self.vertexdata = self.vertexdata | val; //Set our value
    }
    pub fn set_y(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_000000000000_000000_111111_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        let mut val = value;
        val = val << 6;
        val = val & bitmask;
        self.vertexdata = self.vertexdata | val; //Set our value
    }
    pub fn set_z(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_000000000000_111111_000000_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        let mut val = value;
        val = val << 12;
        val = val & bitmask;
        self.vertexdata = self.vertexdata | val; //Set our value
    }
    pub fn set_texID(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_111111111111_000000_000000_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        let mut val = value;
        val = val << 18;
        val = val & bitmask;
        self.vertexdata = self.vertexdata | val; //Set our value
    }
    pub fn set_u_high(&mut self, value : bool) {
        let bitmask : u32 = 0b0_1_000000000000_000000_000000_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        if value {
            self.vertexdata = self.vertexdata | bitmask;
        }
        //No need for an else case - if our value is low the bit will remain 0 from clearing it out.
    }
    pub fn set_v_high(&mut self, value : bool) {
        let bitmask : u32 = 0b1_0_000000000000_000000_000000_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        if value {
            self.vertexdata = self.vertexdata | bitmask;
        }
    }
    
    pub fn get_x(&self) -> u32 {
        let bitmask : u32 = 0b0_0_000000000000_000000_000000_111111;
        let mut val = self.vertexdata & bitmask;
        return val;
    }
    pub fn get_y(&self) -> u32 {
        let bitmask : u32 = 0b0_0_000000000000_000000_111111_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 6;
        return val;
    }
    pub fn get_z(&self) -> u32 {
        let bitmask : u32 = 0b0_0_000000000000_111111_000000_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 12;
        return val;
    }
    pub fn get_texID(&self) -> u32 {
        let bitmask : u32 = 0b0_0_111111111111_000000_000000_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 18;
        return val;
    }
    pub fn get_u_high(&self) -> bool {
        let bitmask : u32 = 0b0_1_000000000000_000000_000000_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 30;
        return val > 0;
    }
    pub fn get_v_high(&self) -> bool {
        let bitmask : u32 = 0b1_0_000000000000_000000_000000_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 31;
        return val > 0;
    }
    
    pub fn from_vertex_uv(vert : Vertex, u : u32, v : u32) -> PackedVertex { 
        let mut ret = PackedVertex { vertexdata : 0,};
        ret.set_x(vert.position[0]);
        ret.set_y(vert.position[1]);
        ret.set_z(vert.position[2]);
        if(u >= 1) {
            ret.set_u_high(true);
        }
        if(v >= 1) {
            ret.set_v_high(true);
        }
        return ret;
    }
}

const POSX_POSY_POSZ_VERT : Vertex  = Vertex{ position : [1,1,1]};
const POSX_POSY_NEGZ_VERT : Vertex  = Vertex{ position : [1,1,0]};
const POSX_NEGY_NEGZ_VERT : Vertex  = Vertex{ position : [1,0,0]};
const POSX_NEGY_POSZ_VERT : Vertex  = Vertex{ position : [1,0,1]};
const NEGX_POSY_NEGZ_VERT : Vertex  = Vertex{ position : [0,1,0]};
const NEGX_POSY_POSZ_VERT : Vertex  = Vertex{ position : [0,1,1]};
const NEGX_NEGY_POSZ_VERT : Vertex  = Vertex{ position : [0,0,1]};
const NEGX_NEGY_NEGZ_VERT : Vertex  = Vertex{ position : [0,0,0]};

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

//Please do not make either of these public.
type Pixel = (f32, f32, f32, f32);
type ImageBuffer = Vec<Vec<Pixel>>;

pub struct MatArtRegistry { 
    //You can only make a MatArtSimple into a cube anyway.
    mat_mapping : HashMap<MaterialID, MatArtSimple>,
    //The key here is "texture name," so here we've got a texture name to Texture Array layer mapping.
    tex_mapping : HashMap<String, u32>,
    //Cached image data, indexable by tex_mapping's values
    tex_data : Vec<ImageBuffer>,
    textures : Option<Box<Texture2dArray<GlutinFacade>>>,
    tex_width : u32, 
    tex_height : u32, 
    max_tex : u32,
    display : GlutinFacade,
}

impl MatArtRegistry { 
    pub fn new(display : GlutinFacade, tex_width : u32, tex_height : u32, max_tex : u32) -> &Self { 
        let result : Box<MatArtRegistry> = Box::new();
        result.mat_mapping = HashMap::new();
        result.display = display;
        result.max_tex = max_tex;
        result.tex_width = tex_width;
        result.tex_height = tex_height;
        result.textures = None();
        
        //textures = Texture2dArray::empty(display, tex_width, tex_height, max_tex); 
        return result;
    }
    pub fn reg_material_art(&self, mat : MaterialID, art : MaterialArt) {
        if(art.get_render_type() == 1)
        if(self.mat_mapping::contains_key(mat)) { 
            self.mat_mapping[mat] = art;
        }
        else {
            self.mat_mapping.insert(mat, art);
        }
    }
    
    fn ld_image(&self, path : String) -> &ImageBuffer {
        let mut texfile = File::open(path).unwrap();
        let image = image::load(&texfile,
                            image::PNG).unwrap().to_rgba();
        let image_dimensions = image.dimensions();
        assert_eq!(image_dimensions, (self.tex_width, self.tex_height));
        //Example code I'm working from used from_raw_rgba_reversed, not sure why.
        let image = glium::texture::RawImage2d::from_raw_rgba(image.into_raw(), image_dimensions);
        return image.data.deref();
    }
}


//The mesh function as we have it now.
pub fn mesh_voxels(vs : &VoxelStorage<MaterialID, u32>, context : &glium::backend::glutin_backend::GlutinFacade, tex_manager : MatArtRegistry) -> glium::VertexBuffer<PackedVertex> {
    let mut localbuffer : Vec<PackedVertex> = Vec::new();
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
                            for vert_iter in 0..6 {
                                let mut temp_vert = FULL_CUBE[ii][vert_iter];
                                temp_vert.position[0] += x;
                                temp_vert.position[1] += y;
                                temp_vert.position[2] += z;
                                let mut u : u32 = 0;
                                let mut v : u32 = 0;
                                //println!("{}", pv.vertexdata);
                                //println!("{}", pv.get_x());
                                //println!("{}", pv.get_y());
                                //println!("{}", pv.get_z());
                                //Do our UV the hacky way.
                                if (vert_iter == 2) || (vert_iter == 3) {
                                    u = 0;
                                    v = 1;
                                }
                                else if (vert_iter == 0) || (vert_iter == 5) {
                                    u = 1;
                                    v = 0;
                                }
                                else if vert_iter == 1 {
                                    u = 0;
                                    v = 0;
                                }
                                else if vert_iter == 4 {
                                    u = 1;
                                    v = 1;
                                }
                                let pv = PackedVertex::from_vertex_uv(temp_vert, u, v);
                                println!("{}", pv.get_u_high());
                                println!("{}", pv.get_v_high());
                                localbuffer.push(pv);
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