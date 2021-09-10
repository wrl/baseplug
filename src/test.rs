#[cfg(test)]
mod tests {
    
    use crate::{
        parameter::EnumModel,
    };

    #[derive(Debug, PartialEq, Eq, Clone)]
    enum ModelEnum {
        A,
        B,
        C,
    }

    impl EnumModel for ModelEnum {

    }

    impl From<f32> for ModelEnum {
        fn from(value: f32) -> Self {
            let value = value.min(1.0).max(0.0);
            match value {
                n if n <= 1.0 / 3.0 => ModelEnum::A,
                n if n <= 2.0 / 3.0 => ModelEnum::B,
                n if n <= 3.0 / 3.0 => ModelEnum::C,
                _ => ModelEnum::C
            }
        }
    }

    impl From<ModelEnum> for f32 {
        fn from(value: ModelEnum) -> Self {
            match value {
                ModelEnum::A => 0.0 / 3.0,
                ModelEnum::B => 1.0 / 3.0,
                ModelEnum::C => 2.0 / 3.0,
            }
        }
    }

    #[test]
    fn from_f32_for_model_enum() {
        assert_eq!(ModelEnum::from(-1.0), ModelEnum::A);
        assert_eq!(ModelEnum::from(0.0), ModelEnum::A);
        assert_eq!(ModelEnum::from(0.5), ModelEnum::B);
        assert_eq!(ModelEnum::from(1.0), ModelEnum::C);
        assert_eq!(ModelEnum::from(2.0), ModelEnum::C);

        assert_eq!(ModelEnum::from(-f32::INFINITY), ModelEnum::A);
        assert_eq!(ModelEnum::from(-f32::NAN), ModelEnum::C);
        assert_eq!(ModelEnum::from(f32::NAN), ModelEnum::C);
        assert_eq!(ModelEnum::from(f32::INFINITY), ModelEnum::C);
    }

    #[test]
    fn from_model_enum_for_f32() {
        let value: f32 = ModelEnum::A.into();
        assert_eq!(value, 0.0);
        let value: f32 = ModelEnum::B.into();
        assert_eq!(value, 1.0 / 3.0);
        let value: f32 = ModelEnum::C.into();
        assert_eq!(value, 2.0 / 3.0);
    }
}
