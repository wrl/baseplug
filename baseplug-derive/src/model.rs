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
    gradient: Option<String>
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

                (ident, _) => panic!("unexpected attribute \"{}\"", ident)
            }
        });

        let name = name.expect("\"name\" is a required parameter field");

        self.parameter_info = Some(ParameterInfo {
            name,
            short_name,
            label,
            unit,
            gradient
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

        let pty = quote!(::baseplug::Param<#model>);

        let ident = &self.ident;
        let name = &param.name;
        let short_name = param.short_name.as_ref()
            .map_or_else(|| quote!(None), |sn| quote!(Some(#sn)));
        let label = param.label.as_ref()
            .map_or_else(|| quote!(""), |l| quote!(#l));

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

        let display_cb = match param.unit.as_ref().map(|x| x.as_str()) {
            Some("Decibels") => quote!(
                |param: &#pty, model: &#model, w: &mut ::std::io::Write| ->
                        ::std::io::Result<()> {
                    let val = model.#ident.dest();

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
                    write!(w, "{}", model.#ident.dest())
                }
            ),
        };

        let set_cb = quote!(
            |param: &#pty, model: &mut #model, val: f32| {
                model.#ident.set(val.xlate_from(param))
            }
        );

        let get_cb = quote!(
            |param: &#pty, model: &#model| -> f32 {
                model.#ident.dest().xlate_out(param)
            }
        );

        Some(quote!(
            const #ident: #pty = ::baseplug::Param {
                name: #name,
                short_name: #short_name,

                unit: ::baseplug::parameter::Unit::#unit,

                param_type: #param_type,
                format: ::baseplug::parameter::Format {
                    display_cb: #display_cb,
                    label: #label,
                },

                set_cb: #set_cb,
                get_cb: #get_cb
            };
        ))
    }
}

pub(crate) fn derive(input: DeriveInput) -> TokenStream {
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

    let snapshot_fields = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) =>
                    quote!(#ident: {
                        let out = self.#ident.snapshot();

                        ::baseplug::SmoothOutput {
                            values: out.values,
                            status: out.status
                        }
                    }),

                Some(WrappingType::Declick) =>
                    quote!(#ident: {
                        let out = self.#ident.snapshot();

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

    let parameters = fields_base.iter()
        .filter_map(|field: &FieldInfo|
            field.parameter_repr(&smoothed_ident));

    let param_array_entries = fields_base.iter()
        .filter_map(|field : &FieldInfo| {
            field.parameter_info.as_ref()
                .map(|_| field.ident)
        });

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
        impl ::baseplug::Model for #model_name {
            type Smooth = #smoothed_ident;
        }

        #[doc(hidden)]
        impl ::baseplug::SmoothModel<#model_name> for #smoothed_ident {
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

            fn snapshot<'proc>(&'proc mut self) -> Self::Process<'proc> {
                #proc_ident {
                    #( #snapshot_fields ),*
                }
            }

            fn process<'proc>(&'proc mut self, nframes: usize) -> Self::Process<'proc> {
                #( #process_statements ;)*

                #proc_ident {
                    #( #get_process_fields ),*
                }
            }
        }

        #[allow(non_upper_case_globals)]
        #model_vis mod params {
            use super::#smoothed_ident;

            use ::baseplug::parameter::{
                Translatable,
                TranslateFrom
            };

            #( pub(crate) #parameters )*
        }

        impl ::baseplug::Parameters<#smoothed_ident> for #smoothed_ident {
            const PARAMS: &'static [&'static ::baseplug::Param<#smoothed_ident>] = &[
                #( &params::#param_array_entries ),*
            ];
        }
    )
}
