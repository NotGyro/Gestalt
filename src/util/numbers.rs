extern crate std;

pub trait USizeAble {
    fn as_usize(&self) -> usize;
    fn from_usize(val : usize) -> Self;
}

impl USizeAble for u8 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u8
    }    
}
impl USizeAble for u16 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u16
    }    
}
impl USizeAble for u32 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u32
    }    
}
impl USizeAble for u64 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u64
    }    
}
