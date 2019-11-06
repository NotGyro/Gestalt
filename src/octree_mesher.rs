use geometry::{Mesh, VertexPositionNormalUVColor, VertexGroup, Material};
use std::sync::Arc;
use util::Transform;
use vulkano::device::Device;
use memory::pool::AutoMemoryPool;
use voxel::subdivmath::OctPos;
use cgmath::Point3;
use world::Chunk;
use hashbrown::HashSet;

/// Struct used internally to represent unoptimized quads.
#[derive(Debug, Clone)]
pub struct InputQuad { pub x: usize, pub y: usize, pub face_visible: bool, pub done: bool, pub block_id: u8 }

/// Represents a quad in an optimized mesh.
#[derive(Debug, Clone)]
pub struct OutputQuad { pub x: usize, pub y: usize, pub w: usize, pub h: usize, width_done: bool, pub block_id: u8 }

/// Cardinal direction a quad is facing.
pub enum QuadFacing {
    Left, Right, Bottom, Top, Front, Back,
}

//fn idx_to_xyz(idx: usize) -> (usize, usize, usize) {
//    ((idx / (64*64)), (idx / 64) % 64, idx % (64*64))
//}

fn xyz_to_idx(x: usize, y: usize, z: usize) -> usize {
    (x*64*64) + (y*64) + z
}

