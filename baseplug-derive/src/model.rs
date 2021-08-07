use std::str::FromStr;

use proc_macro2::*;
use syn::*;

use quote::*;

enum WrappingType {
    Smooth,
    Declick,
    Unsmoothed,
}

impl WrappingType {
    fn for_type(ty: &Path) -> Self {
        if ty.is_ident("f32") {
            Self::Smooth
        } else {
            Self::Declick
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
    dsp_notify: Option<String>,
}

struct FieldInfo<'a> {
    vis: &'a Visibility,
    ident: &'a Ident,
    ty: &'a Type,

    wrapping: Option<WrappingType>,

    bounds: ModelBounds,
    smooth_ms: f32,

    parameter_info: Option<ParameterInfo>,
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

            parameter_info: None,
        };

        for attr in f.attrs.iter() {
            let meta = attr.parse_meta();

            let (ident, nested) = match meta {
                Ok(Meta::List(ref list)) => {
                    (list.path.get_ident().unwrap(), &list.nested)
                },

                Ok(Meta::Path(ref path)) => {
                    if path.is_ident("unsmoothed") {
                        info.wrapping = if let Some(WrappingType::Smooth) = info.wrapping {
                            Some(WrappingType::Unsmoothed)
                        } else {
                            None
                        };
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
            dsp_notify,
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

    fn parameter_repr(&self, model: &Ident, idx: usize) -> Option<TokenStream> {
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
            .map_or_else(|| {
                match param.unit.as_ref().map(|x| x.as_str()) {
                    Some("Decibels") => quote!("dB"),
                    _ => quote!("")
                }
            },
            |l| quote!(#l)
        );

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
            _ => quote!(model.#ident.dsp_value())
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
                info: ::baseplug::ParamInfo {
                    name: #name,
                    short_name: #short_name,
                    label: #label,

                    unit: ::baseplug::parameter::Unit::#unit,
                    param_type: #param_type,

                    idx: #idx,
                },

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
        .map(|FieldInfo { vis, ident, wrapping, ty, parameter_info, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) => {
                    if parameter_info.is_some() {
                        quote!(#vis #ident: ::baseplug::SmoothFloatParam)
                    } else {
                        quote!(#vis #ident: ::baseplug::SmoothFloatEntry)
                    }
                }
                Some(WrappingType::Declick) => {
                    quote!(#vis #ident: ::baseplug::DeclickParam)
                }
                Some(WrappingType::Unsmoothed) => {
                    if parameter_info.is_some() {
                        quote!(#vis #ident: ::baseplug::UnsmoothedFloatParam)
                    } else {
                        quote!(#vis #ident: ::baseplug::UnsmoothedFloatEntry)
                    }
                }
                None => quote!(#vis #ident: #ty)
            }
        });

    let ui_fields = fields_base.iter()
        .map(|FieldInfo { vis, ident, wrapping, ty, parameter_info, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) | Some(WrappingType::Unsmoothed) => {
                    if parameter_info.is_some() {
                        quote!(#vis #ident: ::baseplug::UIFloatParam)
                    } else {
                        quote!(#vis #ident: ::baseplug::UIFloatEntry)
                    }
                }
                Some(WrappingType::Declick) => {
                    quote!(#vis #ident: #ty)
                }
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

                _ => quote!(#vis #ident: &'proc #ty)
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
                
                Some(WrappingType::Unsmoothed) => {
                    quote!(#ident: self.#ident.dsp_value())
                }

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
                
                Some(WrappingType::Unsmoothed) => {
                    quote!(#ident: self.#ident.dsp_value())
                }

                None => quote!(#ident: &self.#ident)
            }
        });

    let set_statements = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) | Some(WrappingType::Unsmoothed) =>
                    quote!(self.#ident.set(from.#ident)),
                Some(WrappingType::Declick) =>
                    quote!(self.#ident.set(from.#ident.clone())),
                None => quote!(self.#ident = from.#ident)
            }
        });

    let from_model_fields = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, parameter_info, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) => {
                    if parameter_info.is_some() {
                        quote!(#ident: ::baseplug::SmoothFloatParam::new(model.#ident, &(params.next().unwrap().info)))
                    } else {
                        quote!(#ident: ::baseplug::SmoothFloatEntry::new(model.#ident))
                    }
                }
                Some(WrappingType::Declick) => {
                    quote!(#ident: ::baseplug::DeclickParam::new(model.#ident))
                }
                Some(WrappingType::Unsmoothed) => {
                    if parameter_info.is_some() {
                        quote!(#ident: ::baseplug::UnsmoothedFloatParam::new(model.#ident, &(params.next().unwrap().info)))
                    } else {
                        quote!(#ident: ::baseplug::UnsmoothedFloatEntry::new(model.#ident))
                    }
                }
                None => quote!(#ident: model.#ident)
            }
        });

    let reset_statements = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) | Some(WrappingType::Unsmoothed) =>
                    quote!(self.#ident.reset(from.#ident)),
                Some(WrappingType::Declick) =>
                    quote!(self.#ident.reset(from.#ident.clone())),
                None => quote!(self.#ident = from.#ident)
            }
        });

    let process_statements = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, parameter_info, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) | Some(WrappingType::Unsmoothed) => {
                    if parameter_info.is_some() {
                        Some(quote!(self.#ident.process(nframes, plug)))
                    } else {
                        Some(quote!(self.#ident.process(nframes)))
                    }
                }
                Some(WrappingType::Declick) => {
                    Some(quote!(self.#ident.process(nframes)))
                }
                None => None
            }
        });

    let set_sample_rate_statements = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, smooth_ms,.. }| {
            match wrapping {
                Some(WrappingType::Smooth) | Some(WrappingType::Declick) => {
                    Some(quote!(self.#ident.set_speed_ms(sample_rate, #smooth_ms)))
                }
                _ => None,
            }
        });
    
    let ui_update_statements = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) | Some(WrappingType::Unsmoothed) => {
                    Some(quote!(self.#ident._poll_update()))
                }
                _ => None,
            }
        });

    let as_model_fields = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) | Some(WrappingType::Unsmoothed) => quote!(#ident: self.#ident.dsp_value()),
                Some(WrappingType::Declick) =>
                    quote!(#ident: self.#ident.dest().clone()),
                None => quote!(#ident: self.#ident)
            }
        });

    let as_model_fields_ui = fields_base.iter()
        .map(|FieldInfo { ident, wrapping, parameter_info, .. }| {
            match wrapping {
                Some(WrappingType::Smooth) | Some(WrappingType::Unsmoothed) => {
                    if parameter_info.is_some() {
                        quote!(#ident: self.#ident.get_ui_param(std::sync::Arc::clone(&ui_host_callback)))
                    } else {
                        quote!(#ident: self.#ident.get_ui_entry())
                    }
                }
                Some(WrappingType::Declick) => {
                    quote!(#ident: self.#ident)
                }
                None => quote!(#ident: self.#ident)
            }
        });

    let smoothed_ident = format_ident!("{}Smooth", model_name);
    let proc_ident = format_ident!("{}Process", model_name);
    let ui_ident = format_ident!("{}UI", model_name);

    let impl_params = format_ident!("_IMPL_PARAMETERS_FOR_{}", model_name);
    
    let mut idx: usize = 0;
    let parameters = fields_base.iter()
        .filter_map(|field: &FieldInfo| {
            let res = field.parameter_repr(&smoothed_ident, idx);
            if res.is_some() {
                idx += 1;
            }
            res
        }
    );

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
        #model_vis struct #ui_ident {
            #( #ui_fields ),*,
            first_frame: bool,
        }

        #[doc(hidden)]
        impl<P: ::baseplug::Plugin> ::baseplug::Model<P> for #model_name {
            type Smooth = #smoothed_ident;
            type UI = #ui_ident;
        }

        #[doc(hidden)]
        impl<P: ::baseplug::Plugin> ::baseplug::SmoothModel<P, #model_name> for #smoothed_ident {
            type Process<'proc> = #proc_ident<'proc>;

            fn from_model(model: #model_name) -> Self {
                let mut params = <<#model_name as ::baseplug::Model<P>>::Smooth as ::baseplug::Parameters<P, #smoothed_ident>>::PARAMS.iter();

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

            fn process<'proc>(&'proc mut self, nframes: usize, plug: &mut P) -> Self::Process<'proc> {
                #( #process_statements ;)*

                #proc_ident {
                    #( #get_process_fields ),*
                }
            }

            fn as_ui_model(&self, ui_host_callback: std::sync::Arc<dyn ::baseplug::UIHostCallback>) -> #ui_ident {
                #ui_ident {
                    #( #as_model_fields_ui ),*,
                    first_frame: true,
                }
            }
        }

        #[doc(hidden)]
        impl ::baseplug::UIModel for #ui_ident {
            fn update(&mut self) {
                // Skip updating on the first frame so the UI gets a chance to get all initial values.
                if self.first_frame {
                    self.first_frame = false;
                } else {
                    #( #ui_update_statements ;)*
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
