
extern crate glium;
extern crate image;

#[allow(unused_parens)]

use voxel::voxelstorage::VoxelStorage;
use voxel::material::MaterialID;
use voxel::material::Material;
use util::voxelutil::*;

use client::materialart::MaterialArt;
use client::materialart::MatArtSimple;

use std::vec::Vec;
use std::collections::HashMap;

use glium::texture::RawImage2d;
use glium::texture::Texture2dArray;
use glium::texture::Texture2dDataSource;
use glium::backend::glutin_backend::GlutinFacade;
use glium::Frame;
use glium::Surface;

use cgmath::{Matrix4, Vector3, Vector4, Point3, InnerSpace};

use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::io::BufReader;
use std::ops::Deref;
use std::cell::RefCell;
use std::mem;
use std::collections::HashSet;

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

fn get_face_verts(side: VoxelAxis) -> [Vertex; 6] {
    match side {
        VoxelAxis::PosiX => return POSITIVE_X_FACE,
        VoxelAxis::NegaX => return NEGATIVE_X_FACE,
        VoxelAxis::PosiY => return POSITIVE_Y_FACE,
        VoxelAxis::NegaY => return NEGATIVE_Y_FACE,
        VoxelAxis::PosiZ => return POSITIVE_Z_FACE,
        VoxelAxis::NegaZ => return NEGATIVE_Z_FACE,
    }
}

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
    pub fn has_tex(&self, name : &String) -> bool { self.tex_mapping.contains_key(name) }
    pub fn index_for_tex(&self, name : &String) -> usize { *self.tex_mapping.get(name).unwrap() }
    pub fn add_tex(&mut self, texname : &String, display : &GlutinFacade) {

        let idx = self.tex_data.len();
        self.tex_mapping.insert(texname.clone(), idx);
        let image = Self::ld_image(display, texname, self.tex_width, self.tex_height); 
        self.tex_data.push(image);
        assert!(self.tex_data.len() < self.max_tex);
    }
    
    fn ld_image(display : &GlutinFacade, path_name : &String, size_x : u32, size_y : u32) -> Vec<u8> {        
        let path = Path::new(path_name.as_str());

        let image = image::open(path).unwrap().to_rgba();
        let image_dimensions = image.dimensions();
        assert_eq!(image_dimensions, (size_x, size_y));
        //According to compiler errors, a Piston image module image struct's into_raw() returns a Vec<u8>.
        println!("Loaded texture file: {}", path_name.clone());
        return image.into_raw();
    }
    
    fn rebuild<'a>(&mut self, display : &GlutinFacade) {
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
    }
}

//This is for the old implementation.
#[derive(Copy, Clone, Debug)]
struct VoxelRenderInfo {
    pub x : u16, 
    pub y : u16,
    pub z : u16,
    pub tex_idx : u32,
}

#[derive(Copy, Clone, Debug)]
struct SideRenderInfo {
    pub side : VoxelAxis,
    pub x : u16, 
    pub y : u16,
    pub z : u16,
    pub tex_idx : u32,
}

pub type MatArtMapping = HashMap<MaterialID, MatArtSimple>;

