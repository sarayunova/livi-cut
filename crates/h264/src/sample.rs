//! Sample storage: 8-bit pictures keep `u8` samples, high bit depth (9 and 10 bits) `u16`. The
//! reconstruction code is generic over [`Sample`] and monomorphised, so the 8-bit path pays
//! nothing for the high bit depth one.

use crate::Plane;
use crate::picture::{AnyFrame, FrameRef};

/// A stored sample type.
pub trait Sample: Copy + Default + PartialEq + Send + Sync + std::fmt::Debug + Into<i32> + 'static {
    /// A sample from a value already clipped to the bit depth.
    fn from_i32(v: i32) -> Self;
    fn typed(f: &AnyFrame) -> Option<&FrameRef<Self>>;
    fn plane(v: Vec<Self>) -> Plane;
    /// An empty output plane with room for `n` samples.
    fn alloc(n: usize) -> Vec<Self>;
}

impl Sample for u8 {
    #[inline(always)]
    fn from_i32(v: i32) -> Self {
        v as u8
    }
    fn typed(f: &AnyFrame) -> Option<&FrameRef<Self>> {
        match f {
            AnyFrame::B8(f) => Some(f),
            AnyFrame::B16(_) => None,
        }
    }
    fn plane(v: Vec<Self>) -> Plane {
        Plane::U8(v)
    }
    fn alloc(n: usize) -> Vec<Self> {
        crate::picture::plane_allocator()(n)
    }
}

impl Sample for u16 {
    #[inline(always)]
    fn from_i32(v: i32) -> Self {
        v as u16
    }
    fn typed(f: &AnyFrame) -> Option<&FrameRef<Self>> {
        match f {
            AnyFrame::B16(f) => Some(f),
            AnyFrame::B8(_) => None,
        }
    }
    fn plane(v: Vec<Self>) -> Plane {
        Plane::U16(v)
    }
    fn alloc(n: usize) -> Vec<Self> {
        Vec::with_capacity(n)
    }
}

/// Clip1Y / Clip1C: `v` limited to 0..=max, max = (1 << BitDepth) - 1.
#[inline(always)]
pub fn clip1<S: Sample>(v: i32, max: i32) -> S {
    S::from_i32(v.clamp(0, max))
}
