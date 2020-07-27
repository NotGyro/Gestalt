use std::fmt::{Display, Debug};
use std::fmt;
use std::error;
use std::error::Error;
use std::result::Result;
use std::cell::RefCell;
use generational_arena::Arena;
use generational_arena::Index as ArenaIndex;

use crate::voxel::voxelmath::*;
use crate::voxel::subdivmath::*;
use crate::voxel::voxelstorage::Voxel;

/// An error reported upon trying to get or set a voxel outside of our range.
#[derive(Debug)]
#[allow(dead_code)]
pub enum SubdivError {
    OutOfBounds,
    OutOfScale,
    DetailNotPresent,
    SplittingBranch,
    ReqLeafGotBranch,
    ReqBranchGotLeaf,
    NotYetLoaded,
    SetInvalidValue,
    InvalidValueAt,
    Other(Box<dyn error::Error + 'static>),
}

impl Display for SubdivError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SubdivError::OutOfBounds => write!(f, "Attempted to access a voxel from a storage, outside of that voxel storage's bounds."),
            SubdivError::OutOfScale =>
                write!(f, "Attempted to access a voxel an invalid scale"),
            SubdivError::DetailNotPresent =>
                write!(f, "Attempted to access a voxel at a level of detail doesn't exist at the provided position."),
            SubdivError::SplittingBranch => write!(f, "Attempted to split a leaf into a branch with child leaves, but this voxel is already a branch."),
            SubdivError::ReqLeafGotBranch => write!(f, "Tried to access a leaf value, but this voxel is a branch."),
            SubdivError::ReqBranchGotLeaf => write!(f, "Tried to access a branch value, but this voxel is a leaf."),
            SubdivError::NotYetLoaded => write!(f, "Attempted to access a voxel position which is not yet loaded."),
            SubdivError::SetInvalidValue => write!(f, "Attempted to set voxel to an invalid value."),
            SubdivError::InvalidValueAt => write!(f, "Voxel contains an invalid value, most likely corrupt."),
            SubdivError::Other(err) => write!(f, "Other voxel error: {}", err),
        }
    }
}