pub struct SimpleVoxelMesher {
    meshes : HashMap<VoxelRange<i32>, Box<glium::VertexBuffer<PackedVertex>>>,
    remesh_list: HashSet<VoxelRange<i32>>,
    texture_manager : TextureArrayDyn,
}
impl SimpleVoxelMesher {
    pub fn new() -> Self {
        SimpleVoxelMesher { 
            meshes : HashMap::new(),
            remesh_list : HashSet::new(),
            texture_manager : TextureArrayDyn::new(64, 64, 4096),
        }
    }
    /// Add the mesh at this location to the list of meshes which need to be re-built.
    pub fn notify_remesh(&mut self, pos : VoxelPos<i32>) {
        for(range, _) in &self.meshes { 
            if(range.contains(pos)) {
                if(!self.remesh_list.contains(range)){
                    self.remesh_list.insert(*range);
                    //If this is on a side of the range, be sure to update the neighboring range.
                    for side in VoxelAxis::iter_all() {
                        if(range.is_on_side(pos, side)) {
                            let neighbor = pos.get_neighbor(side);
                            for(range1, _) in &self.meshes { 
                                if(range1.contains(neighbor)) {
                                    if(!self.remesh_list.contains(range1)){
                                        self.remesh_list.insert(*range1);
                                    }
                                }
                            }
                        }
                    }
                    //Adjacency crap ends here.
                }
            }
        }
    }
    /// Re-mesh every updated mesh in this voxel storage.
    pub fn process_remesh(&mut self, vs : &VoxelStorage<MaterialID, i32>, 
                    display : &GlutinFacade, art_map : &MatArtMapping) {
        //We use drain here to clear the list and iterate through it at the same time
        for coords in self.remesh_list.drain() { 
            if(self.meshes.contains_key(&coords)) {
                self.meshes.remove(&coords);
                self.meshes.insert(coords, Box::new( make_voxel_mesh(vs, display, coords, &mut self.texture_manager, art_map)) );
                //Only add the mesh if we had it before.
            }
        }
        assert!(self.remesh_list.len() == 0);
    }
    
    /// Immediately add a mess for these coordinates to the renderer.
    pub fn force_mesh(&mut self, vs : &VoxelStorage<MaterialID, i32>, display : &GlutinFacade, range : VoxelRange<i32>,
                        art_map : &MatArtMapping) {
        if(self.meshes.contains_key(&range)) {
            self.meshes.remove(&range);
        }
        self.meshes.insert(range, Box::new( make_voxel_mesh(vs, display, range, &mut self.texture_manager, art_map)) );
    }

    /// Draw the meshes within an OpenGL context.
    pub fn draw(&mut self, perspective_matrix : Matrix4<f32>, view_matrix : Matrix4<f32>,
            target : &mut glium::Frame, program : &glium::Program, params : &glium::DrawParameters) {
        let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);
        match self.texture_manager.textures {
            Some(ref textures) => {
                for (bounds, ref mesh) in &self.meshes {
                    //Create a context so uniforms dies and textures is no longer borrowed.
                    {
                        let pos = bounds.lower;
                        let chunk_model_matrix = Matrix4::from_translation(Vector3{ x : pos.x as f32, y : pos.y  as f32, z : pos.z  as f32 });
                        let mvp_matrix = perspective_matrix * view_matrix * chunk_model_matrix;
                        let uniforms = uniform! {
                            mvp: Into::<[[f32; 4]; 4]>::into(mvp_matrix),
                            tex: textures,
                        };
                        target.draw(&(***mesh), &indices, program, &uniforms,
                            params).unwrap();
                    }
                }
            },
            None => (),
        }
    }
}
/*
This implementation is slow as shit and I don't know why???
pub fn make_voxel_mesh(vs : &VoxelStorage<MaterialID, i32>, display : &GlutinFacade, range : VoxelRange<i32>, 
                        textures : &mut TextureArrayDyn, art_map : &MatArtMapping)
                            -> glium::VertexBuffer<PackedVertex> {
    let mut drawable : Vec<SideRenderInfo> = Vec::new();
    let mut rebuild_tex : bool = false;
    for side in VoxelAxis::iter_all() { //Go over each side.
        for origin in range.get_side_iterator(side) { //Iterate over each voxel lying on this edge of the side.
            let mut pos = origin;
            //If we start at, for example, negative X, we need to march towards positive X and stop when we hit positive X bound. 
            let mut cull_next : bool = false;
            while pos.coord_for_axis(side) != range.get_bound(side.opposite()) {
                let result = vs.getv(pos);
                if result.is_some() {
                    let x = pos.x; 
                    let y = pos.y;
                    let z = pos.z;
                    let mat_id = result.unwrap();
                    //println!("{}", mat_id.name);
                    if(art_map.contains_key(&mat_id.clone())) {
                        let art = art_map.get(&mat_id).unwrap();
                        //TODO: Make culling depend on material art.
                        //Is the previous one culling this side?
                        if(!cull_next) {
                            cull_next = true;
                            if(!textures.has_tex(art.texture_name.clone())) {
                                textures.add_tex(art.texture_name.clone(), display); //Load our texture if we haven't already.
                                rebuild_tex = true;
                            }
                            let idx = textures.index_for_tex(art.texture_name.clone());
                            let vri = SideRenderInfo { 
                                side : side,
                                x : (x - range.lower.x) as u16, 
                                y : (y - range.lower.y) as u16, 
                                z : (z - range.lower.z) as u16, 
                                tex_idx : idx as u32 };
                            drawable.push(vri);
                        }
                    }
                    else { 
                        //Don't cull the next block if this one doesn't render in this step (for example, air).
                        cull_next = false;
                    }
                }
                else { 
                    cull_next = false; //Don't cull the next block if this one is a None().
                }
                //If we start at, for example, negative X, we need to march towards positive X and stop when we hit positive X bound. 
                pos = pos.get_neighbor(side.opposite()); //Iterate.
            }
        }
    }
    if(rebuild_tex) { 
        textures.rebuild(display);
    }
    //println!("Found {} drawable cubes.", drawable.len());
    return mesh_step(drawable, display);
}
*/

