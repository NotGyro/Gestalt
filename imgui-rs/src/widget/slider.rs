use std::ops::RangeInclusive;
use std::os::raw::c_void;
use std::ptr;

use crate::internal::DataTypeKind;
use crate::string::ImStr;
use crate::sys;
use crate::Ui;

/// Builder for a slider widget.
#[derive(Copy, Clone, Debug)]
#[must_use]
pub struct Slider<'a, T: DataTypeKind> {
    label: &'a ImStr,
    min: T,
    max: T,
    display_format: Option<&'a ImStr>,
    power: f32,
}

impl<'a, T: DataTypeKind> Slider<'a, T> {
    /// Constructs a new slider builder with the given range.
    pub fn new(label: &ImStr, range: RangeInclusive<T>) -> Slider<T> {
        Slider {
            label,
            min: *range.start(),
            max: *range.end(),
            display_format: None,
            power: 1.0,
        }
    }
    /// Sets the display format using *a C-style printf string*
    #[inline]
    pub fn display_format(mut self, display_format: &'a ImStr) -> Self {
        self.display_format = Some(display_format);
        self
    }
    /// Sets the power (exponent) of the slider values
    #[inline]
    pub fn power(mut self, power: f32) -> Self {
        self.power = power;
        self
    }
    /// Builds a slider that is bound to the given value.
    ///
    /// Returns true if the slider value was changed.
    pub fn build(self, _: &Ui, value: &mut T) -> bool {
        unsafe {
            sys::igSliderScalar(
                self.label.as_ptr(),
                T::KIND as i32,
                value as *mut T as *mut c_void,
                &self.min as *const T as *const c_void,
                &self.max as *const T as *const c_void,
                self.display_format
                    .map(ImStr::as_ptr)
                    .unwrap_or(ptr::null()),
                self.power,
            )
        }
    }
    /// Builds a horizontal array of multiple sliders attached to the given slice.
    ///
    /// Returns true if any slider value was changed.
    pub fn build_array(self, _: &Ui, values: &mut [T]) -> bool {
        unsafe {
            sys::igSliderScalarN(
                self.label.as_ptr(),
                T::KIND as i32,
                values.as_mut_ptr() as *mut c_void,
                values.len() as i32,
                &self.min as *const T as *const c_void,
                &self.max as *const T as *const c_void,
                self.display_format
                    .map(ImStr::as_ptr)
                    .unwrap_or(ptr::null()),
                self.power,
            )
        }
    }
}

/// Builder for a vertical slider widget.
#[derive(Clone, Debug)]
#[must_use]
pub struct VerticalSlider<'a, T: DataTypeKind + Copy> {
    label: &'a ImStr,
    size: [f32; 2],
    min: T,
    max: T,
    display_format: Option<&'a ImStr>,
    power: f32,
}

impl<'a, T: DataTypeKind> VerticalSlider<'a, T> {
    /// Constructs a new vertical slider builder with the given size and range.
    pub fn new(label: &ImStr, size: [f32; 2], range: RangeInclusive<T>) -> VerticalSlider<T> {
        VerticalSlider {
            label,
            size,
            min: *range.start(),
            max: *range.end(),
            display_format: None,
            power: 1.0,
        }
    }
    /// Sets the display format using *a C-style printf string*
    #[inline]
    pub fn display_format(mut self, display_format: &'a ImStr) -> Self {
        self.display_format = Some(display_format);
        self
    }
    /// Sets the power (exponent) of the slider values
    #[inline]
    pub fn power(mut self, power: f32) -> Self {
        self.power = power;
        self
    }
    /// Builds a vertical slider that is bound to the given value.
    ///
    /// Returns true if the slider value was changed.
    pub fn build(self, _: &Ui, value: &mut T) -> bool {
        unsafe {
            sys::igVSliderScalar(
                self.label.as_ptr(),
                self.size.into(),
                T::KIND as i32,
                value as *mut T as *mut c_void,
                &self.min as *const T as *const c_void,
                &self.max as *const T as *const c_void,
                self.display_format
                    .map(ImStr::as_ptr)
                    .unwrap_or(ptr::null()),
                self.power,
            )
        }
    }
}

/// Builder for an angle slider widget.
#[derive(Copy, Clone, Debug)]
#[must_use]
pub struct AngleSlider<'a> {
    label: &'a ImStr,
    min_degrees: f32,
    max_degrees: f32,
    display_format: &'a ImStr,
}

impl<'a> AngleSlider<'a> {
    /// Constructs a new angle slider builder.
    pub fn new(label: &ImStr) -> AngleSlider {
        use crate::im_str;
        AngleSlider {
            label,
            min_degrees: -360.0,
            max_degrees: 360.0,
            display_format: im_str!("%.0f deg"),
        }
    }
    /// Sets the minimum value (in degrees)
    #[inline]
    pub fn min_degrees(mut self, min_degrees: f32) -> Self {
        self.min_degrees = min_degrees;
        self
    }
    /// Sets the maximum value (in degrees)
    #[inline]
    pub fn max_degrees(mut self, max_degrees: f32) -> Self {
        self.max_degrees = max_degrees;
        self
    }
    /// Sets the display format using *a C-style printf string*
    #[inline]
    pub fn display_format(mut self, display_format: &'a ImStr) -> Self {
        self.display_format = display_format;
        self
    }
    /// Builds an angle slider that is bound to the given value (in radians).
    ///
    /// Returns true if the slider value was changed.
    pub fn build(self, _: &Ui, value_rad: &mut f32) -> bool {
        unsafe {
            sys::igSliderAngle(
                self.label.as_ptr(),
                value_rad as *mut _,
                self.min_degrees,
                self.max_degrees,
                self.display_format.as_ptr(),
            )
        }
    }
}
