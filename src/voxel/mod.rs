#![feature(collections)]

pub struct Coord3 <T>  where
    T: PartialEq
     + Add<Output=T> + Sub<Output=T>
     + Mul<Output=T> + Div<Output=T>
     + Rem<Output=T> + fmt::Debug
     + Copy
{ pub x: T, pub y: T, pub z: T, }

/*For voxel data structures with a level of detail, we will
assume that the level of detail is a signed integer, and
calling these methods / treating them as "flat" voxel
structures implies acting on a level of detail of 0. */

/// A basic trait for any 3d grid of data.
/// For this trait, a single level of detail is assumed.
///
/// Type arguments are type of element, type of position / index.
pub trait VoxelStorage<T, P> where
    P: PartialEq
     + Add<Output=P> + Sub<Output=P>
     + Mul<Output=P> + Div<Output=P>
     + Rem<Output=P> + fmt::Debug
     + Copy
{
    fn get(&self, x: P, y: P, z: P) -> T;
    fn getv(&self, coord: Coord3<P>) -> T {
        self.get(coord.x, coord.y, coord.z)
    }
    
    fn set(&mut self, x: P, y: P, z: P, value: T);
    fn setv(&mut self, coord: Coord3<P>, value: T) {
        self.set(coord.x, coord.y, coord.z, value);
    }
    
    ///What format of VoxelStorage is this?
    ///
    ///Returns a UUID encoded as a UTF-8 string.
    fn getFormatID() -> String;

    ///What version of our format is this?
    ///
    ///Note: New revisions of a VoxelFormat class that retain the same
    ///Format ID MUST be backwards-compatible: That is, Format ID A
    ///of version B must be able to read any file in format A of a version
    /// 0, 1, 2, ... B.
    ///
    /// If you need to make breaking changes to a format, make a new UUID.
    fn getFormatRevision() -> i64;
}
///A simple packed array 
///
///Type arguments are type of element, type of position / index.
pub struct VoxelArray<'a, T: 'a, P> {
    sizeX: P, sizeY: P, sizeZ: P,
    data: &'a [mut T],
}
/// Type arguments are type of element, type of position / index.
pub impl<'a, T, P> VoxelStorage<T, P> for VoxelArray<'a, T, P> {
    fn get(&self, x: P, y: P, z: P) -> T {
        self.data[ x + (self.sizeX * y) + ((self.sizeX * self.sizeY) * z)]
    }
    
    fn set(&mut self, x: P, y: P, z: P, value: T)
    {
        self.data[ x + (self.sizeX * y) + ((self.sizeX * self.sizeY) * z)] 
            = value;
    }
    
    ///What format of VoxelStorage is this?
    ///
    ///Returns a UUID encoded as a UTF-8 string.
    fn getFormatID() -> String 
    {
        "Willy Wonka's Wily Willy" //TODO
    }

    ///What version of our format is this?
    fn getFormatRevision() -> i64
    {
        0 //TODO: Automate this as a git hook.
    }
}

