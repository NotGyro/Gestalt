//! Math helper code that describes an octree-style subdividing voxel space, 
//! where each voxel is either a NODE or a LEAF. Each node is cut into eight
//! subunits (split in half along all three cardinal axis), each of which is 
//! 1/2 the length of the parent node along each axis and is 1/8th of its volume.
//! Each subunit is either a node or a leaf. A leaf is where you find your actual
//! voxel data.
//! 
//! In Gestalt, (global) scale 0 is a 1 x 1 x 1 meter cube. The size of the cubes
//! you are looking at is desribed by side_length = 2^scale. Negative scales look
//! at smaller and smaller cubes, as governened by plain old negative exponent rules. 
//! Larger numbers yield larger nodes, smaller numbers yield smaller nodes.
//! Unfortunately, this means that if you want *more* detail you need a *smaller* number.
//! Over-all it seems like the most intuitive and consistent way to go about it, despite
//! that potential cause of confusion.

use voxel::voxelmath::*;
use serde::{Serialize, Deserialize};

use std::fmt;

// Scale gets away with just being a signed byte forever because it's exponential.
// You will NEVER need LOD cubes tinier than 2.9387359e-39 meters (2^-128) to a side, 
// and 1.7014118e+38 meters (2^127) puts Earth's distance from the sun to shame.

pub type Scale = i8;

/// A point in progressive-detail Voxel space. (A cell.)
/// The concept of node or leaf doesn't come into it yet here -
/// this is just a coordinate.
/// Our "pos" value is measured in number of cells at "scale"'s grid from the origin of 0,0,0.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OctPos<T : VoxelCoord> {
	pub scale : Scale,
    pub pos : VoxelPos<T>,
}

impl <T> fmt::Display for OctPos<T> where T : VoxelCoord + fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} at scale {}", self.pos, self.scale)
    }
}

// The scale argument here taken is "distance" of scale. 
// I.e. converting from 2 to 8 gives you a delta_scale of +6.
#[inline]
pub fn scale_coord<T>(x: T, delta_scale: Scale) -> T 
        where T : VoxelCoord {
    if delta_scale == 0 {
        return x;
    }
    else if delta_scale > 0 {
        // Going from smaller to larger scale, you end up with a fewer number of larger cells from origin. 
        return x.div_floor( &(T::one() + T::one() /* 2 */ ).pow(delta_scale as u32));
    }
    else /* Implied delta_scale < 0 */ {
        return x.mul( (T::one() + T::one() /* 2 */ ).pow(delta_scale.abs() as u32) );
    }
}

impl<T> OctPos <T> where T : VoxelCoord {
    #[inline(always)]
    pub fn scale_to(&self, scl: Scale) -> Self {
        let delta_scale = scl - self.scale;
        OctPos {
            scale: scl,
            pos: VoxelPos { x : scale_coord(self.pos.x, delta_scale),
                            y : scale_coord(self.pos.y, delta_scale),
                            z : scale_coord(self.pos.z, delta_scale),
                            },
        }
    }
    #[inline(always)]
    #[allow(dead_code)]
    pub fn scale_into(&mut self, scl: Scale) {
        let delta_scale = scl - self.scale;
        self.pos.x = scale_coord(self.pos.x, delta_scale);
        self.pos.y = scale_coord(self.pos.y, delta_scale);
        self.pos.z = scale_coord(self.pos.z, delta_scale);
    }
    #[inline(always)]
    pub fn from_four(x : T, y : T, z : T, scl : Scale) -> OctPos <T> {
        OctPos {
            scale: scl,
            pos: VoxelPos { x : x,
                            y : y,
                            z : z,
                            },
        }
    }
}

macro_rules! opos {
    (($x:expr, $y:expr, $z:expr) @ $w:expr) => { OctPos::from_four($x, $y, $z, $w) }
}

/// Does your OctPos pos at its given scale fit in a root node of scale scl?
/// It is assumed this is normalized so the root node's origin is at 0,0,0
/// and pos is describing an offset from this origin.
#[inline(always)]
pub fn pos_within_node<T>(pos: OctPos<T>, root_scl : Scale) -> bool where T: VoxelCoord {
    //Check low edge
    if (pos.pos.x < T::zero()) || (pos.pos.y < T::zero()) || (pos.pos.z < T::zero()) {
        return false;
    }
    //Construct a value that is exactly one beyond our max value in every direction.
    let high_edge: OctPos<T> = opos!((T::one(), T::one(), T::one()) @ root_scl);
    let high_edge = high_edge.scale_to(pos.scale);
    //Must be less than the value which is one beyond our node's bounds in every direction.
    (pos.pos.x < high_edge.pos.x ) && (pos.pos.y < high_edge.pos.y) && (pos.pos.z < high_edge.pos.z)
}

#[test]
fn test_scale() {
    let mut position : OctPos<i32> = OctPos { 
        scale: 2,
        pos: VoxelPos{ x: 16, y: 35, z: -5},
    };
    position.scale_into(4);
    assert_eq!(position.pos.x, 4);
    assert_eq!(position.pos.y, 8);
    assert_eq!(position.pos.z, -2);

    let a : i64 = rand::random();
    let scl : i8 = 4;
    let pos : OctPos<i64> = OctPos {
        scale: 0,
        pos: VoxelPos{ x: a, y: a, z: a},
    };
    let new_pos = pos.scale_to(scl);
    assert_eq!(new_pos.pos.x, a.div_floor( & (2 as i64).pow(scl as u32)) );
}