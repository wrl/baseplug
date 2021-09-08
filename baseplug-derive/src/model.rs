use std::str::FromStr;

use proc_macro2::*;
use syn::*;

use quote::*;

enum WrappingType {
    Smooth,
    Declick
}

impl WrappingType {
    fn for_type(ty: &Path) -> Self {
        if ty.is_ident("f32") {
            Self::Smooth
        } else {
            Self::Declick
        }
    }

    fn as_token_stream(&self) -> TokenStream {
        use WrappingType::*;

        match self {
            Smooth => quote!(::baseplug::Smooth),
            Declick => quote!(::baseplug::Declick)
        }
    }
}

#[derive(Debug)]
struct ModelBounds {
    min: f32,
    max: f32
}

impl Default for ModelBounds {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 1.0,
        }
    }
}

struct ParameterInfo {
    name: String,
    short_name: Option<String>,
    label: Option<String>,
    unit: Option<String>,
    gradient: Option<String>,
    dsp_notify: Option<String>
}

struct FieldInfo<'a> {
    vis: &'a Visibility,
    ident: &'a Ident,
    ty: &'a Type,

    wrapping: Option<WrappingType>,

    bounds: ModelBounds,
    smooth_ms: f32,

    parameter_info: Option<ParameterInfo>
}

impl<'a> FieldInfo<'a> {
    fn from_field(f: &'a Field) -> Self {
        // FIXME: pub?
        let vis = &f.vis;
        let ident = f.ident.as_ref().unwrap();
        let ty = &f.ty;

        let mut info = FieldInfo {
            vis,
            ident,
            ty,

            wrapping: match &f.ty {
                Type::Path(ref p) => Some(WrappingType::for_type(&p.path)),
                _ => None
            },

            bounds: ModelBounds::default(),
            smooth_ms: 5.0f32,

            parameter_info: None
        };

        for attr in f.attrs.iter() {
            let meta = attr.parse_meta();

            let (ident, nested) = match meta {
                Ok(Meta::List(ref list)) => {
                    (list.path.get_ident().unwrap(), &list.nested)
                },

                Ok(Meta::Path(ref path)) => {
                    if path.is_ident("unsmoothed") {
                        info.wrapping = None;
                    }

                    continue
                },

                _ => continue,
            };

            match &*ident.to_string() {
                "model" => info.populate_model_attrs(nested),
                "parameter" => info.populate_parameter_attrs(nested),
                ident => panic!("unexpected attribute {}", ident)
            }
        }

        info
    }

