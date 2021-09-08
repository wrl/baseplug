#[cfg(test)]
mod tests {
    
    use crate::{
        EnumModel,
        Param,
        parameter::Unit,
        parameter::Type,
        parameter::Gradient,
        parameter::Format,
        Declick,
        Plugin,
    };

    //parameters.rs tests
    #[test]
    fn reset_ok() {
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
                    n if n <= 0.0 / 3.0 => ModelEnum::A,
                    n if n <= 1.0 / 3.0 => ModelEnum::B,
                    n if n <= 2.0 / 3.0 => ModelEnum::C,
                    _ => unreachable!(),
                }
            }
        }

        impl Into<f32> for ModelEnum {
            fn into(self) -> f32 {
                match &self {
                    ModelEnum::A => 0.0 / 3.0,
                    ModelEnum::B => 1.0 / 3.0,
                    ModelEnum::C => 2.0 / 3.0,
                }
            }
        }

        struct TestModelSmooth {
            modelEnum: Declick<ModelEnum>,
        }

        /*let param = Param {
            name: "test",
            short_name: None,
            unit: Unit::Generic,
            param_type: Type::Numeric {
                min: 0.0,
                max: 0.0,
                gradient: Gradient::Linear
            },
            format: Format {
                display_cb: |param: &Param<P, TestModelSmooth>,
                            model: &TestModelSmooth,
                            w: &mut ::std::io::Write|
                    -> ::std::io::Result<()> {
                    w.write_fmt(::core::fmt::Arguments::new_v1(
                        &[""],
                        &match (&model.modelEnum.dest(),) {
                            (arg0,) => [::core::fmt::ArgumentV1::new(
                                arg0,
                                ::core::fmt::Display::fmt,
                            )],
                        },
                    ))
                },
                label: "",
            },
        
            pub dsp_notify: Option<fn(&mut P)>,
        
            pub set_cb: fn(&Param<P, Model>, &mut Model, f32),
            pub get_cb: fn(&Param<P, Model>, &Model) -> f32
        };

        let test = ModelEnum::xlate_in(, 0.0);*/
    }
}
