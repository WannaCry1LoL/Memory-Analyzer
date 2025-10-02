use std::{ptr, mem};

pub trait MemoryPattern: Sized {
	fn size() -> usize;
	unsafe fn matches(&self, ptr: *const u8) -> bool;
}

macro_rules! impl_mem_pattern_int {
    ($($t:ty),*) => {
        $(
            impl MemoryPattern for $t {
                fn size() -> usize { mem::size_of::<$t>() }

                #[allow(unsafe_op_in_unsafe_fn)]
                unsafe fn matches(&self, ptr: *const u8) -> bool {
                    let val = ptr::read_unaligned(ptr as *const $t);
                    val == *self
                }
            }
        )*
    };
}

impl_mem_pattern_int!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

impl MemoryPattern for f32 {
	fn size() -> usize { 4 }
	
	#[allow(unsafe_op_in_unsafe_fn)]
	unsafe fn matches(&self, ptr: *const u8) -> bool {
		let val = ptr::read_unaligned(ptr as *const f32);
		(val - *self).abs() < f32::EPSILON
	}
}

impl MemoryPattern for f64 {
	fn size() -> usize { 8 }
	
	#[allow(unsafe_op_in_unsafe_fn)]
	unsafe fn matches(&self, ptr: *const u8) -> bool {
		let val = ptr::read_unaligned(ptr as *const f64);
		(val - *self).abs() < f64::EPSILON
	}
}