fn generate_slice(ids: &[u8; 64*64*64], facing: QuadFacing, layer: usize) -> Vec<OutputQuad> {
    // used to mark quads that overlap quads on other layers as not visible to cull them
    let adjacent_index_offset: i32 = match facing {
        QuadFacing::Left   => -64*64,
        QuadFacing::Right  =>  64*64,
        QuadFacing::Bottom => -64,
        QuadFacing::Top    =>  64,
        QuadFacing::Front  => -1,
        QuadFacing::Back   =>  1
    };

    let mut input_quads = Vec::new();
    for y in 0..64 {
        for x in 0..64 {
            match facing {
                QuadFacing::Left | QuadFacing::Right => {
                    // iterate across a slice where the first coord is fixed as the layer number and
                    // local x and y represent the two axes of the slice
                    let index = xyz_to_idx(layer, x, y);
                    // index of adjacent block
                    let adj_index: i32 = index as i32 + adjacent_index_offset;
                    // face isn't visible if it's air (0) or has a valid non-air block in front of it
                    let mut face_visible = ids[index] != 0 && !(adj_index >= 0 && adj_index < 64*64*64 && ids[adj_index as usize] != 0);
                    if adj_index / (64*64) == 0 { face_visible = true; }
                    input_quads.push(InputQuad { x, y, face_visible, done: false, block_id: ids[index] });
                },
                QuadFacing::Top | QuadFacing::Bottom => {
                    let index = xyz_to_idx(x, layer, y);
                    let adj_index: i32 = index as i32 + adjacent_index_offset;
                    let mut face_visible = ids[index] != 0 && !(adj_index >= 0 && adj_index < 64*64*64 && ids[adj_index as usize] != 0);
                    if (adj_index / 64) % 64 == 0 { face_visible = true; }
                    input_quads.push(InputQuad { x, y, face_visible, done: false, block_id: ids[index] });
                },
                QuadFacing::Front | QuadFacing::Back => {
                    let index = xyz_to_idx(x, y, layer);
                    let adj_index: i32 = index as i32 + adjacent_index_offset;
                    let mut face_visible = ids[index] != 0 && !(adj_index >= 0 && adj_index < 64*64*64 && ids[adj_index as usize] != 0);
                    if adj_index % 64 == 0 { face_visible = true; }
                    input_quads.push(InputQuad { x, y, face_visible, done: false, block_id: ids[index] });
                }
            }
        }
    }

    let mut output_quads = Vec::new();
    let mut current_quad: Option<OutputQuad> = None;
    let mut i = 0;
    while i < 64*64 {
        let mut q = input_quads.get_mut(i).unwrap().clone();
        if current_quad.is_none() {
            if q.face_visible && !q.done {
                current_quad = Some(OutputQuad { x: q.x, y: q.y, w: 1, h: 1, width_done: false, block_id: q.block_id });
                q.done = true;
            }
            i += 1;
            continue;
        }
        let mut current = current_quad.unwrap();
        if !current.width_done {
            // is quad on the same row?
            if q.x > current.x {
                // moving right, check for quad
                if q.face_visible && !q.done && q.block_id == current.block_id {
                    q.done = true;
                    current.w += 1;
                }
                else {
                    // found a gap, done with right expansion
                    current.width_done = true;
                }
            }
            else {
                // quad below start, meaning next row, done with right expansion
                current.width_done = true;
            }
        }
        if current.width_done {
            let mut y = current.y + 1;
            if y < 64 {
                loop {
                    let x_min = current.x;
                    let x_max = current.x + current.w;
                    let mut ok = true;
                    for x in x_min..x_max {
                        if !input_quads[y*64+x].face_visible || input_quads[y*64+x].done || input_quads[y*64+x].block_id != current.block_id {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        for x in x_min..x_max {
                            input_quads[y*64+x].done = true;
                        }
                        current.h += 1;
                        y += 1;
                        if y >= 64 { break; }
                    }
                    else { break; }
                }
            }
            output_quads.push(current);
            current_quad = None;
            continue;
        }
        i += 1;
        // when i == 16*16, loop would end without adding quad
        if i == 64*64 {
            output_quads.push(current.clone());
            break;
        }
        current_quad = Some(current);
    }

    output_quads
}

pub struct OctreeMesher;

impl OctreeMesher {
    pub fn generate_mesh(chunk: &mut Chunk, device: Arc<Device>, memory_pool: AutoMemoryPool) {
        let mut mesh = Mesh::new();

        let mut ids = [0u8; 64*64*64];
        let mut unique_ids = HashSet::new();

        // collect block info in flat array
        chunk.data.root.traverse(OctPos::from_four(0, 0, 0, 6), &mut |pos: OctPos<u32>, block_id: u8| {
            let x = pos.pos.x as u32;
            let y = pos.pos.y as u32;
            let z = pos.pos.z as u32;
            let size = 2u32.pow(pos.scale as u32);
            for xo in 0..size {
                for yo in 0..size {
                    for zo in 0..size {
                        let idx = (
                            ((x + xo) * 64 * 64)  +  ((y + yo) * 64)  +  (z + zo)
                        ) as usize;
                        ids[idx] = block_id;
                        if block_id != 0 {
                            unique_ids.insert(block_id);
                        }
                    }
                }
            }
        });

        // generate optimized quads from slices
        let mut quad_lists = Vec::new();
        for layer in 0..64 {
            // ( facing, layer number, Vec< OutputQuad > )
            quad_lists.push((QuadFacing::Left, layer, generate_slice(&ids, QuadFacing::Left, layer)));
            quad_lists.push((QuadFacing::Right, layer, generate_slice(&ids, QuadFacing::Right, layer)));

            quad_lists.push((QuadFacing::Bottom, layer, generate_slice(&ids, QuadFacing::Bottom, layer)));
            quad_lists.push((QuadFacing::Top, layer, generate_slice(&ids, QuadFacing::Top, layer)));

            quad_lists.push((QuadFacing::Front, layer, generate_slice(&ids, QuadFacing::Front, layer)));
            quad_lists.push((QuadFacing::Back, layer, generate_slice(&ids, QuadFacing::Back, layer)));
        }

        for id in unique_ids.iter() {
            let mut vertices = Vec::new() as Vec<VertexPositionNormalUVColor>;
            let mut indices = Vec::new() as Vec<u32>;
            let mut o = 0;
            for (facing, layer, list) in quad_lists.iter() {
                for quad in list {
                    if quad.block_id != *id { continue; }
                    let layerf = *layer as f32;
                    match facing {
                        QuadFacing::Left => {
                            vertices.push(VertexPositionNormalUVColor { position: [ layerf, quad.x as f32,          (quad.y+quad.h) as f32 ], normal: [ -1.0, 0.0, 0.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ layerf, (quad.x+quad.w) as f32, (quad.y+quad.h) as f32 ], normal: [ -1.0, 0.0, 0.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ layerf, (quad.x+quad.w) as f32, quad.y as f32          ], normal: [ -1.0, 0.0, 0.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ layerf, quad.x as f32,          quad.y as f32          ], normal: [ -1.0, 0.0, 0.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Right => {
                            vertices.push(VertexPositionNormalUVColor { position: [ layerf + 1.0, (quad.x+quad.w) as f32, (quad.y+quad.h) as f32 ], normal: [ 1.0, 0.0, 0.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ layerf + 1.0, quad.x as f32,          (quad.y+quad.h) as f32 ], normal: [ 1.0, 0.0, 0.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ layerf + 1.0, quad.x as f32,          quad.y as f32          ], normal: [ 1.0, 0.0, 0.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ layerf + 1.0, (quad.x+quad.w) as f32, quad.y as f32          ], normal: [ 1.0, 0.0, 0.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Bottom => {
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, layerf, (quad.y+quad.h) as f32 ], normal: [ 0.0, -1.0, 0.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          layerf, (quad.y+quad.h) as f32 ], normal: [ 0.0, -1.0, 0.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          layerf, quad.y as f32          ], normal: [ 0.0, -1.0, 0.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, layerf, quad.y as f32          ], normal: [ 0.0, -1.0, 0.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Top => {
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          layerf + 1.0, (quad.y+quad.h) as f32 ], normal: [ 0.0, 1.0, 0.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, layerf + 1.0, (quad.y+quad.h) as f32 ], normal: [ 0.0, 1.0, 0.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, layerf + 1.0, quad.y as f32          ], normal: [ 0.0, 1.0, 0.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          layerf + 1.0, quad.y as f32          ], normal: [ 0.0, 1.0, 0.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Front => {
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          (quad.y+quad.h) as f32, layerf ], normal: [ 0.0, 0.0, -1.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, (quad.y+quad.h) as f32, layerf ], normal: [ 0.0, 0.0, -1.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, quad.y as f32,          layerf ], normal: [ 0.0, 0.0, -1.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          quad.y as f32,          layerf ], normal: [ 0.0, 0.0, -1.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Back => {
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, (quad.y+quad.h) as f32, layerf + 1.0 ], normal: [ 0.0, 0.0, 1.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          (quad.y+quad.h) as f32, layerf + 1.0 ], normal: [ 0.0, 0.0, 1.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          quad.y as f32,          layerf + 1.0 ], normal: [ 0.0, 0.0, 1.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, quad.y as f32,          layerf + 1.0 ], normal: [ 0.0, 0.0, 1.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                    }
                    indices.push(0+o); indices.push(1+o); indices.push(2+o);
                    indices.push(2+o); indices.push(3+o); indices.push(0+o);
                    o += 4;
                }
            }
            mesh.vertex_groups.push(Arc::new(VertexGroup::new(vertices, indices, *id, device.clone(), memory_pool.clone())));
        }


        mesh.transform = Transform::from_position(Point3::new(chunk.position.0 as f32 * 64.0,
                                                              chunk.position.1 as f32 * 64.0,
                                                              chunk.position.2 as f32 * 64.0));

        mesh.materials.push(Material { albedo_map_name: String::from(""), specular_exponent: 0.0, specular_strength: 0.0 });
        mesh.materials.push(Material { albedo_map_name: String::from("stone"), specular_exponent: 128.0, specular_strength: 1.0 });
        mesh.materials.push(Material { albedo_map_name: String::from("dirt"), specular_exponent: 16.0, specular_strength: 0.5 });
        mesh.materials.push(Material { albedo_map_name: String::from("grass"), specular_exponent: 64.0, specular_strength: 0.7 });

        chunk.mesh = mesh;
    }
}