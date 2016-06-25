extern crate std;
use voxel::voxelstorage::VoxelStorage;
use core::ops::Index;
use std::vec::Vec;
use std::collections::HashMap;

/* Takes an underlying VoxelStorage and makes a palette of its values to another type.
First type argument is resulting type, second type argument is underlying type, third is position
*/
pub struct VoxelPallete<T : Copy + Hash, U, P = u32> 
            where P : Eq + Ord + Add + Sub + Mul + Div,
            U : Index + Eq + Ord + Add + Sub + Mul + Div { 
                
    base : &mut VoxelStorage<U, P>,
    index : Vec<T>,
    revindex : HashMap<T, U>,
}

impl <T : Copy + Hash, U, P = u32> VoxelStorage<T, P> for VoxelPallete<T, P> 
            where P : Eq + Ord + Add + Sub + Mul + Div,
            U : Index + Eq + Ord + Add + Sub + Mul + Div {
                
    fn get(&self, x: P, y: P, z: P) -> Option<T> {
    	let voxmaybe = self.base.get(x,y,z);
        if voxmaybe.is_some() {
            let val = voxmaybe.unwrap();
            if(val >= index.len()) {
                panic!("Invalid value for voxel palette! Either the map is corrupt or something is very wrong.");
            }
            return Some(index[val]);
        }
        return None();
    }

    fn set(&mut self, x: P, y: P, z: P, value: T) {
        if revindex.contains_key(value) {
    	    self.base.set(x,y,z, revindex.get(value));
        }
        else {
            let newidx = index.len();
            index.push(value);
            revindex.insert(value, newidx);
            self.base.set(x,y,z, newidx);
        }
    }

    fn get_x_upper(&self) -> Option<P> {
    	self.base.get_x_upper()
    }
    fn get_y_upper(&self)  -> Option<P> {
    	self.base.get_y_upper()
    }
    fn get_z_upper(&self)  -> Option<P> {
    	self.base.get_z_upper()
    }
    
    fn get_x_lower(&self) -> Option<P> {
    	self.base.get_x_lower()
    }
    fn get_y_lower(&self)  -> Option<P>{
    	self.base.get_y_lower()
    }
    fn get_z_lower(&self)  -> Option<P>{
    	self.base.get_z_lower()
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn load(&mut self, reader: &mut Read) { 
        //TODO: Include the palette in here.
		base.load(reader);
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn save(&mut self, writer: &mut Write) {
        //TODO: Include the palette in here.
		base.save(writer);
    }
}