use crate::sys;
use crate::Ui;

/// # Content region
impl<'ui> Ui<'ui> {
    /// Returns the current content boundaries (in *window coordinates*)
    pub fn content_region_max(&self) -> [f32; 2] {
        unsafe { sys::igGetContentRegionMax_nonUDT2().into() }
    }
    /// Equal to `ui.content_region_max()` - `ui.cursor_pos()`
    pub fn content_region_avail(&self) -> [f32; 2] {
        unsafe { sys::igGetContentRegionAvail_nonUDT2().into() }
    }
    /// Content boundaries min (in *window coordinates*).
    ///
    /// Roughly equal to [0.0, 0.0] - scroll.
    pub fn window_content_region_min(&self) -> [f32; 2] {
        unsafe { sys::igGetWindowContentRegionMin_nonUDT2().into() }
    }
    /// Content boundaries max (in *window coordinates*).
    ///
    /// Roughly equal to [0.0, 0.0] + size - scroll.
    pub fn window_content_region_max(&self) -> [f32; 2] {
        unsafe { sys::igGetWindowContentRegionMax_nonUDT2().into() }
    }
    pub fn window_content_region_width(&self) -> f32 {
        unsafe { sys::igGetWindowContentRegionWidth() }
    }
}