impl Error for SubdivError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None //I would love to have it to handle Other correctly but nope, the sized variable requirement isn't having it.
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubdivNode<L: Voxel, B: Voxel> {
    Leaf(L),
    Branch(B),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SubdivNodeKind {
    Leaf,
    Branch,
}

impl<L: Voxel, B: Voxel> SubdivNode<L,B> {
    #[allow(dead_code)]
    pub fn kind(&self) -> SubdivNodeKind {
        match self {
            SubdivNode::Leaf(_) => SubdivNodeKind::Leaf,
            SubdivNode::Branch(_) => SubdivNodeKind::Leaf,
        }
    }
}

#[allow(dead_code)]
pub type NodeNoLOD<T> = SubdivNode<(), T>;

impl <L, B> Default for SubdivNode<L, B> where L : Voxel + Default, B : Voxel {
    #[inline]
    fn default() -> Self { SubdivNode::Leaf(L::default()) }
}

#[allow(dead_code, non_camel_case_types)]
pub enum NodeChildIndex {
    X0_Y0_Z0,
    X0_Y0_Z1,
    X0_Y1_Z0,
    X0_Y1_Z1,
    X1_Y0_Z0,
    X1_Y0_Z1,
    X1_Y1_Z0,
    X1_Y1_Z1,
}

// Bitwise encoding, last three bits of a u8. It goes x, y, then z. (z changes first) Each is 0 if closer to origin, 1 if further.
impl NodeChildIndex {
    #[allow(dead_code)]
    fn num_representation(&self) -> usize {
        match self {
            NodeChildIndex::X0_Y0_Z0 => 0,
            NodeChildIndex::X0_Y0_Z1 => 1,
            NodeChildIndex::X0_Y1_Z0 => 2,
            NodeChildIndex::X0_Y1_Z1 => 3,
            NodeChildIndex::X1_Y0_Z0 => 4,
            NodeChildIndex::X1_Y0_Z1 => 5,
            NodeChildIndex::X1_Y1_Z0 => 6,
            NodeChildIndex::X1_Y1_Z1 => 7,
        }
    }
    #[allow(dead_code)]
    fn from_num_representation(val : usize) -> Self {
        match val {
            0 => NodeChildIndex::X0_Y0_Z0,
            1 => NodeChildIndex::X0_Y0_Z1,
            2 => NodeChildIndex::X0_Y1_Z0,
            3 => NodeChildIndex::X0_Y1_Z1,
            4 => NodeChildIndex::X1_Y0_Z0,
            5 => NodeChildIndex::X1_Y0_Z1,
            6 => NodeChildIndex::X1_Y1_Z0,
            7 => NodeChildIndex::X1_Y1_Z1,
            _ => panic!(),
        }
    }
}

#[inline(always)]
pub fn index_encode<T: VoxelCoord>(pos : VoxelPos<T>, scl : Scale) -> usize {
    let mut idx : usize = 0;
    // Scale i is 2^(root scale - i) of coord.scale -sized cubes.
    // Therefore, we're looking at the bit at index (root scale - i).
    let mask = T::one().unsigned_shl( scl as u32);
    if pos.x.bitand(mask) != T::zero() {
        //x lives in the 4s place in a NodeChildIndex
        idx += 4
    }
    if pos.y.bitand(mask) != T::zero() {
        //y lives in the 2s place in a NodeChildIndex
        idx += 2
    }
    if pos.z.bitand(mask) != T::zero() {
        //z lives in the 1s place in a NodeChildIndex
        idx += 1
    }
    //assert!(idx < 8);
    //println!("Calling index_encode() on VoxelPos {} with scale {} yields child index {}", pos, scl, idx);
    return idx;
}

//fn get(&self, coord: OctPos<P>) -> Result<T, SubdivError> {
//Ok(self.get_details(coord)?.0)
//}
// Returns a tuple of your voxel and the level of detail at which your voxel was found.
//fn get_details(&self, coord: OctPos<P>) -> Result<(T, Scale), SubdivError>;

/// A basic trait for any 3d grid data structure with level-of-detail / grid data, with read capability
/// Type arguments are type of voxel, type of position.
///
/// (Type of positon must be an integer, but I'm still using
/// genericism here because it should be possible to use
/// any bit length of integer, or even a bigint implementation).
///
/// This is for any voxel data source that does LOD - you could be sampling perlin noise, for example.
pub trait SubdivSource<T: Voxel, P: VoxelCoord> {
    fn get(&self, coord: OctPos<P>) -> Result<T, SubdivError>;

    fn get_max_scale(&self) -> Scale { 127 }
    fn get_min_scale(&self) -> Scale { -128 }
}

/// A basic trait for any 3d grid data structure with level-of-detail / grid data, with write capability
/// Type arguments are type of voxel, type of position.
///
/// (Type of positon must be an integer, but I'm still using
/// genericism here because it should be possible to use
/// any bit length of integer, or even a bigint implementation).
///
/// This is for any voxel data source that does LOD - you could be sampling perlin noise, for example.
pub trait SubdivDrain<T: Voxel, P: VoxelCoord> {
    fn set(&mut self, coord: OctPos<P>, value: T) -> Result<(), SubdivError>;
}

/// Any SubdivStorage which has defined, finite bounds.
/// Must be able to store a valid voxel for any position within
/// the range provided by get_bounds().
/// Usually, this implies that the SubdivStorage is not paged.
pub trait SubdivStorageBounded<P: VoxelCoord> {
    fn get_bounds(&self) -> VoxelRange<P>;
}

/// A basic trait for any 3d grid data structure with level-of-detail / grid data.
/// Type arguments are type of leaf element, type of branch (LOD) element, type of position.
/// The term "Branch" here is used to differentiate between a deliberately-ambiguous Node type,
/// which is any entry at a position and level of detail, and the Branch type, which is necessarily
/// not a leaf.
///
/// (Type of positon must be an integer, but I'm still using
/// genericism here because it should be possible to use
/// any bit length of integer, or even a bigint implementation).
///
/// This is for anything that acts like an Octree but it doesn't have to *be* an Octree.
/// What this means is: This Voxel Storage must have a concept of "Branches", which themselves
/// are not where the data lives but point to "Leaves" at smaller scale / higher detail.
pub trait OctreeSource<L: Voxel, D: Voxel, P: VoxelCoord> : SubdivSource<SubdivNode<L, D>, P> {

    /// Gets you the value of the node as well as information about the scale at which you found this value -
    /// e.g. if you find an 8x8x8 leaf (scale 3) that contains your position at the 2x2x2 scale (scale 1),
    /// it'll let you know that this is a leaf at scale 3.
    fn get_details(&self, coord: OctPos<P>) -> Result<(SubdivNode<L, D>, Scale), SubdivError>;

    /// Get information on all 8 of the smaller nodes that live inside the selected larger node.
    /// This is not a deep copy - hence, returns LOD information for branch nodes.
    fn get_children(&self, coord: OctPos<P>) -> Result<[SubdivNode<L, D>; 8], SubdivError> {
        let small_pos = coord.scale_to(coord.scale - 1);
        // If this is a leaf it by definition does not have children.
        if let SubdivNodeKind::Leaf = self.get(coord)?.kind() {
            return Err(SubdivError::DetailNotPresent);
        }
        if small_pos.scale < self.get_min_scale() {
            return Err(SubdivError::OutOfScale);
        }
        // "One" and "zero" here are offset from the origin, origin being small_pos.
        let zerox_zeroy_zeroz = small_pos.clone();
        let zerox_zeroy_onez = OctPos{scale: small_pos.scale, pos: vpos!(small_pos.pos.x, small_pos.pos.y, small_pos.pos.z + P::one())};
        let zerox_oney_zeroz = OctPos{scale: small_pos.scale, pos: vpos!(small_pos.pos.x, small_pos.pos.y + P::one(), small_pos.pos.z)};
        let zerox_oney_onez = OctPos{scale: small_pos.scale, pos: vpos!(small_pos.pos.x, small_pos.pos.y + P::one(), small_pos.pos.z + P::one())};
        let onex_zeroy_zeroz = OctPos{scale: small_pos.scale, pos: vpos!(small_pos.pos.x + P::one(), small_pos.pos.y, small_pos.pos.z)};
        let onex_zeroy_onez = OctPos{scale: small_pos.scale, pos: vpos!(small_pos.pos.x + P::one(), small_pos.pos.y, small_pos.pos.z + P::one())};
        let onex_oney_zeroz = OctPos{scale: small_pos.scale, pos: vpos!(small_pos.pos.x + P::one(), small_pos.pos.y + P::one(), small_pos.pos.z)};
        let onex_oney_onez = OctPos{scale: small_pos.scale, pos: vpos!(small_pos.pos.x + P::one(), small_pos.pos.y + P::one(), small_pos.pos.z + P::one())};
        Ok([self.get(zerox_zeroy_zeroz)?,
            self.get(zerox_zeroy_onez)?,
            self.get(zerox_oney_zeroz)?,
            self.get(zerox_oney_onez)?,
            self.get(onex_zeroy_zeroz)?,
            self.get(onex_zeroy_onez)?,
            self.get(onex_oney_zeroz)?,
            self.get(onex_oney_onez)?])
    }
    fn node_kind(&self, pos: OctPos<P>) -> Result<SubdivNodeKind, SubdivError> { Ok(self.get(pos)?.kind()) }
}

pub trait OctreeDrain<L: Voxel, D: Voxel, P: VoxelCoord> : OctreeSource<L, D, P> {
    /// Sets a voxel value without checking to see if its parent branch needs to be recombined.
    /// You cannot set something directly *to* a branch.
    fn set_raw(&mut self, coord: OctPos<P>, leaf_val: L) -> Result<(), SubdivError>;

    /// Sets a voxel in the octree, and also checks to see if its parent node can be merged.
    /// After that, checks recursively for nodes which can be combined.
    fn set_and_merge(&mut self, coord: OctPos<P>, leaf_val: L) -> Result<(), SubdivError>;

    /// Sets the branch data at a certain location.
    /// Will error if this location is or is below a leaf.
    fn set_branch_data(&mut self, coord: OctPos<P>, branch_val: D) -> Result<(), SubdivError>;

    /// Sets branch-associated data at the specified location. If it's a leaf, do nothing.
    fn try_set_branch_data(&mut self, coord: OctPos<P>, branch_val: D) -> Result<(), SubdivError> {
        match self.set_branch_data(coord, branch_val) {
            Ok(()) => Ok(()),
            Err(SubdivError::ReqBranchGotLeaf) => Ok(()),
            Err(other_err) => Err(other_err),
        }
    }
}

/*
impl<L, D> NaiveOctreeBranch<L, D> where L: Voxel, D: Voxel {
    #[allow(dead_code)]
    fn rebuild_lod(&mut self, pool: &mut Arena<NaiveOctreeBranch<L,D>>) {
        let value0 = match &mut self.children[0] {
            SubdivNode::Leaf(leaf_child) => D::represent(&leaf_child),
            SubdivNode::Branch(branch_index) => {
                let child = pool.get_mut(*branch_index).unwrap();
                child.lod_data.clone()
            },
        };
        let value1 = match &mut self.children[1] {
            SubdivNode::Leaf(leaf_child) => D::represent(&leaf_child),
            SubdivNode::Branch(branch_index) => {
                let child = pool.get_mut(*branch_index).unwrap();
                child.lod_data.clone()
            },
        };
        ...

        let mut lod_info : [D; 8] = [value0,
                                    value1,
                                    value2,
                                    value3,
                                    value4,
                                    value5,
                                    value6,
                                    value7];
        self.lod_data.downsample_from(&lod_info);
    }
}*/

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NaiveOctreeBranch<L: Voxel, D: Voxel> {
    pub children : [SubdivNode<L, ArenaIndex>; 8],
    // Any data that lives at the branch level.
    pub branch_data : D,
}
type OctreePool<L, D> = RefCell<Arena<NaiveOctreeBranch<L, D>>>;

type NaiveOctreeNode<L> = SubdivNode<L, ArenaIndex>;

impl<L,D> NaiveOctreeBranch<L,D> where L: Voxel, D: Voxel {
    fn garbage_collect(&self, arena: &mut OctreePool<L, D>) {
        //Recursively delete all sub-nodes.
        for node in self.children.iter() {
            if let SubdivNode::Branch(child_idx) = node {
                let arena_ref = arena.borrow();
                let child_branch = arena_ref.get(*child_idx).unwrap().clone();
                drop(arena_ref);
                child_branch.garbage_collect(arena);
            }
        }
        for node in self.children.iter() {
            if let SubdivNode::Branch(child_idx) = node {
                arena.borrow_mut().remove(*child_idx);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct NaiveVoxelOctree<L, D> where L: Voxel, D: Voxel {
    pub scale : Scale,
    pub root : NaiveOctreeNode<L>,
    pub pool : OctreePool<L,D>,
}

impl<L,D> NaiveVoxelOctree<L,D> where L: Voxel, D: Voxel {
    pub fn new(starting_leaf: L, scale: Scale) -> Self {
        NaiveVoxelOctree{scale : scale , root: SubdivNode::Leaf(starting_leaf), pool:
        RefCell::new(Arena::with_capacity(512))}
    }

    #[allow(dead_code)]
    pub fn traverse<F>(&self, _pos: OctPos<u32>, _func: &mut F) where F: FnMut(OctPos<u32>, L) {
//        let node = &self.get(pos).unwrap();
//        match node {
//            SubdivNode::Leaf(l) => {
//                func(pos, l.clone());
//            }
//            SubdivNode::Branch(b) => {
//                if pos.scale > 0 {
//                    for (i, _) in self.pool.borrow().get(*b).unwrap().children.iter().enumerate() {
//                        let idx = NodeChildIndex::from_num_representation(i);
//                        let s = 2u32.pow((pos.scale-1) as u32);
//                        let offset = match idx {
//                            NodeChildIndex::X0_Y0_Z0 => (0, 0, 0),
//                            NodeChildIndex::X0_Y0_Z1 => (0, 0, s),
//                            NodeChildIndex::X0_Y1_Z0 => (0, s, 0),
//                            NodeChildIndex::X0_Y1_Z1 => (0, s, s),
//                            NodeChildIndex::X1_Y0_Z0 => (s, 0, 0),
//                            NodeChildIndex::X1_Y0_Z1 => (s, 0, s),
//                            NodeChildIndex::X1_Y1_Z0 => (s, s, 0),
//                            NodeChildIndex::X1_Y1_Z1 => (s, s, s),
//                        };
//                        self.traverse(OctPos::from_four(
//                            pos.pos.x+offset.0,
//                            pos.pos.y+offset.1,
//                            pos.pos.z+offset.2,
//                            pos.scale - 1),
//                                       func);
//                    }
//                }
//            }
//        }
    }

//    #[allow(dead_code)]
//    pub fn traverse_to_depth<F>(&self, pos: OctPos<u32>, func: &mut F, min_scale: Scale) where F: FnMut(OctPos<u32>, L) {
//        match &self.root {
//            SubdivNode::Leaf(l) => {
//                func(pos, l.clone());
//            }
//            SubdivNode::Branch(b) => {
//                if pos.scale <= min_scale {
//                    return;
//                }
//                for (i, _) in self.pool.borrow().get(*b).unwrap().children.iter().enumerate() {
//                    let idx = NodeChildIndex::from_num_representation(i);
//                    let s = 2u32.pow((pos.scale-1) as u32);
//                    let offset = match idx {
//                        NodeChildIndex::X0_Y0_Z0 => (0, 0, 0),
//                        NodeChildIndex::X0_Y0_Z1 => (0, 0, s),
//                        NodeChildIndex::X0_Y1_Z0 => (0, s, 0),
//                        NodeChildIndex::X0_Y1_Z1 => (0, s, s),
//                        NodeChildIndex::X1_Y0_Z0 => (s, 0, 0),
//                        NodeChildIndex::X1_Y0_Z1 => (s, 0, s),
//                        NodeChildIndex::X1_Y1_Z0 => (s, s, 0),
//                        NodeChildIndex::X1_Y1_Z1 => (s, s, s),
//                    };
//                    self.traverse_to_depth(OctPos::from_four(
//                        pos.pos.x+offset.0,
//                        pos.pos.y+offset.1,
//                        pos.pos.z+offset.2,
//                        pos.scale - 1),
//                                   func, min_scale);
//                }
//            }
//        }
//    }
}
/*
impl<L> NaiveOctreeNode<L> where L: Voxel {
    pub fn split_into_branch<D: Voxel + Default>(&mut self, arena: &mut OctreePool<L, D>) {
        if let SubdivNode::Leaf(leaf_value) = self {
            let children : [NaiveOctreeNode<L>; 8] = [SubdivNode::Leaf(leaf_value.clone()),
                                                    SubdivNode::Leaf(leaf_value.clone()),
                                                    SubdivNode::Leaf(leaf_value.clone()),
                                                    SubdivNode::Leaf(leaf_value.clone()),
                                                    SubdivNode::Leaf(leaf_value.clone()),
                                                    SubdivNode::Leaf(leaf_value.clone()),
                                                    SubdivNode::Leaf(leaf_value.clone()),
                                                    SubdivNode::Leaf(leaf_value.clone()),];
            *self = NaiveOctreeNode::Branch(arena.borrow_mut().insert(NaiveOctreeBranch {
                    children,
                    branch_data: D::default(),
            }));
        }
    }
}*/

impl<L, D, P> OctreeSource<L, D, P> for NaiveVoxelOctree<L, D> where L: Voxel, D: Voxel, P: VoxelCoord {
    #[inline]
    fn get_details(&self, coord: OctPos<P>) -> Result<(SubdivNode<L, D>, Scale), SubdivError> {
        if coord.scale > self.scale {
            //Trying to set a voxel larger than our root node.
            return Err(SubdivError::OutOfScale);
        } else if !( pos_within_node(coord, self.scale) ){
            //Selected node cannot possibly exist in our octree.
            return Err( SubdivError::OutOfBounds);
        }
        //Make sure that root is not a leaf so we can do the algorithm properly.
        match &self.root {
            SubdivNode::Leaf(l) => return Ok((SubdivNode::Leaf(l.clone()), self.scale)),
            SubdivNode::Branch(root_branch_index) => {
                //Cool, we get to do our algorithm.
                let mut parent_idx = *root_branch_index;
                let mut current_scale = self.scale-1;
                while current_scale >= coord.scale {
                    //Set up decision making here.
                    //Get a node - Borrow a reference to our current node.
                    let pool_ref = self.pool.borrow();
                    let current_branch = pool_ref.get(parent_idx).unwrap();
                    let current_node = current_branch.children[index_encode(coord.pos, current_scale-coord.scale)].clone();
                    drop(current_branch);
                    drop(pool_ref);
                    //Have we hit our target?
                    if current_scale == coord.scale {
                        match current_node {
                            SubdivNode::Leaf(leaf_dat) => return Ok( (SubdivNode::Leaf(leaf_dat.clone()), current_scale)  ),
                            SubdivNode::Branch(branch_idx) => return Ok((SubdivNode::Branch(
                                self.pool.borrow().get(branch_idx).unwrap()
                                    .branch_data.clone()), current_scale) )
                            ,
                        }
                    }
                    else {
                        // We have not yet gotten to target.
                        match current_node {
                            //Target is below our scale but we are a leaf. That space partition is part of this one.
                            SubdivNode::Leaf(leaf_dat) => return Ok(  (SubdivNode::Leaf(leaf_dat.clone()), current_scale)  ),
                            SubdivNode::Branch(branch_index) => {
                                parent_idx = branch_index;
                                current_scale -= 1
                            },
                        }
                    }
                }
            }
        }
        Err(SubdivError::DetailNotPresent)
    }
}

impl<L, D, P> SubdivSource<SubdivNode<L, D>, P> for NaiveVoxelOctree<L, D> where L: Voxel, D: Voxel, P: VoxelCoord {
    //Cannot get a node bigger than our root node.
    fn get_max_scale(&self) -> Scale { self.scale }
    fn get(&self, coord: OctPos<P>) -> Result<SubdivNode<L, D>, SubdivError> {
        Ok(self.get_details(coord)?.0)
    }
}
/*
// Ok(bool) means true if we're still checking for consolidation on the way out, false means we've
// already determined that we can't consolidate any more
fn set_recurse<L: Voxel, D: Voxel + Default, P: VoxelCoord>
        (node: &mut NaiveOctreeNode<L>, coord: OctPos<P>,
            current_scale: Scale, target_scale: Scale, value: &L, arena: &mut OctreePool<L,D>)
        -> Result<bool, SubdivError> {
    if current_scale < target_scale {
        return Err(SubdivError::DetailNotPresent);
    }

    // Have we hit our target?
    if current_scale == target_scale {
        *node = SubdivNode::Leaf(value.clone());
        return Ok(true);
    }
    else {
        // We have not yet gotten to target.
        if let SubdivNode::Leaf(_) = *node {
            // Target is below our scale. We will need to create a child node, and recurse on it.
            (*node).split_into_branch(&mut arena);
        }
        match *node {
            SubdivNode::Branch(branch_index) => {
                // Not found and this is a branch. Time for recursion.
                // Our child nodes are implicitly at our scale -1.
                let child = &mut arena.get_mut(branch_index).unwrap()
                                .children[index_encode(coord.pos, current_scale-coord.scale-1)];
                match set_recurse(child, coord, current_scale-1, target_scale, value, arena) {
                    Ok(b) => {
                        // consolidate?
                        if b {
                            let branch_node = & arena.get(branch_index).unwrap();
                            println!("{}", branch_node.children.len());
                            // check for homogenous children
                            for c in branch_node.children.iter() {
                                match c {
                                    SubdivNode::Leaf(l) => {
                                        if l != value {
                                            // non-matching voxel found, stop checking
                                            return Ok(false);
                                        }
                                    },
                                    SubdivNode::Branch(_) => {
                                        // since homogenous branches should already be consolidated,
                                        // a branch can be assumed to be non-homogenous
                                        return Ok(false);
                                    }
                                }
                            }
                            // if we've made it to this point, each child has been checked and is
                            // the same. now we consolidate this branch into a leaf
                            *node = SubdivNode::Leaf(value.clone());
                            // go up one level, keep checking
                            return Ok(true);
                        }
                    },
                    Err(e) => { return Err(e); }
                }

            }
            _ => unreachable!(), // We just split this into a branch if it's a leaf.
        }
    }
    Ok(false)
}
*/
impl<L, D, P> OctreeDrain<L, D, P> for NaiveVoxelOctree<L, D> where L: Voxel, D: Voxel + Default, P: VoxelCoord {
    /// Sets a voxel value without checking to see if its parent branch needs to be recombined.
    /// You cannot set something directly *to* a branch.
    fn set_raw(&mut self, coord: OctPos<P>, leaf_val: L) -> Result<(), SubdivError> {
        if coord.scale > self.scale {
            //Trying to set a voxel larger than our root node.
            return Err(SubdivError::OutOfScale);
        } else if !( pos_within_node(coord, self.scale) ){
            //Selected node cannot possibly exist in our octree.
            return Err( SubdivError::OutOfBounds);
        }
        if coord.scale == self.scale {
            match &self.root {
                SubdivNode::Leaf(_) => self.root = SubdivNode::Leaf(leaf_val),
                SubdivNode::Branch(root_branch_index) => {
                    let pool_borrow = self.pool.borrow();
                    //Recursive garbage collect the old node.
                    let branch = pool_borrow.get(*root_branch_index).unwrap().clone();
                    drop(pool_borrow);
                    branch.garbage_collect(&mut self.pool);

                    let mut pool_borrow = self.pool.borrow_mut();

                    pool_borrow.remove(*root_branch_index);

                    self.root = SubdivNode::Leaf(leaf_val);
                },
            }
        }
        else {
            if let SubdivNode::Leaf(l) = &self.root {
                let children : [NaiveOctreeNode<L>; 8] = [SubdivNode::Leaf(l.clone()),
                    SubdivNode::Leaf(l.clone()),
                    SubdivNode::Leaf(l.clone()),
                    SubdivNode::Leaf(l.clone()),
                    SubdivNode::Leaf(l.clone()),
                    SubdivNode::Leaf(l.clone()),
                    SubdivNode::Leaf(l.clone()),
                    SubdivNode::Leaf(l.clone()),];
                self.root = NaiveOctreeNode::Branch(self.pool.borrow_mut().insert(NaiveOctreeBranch {
                    children,
                    branch_data: D::default(),
                }));
            }
            match &self.root {
                SubdivNode::Branch(root_branch_index) => {
                    let mut parent_idx = *root_branch_index;
                    let mut current_scale = self.scale-1;
                    while current_scale >= coord.scale {
                        //Set up decision making here.
                        //Get a node - Borrow a reference to our current node.
                        let pool_borrow = self.pool.borrow();
                        let current_node_info = pool_borrow.get(parent_idx).unwrap()
                            .children[index_encode(coord.pos, current_scale-coord.scale)].clone();
                        drop(pool_borrow);
                        //Have we hit our target?
                        if current_scale == coord.scale {
                            match current_node_info {
                                SubdivNode::Leaf(_) => {
                                    let mut pool_borrow = self.pool.borrow_mut();
                                    let current_node = &mut pool_borrow.get_mut(parent_idx).unwrap()
                                        .children[index_encode(coord.pos, current_scale-coord.scale)];
                                    *current_node = SubdivNode::Leaf(leaf_val);
                                    return Ok(());
                                },
                                SubdivNode::Branch(branch_idx) => {
                                    //Recursive garbage collect the old node.
                                    let pool_borrow = self.pool.borrow();
                                    let node_to_collect = pool_borrow.get(branch_idx).unwrap().clone();
                                    drop(pool_borrow);
                                    node_to_collect.garbage_collect(&mut self.pool);

                                    let mut pool_borrow = self.pool.borrow_mut();
                                    let current_node = &mut pool_borrow.get_mut(parent_idx).unwrap()
                                        .children[index_encode(coord.pos, current_scale-coord.scale)];
                                    *current_node = SubdivNode::Leaf(leaf_val);

                                    return Ok(());
                                }
                            }
                        }
                        else {
                            // We have not yet gotten to target.
                            if let SubdivNode::Leaf(l) = current_node_info {
                                //Target is below our scale. We will need to create a child node, and recurse on it.
                                let children : [NaiveOctreeNode<L>; 8] = [SubdivNode::Leaf(l.clone()),
                                    SubdivNode::Leaf(l.clone()),
                                    SubdivNode::Leaf(l.clone()),
                                    SubdivNode::Leaf(l.clone()),
                                    SubdivNode::Leaf(l.clone()),
                                    SubdivNode::Leaf(l.clone()),
                                    SubdivNode::Leaf(l.clone()),
                                    SubdivNode::Leaf(l.clone()),];
                                let mut pool_borrow = self.pool.borrow_mut();
                                let idx = pool_borrow.insert(NaiveOctreeBranch {
                                    children,
                                    branch_data: D::default(),
                                }).clone();
                                drop(pool_borrow);
                                let mut pool_ref = self.pool.borrow_mut();
                                let elem = &mut pool_ref.get_mut(parent_idx).unwrap()
                                    .children[index_encode(coord.pos, current_scale-coord.scale)];
                                *elem = NaiveOctreeNode::Branch(idx);
                                parent_idx = idx;
                            }
                            else {
                                match current_node_info {
                                    SubdivNode::Branch(branch_index) => {
                                        parent_idx = branch_index;
                                    },
                                    _ => unreachable!(), // We just split this into a branch if it's a leaf.
                                }
                            }
                            current_scale -= 1;
                        }
                    }
                },
                SubdivNode::Leaf(_) => unreachable!(),
            }
        }
        Err(SubdivError::DetailNotPresent)
    }

    /// Sets a voxel in the octree, and also checks to see if its parent node can be merged.
    /// After that, checks recursively for nodes which can be combined.
    fn set_and_merge(&mut self, _coord: OctPos<P>, _leaf_val: L) -> Result<(), SubdivError> {
        /*match set_recurse(&mut self.root, coord, self.scale, coord.scale, &leaf_val, &mut self.pool) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }*/
        unimplemented!()
    }

    /// Sets the branch data at a certain location.
    /// Will error if this location is or is below a leaf.
    fn set_branch_data(&mut self, coord: OctPos<P>, branch_val: D) -> Result<(), SubdivError> {
        if coord.scale > self.scale {
            //Trying to set a voxel larger than our root node.
            return Err(SubdivError::OutOfScale);
        } else if !( pos_within_node(coord, self.scale) ){
            //Selected node cannot possibly exist in our octree.
            return Err( SubdivError::OutOfBounds);
        }
        if coord.scale == self.scale {
            match &self.root {
                SubdivNode::Leaf(_) => return Err(SubdivError::ReqBranchGotLeaf),
                SubdivNode::Branch(root_branch_index) => {
                    let mut pool_ref = self.pool.borrow_mut();
                    let mut elem = &mut pool_ref.get_mut(*root_branch_index).unwrap();
                    (*elem).branch_data = branch_val;
                    return Ok(());
                },
            }
        }
        else {
            match &self.root {
                SubdivNode::Branch(root_branch_index) => {
                    let mut parent_idx = *root_branch_index;
                    let mut current_scale = self.scale-1;
                    while current_scale >= coord.scale {
                        //Set up decision making here.
                        //Get a node - Borrow a reference to our current node.
                        let pool_borrow = self.pool.borrow();
                        let current_node_info = pool_borrow.get(parent_idx).unwrap()
                            .children[index_encode(coord.pos, current_scale-coord.scale)].clone();
                        drop(pool_borrow);
                        //Have we hit our target?
                        if current_scale == coord.scale {
                            match current_node_info {
                                SubdivNode::Leaf(_) => return Err(SubdivError::ReqBranchGotLeaf),
                                SubdivNode::Branch(branch_idx) => {
                                    self.pool.borrow_mut().get_mut(branch_idx).unwrap().branch_data = branch_val;
                                    return Ok(());
                                }
                            }
                        }
                        else {
                            match current_node_info {
                                SubdivNode::Branch(branch_index) => {
                                    parent_idx = branch_index;
                                    current_scale -= 1;
                                },
                                SubdivNode::Leaf(_) => return Err(SubdivError::ReqBranchGotLeaf), // Leaf is above us, no branch here.
                            }

                        }
                    }
                },
                SubdivNode::Leaf(_) => return Err(SubdivError::ReqBranchGotLeaf),
            }
        }
        Err(SubdivError::DetailNotPresent)
    }
}

impl<L, D, P> SubdivDrain<L, P> for NaiveVoxelOctree<L, D> where L: Voxel, D: Voxel + Default, P: VoxelCoord {
    fn set(&mut self, coord: OctPos<P>, value: L) -> Result<(), SubdivError> {
        //TODO: Change this to set_and_merge when it's ready.
        self.set_raw(coord, value)
    }
}

#[test]
fn test_octree() {
    //Scale 6: a 64 meter x 64 meter x 64 meter chunk
    let mut tree : NaiveVoxelOctree<String, ()> = NaiveVoxelOctree::new("".to_owned(), 6);

    //At the 32x32x32 meter node level
    let first_pos : OctPos <u32> = opos!((1, 0, 0) @ 5);
    //At the 2x2x2 meter node level
    let second_pos : OctPos <u32> = opos!((15, 3, 24) @ 1);
    //At the 1x1x1 meter node level
    let third_pos : OctPos <u32> = opos!((2, 2, 3) @ 0);
    //Back at the 2x2x2 meter node level
    let fourth_pos : OctPos <u32> = opos!((1, 0, 1) @ 1);

    tree.set_raw(first_pos, "First!".to_owned() ).unwrap();
    tree.set_raw(second_pos, "Second!".to_owned() ).unwrap();
    tree.set_raw(third_pos, "Third!".to_owned() ).unwrap();

    assert_eq!(tree.get(first_pos).unwrap(), SubdivNode::Leaf("First!".to_owned()) );
    assert_eq!(tree.get(second_pos).unwrap(), SubdivNode::Leaf("Second!".to_owned()) );
    assert_eq!(tree.get(third_pos).unwrap(), SubdivNode::Leaf("Third!".to_owned()) );
    assert_eq!(tree.get(opos!((33, 2, 8)@ 0)).unwrap(), SubdivNode::Leaf("First!".to_owned()) );

    tree.set_raw(fourth_pos, "Fourth!".to_owned() ).unwrap();

    //We are looking at a 16x16x16 node.
    let _big_node : OctPos <u32> = opos!((0, 0, 0) @ 4);
    //if let SubdivNode::Branch(ref branch_dat) = tree.get()
}