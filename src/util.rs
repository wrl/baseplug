macro_rules! offset_of {
    ($container:ty, $field:ident) => (
        &(*(::std::ptr::null_mut::<$container>())).$field as *const _ as usize
    )
}

macro_rules! container_of {
    ($ptr:ident, $container:ty, $field:ident) => ({
        (($ptr as usize) - offset_of!($container, $field)) as *mut $container
    })
}

#[inline]
pub fn db_to_coeff(db: f32) -> f32 {
    if db < -90.0 {
        0.0
    } else {
        10.0f32.powf(0.05 * db)
    }
}

#[inline]
pub fn coeff_to_db(coeff: f32) -> f32 {
    if coeff <= 0.00003162277 {
        -90.0
    } else {
        20.0 * coeff.log(10.0)
    }
}
