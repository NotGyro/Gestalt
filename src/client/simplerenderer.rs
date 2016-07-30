
extern crate glium;
extern crate image;

#[allow(unused_parens)]

use voxel::voxelstorage::VoxelStorage;
use std::vec::Vec;
use std::collections::HashMap;
use voxel::material::MaterialID;
use voxel::material::Material;
use glium::texture::RawImage2d;
use glium::texture::Texture2dArray;
use glium::texture::Texture2dDataSource;
use glium::backend::glutin_backend::GlutinFacade;
use client::materialart::MaterialArt;
use client::materialart::MatArtSimple;
use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::ops::Deref;
use std::cell::RefCell;
use std::mem;

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
    pub fn set_tex_id(&mut self, value : u32) {
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
    pub fn get_tex_id(&self) -> u32 {
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

//pub type Pixel = (f32, f32, f32, f32);
//pub type ImageBuffer = Vec<u8>;

/*
pub struct MatArtRegistry<'a> { 
    //You can only make a MatArtSimple into a cube anyway.
    mat_mapping : HashMap<MaterialID, MatArtSimple>,
    //The key here is "texture name," so here we've got a texture name to Texture Array layer mapping.
    tex_mapping : HashMap<String, usize>,
    //Cached image data, indexable by tex_mapping's values
    tex_data : Vec<ImageBuffer>,
    textures : Box< Texture2dArray >,
    pub tex_width : u32, 
    pub tex_height : u32, 
    pub max_tex : usize,
    pub display : &'a GlutinFacade,
}

impl <'a> MatArtRegistry<'a> { 
    pub fn new(facade : &'a GlutinFacade, twidth : u32, theight : u32, tmax : usize) -> MatArtRegistry { 
        MatArtRegistry {
            mat_mapping : HashMap::new(),
            tex_mapping : HashMap::new(),
            tex_data : Vec::new(),
            display : facade,
            max_tex : tmax,
            tex_width : twidth,
            tex_height : theight,
            textures : Box::new(Texture2dArray::empty(facade, 1, 1, 1).unwrap()),
        }
    }
    pub fn has_art(self, mat : MaterialID) -> bool { self.mat_mapping.contains_key(&mat) }
    pub fn reg_material_art(mut self, mat : MaterialID, art : MatArtSimple) {
        self.mat_mapping.insert(mat, art.clone());
        
        let idx = self.tex_data.len();
        self.tex_mapping.insert(art.texture_name.clone(), idx);
        let image = Self::ld_image(art.texture_name.clone(), self.tex_width, self.tex_height); 
        self.tex_data.push(image);
    }
    
    fn ld_image(path : String, size_x : u32, size_y : u32) -> ImageBuffer {
        let mut texfile = File::open(path).unwrap();
        let image = image::load(&texfile,
                            image::PNG).unwrap().to_rgba();
        let image_dimensions = image.dimensions();
        assert_eq!(image_dimensions, (size_x, size_y));
        //Example code I'm working from used from_raw_rgba_reversed, not sure why.
        //let ri2d = RawImage2d::from_raw_rgba(image.into_raw(), image_dimensions);
        //let () = image; //A hack to get the compiler to print me the type of image.
        return unsafe { mem::transmute(image.into_raw()) };
    }
    
    fn rebuild_textures(mut self) {
        let arr_result = Texture2dArray::new(self.display, self.tex_data.clone());
        match arr_result {
            Ok(v) => self.textures = Box::new(v),
            Err(e) => println!("{}", e),
        }
    }
    
    fn get_index_for(self, id : MaterialID) -> usize {
        let art = self.mat_mapping.get(&id).unwrap();
        return *self.tex_mapping.get(&art.texture_name.clone()).unwrap();
    }
}*/
pub struct TextureArrayDyn { 
    //The key here is "texture name," so here we've got a texture name to Texture Array layer mapping.
    tex_mapping : HashMap<String, usize>,
    //Cached image data, indexable by tex_mapping's values
    tex_data : Vec<Vec<u8>>,
    pub textures : Option<Texture2dArray>,
    pub tex_width : u32, 
    pub tex_height : u32, 
    pub max_tex : usize,
}
impl TextureArrayDyn { 
    pub fn new(twidth : u32, theight : u32, tmax : usize) -> TextureArrayDyn { 
        TextureArrayDyn {
            tex_mapping : HashMap::new(),
            tex_data : Vec::new(),
            max_tex : tmax,
            tex_width : twidth,
            tex_height : theight,
            textures : None,
        }
    }
    pub fn has_tex(&self, name : String) -> bool { self.tex_mapping.contains_key(&name) }
    pub fn index_for_tex(&self, name : String) -> usize { *self.tex_mapping.get(&name).unwrap() }
    pub fn add_tex(&mut self, texname : String, display : &GlutinFacade) {

        let idx = self.tex_data.len();
        self.tex_mapping.insert(texname.clone(), idx);
        let image = Self::ld_image(display, texname, self.tex_width, self.tex_height); 
        self.tex_data.push(image);
        assert!(self.tex_data.len() < self.max_tex);
    }
    
    fn ld_image(display : &GlutinFacade, path : String, size_x : u32, size_y : u32) -> Vec<u8> {
        let mut texfile = File::open(path).unwrap();
        let image = image::load(&texfile,
                            image::PNG).unwrap().to_rgba();
        let image_dimensions = image.dimensions();
        assert_eq!(image_dimensions, (size_x, size_y));
        //According to compiler errors, a Piston image module image struct's into_raw() returns a Vec<u8>.
        return image.into_raw();
        //let buffer = image.into_raw();
        //return glium::texture::RawImage2d::from_raw_rgba(buffer, (size_x, size_y));
    }
    
    fn rebuild<'a>(&mut self, display : &GlutinFacade) {
        println!(line!());
        let mut converted_buffer : Vec< RawImage2d<'a, u8>> = Vec::new();
        //Satisfy glium's type demands
        for image in self.tex_data.iter() {
            converted_buffer.push(RawImage2d::from_raw_rgba((*image).clone(), (self.tex_width, self.tex_height)));
        }
        let arr_result = Texture2dArray::new(display, converted_buffer);
        match arr_result {
            Ok(v) => self.textures = Some(v),
            Err(e) => {
                self.textures = None;
                println!("{}", e) },
        }
        println!(line!());
    }
}
#[derive(Copy, Clone, Debug)]
struct VoxelRenderInfo {
    pub x : u16, 
    pub y : u16,
    pub z : u16,
    pub tex_idx : u32,
}
pub type MatArtMapping = HashMap<MaterialID, MatArtSimple>;

pub fn make_voxel_mesh(vs : &VoxelStorage<MaterialID, u16>, display : &GlutinFacade, textures : &mut TextureArrayDyn, art_map : &MatArtMapping)
                            -> glium::VertexBuffer<PackedVertex> {
    println!(line!());
    //This function is still very not data-oriented and will probably cause the borrow checker to become very upset.
    println!(line!());
    let mut drawable : Vec<VoxelRenderInfo> = Vec::new();
    let mut rebuild_tex : bool = false;
    for x in vs.get_x_lower().unwrap() .. vs.get_x_upper().unwrap() as u16 {
        for y in vs.get_y_lower().unwrap() .. vs.get_y_upper().unwrap() as u16 {
            for z in vs.get_z_lower().unwrap() .. vs.get_z_upper().unwrap() as u16 {
                let result = vs.get(x, y, z);
                if result.is_some() {
                    let mat_id = result.unwrap();
                    if(art_map.contains_key(&mat_id.clone())) {
                        let art = art_map.get(&mat_id).unwrap();
                        if(!textures.has_tex(art.texture_name.clone())) {
                            textures.add_tex(art.texture_name.clone(), display); //Load our texture if we haven't already.
                            rebuild_tex = true;
                        }
                        let idx = textures.index_for_tex(art.texture_name.clone());
                        let vri = VoxelRenderInfo { x : x, y : y, z : z, tex_idx : idx as u32 };
                        drawable.push(vri);
                    }
                }
            }
        }
    }
    println!(line!());
    if(rebuild_tex) { 
        textures.rebuild(display);
    }
    println!(line!());
    return mesh_step(drawable, display);
}

fn mesh_step(drawable : Vec<VoxelRenderInfo>, display : &GlutinFacade) -> glium::VertexBuffer<PackedVertex> {
    let mut localbuffer : Vec<PackedVertex> = Vec::new();
    for voxel in drawable.iter() {
        //Iterate over faces:
        for face in 0..6 {
            for vert_iter in 0..6 {
                let x = voxel.x;
                let y = voxel.y;
                let z = voxel.z;
                let mut temp_vert = FULL_CUBE[face][vert_iter];
                temp_vert.position[0] += x as u32;
                temp_vert.position[1] += y as u32;
                temp_vert.position[2] += z as u32;
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
                let mut pv = PackedVertex::from_vertex_uv(temp_vert, u, v);
                pv.set_tex_id(voxel.tex_idx);
                localbuffer.push(pv);
            }
        }
    }
    let vertexbuffer = glium::vertex::VertexBuffer::new(display, localbuffer.as_slice()).unwrap();
    return vertexbuffer;
}
/*
//The Voxel Storage argument should actually be a Voxel Storage of texture array indicies. 0 is reserved for "none."
pub fn mesh_voxels<'a>(vs : &VoxelStorage<u32, u32>, display : &glium::backend::glutin_backend::GlutinFacade, tex_manager : &TextureArrayDyn) -> glium::VertexBuffer<PackedVertex> {
    let mut localbuffer : Vec<PackedVertex> = Vec::new();
    //Unwrap is okay here since you cannot mesh something infinite in size.
    for x in vs.get_x_lower().unwrap() .. vs.get_x_upper().unwrap() as u32 {
        for y in vs.get_y_lower().unwrap() .. vs.get_y_upper().unwrap() as u32 {
            for z in vs.get_z_lower().unwrap() .. vs.get_z_upper().unwrap() as u32 {
                let result = vs.get(x, y, z);
                if result.is_some() {
                    let matID = result.unwrap();
                    if(tex_manager.has_art(matID.clone())) {
                    //if matID != "Air" {
                        //We have a filled cube.
                        let texID = tex_manager.get_index_for(matID);
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
                                let mut pv = PackedVertex::from_vertex_uv(temp_vert, u, v);
                                pv.set_tex_id(texID as u32);
                                localbuffer.push(pv);
                            }
                        }
                    }
                }
            }
        }
    }
    let vertexbuffer = glium::vertex::VertexBuffer::new(display, localbuffer.as_slice()).unwrap();
    return vertexbuffer;
} */