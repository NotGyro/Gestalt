const FRAGMENT_SIZE : u64 = 512u64;

// Thinking out loud, 512 bytes seems like a good size for a fragment? 
#[repr(C)]
pub struct BulkTransferHeader {
    /// Which specific transfer operation (per-session) does this refer to?
    pub transfer_id: u16,
    /// Which element of this transfer are we reading?
    pub file_id: u16,
    /// Which specific span of the file does this contain?
    pub fragment: u32,
}
impl BulkTransferHeader {
    pub const fn packed_size() -> usize { 
        8 // 2 bytes transfer id, 2 bytes file id, 4 bytes fragment id. 
    }

    pub fn read_from_bytes(slice: &[u8]) -> Self {
        let transfer_id: u16 = u16::from_le_bytes([slice[0], slice[1]]);
        let file_id: u16 = u16::from_le_bytes([slice[2], slice[3]]);
        let fragment = u32::from_le_bytes([slice[4], slice[5], slice[6], slice[7]]);

        BulkTransferHeader {
            transfer_id,
            file_id,
            fragment,
        }
    }

    pub fn write_to_bytes(&self, buf: &mut [u8]) { 
        let transfer_id_bytes = self.transfer_id.to_le_bytes();
        let tr_sub_buf = &mut buf[..2];
        tr_sub_buf.copy_from_slice(&transfer_id_bytes);
        
        let fi_sub_buf = &mut buf[2..4];
        let file_id_bytes = self.file_id.to_le_bytes();
        fi_sub_buf.copy_from_slice(&file_id_bytes);
        
        let frag_sub_buf = &mut buf[4..8];
        let frag_bytes = self.fragment.to_le_bytes();
        frag_sub_buf.copy_from_slice(&frag_bytes);
    }
}