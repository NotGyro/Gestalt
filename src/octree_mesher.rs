use voxel::subdivstorage::NaiveVoxelOctree;
use geometry::{Mesh, VertexPositionNormalUVColor, VertexGroup, Material};
use std::sync::Arc;
use util::Transform;
use vulkano::device::Device;
use memory::pool::AutoMemoryPool;
use voxel::subdivmath::OctPos;

#[derive(Debug, Clone)]
pub struct Quad { x: u32, y: u32, z: u32, w: u32, h: u32 }

pub struct OctreeMesher;

impl OctreeMesher {
    pub fn generate_mesh(chunk: &NaiveVoxelOctree<u32, ()>, device: Arc<Device>, memory_pool: AutoMemoryPool) -> Mesh {
        let mut mesh = Mesh::new();
        let mut vertices = Vec::new();
        let mut indices = Vec::new() as Vec<u32>;
        let mut o = 0; // offset

        chunk.root.traverse(OctPos::from_four(0, 0, 0, 6), &mut |pos: OctPos<u32>, block_id: u32| {
            if block_id == 1 {
                let x = pos.pos.x as f32;
                let y = pos.pos.y as f32;
                let z = pos.pos.z as f32;
                let s = 2u32.pow(pos.scale as u32) as f32; // size

                // left
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y,   z+s ], normal: [ -1.0, 0.0, 0.0 ], uv: [ 0.0, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y+s, z+s ], normal: [ -1.0, 0.0, 0.0 ], uv: [ s,   0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y+s, z   ], normal: [ -1.0, 0.0, 0.0 ], uv: [ s,   s   ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y,   z   ], normal: [ -1.0, 0.0, 0.0 ], uv: [ 0.0, s   ], color: [ 1.0, 1.0, 1.0 ] });

                // right
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y+s, z+s ], normal: [  1.0, 0.0, 0.0 ], uv: [ s,   0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y,   z+s ], normal: [  1.0, 0.0, 0.0 ], uv: [ 0.0, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y,   z   ], normal: [  1.0, 0.0, 0.0 ], uv: [ 0.0, s   ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y+s, z   ], normal: [  1.0, 0.0, 0.0 ], uv: [ s,   s   ], color: [ 1.0, 1.0, 1.0 ] });

                // bottom
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y,   z+s ], normal: [ 0.0, -1.0, 0.0 ], uv: [ s,   0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y,   z+s ], normal: [ 0.0, -1.0, 0.0 ], uv: [ 0.0, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y,   z   ], normal: [ 0.0, -1.0, 0.0 ], uv: [ 0.0, s   ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y,   z   ], normal: [ 0.0, -1.0, 0.0 ], uv: [ s,   s   ], color: [ 1.0, 1.0, 1.0 ] });

                // top
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y+s, z+s ], normal: [ 0.0,  1.0, 0.0 ], uv: [ 0.0, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y+s, z+s ], normal: [ 0.0,  1.0, 0.0 ], uv: [ s,   0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y+s, z   ], normal: [ 0.0,  1.0, 0.0 ], uv: [ s,   s   ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y+s, z   ], normal: [ 0.0,  1.0, 0.0 ], uv: [ 0.0, s   ], color: [ 1.0, 1.0, 1.0 ] });

                // front
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y+s, z   ], normal: [ 0.0, 0.0, -1.0 ], uv: [ 0.0, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y+s, z   ], normal: [ 0.0, 0.0, -1.0 ], uv: [ s,   0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y,   z   ], normal: [ 0.0, 0.0, -1.0 ], uv: [ s,   s   ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y,   z   ], normal: [ 0.0, 0.0, -1.0 ], uv: [ 0.0, s   ], color: [ 1.0, 1.0, 1.0 ] });

                // back
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y+s, z+s ], normal: [ 0.0, 0.0,  1.0 ], uv: [ s,   0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y+s, z+s ], normal: [ 0.0, 0.0,  1.0 ], uv: [ 0.0, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x,   y,   z+s ], normal: [ 0.0, 0.0,  1.0 ], uv: [ 0.0, s   ], color: [ 1.0, 1.0, 1.0 ] });
                vertices.push(VertexPositionNormalUVColor { position: [ x+s, y,   z+s ], normal: [ 0.0, 0.0,  1.0 ], uv: [ s,   s   ], color: [ 1.0, 1.0, 1.0 ] });

                for _ in 0..6 {
                    indices.push(0+o); indices.push(1+o); indices.push(2+o);
                    indices.push(2+o); indices.push(3+o); indices.push(0+o);
                    o += 4;
                }
            }
        });

        println!("quads: {}", indices.len() / 6);

        mesh.vertex_groups.push(Arc::new(VertexGroup::new(vertices, indices, 1u8, device.clone(), memory_pool.clone())));
        mesh.transform = Transform::new();

        mesh.materials.push(Material { albedo_map_name: String::from(""), specular_exponent: 0.0, specular_strength: 0.6 });
        mesh.materials.push(Material { albedo_map_name: String::from("grass"), specular_exponent: 64.0, specular_strength: 0.7 });

        mesh
    }
}