    fn populate_parameter_attrs(&mut self,
        nested: &syn::punctuated::Punctuated<syn::NestedMeta, syn::token::Comma>) {
        if self.parameter_info.is_some() {
            panic!("duplicate parameter info for model field");
        }

        let mut name = None;
        let mut short_name = None;
        let mut label = None;
        let mut unit = None;
        let mut gradient = None;
        let mut dsp_notify = None;

        nested.iter()
            .filter_map(|attr| {
                match attr {
                    NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. })) => {
                        let lit = match lit {
                            Lit::Str(s) => s.value(),
                            _ => return None
                        };

                        path.get_ident()
                            .map(|ident| (ident, lit))
                    },

                    _ => None
                }
            })
        .for_each(|(ident, lit)| {
            match (&*ident.to_string(), lit) {
                ("name", s) => name = Some(s),
                ("short_name", s) => short_name = Some(s),
                ("label", s) => label = Some(s),
                ("unit", s) => unit = Some(s),
                ("gradient", s) => gradient = Some(s),
                ("dsp_notify", s) => dsp_notify = Some(s),

                (ident, _) => panic!("unexpected attribute \"{}\"", ident)
            }
        });

        let name = name.expect("\"name\" is a required parameter field");

        self.parameter_info = Some(ParameterInfo {
            name,
            short_name,
            label,
            unit,
            gradient,
            dsp_notify
        });
    }

    fn populate_model_attrs(&mut self,
        nested: &syn::punctuated::Punctuated<syn::NestedMeta, syn::token::Comma>) {
        nested.iter()
            .filter_map(|attr| {
                match attr {
                    NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. })) =>
                        path.get_ident()
                        .map(|ident| (ident, lit)),
                    _ => None
                }
            })
        .for_each(|(ident, lit)| {
            match (&*ident.to_string(), lit) {
                ("min", Lit::Float(f)) => self.bounds.min = f.base10_parse().unwrap(),
                ("max", Lit::Float(f)) => self.bounds.max = f.base10_parse().unwrap(),
                ("smooth_ms", Lit::Float(f)) => self.smooth_ms = f.base10_parse().unwrap(),
                _ => ()
            }
        });
    }

    fn parameter_repr(&self, model: &Ident) -> Option<TokenStream> {
        let param = match self.parameter_info {
            Some(ref p) => p,
            None => return None
        };

        let pty = quote!(::baseplug::Param<P, #model>);

        let ident = &self.ident;
        let name = &param.name;
        let short_name = param.short_name.as_ref()
            .map_or_else(|| quote!(None), |sn| quote!(Some(#sn)));
        let label = param.label.as_ref()
            .map_or_else(|| quote!(""), |l| quote!(#l));

        let dsp_notify = param.dsp_notify.as_ref()
            .map_or_else(|| quote!(None), |dn| {
                let dn = TokenStream::from_str(dn).unwrap();
                quote!(Some(#dn))
            });

        let unit = param.unit.as_ref()
            .map_or_else(
                || quote!(Generic),
                |u| TokenStream::from_str(u).unwrap());

        let param_type = {
            let min = self.bounds.min;
            let max = self.bounds.max;

            let gradient = param.gradient.as_ref()
                .map_or_else(
                    || quote!(Linear),
                    |l| TokenStream::from_str(l).unwrap());

            quote!(
                ::baseplug::parameter::Type::Numeric {
                    min: #min,
                    max: #max,

                    gradient: ::baseplug::parameter::Gradient::#gradient
                }
            )
        };

        let model_get = match self.wrapping {
            None => quote!(model.#ident),
            _ => quote!(model.#ident.dest())
        };

        let display_cb = match param.unit.as_ref().map(|x| x.as_str()) {
            Some("Decibels") => quote!(
                |param: &#pty, model: &#model, w: &mut ::std::io::Write| ->
                        ::std::io::Result<()> {
                    let val = #model_get;

                    if val <= 0.00003162278 {
                        write!(w, "-inf")
                    } else {
                        write!(w, "{:.1}", ::baseplug::util::coeff_to_db(val))
                    }
                }
            ),

            _ => quote!(
                |param: &#pty, model: &#model, w: &mut ::std::io::Write| ->
                        ::std::io::Result<()> {
                    write!(w, "{}", #model_get)
                }
            ),
        };

        let set_cb = match self.wrapping {
            None => quote!(
                |param: &#pty, model: &mut #model, val: f32| {
                    model.#ident = val.xlate_from(param);
                }
            ),

            _ => quote!(
                |param: &#pty, model: &mut #model, val: f32| {
                    model.#ident.set(val.xlate_from(param))
                }
            )
        };

        let get_cb = quote!(
            |param: &#pty, model: &#model| -> f32 {
                #model_get.xlate_out(param)
            }
        );

        Some(quote!(
            ::baseplug::Param {
                name: #name,
                short_name: #short_name,

                unit: ::baseplug::parameter::Unit::#unit,

                param_type: #param_type,
                format: ::baseplug::parameter::Format {
                    display_cb: #display_cb,
                    label: #label
                },

                dsp_notify: #dsp_notify,

                set_cb: #set_cb,
                get_cb: #get_cb
            }
        ))
    }
}

pub(crate) fn derive(input: DeriveInput) -> TokenStream {
    match &input.data {
        syn::Data::Struct(_) => {
            struct_derive(input)
        },
        syn::Data::Enum(_) => {
            enum_derive(input)
        },
        _ => panic!("derive")
    }    
}

fn struct_derive(input: DeriveInput) -> TokenStream {
    let attrs = &input.attrs;
    let model_vis = &input.vis;
    let model_name = &input.ident;

    let fields = match input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(ref n), ..
        }) => &n.named,

        _ => panic!()
    };

    let fields_base: Vec<_> = fields.iter()
        .map(FieldInfo::from_field)
        .collect();

    let model_fields = fields_base.iter()
        .map(|FieldInfo { vis, ident, ty, .. }| {
            quote!(#vis #ident: #ty)
        });

    let smoothed_fields = fields_base.iter()
        .map(|FieldInfo { vis, ident, wrapping, ty, .. }| {
            match wrapping {
                Some(wrap_type) => {
                    let smoothed_type = wrap_type.as_token_stream();
                    quote!(#vis #ident: #smoothed_type<#ty>)
                },

                None => quote!(#vis #ident: #ty)
            }
        });

    let proc_fields = fields_base.iter()
        .map(|FieldInfo { vis, ident, wrapping, ty, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) =>
                    quote!(#vis #ident:
                        ::baseplug::SmoothOutput<'proc, #ty>),

                Some(WrappingType::Declick) =>
                    quote!(#vis #ident:
                        ::baseplug::DeclickOutput<'proc, #ty>),

                None => quote!(#vis #ident: &'proc #ty)
            }
        });

    let get_process_fields = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) =>
                    quote!(#ident: {
                        let out = self.#ident.output();

                        ::baseplug::SmoothOutput {
                            values: &out.values[..nframes],
                            status: out.status
                        }
                    }),

                Some(WrappingType::Declick) =>
                    quote!(#ident: {
                        let out = self.#ident.output();

                        ::baseplug::DeclickOutput {
                            from: out.from,
                            to: out.to,
                            fade: &out.fade[..nframes],
                            status: out.status
                        }
                    }),

                None => quote!(#ident: &self.#ident)
            }
        });

    let current_value_fields = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) =>
                    quote!(#ident: {
                        let out = self.#ident.current_value();

                        ::baseplug::SmoothOutput {
                            values: out.values,
                            status: out.status
                        }
                    }),

                Some(WrappingType::Declick) =>
                    quote!(#ident: {
                        let out = self.#ident.current_value();

                        ::baseplug::DeclickOutput {
                            from: out.from,
                            to: out.to,
                            fade: out.fade,
                            status: out.status
                        }
                    }),

                None => quote!(#ident: &self.#ident)
            }
        });

    let set_statements = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) =>
                    quote!(self.#ident.set(from.#ident)),
                Some(WrappingType::Declick) =>
                    quote!(self.#ident.set(from.#ident.clone())),
                None => quote!(self.#ident = from.#ident)
            }
        });

    let from_model_fields = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) =>
                    quote!(#ident: ::baseplug::Smooth::new(model.#ident)),
                Some(WrappingType::Declick) =>
                    quote!(#ident: ::baseplug::Declick::new(model.#ident)),
                None => quote!(#ident: model.#ident)
            }
        });

    let reset_statements = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) =>
                    quote!(self.#ident.reset(from.#ident)),
                Some(WrappingType::Declick) =>
                    quote!(self.#ident.reset(from.#ident.clone())),
                None => quote!(self.#ident = from.#ident)
            }
        });

    let process_statements = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            wrapping.as_ref().map(|_|
                quote!(self.#ident.process(nframes)))
        });

    let set_sample_rate_statements = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, smooth_ms, .. }| {
            wrapping.as_ref().map(|_|
                quote!(self.#ident.set_speed_ms(sample_rate, #smooth_ms)))
        });

    let as_model_fields = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) => quote!(#ident: self.#ident.dest()),
                Some(WrappingType::Declick) =>
                    quote!(#ident: self.#ident.dest().clone()),
                None => quote!(#ident: self.#ident)
            }
        });

    let smoothed_ident = format_ident!("{}Smooth", model_name);
    let proc_ident = format_ident!("{}Process", model_name);

    let impl_params = format_ident!("_IMPL_PARAMETERS_FOR_{}", model_name);

    let parameters = fields_base.iter()
        .filter_map(|field: &FieldInfo|
            field.parameter_repr(&smoothed_ident));

    quote!(
        #( #attrs )*
        #model_vis struct #model_name {
            #( #model_fields ),*
        }

        #[doc(hidden)]
        #model_vis struct #smoothed_ident {
            #( #smoothed_fields ),*
        }

        #model_vis struct #proc_ident<'proc> {
            #( #proc_fields ),*
        }

        #[doc(hidden)]
        impl<P: ::baseplug::Plugin> ::baseplug::Model<P> for #model_name {
            type Smooth = #smoothed_ident;
        }

        #[doc(hidden)]
        impl<P: ::baseplug::Plugin> ::baseplug::SmoothModel<P, #model_name> for #smoothed_ident {
            type Process<'proc> = #proc_ident<'proc>;

            fn from_model(model: #model_name) -> Self {
                Self {
                    #( #from_model_fields ),*
                }
            }

            fn as_model(&self) -> #model_name {
                #model_name {
                    #( #as_model_fields ),*
                }
            }

            fn set(&mut self, from: &#model_name) {
                #( #set_statements ;)*
            }

            fn reset(&mut self, from: &#model_name) {
                #( #reset_statements ;)*
            }

            fn set_sample_rate(&mut self, sample_rate: f32) {
                #( #set_sample_rate_statements ;)*
            }

            fn current_value<'proc>(&'proc mut self) -> Self::Process<'proc> {
                #proc_ident {
                    #( #current_value_fields ),*
                }
            }

            fn process<'proc>(&'proc mut self, nframes: usize) -> Self::Process<'proc> {
                #( #process_statements ;)*

                #proc_ident {
                    #( #get_process_fields ),*
                }
            }
        }

        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const #impl_params: () = {
            use ::baseplug::parameter::{
                Translatable,
                TranslateFrom
            };

            impl<P: ::baseplug::Plugin> ::baseplug::Parameters<P, #smoothed_ident> for #smoothed_ident {
                const PARAMS: &'static [&'static ::baseplug::Param<P, #smoothed_ident>] = &[
                    #( & #parameters ),*
                ];
            }
        };
    )
}

