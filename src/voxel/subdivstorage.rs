extern crate std;
extern crate num;

use voxel::voxelmath::*;
use voxel::subdivmath::*;
use std::fmt::{Display, Debug};
use std::fmt;
use std::error;
use std::error::Error;
use std::result::Result;
use voxel::voxelstorage::Voxel;

/// An error reported upon trying to get or set a voxel outside of our range.
#[derive(Debug)]
#[allow(dead_code)]
pub enum SubdivError {
    OutOfBounds,
    OutOfScale,
    DetailNotPresent,
    SplittingBranch,
    ReqLeafGotBranch,
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
            SubdivError::ReqLeafGotBranch => write!(f, "Tried to get a leaf value, but this voxel is a branch."),
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
    pub fn new_leaf(val: L) -> SubdivNode<L, B> { SubdivNode::Leaf(val) }
}

#[allow(dead_code)]
pub type NodeNoLOD<T> = SubdivNode<(), T>;

impl <L, B> Default for SubdivNode<L, B> where L : Voxel, B : Voxel {
    #[inline]
    fn default() -> Self { SubdivNode::Leaf(L::default()) }
}

#[allow(dead_code)]
pub enum NodeChildIndex { 
    ZeroXZeroYZeroZ, 
    ZeroXZeroYOneZ, 
    ZeroXOneYZeroZ, 
    ZeroXOneYOneZ, 
    OneXZeroYZeroZ,
    OneXZeroYOneZ,
    OneXOneYZeroZ,
    OneXOneYOneZ,
}

// Bitwise encoding, last three bits of a u8. It goes x, y, then z. (z changes first) Each is 0 if closer to origin, 1 if further.
impl NodeChildIndex {
    #[allow(dead_code)]
    fn num_representation(&self) -> usize {
        match self { 
            NodeChildIndex::ZeroXZeroYZeroZ => 0, 
            NodeChildIndex::ZeroXZeroYOneZ => 1,
            NodeChildIndex::ZeroXOneYZeroZ => 2, 
            NodeChildIndex::ZeroXOneYOneZ => 3,
            NodeChildIndex::OneXZeroYZeroZ => 4,
            NodeChildIndex::OneXZeroYOneZ => 5,
            NodeChildIndex::OneXOneYZeroZ => 6,
            NodeChildIndex::OneXOneYOneZ => 7,
        }
    }
    #[allow(dead_code)]
    fn from_num_representation(val : usize) -> Self {
        match val {
            0 => NodeChildIndex::ZeroXZeroYZeroZ,
            1 => NodeChildIndex::ZeroXZeroYOneZ,
            2 => NodeChildIndex::ZeroXOneYZeroZ, 
            3 => NodeChildIndex::ZeroXOneYOneZ,
            4 => NodeChildIndex::OneXZeroYZeroZ,
            5 => NodeChildIndex::OneXZeroYOneZ,
            6 => NodeChildIndex::OneXOneYZeroZ,
            7 => NodeChildIndex::OneXOneYOneZ,
            _ => panic!(),
        }
    }
}

