use hashbrown::HashMap;

use crate::common::voxelmath::VoxelPos;

use super::{voxelstorage::Voxel, voxelarray::VoxelArrayStatic, VoxelError, VoxelStorage};

pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_SIZE_CUBED: usize = CHUNK_SIZE*CHUNK_SIZE*CHUNK_SIZE;

pub struct ChunkSmall<T:Voxel>  {
    //Attempting to use the constant causes Rust to freak out for some reason
    //so I simply type 16
    pub inner: VoxelArrayStatic<u8, 16>,
    pub palette: [T; 256],
    pub reverse_palette: HashMap<T,u8>,
    pub highest_idx: u8,
    // Used by the serializer to tell if the palette has changed.
    pub palette_dirty: bool,
}

impl<T:Voxel> ChunkSmall<T> {
    #[inline(always)]
    pub fn get_raw(&self, coord: VoxelPos<u16>) -> &u8 {
        //The intent here is so that bounds checking is only done ONCE for this structure. 
        self.inner.get_raw(coord)
    }
    #[inline(always)]
    pub fn get(&self, coord: VoxelPos<u16>) -> &T {
        &self.palette[*self.get_raw(coord) as usize]
    }
    #[inline(always)]
    pub fn set_raw(&mut self, coord: VoxelPos<u16>, value: u8) {
        self.inner.set_raw(coord, value);
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: T) -> Option<u8> {
        self.reverse_palette.get(&tile).map(|v| *v)
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<&T> {
        if idx > 255 { return None };
        if idx > self.highest_idx as u16 { return None };
        Some(&self.palette[idx as usize])
    }
    ///Use this chunk to construct a chunk with u16 tiles rather than u8 ones. 
    #[inline]
    pub fn expand(&self) -> ChunkLarge<T> {
        let mut new_palette : HashMap<u16, T> = HashMap::new();
        for (i, entry) in self.palette.iter().enumerate() {
            new_palette.insert(i as u16, entry.clone());
        }
        let mut new_inner = VoxelArrayStatic::new(0);

        for i in 0..CHUNK_SIZE_CUBED { 
            let tile = self.inner.get_raw_i(i);
            new_inner.set_raw_i(i, *tile as u16);
        }

        let mut new_reverse_palette : HashMap<T, u16> = HashMap::default();
        for (key, value) in self.reverse_palette.iter() {
            new_reverse_palette.insert(key.clone(), *value as u16);
        }
        ChunkLarge { inner: new_inner,
            palette: new_palette,
            reverse_palette: new_reverse_palette,
            palette_dirty: true,
        }
    }
    /// Adds a Tile ID to its palette. If we succeeded in adding it, return the associated index. 
    /// If it already exists, return the associated index. If we're out of room, return None.
    #[inline]
    pub fn add_to_palette(&mut self, tile: T) -> Option<u16> {
        match self.reverse_palette.get(&tile) {
            Some(idx) => {
                //Already in the palette. 
                Some(*idx as u16)
            },
            None => {
                self.palette_dirty = true;
                //We have run out of space.
                if self.highest_idx >= 255 { 
                    return None;
                }
                else { 
                    self.highest_idx += 1;
                    let idx = self.highest_idx;
                    self.palette[idx as usize] = tile.clone();
                    self.reverse_palette.insert(tile, idx);
                    Some(idx as u16)
                }
            }
        }
    }
}

//In a 16*16*16, a u16 encodes a number larger than the total number of possible voxel positions anyway.
pub struct ChunkLarge<T:Voxel> {
    //Attempting to use the constant causes Rust to freak out for some reason
    //so I simply type 16
    pub inner: VoxelArrayStatic<u16, 16>,
    pub palette: HashMap<u16, T>,
    pub reverse_palette: HashMap<T, u16>,
    pub palette_dirty: bool,
}


impl<T:Voxel> ChunkLarge<T> {
    #[inline(always)]
    pub fn get_raw(&self, coord: VoxelPos<u16>) -> &u16 {
        self.inner.get_raw(coord)
    }
    #[inline(always)]
    pub fn get(&self, coord: VoxelPos<u16>) -> &T {
        //Get our int data and use it as an index for our palette. Yay constant-time!  
        &self.palette[&self.inner.get_raw(coord)]
    }
    #[inline(always)]
    pub fn set_raw(&mut self, coord: VoxelPos<u16>, value: u16) {
        self.inner.set_raw(coord, value);
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: T) -> Option<u16> {
        self.reverse_palette.get(&tile).map(|v| *v)
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<&T> {
        self.palette.get(&idx)
    }
    /// Adds a Tile ID to its palette. If we succeeded in adding it, return the associated index. 
    /// If it already exists, return the associated index. If we're out of room, return None.
    #[inline]
    pub fn add_to_palette(&mut self, tile: T) -> u16 {
        match self.reverse_palette.get(&tile) {
            Some(idx) => {
                //Already in the palette. 
                *idx as u16
            },
            None => {
                self.palette_dirty = true;
                let next_idx : u16 = self.palette.len() as u16;
                self.palette.insert(next_idx, tile.clone());
                self.reverse_palette.insert(tile, next_idx);
                next_idx
            }
        }
    }
}

pub enum ChunkData<T:Voxel> {
    ///Chunk that is all one value (usually this is for chunks that are 100% air). Note that, after being converted, idx 0 maps to 
    Uniform(T),
    ///Chunk that maps palette to 8-bit values.
    Small(Box<ChunkSmall<T>>),
    ///Chunk that maps palette to 16-bit values.
    Large(Box<ChunkLarge<T>>),
}

pub struct Chunk<T:Voxel> {
    pub revision: u64,
    pub data: ChunkData<T>,
}


impl<T:Voxel> Chunk<T> {
    pub fn new(default_voxel: T) -> Self {
        Chunk{
            revision: 0,
            data: ChunkData::Uniform(default_voxel),
        }
    }