pub fn make_voxel_mesh(vs : &VoxelStorage<MaterialID, i32>, display : &GlutinFacade, range : VoxelRange<i32>, 
                        textures : &mut TextureArrayDyn, art_map : &MatArtMapping)
                            -> glium::VertexBuffer<PackedVertex> {
    let mut drawable : Vec<SideRenderInfo> = Vec::new();
    let mut rebuild_tex : bool = false;

    for pos in range {
        let result = vs.getv(pos);
        if result.is_some() {
            let x = pos.x; 
            let y = pos.y;
            let z = pos.z;
            let mat_id = result.unwrap();
            //println!("{}", mat_id.name);
            if(art_map.contains_key(&mat_id)) {
                let art = art_map.get(&mat_id).unwrap();
                if(!textures.has_tex(&art.texture_name)) {
                    textures.add_tex(&art.texture_name, display); //Load our texture if we haven't already.
                    rebuild_tex = true;
                }
                let idx = textures.index_for_tex(&art.texture_name);
                for side in VoxelAxis::iter_all() {
                    let mut cull = false;
                    let neighbor = vs.getv(pos.get_neighbor(side));
                    if(neighbor.is_some()) {
                        let neighbor_mat = neighbor.unwrap();
                        if(art_map.contains_key(&neighbor_mat)) {
                            cull = true; //TODO: Make this depend on specific MaterialArts.
                        }
                    }
                    if cull == false {
                        let vri = SideRenderInfo { 
                            side : side,
                            x : (x - range.lower.x) as u16, 
                            y : (y - range.lower.y) as u16, 
                            z : (z - range.lower.z) as u16, 
                            tex_idx : idx as u32 };
                        drawable.push(vri);
                    }
                }
            }
        }
    }
    if(rebuild_tex) { 
        textures.rebuild(display);
    }
    //println!("Found {} drawable cubes.", drawable.len());
    return mesh_step(drawable, display);
}

fn mesh_step(drawable : Vec<SideRenderInfo>, display : &GlutinFacade) -> glium::VertexBuffer<PackedVertex> {
    //TODO: The stuff which will make only certain sides even try to render, depending on the player's current angle.
    let mut localbuffer : Vec<PackedVertex> = Vec::new();
    for voxel in drawable.iter() {
        for vert_iter in 0..6 {
            let x = voxel.x;
            let y = voxel.y;
            let z = voxel.z;
            let mut temp_vert = get_face_verts(voxel.side)[vert_iter];
            temp_vert.position[0] += x as u32;
            temp_vert.position[1] += y as u32;
            temp_vert.position[2] += z as u32;
            let mut u : u32 = 0;
            let mut v : u32 = 0;

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
    let vertexbuffer = glium::vertex::VertexBuffer::new(display, localbuffer.as_slice()).unwrap();
    return vertexbuffer;
}