#[inline(always)]
pub fn index_for_scale_at_pos<T: VoxelCoord>(pos : VoxelPos<T>, scl : Scale) -> usize {
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
    //println!("Calling index_for_scale_at_pos() on VoxelPos {} with scale {} yields child index {}", pos, scl, idx);
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
pub trait SubdivVoxelSource<T: Voxel, P: VoxelCoord> {
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
pub trait SubdivVoxelDrain<T: Voxel, P: VoxelCoord> {
    fn set(&mut self, coord: OctPos<P>, value: T) -> Result<(), SubdivError>;
}

/// Any SubdivVoxelStorage which has defined, finite bounds.
/// Must be able to store a valid voxel for any position within
/// the range provided by get_bounds().
/// Usually, this implies that the SubdivVoxelStorage is not paged.
pub trait SubdivVoxelStorageBounded<P: VoxelCoord> { 
    fn get_bounds(&self) -> VoxelRange<P>;
}

/// Any LOD data which can be generated for an octree branch from its child nodes.
pub trait LODData<T: Voxel> : Voxel {
    fn represent(voxel: &T) -> Self;
    fn downsample_from(&mut self, child_values: &[Self; 8]);
}

impl<T> LODData<T> for () where T: Voxel { 
    fn represent(_voxel: &T) -> Self { () }
    fn downsample_from(&mut self, _child_values: &[Self; 8]) {}
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
pub trait OctreeSource<L: Voxel, D: Voxel + LODData<L>, P: VoxelCoord> : SubdivVoxelSource<SubdivNode<L, D>, P> {

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
/*
pub trait OctreeDrain<L: Voxel, D: Voxel + LODData<L>, P: VoxelCoord> : OctreeSource<L, D, P> {
    /// You cannot set something directly *to* a branch.
    fn set(&self, coord: OctPos<P>, leaf_val: L) -> Result<(), SubdivError>;

    /// Notify ourselves that the children of this branch node have changed, figure out what to do about that.
    /// Default implementation does nothing.
    /// Please call this with the position of the lowest common octree cell containing all modifications since last recalc.  
    fn rebuild_lod(&self, pos: OctPos<P>) {}
}*/

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NaiveOctreeBranch<L: Voxel, D: Voxel + LODData<L>> {
    pub children : [SubdivNode<L, Box<NaiveOctreeBranch<L, D>> >; 8],
    // Any data that lives at the branch level.
    pub lod_data : D,
}

impl<L, D> NaiveOctreeBranch<L, D> where L: Voxel, D: Voxel + LODData<L> {
    #[allow(dead_code)]
    fn rebuild_lod(&mut self) {
        let mut lod_info : [D; 8] = [D::default(),
                                    D::default(),
                                    D::default(),
                                    D::default(),
                                    D::default(),
                                    D::default(),
                                    D::default(),
                                    D::default()];
        for i in 0..8 {
            match &mut self.children[i] { 
                SubdivNode::Leaf(leaf_child) => lod_info[i] = D::represent(&leaf_child),
                SubdivNode::Branch(ref mut branch_child) => { 
                    branch_child.rebuild_lod(); 
                    lod_info[i] = branch_child.lod_data.clone();
                },
            }
        }
        self.lod_data.downsample_from(&lod_info);
    }
}

pub type NaiveOctreeNode<L, D> = SubdivNode<L, Box<NaiveOctreeBranch<L, D>> >;

#[derive(Clone, Debug)]
pub struct NaiveVoxelOctree<L, D> where L: Voxel, D: Voxel + LODData<L> {
    pub scale : Scale,
    pub root : NaiveOctreeNode<L, D>,
}

impl<L, D> NaiveOctreeNode<L, D> where L: Voxel, D: Voxel + LODData<L> {
    pub fn split_into_branch(&mut self) { 
        if let SubdivNode::Leaf(leaf_value) = self {
            let children : [NaiveOctreeNode<L, D>; 8] = [SubdivNode::new_leaf(leaf_value.clone()),
                                                    SubdivNode::new_leaf(leaf_value.clone()),
                                                    SubdivNode::new_leaf(leaf_value.clone()),
                                                    SubdivNode::new_leaf(leaf_value.clone()),
                                                    SubdivNode::new_leaf(leaf_value.clone()),
                                                    SubdivNode::new_leaf(leaf_value.clone()),
                                                    SubdivNode::new_leaf(leaf_value.clone()),
                                                    SubdivNode::new_leaf(leaf_value.clone()),];
            *self = NaiveOctreeNode::Branch(Box::new(NaiveOctreeBranch {
                    children: children,
                    lod_data: D::represent(&leaf_value),
            }));
        } 
        //else {
        //}
    }
    #[allow(dead_code)]
    pub fn rebuild_lod(&mut self) {
        if let SubdivNode::Branch(ref mut branch_self) = self {
            branch_self.rebuild_lod();
        } 
        //else {
            //We are a leaf node, no LOD to rebuild.
        //}
    }
}

impl<L, D, P> OctreeSource<L, D, P> for NaiveVoxelOctree<L, D> where L: Voxel, D: Voxel + LODData<L>, P: VoxelCoord {
    
    #[inline]
    fn get_details(&self, coord: OctPos<P>) -> Result<(SubdivNode<L, D>, Scale), SubdivError> {
        if coord.scale > self.scale {
            //Trying to set a voxel larger than our root node.
            return Err(SubdivError::OutOfScale);
        } else if !( pos_within_node(coord, self.scale) ){
            //Selected node cannot possibly exist in our octree.
            return Err( SubdivError::OutOfBounds);
        }
        unsafe {
            let mut current_node = &self.root as *const NaiveOctreeNode<L,D>;
            let mut current_scale = self.scale;
            while current_scale >= coord.scale {
                //Have we hit our target?
                if current_scale == coord.scale {
                    match &*current_node { 
                        SubdivNode::Leaf(leaf_dat) => return Ok( (SubdivNode::Leaf(leaf_dat.clone()), current_scale)  ),
                        SubdivNode::Branch(branch_dat) => return Ok( (SubdivNode::Branch(branch_dat.lod_data.clone()), current_scale) ),
                    }
                }
                else {
                    // We have not yet gotten to target.
                    match &*current_node { 
                        //Target is below our scale but we are a leaf. That space partition is part of this one.
                        SubdivNode::Leaf(leaf_dat) => return Ok(  (SubdivNode::Leaf(leaf_dat.clone()), current_scale)  ),
                        SubdivNode::Branch(branch_dat) => {
                            //Not found and this is a branch. Time for recursion.
                            //Our child nodes are implicitly at our scale -1.
                            current_node = &branch_dat.children[index_for_scale_at_pos(coord.pos, current_scale-coord.scale-1)] as *const NaiveOctreeNode<L,D>;
                        }
                    }
                }
                current_scale -= 1;
            }
            Err(SubdivError::DetailNotPresent)
        }
    }
}

impl<L, D, P> SubdivVoxelSource<SubdivNode<L, D>, P> for NaiveVoxelOctree<L, D> where L: Voxel, D: Voxel + LODData<L>, P: VoxelCoord {
    
    #[inline]
    fn get(&self, coord: OctPos<P>) -> Result<SubdivNode<L, D>, SubdivError> {
        Ok(self.get_details(coord)?.0)
    }
    //Cannot get a node bigger than our root node.
    fn get_max_scale(&self) -> Scale { self.scale }
}

impl<L, D, P> SubdivVoxelDrain<L, P> for NaiveVoxelOctree<L, D> where L: Voxel, D: Voxel + LODData<L>, P: VoxelCoord {
    
    fn set(&mut self, coord: OctPos<P>, value: L) -> Result<(), SubdivError> {
        if coord.scale > self.scale {
            //Trying to set a voxel larger than our root node.
            return Err(SubdivError::OutOfScale);
        } else if !( pos_within_node(coord, self.scale) ){
            //Selected node cannot possibly exist in our octree.
            return Err( SubdivError::OutOfBounds);
        }
        unsafe { 
            let mut current_node = &mut self.root as *mut NaiveOctreeNode<L,D>;
            let mut current_scale = self.scale;
            while current_scale >= coord.scale {
                        //Have we hit our target?
                if current_scale == coord.scale {
                    *current_node = SubdivNode::Leaf(value);
                    return Ok(());
                }
                else {
                    // We have not yet gotten to target.
                    if let SubdivNode::Leaf(_) = &mut *current_node {
                        //Target is below our scale. We will need to create a child node, and recurse on it.
                        (*current_node).split_into_branch();
                    }
                    match *current_node { 
                        SubdivNode::Branch(ref mut branch_dat) => {
                            //Not found and this is a branch. Time for recursion.
                            //Our child nodes are implicitly at our scale -1.
                            current_node = &mut branch_dat.children[index_for_scale_at_pos(coord.pos, current_scale-coord.scale-1)] as *mut NaiveOctreeNode<L,D>;
                        }
                        _ => unreachable!(), // We just split this into a branch if it's a leaf.
                    }
                }
                current_scale -= 1;
            }
        }
        Err(SubdivError::DetailNotPresent)
    }
}

//For testing purposes 
impl LODData<String> for Vec<String> {
    fn represent(voxel: &String) -> Self { return vec!(voxel.clone()) }
    fn downsample_from(&mut self, child_values: &[Self; 8]) {
        self.clear();
        for val in child_values {
            for sub_val in val { 
                if !( self.contains(sub_val) ) {
                    self.push(sub_val.clone());
                }
            }
        }
    }
}

#[test]
fn test_octree() {
    //Scale 6: a 64 meter x 64 meter x 64 meter chunk
    let mut tree : NaiveVoxelOctree<String, Vec<String>> = NaiveVoxelOctree{scale : 6 , root: NaiveOctreeNode::new_leaf("".to_owned() )};

    //At the 32x32x32 meter node level
    let first_pos : OctPos <u32> = opos!((1, 0, 0) @ 5);
    //At the 2x2x2 meter node level
    let second_pos : OctPos <u32> = opos!((15, 3, 24) @ 1);
    //At the 1x1x1 meter node level
    let third_pos : OctPos <u32> = opos!((2, 2, 3) @ 0);
    //Back at the 2x2x2 meter node level
    let fourth_pos : OctPos <u32> = opos!((1, 0, 1) @ 1);

    tree.set(first_pos, "First!".to_owned() ).unwrap();
    tree.set(second_pos, "Second!".to_owned() ).unwrap();
    tree.set(third_pos, "Third!".to_owned() ).unwrap();
    
    tree.root.rebuild_lod();

    assert_eq!(tree.get(first_pos).unwrap(), SubdivNode::Leaf("First!".to_owned()) );
    assert_eq!(tree.get(second_pos).unwrap(), SubdivNode::Leaf("Second!".to_owned()) );
    assert_eq!(tree.get(third_pos).unwrap(), SubdivNode::Leaf("Third!".to_owned()) );
    assert_eq!(tree.get(opos!((33, 2, 8)@ 0)).unwrap(), SubdivNode::Leaf("First!".to_owned()) );

    if let SubdivNode::Branch(ref tree_root) = tree.root {
        assert!(tree_root.lod_data.contains(&"First!".to_owned()));
        assert!(tree_root.lod_data.contains(&"Second!".to_owned()));
        assert!(tree_root.lod_data.contains(&"Third!".to_owned()));
    }
    else {
        panic!();
    }
    
    tree.set(fourth_pos, "Fourth!".to_owned() ).unwrap();
    tree.root.rebuild_lod();

    //We are looking at a 16x16x16 node.
    let big_node : OctPos <u32> = opos!((0, 0, 0) @ 4);
    
    if let SubdivNode::Branch(ref lod) = tree.get(big_node).unwrap() {
        assert!(lod.contains(&"Third!".to_owned()));
        assert!(lod.contains(&"Fourth!".to_owned()));
        assert!(!lod.contains(&"First!".to_owned()));
        assert!(!lod.contains(&"Second!".to_owned()));
    }
    else { 
        panic!();
    }
    
    println!("We have: {:?}", tree);
    //if let SubdivNode::Branch(ref branch_dat) = tree.get()
}