use std::ops::{Add, Div, Mul, Neg, Sub};

pub trait Num: 
Copy
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
{
    fn zero() -> Self;
    fn one() -> Self;
    fn from_f32(x: f32) -> Self;
}

macro_rules! impl_real {
    ( $($t:ty),* ) => {
    $( impl Num for $t {
        #[inline] fn zero() -> Self { 0.0 }
        #[inline] fn one() -> Self { 1.0 }
        #[inline] fn from_f32(x: f32) -> Self { x as Self }
    }) *
    }
}
impl_real! { f32, f64 }

macro_rules! impl_integer {
    ( $($t:ty),* ) => {
    $( impl Num for $t {
        #[inline] fn zero() -> Self { 0 }
        #[inline] fn one() -> Self { 1 }
        #[inline] fn from_f32(x: f32) -> Self { x as Self }
    }) *
    }
}
impl_integer! { i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize }

pub trait Real:
    Num
    + Neg<Output = Self>
    + PartialOrd
{
    fn abs(self) -> Self;
    fn exp(self) -> Self;
    fn log2(self) -> Self;
}

macro_rules! impl_real {
    ( $($t:ty),* ) => {
    $( impl Real for $t {
        #[inline] fn abs(self) -> Self { self.abs() }
        #[inline] fn exp(self) -> Self { self.exp() }
        #[inline] fn log2(self) -> Self { self.log2() }
    }) *
    }
}
impl_real! { f32, f64 }

pub trait AsPrimitive<T: Copy>: Copy {
    fn as_(self) -> T;
}

macro_rules! impl_as_primitive {
    (@ $T: ty => $(#[$cfg:meta])* impl $U: ty ) => {
        $(#[$cfg])*
        impl AsPrimitive<$U> for $T {
            #[inline] fn as_(self) -> $U { self as $U }
        }
    };
    (@ $T: ty => { $( $U: ty ),* } ) => {$(
        impl_as_primitive!(@ $T => impl $U);
    )*};
    ($T: ty => { $( $U: ty ),* } ) => {
        impl_as_primitive!(@ $T => { $( $U ),* });
        impl_as_primitive!(@ $T => { u8, u16, u32, u64, u128, usize });
        impl_as_primitive!(@ $T => { i8, i16, i32, i64, i128, isize });
    };
}

impl_as_primitive!(f32 => { f32, f64 });
impl_as_primitive!(f64 => { f32, f64 });

pub trait Discrete:
    Num
    + Eq
{

}

macro_rules! impl_signed_num {
    ( $($t:ty),* ) => {
    $( impl Discrete for $t {

    }) *
    }
}
impl_signed_num! { i8, i16, i32, i64, i128, isize }

macro_rules! impl_unsigned_num {
    ( $($t:ty),* ) => {
    $( impl Discrete for $t {

    }) *
    }
}
impl_unsigned_num! { u8, u16, u32, u64, u128, usize }