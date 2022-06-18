/// Basically a simple Vec<u8> with std::io::Write implemented over it - but in this case, it grows rather than returning an error if it hits the currently-allocated capacity of the buffer.
pub struct GrowableBuf { 
    inner: Vec<u8>,
    maximum: usize,
}
impl GrowableBuf { 
    pub fn new(underlying_buffer: Vec<u8>, maximum: usize) -> Self { 
        Self { 
            inner: underlying_buffer,
            maximum
        }
    }
    pub fn into_inner(self) -> Vec<u8> { 
        self.inner
    }
}

impl std::io::Write for GrowableBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let current_len = self.inner.len(); 
        let buf_len = buf.len();
        if current_len + buf_len > self.maximum { 
            Err( std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Ran out of space in a growable buffer - max size is {} and we tried to add {} bytes to a buffer which contains {}", self.maximum, buf_len, current_len)))
        }
        else {
            self.inner.extend_from_slice(buf);
            Ok(buf_len)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // This writes to the underlying structure instantly. No caching. 
        Ok(())
    }
}