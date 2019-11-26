use crate::voxel::subdivmath::{OctPos, Scale};
use crate::voxel::voxelmath::VoxelCoord;

// TODO: proper errors
type VoxelStructureError = String;

// Types:
// T: main voxel data type
// L: LOD data (can be () for types that don't use LOD data)
// P: coordinate type
// S: storage type (generally container for T)

pub trait VoxelSourceAbstract<T, L, P: VoxelCoord> {
    fn get(&self, coord: OctPos<P>) -> Result<&T, VoxelStructureError>;

    fn get_max_scale(&self) -> Scale { 8 }
    fn get_min_scale(&self) -> Scale { -8 }

    fn get_lod_data(&self, _coord: OctPos<P>) -> Option<&L> { None }

    // iterator (will eventually return an iterator type)
    // this iterates over the entire storage space
    fn iter(&self) { unimplemented!(); }

    // traverse iterates over just "the important bits" e.g. leaf nodes for an octree
    // uses a callback instead of an iterator
    fn traverse<F>(&self, func: &mut F) where F: FnMut(OctPos<P>, &T) -> bool;
}

pub trait VoxelDrainAbstract<T, P: VoxelCoord> {
    fn set(&mut self, coord: OctPos<P>, value: T) -> Result<(), VoxelStructureError>;

    fn iter_mut(&mut self) { unimplemented!(); }
    fn traverse_mut<F>(&mut self, func: &mut F) where F: FnMut(OctPos<P>, &mut T) -> bool;
}

pub trait VoxelStorageAbstract<S> {
    fn replace_data(&mut self, new_data: S);
}

pub trait VoxelStorage<T, L, P, S>:
    VoxelSourceAbstract<T, L, P> + VoxelDrainAbstract<T, P> + VoxelStorageAbstract<S>
        where P: VoxelCoord {}