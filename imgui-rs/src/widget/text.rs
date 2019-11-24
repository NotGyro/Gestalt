use std::os::raw::c_char;

use crate::string::ImStr;
use crate::style::StyleColor;
use crate::Ui;

static FMT: &'static [u8] = b"%s\0";

#[inline]
fn fmt_ptr() -> *const c_char {
    FMT.as_ptr() as *const c_char
}

/// # Widgets: Text
impl<'ui> Ui<'ui> {
    /// Renders simple text
    pub fn text<T: AsRef<str>>(&self, text: T) {
        let s = text.as_ref();
        unsafe {
            let start = s.as_ptr();
            let end = start.add(s.len());
            sys::igTextUnformatted(start as *const c_char, end as *const c_char);
        }
    }
    /// Renders simple text using the given text color
    pub fn text_colored<T: AsRef<str>>(&self, color: [f32; 4], text: T) {
        let style = self.push_style_color(StyleColor::Text, color);
        self.text(text);
        style.pop(self);
    }
    /// Renders simple text using `StyleColor::TextDisabled` color
    pub fn text_disabled<T: AsRef<str>>(&self, text: T) {
        let color = self.style_color(StyleColor::TextDisabled);
        let style = self.push_style_color(StyleColor::Text, color);
        self.text(text);
        style.pop(self);
    }
    /// Renders text wrapped to the end of window (or column)
    pub fn text_wrapped(&self, text: &ImStr) {
        unsafe { sys::igTextWrapped(fmt_ptr(), text.as_ptr()) }
    }
    /// Render a text + label combination aligned the same way as value+label widgets
    pub fn label_text(&self, label: &ImStr, text: &ImStr) {
        unsafe { sys::igLabelText(label.as_ptr(), fmt_ptr(), text.as_ptr()) }
    }
    /// Renders text with a little bullet aligned to the typical tree node
    pub fn bullet_text(&self, text: &ImStr) {
        unsafe { sys::igBulletText(fmt_ptr(), text.as_ptr()) }
    }
}