    #[inline(always)]
    pub fn get_raw(&self, pos: VoxelPos<u16>) -> u16 {
        match &self.data {
            ChunkData::Uniform(_) => 0,
            ChunkData::Small(inner) => *inner.get_raw(pos) as u16,
            ChunkData::Large(inner) => *inner.get_raw(pos),
        }
    }
    #[inline(always)]
    pub fn set_raw(&mut self, pos: VoxelPos<u16>, value: u16) {
        match &mut self.data {
            //TODO: Smarter way of handling this case. Currently, just don't. 
            //I don't want to return a result type HERE for performance reasons.
            ChunkData::Uniform(_) => if value != 0 { panic!("Attempted to set_raw() on a Uniform chunk!")}, 
            ChunkData::Small(ref mut inner) => inner.set_raw(pos, value as u8),
            ChunkData::Large(ref mut inner) => inner.set_raw(pos, value),
        };
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: T) -> Option<u16> {
        match &self.data {
            ChunkData::Uniform(val) => { 
                if tile == *val { 
                    Some(0)
                }
                else { 
                    None
                }
            }, 
            ChunkData::Small(inner) => inner.index_from_palette(tile).map(|v| v as u16),
            ChunkData::Large(inner) => inner.index_from_palette(tile),
        }
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<&T> {
        match &self.data {
            ChunkData::Uniform(val) => {
                if idx == 0 { 
                    Some(val)
                }
                else { 
                    None
                }
            }, 
            ChunkData::Small(inner) => inner.tile_from_index(idx),
            ChunkData::Large(inner) => inner.tile_from_index(idx),
        }
    }
    #[inline(always)]
    pub fn is_palette_dirty(&self) -> bool {
        match &self.data {
            ChunkData::Uniform(_) => false,
            ChunkData::Small(inner) => inner.palette_dirty,
            ChunkData::Large(inner) => inner.palette_dirty,
        }
    }
    #[inline(always)]
    pub fn mark_palette_dirty_status(&mut self, set_to: bool) {
        match &mut self.data {
            ChunkData::Uniform(_) => {},
            ChunkData::Small(ref mut inner) => inner.palette_dirty = set_to,
            ChunkData::Large(ref mut inner) => inner.palette_dirty = set_to,
        }
    }
    #[inline]
    pub fn add_to_palette(&mut self, tile: T) -> u16 {
        match &mut self.data {
            ChunkData::Uniform(val) => {
                if tile == *val {
                    0
                }
                else {
                    // Convert to a ChunkSmall.
                    let structure = VoxelArrayStatic::new(0); //0 will be *val
                    
                    let mut palette : [T; 256] = unsafe {
                        let mut array: [T; 256] = std::mem::uninitialized();
                        for element in array.iter_mut() {
                            std::ptr::write(element, val.clone());
                        }
                        array
                    };
                    palette[1] = tile.clone();
                    let mut reverse_palette: HashMap<T, u8> = HashMap::default();
                    reverse_palette.insert(val.clone(), 0);
                    reverse_palette.insert(tile, 1);
                    self.data = ChunkData::Small(Box::new(ChunkSmall {
                        inner: structure,
                        palette: palette,
                        reverse_palette: reverse_palette,
                        highest_idx: 1,
                        palette_dirty: false,
                    }));
                    1
                }
            },
            ChunkData::Small(inner) => {
                match inner.add_to_palette(tile.clone()) {
                    Some(idx) => {
                        idx
                    },
                    None => {
                        //We need to expand it.
                        let mut new_inner = Box::new(inner.expand());
                        let idx = new_inner.add_to_palette(tile); //We just went from u8s to u16s, the ID space has quite certainly 
                        self.data = ChunkData::Large(new_inner);
                        idx
                    },
                }
            },
            ChunkData::Large(inner) => inner.add_to_palette(tile),
        }
    }
}

impl<T: Voxel> VoxelStorage<T, u16> for Chunk<T> { 
    #[inline(always)]
    fn get(&self, pos: VoxelPos<u16>) -> Result<&T, VoxelError> {
        match &self.data {
            ChunkData::Uniform(val) => Ok(val), 
            ChunkData::Small(inner) => Ok(inner.get(pos)),
            ChunkData::Large(inner) => Ok(inner.get(pos)),
        }
    }
    #[inline]
    fn set(&mut self, pos: VoxelPos<u16>, tile: T) -> Result<(), VoxelError> {
        let idx = self.add_to_palette(tile.clone());
        //Did we just change something?
        if self.get(pos)? != &tile {
            //Increment revision.
            self.revision += 1;
        }
        self.set_raw(pos, idx);

        Ok(())
    }
}

#[test]
fn chunk_index_reverse() {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    for _ in 0..4096 {
        let x = rng.gen_range(0..CHUNK_SIZE);
        let y = rng.gen_range(0..CHUNK_SIZE);
        let z = rng.gen_range(0..CHUNK_SIZE);

        let i_value = crate::world::voxelarray::chunk_xyz_to_i(x, y, z, CHUNK_SIZE);
        let (x1, y1, z1) = crate::world::voxelarray::chunk_i_to_xyz(i_value, CHUNK_SIZE);

        assert_eq!( x, x1 );
        assert_eq!( y, y1 );
        assert_eq!( z, z1 );
    }
}

#[test]
fn chunk_index_bounds() {
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                assert!(crate::world::voxelarray::chunk_xyz_to_i(x, y, z, CHUNK_SIZE) < CHUNK_SIZE_CUBED);
            }
        }
    }
}


