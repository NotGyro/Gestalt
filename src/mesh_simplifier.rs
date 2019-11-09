////! Simplified mesh generator.
//
//use world_vg::Chunk;
//
///// Struct used internally to represent unoptimized quads.
//#[derive(Clone)]
//pub struct InputQuad { x: usize, y: usize, exists: bool, done: bool, pub block_id: usize }
///// Struct returned as output from the generator; represents quads in an optimized mesh.
//#[derive(Debug, Clone)]
//pub struct OutputQuad { pub x: usize, pub y: usize, pub w: usize, pub h: usize, width_done: bool, pub block_id: usize }
//
//
///// Cardinal direction a quad is facing.
//pub enum QuadFacing {
//    Left, Right, Top, Bottom, Front, Back
//}
//
//
///// Simplified mesh generator.
/////
///// Generates a list of quads to render a chunk, optimized using greedy meshing, and with inner faces culled.
//pub struct MeshSimplifier;
//
//
//impl MeshSimplifier {
//    /// Generates a simplified mesh from the given chunk.
//    pub fn generate_mesh(chunk: &Chunk) -> Vec<(QuadFacing, usize, Vec<OutputQuad>)> {
//        let mut output = Vec::new();
//
//        for layer in 0..16 {
//            output.push((QuadFacing::Left, layer, MeshSimplifier::generate_slice(chunk, QuadFacing::Left, layer)));
//            output.push((QuadFacing::Right, layer, MeshSimplifier::generate_slice(chunk, QuadFacing::Right, layer)));
//
//            output.push((QuadFacing::Bottom, layer, MeshSimplifier::generate_slice(chunk, QuadFacing::Bottom, layer)));
//            output.push((QuadFacing::Top, layer, MeshSimplifier::generate_slice(chunk, QuadFacing::Top, layer)));
//
//            output.push((QuadFacing::Front, layer, MeshSimplifier::generate_slice(chunk, QuadFacing::Front, layer)));
//            output.push((QuadFacing::Back, layer, MeshSimplifier::generate_slice(chunk, QuadFacing::Back, layer)));
//        }
//        output
//    }
//
//
//    /// Generates one 2d slice of the mesh.
//    pub fn generate_slice(chunk: &Chunk, facing: QuadFacing, layer: usize) -> Vec<OutputQuad> {
//        // used to mark quads that overlap quads on other layers as non-existent to cull them
//        let adjacent_index_offset: i32 = match facing {
//            QuadFacing::Left => -16*16,
//            QuadFacing::Right => 16*16,
//            QuadFacing::Bottom => -16,
//            QuadFacing::Top => 16,
//            QuadFacing::Front => -1,
//            QuadFacing::Back => 1
//        };
//
//        let mut input_quads = Vec::new();
//        for y in 0..16 {
//            for x in 0..16 {
//                match facing {
//                    QuadFacing::Left | QuadFacing::Right => {
//                        let index = Chunk::xyz_to_i(layer as i32, x as i32, y as i32);
//                        let adj_index = index as i32 + adjacent_index_offset;
//                        let exists = chunk.ids[index] != 0 && !(adj_index >= 0 && adj_index < 16*16*16 && chunk.ids[adj_index as usize] != 0);
//                        input_quads.push(InputQuad { x, y, exists, done: false, block_id: chunk.ids[index] as usize });
//                    },
//                    QuadFacing::Top | QuadFacing::Bottom => {
//                        let index = Chunk::xyz_to_i(x as i32, layer as i32, y as i32);
//                        let adj_index = index as i32 + adjacent_index_offset;
//                        let mut exists = chunk.ids[index] != 0 && !(adj_index >= 0 && adj_index < 16*16*16 && chunk.ids[adj_index as usize] != 0);
//                        if (adj_index / 16) % 16 == 0 { exists = true; }
//                        input_quads.push(InputQuad { x, y, exists, done: false, block_id: chunk.ids[index] as usize });
//                    },
//                    QuadFacing::Front | QuadFacing::Back => {
//                        let index = Chunk::xyz_to_i(x as i32, y as i32, layer as i32);
//                        let adj_index = index as i32 + adjacent_index_offset;
//                        let mut exists = chunk.ids[index] != 0 && !(adj_index >= 0 && adj_index < 16*16*16 && chunk.ids[adj_index as usize] != 0);
//                        if adj_index % 16 == 0 { exists = true; }
//                        input_quads.push(InputQuad { x, y, exists, done: false, block_id: chunk.ids[index] as usize });
//                    }
//                }
//            }
//        }
//
//        let mut output_quads = Vec::new();
//        let mut current_quad: Option<OutputQuad> = None;
//        let mut i = 0;
//        while i < 16*16 {
//            let mut q = input_quads.get_mut(i).unwrap().clone();
//            if current_quad.is_none() {
//                if q.exists && !q.done {
//                    current_quad = Some(OutputQuad { x: q.x, y: q.y, w: 1, h: 1, width_done: false, block_id: q.block_id });
//                    q.done = true;
//                }
//                i += 1;
//                continue;
//            }
//            let mut current = current_quad.unwrap();
//            if !current.width_done {
//                // is quad on the same row?
//                if q.x > current.x {
//                    // moving right, check for quad
//                    if q.exists && !q.done && q.block_id == current.block_id {
//                        q.done = true;
//                        current.w += 1;
//                    }
//                    else {
//                        // found a gap, done with right expansion
//                        current.width_done = true;
//                    }
//                }
//                else {
//                    // quad below start, meaning next row, done with right expansion
//                    current.width_done = true;
//                }
//            }
//            if current.width_done {
//                let mut y = current.y + 1;
//                if y < 16 {
//                    loop {
//                        let x_min = current.x;
//                        let x_max = current.x + current.w;
//                        let mut ok = true;
//                        for x in x_min..x_max {
//                            if !input_quads[y*16+x].exists || input_quads[y*16+x].done || input_quads[y*16+x].block_id != current.block_id {
//                                ok = false;
//                                break;
//                            }
//                        }
//                        if ok {
//                            for x in x_min..x_max {
//                                input_quads[y*16+x].done = true;
//                            }
//                            current.h += 1;
//                            y += 1;
//                            if y >= 16 { break; }
//                        }
//                        else { break; }
//                    }
//                }
//                output_quads.push(current);
//                current_quad = None;
//                continue;
//            }
//            i += 1;
//            // when i == 16*16, loop would end without adding quad
//            if i == 16*16 {
//                output_quads.push(current.clone());
//                break;
//            }
//            current_quad = Some(current);
//        }
//
//        output_quads
//    }
//}