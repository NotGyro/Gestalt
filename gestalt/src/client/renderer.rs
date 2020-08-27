use crate::world::*;
use crate::world::chunk::*;
use crate::client::tileart::*;

use crate::util::voxelmath::*;

use std::collections::HashMap;

use glium::texture::RawImage2d;
use glium::texture::Texture2dArray;
use glium::Surface;
use glium::backend::glutin::Display;

use cgmath::{Matrix4, Vector3}; //, Vector4, Point3, InnerSpace};

use ustr::*;

use std::path::Path;
use std::collections::HashSet;

custom_error!{ pub ImageLoadError
    MissingFile{tile:TileId, file:String} = "Attempted to associate tile {tile} with texture {file} which hasn't been loaded.",
}


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
        let val = self.vertexdata & bitmask;
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
        if u >= 1 {
            ret.set_u_high(true);
        }
        if v >= 1 {
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
    tex_mapping : UstrMap<usize>,
    //Tile to idx
    pub tile_mapping : UstrMap<(usize, TileArtSimple)>,
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
            tex_mapping : UstrMap::default(),
            tile_mapping : UstrMap::default(),
            tex_data : Vec::new(),
            max_tex : tmax,
            tex_width : twidth,
            tex_height : theight,
            textures : None,
        }
    }
    pub fn has_tex(&self, name : &Ustr) -> bool { self.tex_mapping.contains_key(name) }
    pub fn index_for_tex(&self, name : &Ustr) -> usize { *self.tex_mapping.get(name).unwrap() }
    pub fn art_for_tile(&self, name : &Ustr) -> (usize, TileArtSimple) { self.tile_mapping.get(name).unwrap().clone() }
    pub fn has_tile(&self, name : &Ustr) -> bool { self.tile_mapping.contains_key(name) }
    pub fn add_tex(&mut self, texname : Ustr) {

        let idx = self.tex_data.len();
        self.tex_mapping.insert(texname.clone(), idx);
        let image = Self::ld_image(texname, self.tex_width, self.tex_height); 
        self.tex_data.push(image);
        assert!(self.tex_data.len() < self.max_tex);
    }
    
    pub fn ld_image(path_name : Ustr, size_x : u32, size_y : u32) -> Vec<u8> {        
        let path = Path::new(path_name.as_str());

        let image = image::open(path).unwrap().to_rgba();
        let image_dimensions = image.dimensions();
        assert_eq!(image_dimensions, (size_x, size_y));
        //According to compiler errors, a Piston image module image struct's into_raw() returns a Vec<u8>.
        info!(Renderer, "Loaded texture file: {}", path_name.clone());
        return image.into_raw();
    }
    
    pub fn rebuild<'a>(&mut self, display : &Display) {
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
                error!(Renderer, "Could not add a texture array: {}", e) },
        }
    }
    pub fn associate_tile(&mut self, display : &Display, tile: TileId, art: TileArtSimple) -> Result<(), Box<dyn std::error::Error>> {
        match art.visible {
            true => {
                self.add_tex(art.texture_name);
                let tex = self.tex_mapping.get(&art.texture_name);
                let tex_unwrap = tex.ok_or(ImageLoadError::MissingFile{tile: tile, file:art.texture_name.to_string()})?;
                self.tile_mapping.insert(tile, (*tex_unwrap, art));
                self.rebuild(display);
            },
            false => {
                //self.tile_mapping.insert(tile, (0, art));
            }
        }
        Ok(())
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

#[derive(Clone)]
enum ArtCacheInner {
    Uniform( (usize, TileArtSimple) ),
    Small( [(usize, TileArtSimple);256] ),
    Large( HashMap<u16, (usize, TileArtSimple)>),
}
#[derive(Clone)]
struct ArtCache(Option<ArtCacheInner>);

impl ArtCache {
    pub fn build_from(chunk: &Chunk, tex: &TextureArrayDyn) -> Self {
        match &chunk.inner {
            ChunkInner::Uniform(val) => {
                if tex.has_tile(&val) {
                    let art = tex.art_for_tile(&val);
                    ArtCache{0: Some(
                        ArtCacheInner::Uniform( art )
                    )}
                }
                else {
                    ArtCache{0: None}
                }
            }
            ChunkInner::Small(inner) => {
                let mut length : usize = 0;
                let mut list: [(usize, TileArtSimple); 256] = arr![(0,TileArtSimple::default());256];
                for i in 0..(inner.highest_idx as usize) {
                    length += 1;
                    let value = inner.palette[i];
                    let art = tex.art_for_tile(&value);
                    list[i] = art;
                }
                if length != 0 {
                    ArtCache{0: Some(
                        ArtCacheInner::Small(list)
                    )}
                }
                else {
                    ArtCache{0: None}
                }
            }
            ChunkInner::Large(inner) => {
                let mut list: HashMap<u16, (usize, TileArtSimple)> = HashMap::new();
                for (key, value) in inner.palette.iter() {
                    let art = tex.art_for_tile(&value);
                    list.insert(*key, art);
                }
                if list.len() != 0 {
                    ArtCache{0: Some(
                        ArtCacheInner::Large(list)
                    )}
                }
                else {
                    ArtCache{0: None}
                }
            }
        }
    }
    #[inline(always)]
    pub fn get_mapping(&self, idx: u16) -> Option<(usize, TileArtSimple)> {
        if let Some(inner) = &self.0 {
            match inner {
                ArtCacheInner::Uniform(single) => {
                    if idx == 0 {
                        Some(single.clone())
                    }            
                    else { 
                        None
                    }
                } 
                ArtCacheInner::Small(list) => { 
                    if idx > 255 {
                        None
                    } 
                    else {
                        Some(list[idx as usize].clone())
                    }
                }
                ArtCacheInner::Large(list)  => {
                    list.get(&idx).map(|e| e.clone())
                }
            }
        }
        else {
            None
        }
    }
}

pub struct ChunkMesh {
    pub mesh: glium::VertexBuffer<PackedVertex>,
}
pub struct Renderer {
    meshes : HashMap<ChunkPos, ChunkMesh>,
    remesh_list: HashSet<ChunkPos>,
    pub texture_manager : TextureArrayDyn,
}
impl Renderer {
    pub fn new() -> Self {
        Renderer { 
            meshes : HashMap::new(),
            remesh_list : HashSet::new(),
            texture_manager : TextureArrayDyn::new(64, 64, 4096),
        }
    }
    /// Add the mesh at this location to the list of meshes which need to be re-built.
    pub fn notify_remesh(&mut self, pos : VoxelPos<i32>) {
        for(our_chunk, _) in &self.meshes {
            let range = chunk::CHUNK_RANGE.get_shifted(*our_chunk);
            if range.contains(pos) {
                if !self.remesh_list.contains(our_chunk) {
                    self.remesh_list.insert(*our_chunk);
                    //If this is on a side of the range, be sure to update the neighboring range.
                    voxel_sides_unroll!(DIR, {
                        if range.is_on_side(pos, DIR)  {
                            
                            let neighbor = pos.get_neighbor(DIR);
                            for (neighbor_chunk, _)  in &self.meshes { 
                                let neighbor_range = chunk::CHUNK_RANGE.get_shifted(*neighbor_chunk);
                                if neighbor_range.contains(neighbor)  {
                                    if !self.remesh_list.contains(neighbor_chunk) {
                                        self.remesh_list.insert(*neighbor_chunk);
                                    }
                                }
                            }
                        }
                    });
                    //Adjacency crap ends here.
                }
            }
        }
    }
    /// Re-mesh every updated mesh in this voxel storage.
    pub fn process_remesh(&mut self, vs : &Space, 
                    display : &Display) {
        //We use drain here to clear the list and iterate through it at the same time
        for coords in self.remesh_list.drain() {
            if self.meshes.contains_key(&coords) {
                self.meshes.remove(&coords);
                // TODO: More graceful error handling than an unwrap.
                self.meshes.insert(coords, make_voxel_mesh(vs.borrow_chunk(coords).unwrap(), display, &mut self.texture_manager) );
                //Only add the mesh if we had it before.
            }
        }
        assert!(self.remesh_list.len() == 0);
    }
    
    /// Immediately add a mess for these coordinates to the renderer.
    pub fn force_mesh(&mut self, vs : &Space, chunk : ChunkPos, display : &Display) {
        if self.meshes.contains_key(&chunk) {
            self.meshes.remove(&chunk);
        }
        // TODO: More graceful error handling than an unwrap.
        self.meshes.insert(chunk, make_voxel_mesh(vs.borrow_chunk(chunk).unwrap(), display, &self.texture_manager));
    }

    /// Draw the meshes within an OpenGL context.
    pub fn draw<'a>(&'a mut self, perspective_matrix : Matrix4<f32>, view_matrix : Matrix4<f32>,
            target : &mut glium::Frame, program : &glium::Program, params : &glium::DrawParameters) {
        let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);
        match self.texture_manager.textures {
            Some(ref textures) => {
                for (chunk_pos, ref mesh) in &self.meshes {
                    let pos = space::chunk_to_world_pos(*chunk_pos);
                    let chunk_model_matrix = Matrix4::from_translation(Vector3{ x : pos.x as f32, y : pos.y  as f32, z : pos.z  as f32 });
                    let mvp_matrix = perspective_matrix * view_matrix * chunk_model_matrix;
                    let uniforms = uniform! {
                        mvp: Into::<[[f32; 4]; 4]>::into(mvp_matrix),
                        tex: textures,
                    };
                    target.draw( mesh.mesh.slice( .. ).unwrap(), &indices, program, &uniforms,
                        params).unwrap();
                }
            },
            None => (),
        }
    }
}