fn enum_derive(input: DeriveInput) -> TokenStream {
    let attrs = &input.attrs;
    let model_vis = &input.vis;
    let model_name = &input.ident;
    let data = &input.data;

    let variant_names = match data {
        Data::Enum(data_enum) => {
            data_enum.variants.iter().map(|v| &v.ident)
        },

        _ => panic!()
    };

    let variant_count = match data {
        Data::Enum(data_enum) => {
            data_enum.variants.iter().count()
        },

        _ => panic!()
    };

    let variant_names_display = variant_names.clone();
    let variant_names_string = variant_names.clone().map(|x| x.to_string());

    let variant_names_from_f32 = variant_names.clone();
    let mut variant_index_from_f32 = Vec::new();
    for i in 1..variant_count + 1 {
        variant_index_from_f32.push(i as f32);
    }

    let variant_names_from_model = variant_names.clone();
    let mut variant_index_from_model = Vec::new();
    for i in 1..variant_count + 1 {
        variant_index_from_model.push(i as f32);
    }

    quote!(
        #( #attrs )*
        #model_vis enum #model_name {
            #( #variant_names ),*
        }

        #[doc(hidden)]
        impl std::fmt::Display for #model_name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                match *self {
                    #(#model_name::#variant_names_display => write!(f, #variant_names_string)),*
                }
            }
        }

        #[doc(hidden)]
        impl baseplug::parameter::EnumModel for #model_name {
        }

        #[doc(hidden)]
        impl From<f32> for #model_name {
            fn from(value: f32) -> Self {
                let value = value.min(1.0).max(0.0);
                match value {
                    #(n if n <= #variant_index_from_f32 / #variant_count as f32 => #model_name::#variant_names_from_f32,)*
                    _ => unreachable!(),
                }
            }
        }

        #[doc(hidden)]
        impl From<#model_name> for f32 {
            fn from(value: #model_name) -> Self {
                match value {
                    #(#model_name::#variant_names_from_model => #variant_index_from_model / #variant_count as f32,)*
                }
            }
        }  
    )   
}