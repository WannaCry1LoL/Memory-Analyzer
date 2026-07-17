use std::mem::size_of;
use std::simd::Simd;

pub const SIMD_REGISTER_WIDTH: usize = if cfg!(target_feature = "avx512f") {
	64
} else {
	32
};

pub const SIMD_REGISTER_WIDTH_BITS: usize = SIMD_REGISTER_WIDTH * 8;

pub trait ToSimd: Copy {
	const SIZE: usize = size_of::<Self>();
	const LANES: usize = SIMD_REGISTER_WIDTH / Self::SIZE;
	type SimdType;
}

macro_rules! impl_to_simd {
    ($($t:ty),*) => {
        $(
            impl ToSimd for $t {
                type SimdType = Simd<Self, { <$t as ToSimd>::LANES }>;
            }
        )*
    };
}

impl_to_simd!(u8, i8, u16, i16, u32, i32, u64, i64, f32, f64);

pub type PrimitiveToSimd<T> = <T as ToSimd>::SimdType;
