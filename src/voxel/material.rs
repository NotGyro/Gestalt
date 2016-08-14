
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct MaterialID {
    pub name : String,
}

#[derive(Clone)]
pub struct MaterialIndex { }

impl MaterialIndex { 
    pub fn new() -> Self {
        MaterialIndex { }
    }
    pub fn for_name(&self, n : String) -> MaterialID {
        MaterialID { name : n }
    }
    pub fn name_of(&self, mat : MaterialID ) -> String { 
        mat.name.clone()
    }
}

/* A Material in Gestalt represents any solid voxel that is part of the game-world. Stone walls, dirt, air, etc...
The representation in memory and on disk of a Material must be something you can boil down to a MaterialID. There can be
separate metadata, but the primary thing saying "there is a material here" must be that a cell in a VoxelStorage can
evaluate to a MaterialID which is then linked to the Material.

In the game I'm trying to make, there will be separate BlockMaterials and TerrainMaterials, with TerrainMaterials meshing
via marching cubes to a smooth mesh and BlockMaterials becoming Minecraft-like cubes. Not quite sure how to architect that yet -
separate types, or something that acts like inheritance from a common Material class would in a straight OO language?

Note I mentioned solids because fluids will be a different beast entirely - the voxel itself will be either a floating point
value or some range represented with an integer, and the world layer it is contained in will imply the type of the fluid.
*/
pub trait Material {
    fn get_id() -> MaterialID;
}