#[test]
fn assignemnts_to_chunk() {
    use rand::Rng;

    let u1 = String::from("air");
    let u2 = String::from("stone");
    let mut test_chunk = Chunk{revision: 0, data: ChunkData::Uniform(u1.clone()) };

    {
        test_chunk.set(vpos!(1,1,1), u1.clone()).unwrap();
        
        assert_eq!(test_chunk.get(vpos!(1,1,1)).unwrap(), &u1);
    }

    let mut valid_result = false;
    if let ChunkData::Uniform(_) = test_chunk.data {
        valid_result = true;
    }
    assert!(valid_result);

    //Make sure Uniform chunks work the way they're supposed to. 
    
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let pos = vpos!(x as u16, y as u16, z as u16);
                assert_eq!(test_chunk.get(pos).unwrap(), &u1);
                //We should also be able to set every tile of the uniform to the uniform's value, and it'll do nothing.
                test_chunk.set(pos, u1.clone()).unwrap();
            }
        }
    }

    //Implicitly expand it to a Small chunk rather than a Uniform chunk. 
    {
        test_chunk.set(vpos!(2,2,2), u2.clone()).unwrap();

        assert_eq!(test_chunk.get(vpos!(2,2,2)).unwrap(), &u2);
    }

    let mut valid_result = false;
    if let ChunkData::Small(_) = test_chunk.data {
        valid_result = true;
    }
    assert!(valid_result);

    //Make sure that our new ChunkSmall is still the Uniform's tile everywhere except the position where we assigned something else.
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let pos = vpos!(x as u16, y as u16, z as u16);
                if x == 2 && y == 2 && z == 2 {
                    assert_eq!(test_chunk.get(pos).unwrap(), &u2);
                }
                else { 
                    assert_eq!(test_chunk.get(pos).unwrap(), &u1);
                }
            }
        }
    }

    let mut rng = rand::thread_rng();

    {
        for i in 0..253 {
            
            let x = rng.gen_range(0..CHUNK_SIZE);
            let y = rng.gen_range(0..CHUNK_SIZE);
            let z = rng.gen_range(0..CHUNK_SIZE); 
            let pos = vpos!(x as u16, y as u16, z as u16);

            let tile = format!("{}.test",i);

            test_chunk.set(pos, tile.clone()).unwrap();

            assert_eq!(test_chunk.get(pos).unwrap(), &tile);
        }
    }

    let mut valid_result = false;
    if let ChunkData::Small(_) = test_chunk.data {
        valid_result = true;
    }
    assert!(valid_result);

    //Make sure we can assign to everywhere in our chunk bounds.
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let pos = vpos!(x as u16, y as u16, z as u16);
                test_chunk.set(pos, u1.clone()).unwrap();
                assert_eq!(test_chunk.get(pos).unwrap(), &u1);
            }
        }
    }

    {
        for i in 253..1024 {
            
            let x = rng.gen_range(0..CHUNK_SIZE);
            let y = rng.gen_range(0..CHUNK_SIZE);
            let z = rng.gen_range(0..CHUNK_SIZE); 
            let pos = vpos!(x as u16, y as u16, z as u16);

            let tile = format!("{}.test",i);
            
            test_chunk.set(pos, tile.clone()).unwrap();

            assert_eq!(test_chunk.get(pos).unwrap(), &tile);
        }
    }
    let mut valid_result = false;
    if let ChunkData::Large(_) = test_chunk.data {
        valid_result = true;
    }
    assert!(valid_result);
}