pub fn make_voxel_mesh(chunk : &Chunk, display : &Display, textures : &TextureArrayDyn)
                            -> ChunkMesh {
    let art_cache = ArtCache::build_from(chunk, &textures);
    let mut drawable : Vec<SideRenderInfo> = Vec::new();

    //The air hack: Return really early if it's a unfiorm mesh of a non-drawable tile. 
    /*if let Some(inner) = &art_cache.0 { 
        if let ArtCacheInner::Uniform(innerinner) = inner {
            if !innerinner.1.visible {
                let localbuffer : Vec<PackedVertex> = Vec::new();
                let vertexbuffer = glium::vertex::VertexBuffer::new(display, localbuffer.as_slice()).unwrap();
                return ChunkMesh{mesh: vertexbuffer};
            }
        }
    }
    else {
        let localbuffer : Vec<PackedVertex> = Vec::new();
        let vertexbuffer = glium::vertex::VertexBuffer::new(display, localbuffer.as_slice()).unwrap();
        return ChunkMesh{mesh: vertexbuffer};
    }*/

    for i in 0..CHUNK_VOLUME {
        let tile = chunk.get_raw_i(i);
        if let Some(art) = art_cache.get_mapping(tile) {
            // Skip it if it's air.
            if !art.1.visible { continue }
            // TODO: Stop repeating myself here. 
            if let Some(neighbor_index) = chunk::get_pos_x_offset(i) {
                let neighbor_tile = chunk.get_raw_i(neighbor_index);
                if let Some(neighbor_art) = art_cache.get_mapping(neighbor_tile) {
                    if !neighbor_art.1.visible {
                        let (x,y,z) = chunk_i_to_xyz(i);
                        let vri = SideRenderInfo {
                            side : VoxelAxis::PosiX,
                            x : x as u16, 
                            y : y as u16,
                            z : z as u16,
                            tex_idx : art.0 as u32 };
                        drawable.push(vri);
                    }
                }
            }
            if let Some(neighbor_index) = chunk::get_neg_x_offset(i) {
                let neighbor_tile = chunk.get_raw_i(neighbor_index);
                if let Some(neighbor_art) = art_cache.get_mapping(neighbor_tile) {
                    if !neighbor_art.1.visible {
                        let (x,y,z) = chunk_i_to_xyz(i);
                        let vri = SideRenderInfo {
                            side : VoxelAxis::NegaX,
                            x : x as u16, 
                            y : y as u16,
                            z : z as u16,
                            tex_idx : art.0 as u32 };
                        drawable.push(vri);
                    }
                }
            }
            if let Some(neighbor_index) = chunk::get_pos_y_offset(i) {
                let neighbor_tile = chunk.get_raw_i(neighbor_index);
                if let Some(neighbor_art) = art_cache.get_mapping(neighbor_tile) {
                    if !neighbor_art.1.visible {
                        let (x,y,z) = chunk_i_to_xyz(i);
                        let vri = SideRenderInfo {
                            side : VoxelAxis::PosiY,
                            x : x as u16, 
                            y : y as u16,
                            z : z as u16,
                            tex_idx : art.0 as u32 };
                        drawable.push(vri);
                    }
                }
            }
            if let Some(neighbor_index) = chunk::get_neg_y_offset(i) {
                let neighbor_tile = chunk.get_raw_i(neighbor_index);
                if let Some(neighbor_art) = art_cache.get_mapping(neighbor_tile) {
                    if !neighbor_art.1.visible {
                        let (x,y,z) = chunk_i_to_xyz(i);
                        let vri = SideRenderInfo {
                            side : VoxelAxis::NegaY,
                            x : x as u16, 
                            y : y as u16,
                            z : z as u16,
                            tex_idx : art.0 as u32 };
                        drawable.push(vri);
                    }
                }
            }
            if let Some(neighbor_index) = chunk::get_pos_z_offset(i) {
                let neighbor_tile = chunk.get_raw_i(neighbor_index);
                if let Some(neighbor_art) = art_cache.get_mapping(neighbor_tile) {
                    if !neighbor_art.1.visible {
                        let (x,y,z) = chunk_i_to_xyz(i);
                        let vri = SideRenderInfo {
                            side : VoxelAxis::PosiZ,
                            x : x as u16, 
                            y : y as u16,
                            z : z as u16,
                            tex_idx : art.0 as u32 };
                        drawable.push(vri);
                    }
                }
            }
            if let Some(neighbor_index) = chunk::get_neg_z_offset(i) {
                let neighbor_tile = chunk.get_raw_i(neighbor_index);
                if let Some(neighbor_art) = art_cache.get_mapping(neighbor_tile) {
                    if !neighbor_art.1.visible {
                        let (x,y,z) = chunk_i_to_xyz(i);
                        let vri = SideRenderInfo {
                            side : VoxelAxis::NegaZ,
                            x : x as u16, 
                            y : y as u16,
                            z : z as u16,
                            tex_idx : art.0 as u32 };
                        drawable.push(vri);
                    }
                }
            }
        }
    }
    //println!("Found {} drawable cubes.", drawable.len());
    return ChunkMesh{mesh: mesh_step(drawable, display)};
}

#[inline(always)]
fn mesh_step(drawable : Vec<SideRenderInfo>, display : &Display) -> glium::VertexBuffer<PackedVertex> {
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