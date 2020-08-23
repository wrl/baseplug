use crate::{Model, Param, Parameters};
use std::os::raw::c_void;
use std::ptr;
use vst3_com::ComPtr;
use vst3_sys::base::{
    char8, kInvalidArgument, kResultFalse, kResultOk, IPluginFactory,
    PClassInfo, PFactoryInfo,
};
pub mod prelude {
    pub use vst3_sys::vst::Event as Vst3Event;
    pub use vst3_sys::{
        base::{IBStream, IPluginBase, TBool, FIDString},
        utils::VstPtr,
        vst::{
            BusDirection, BusInfo, IAudioProcessor, IComponent, IEventList, IParamValueQueue,
            IParameterChanges, IoMode, MediaType, ProcessData, ProcessSetup, RoutingInfo,
            IEditController, SpeakerArrangement, IComponentHandler, TChar, ParameterInfo,
            IUnitInfo, UnitInfo, ProgramListInfo
        },
        IID, VST3,
    };
}

use prelude::*;
struct ClassData {
    cid: IID,
    constructor: Box<dyn Fn() -> *mut c_void>,
    name: String,
    category: String,
}

pub type IBStreamPtr = VstPtr<dyn IBStream>;

#[doc(hidden)]
pub fn num_params_for<T: Model>() -> usize {
    T::Smooth::PARAMS.len()
}

#[doc(hidden)]
pub fn param_for_vst3_id<T: Model>(id: u32) -> Option<&'static Param<T::Smooth>> {
    T::Smooth::PARAMS.get(id as usize).copied()
}

#[doc(hidden)]
#[allow(non_snake_case)]
pub fn utf8_to_String128(s: &str) -> vst3_sys::vst::String128 {
    let mut self_ = [0; 128];
    for (u, i) in s.encode_utf16().zip(self_.iter_mut()) {
        *i = (0x7f & u) as i16;
    }
    self_
}

/// Helper function to pull data out of an IBStream.
#[doc(hidden)]
pub fn drain_ibstream(ib: IBStreamPtr) -> Option<Vec<u8>> {
    Some(IBstreamDrain::new(ib)?.collect())
}
struct IBstreamDrain {
    ptr: ComPtr<dyn IBStream>,
    buf: Vec<u8>,
    idx: usize,
}

impl IBstreamDrain {
    fn new(ptr: IBStreamPtr) -> Option<Self> {
        Some(Self {
            ptr: ptr.upgrade()?,
            buf: Vec::with_capacity(128),
            idx: 0,
        })
    }
}

impl Iterator for IBstreamDrain {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        if self.idx == self.buf.len() {
            let mut read = 0;
            self.buf.resize(128, 0);
            unsafe {
                if (self.ptr.read(
                    &mut self.buf[0] as *mut u8 as *mut _,
                    self.buf.capacity() as i32,
                    &mut read as *mut _,
                ) != 0)
                    || read == 0
                {
                    return None;
                } else {
                    self.buf.set_len(read as usize);
                    self.idx = 0;
                }
            }
        }
        self.idx += 1;
        Some(self.buf[self.idx - 1])
    }
}

#[VST3(implements(IPluginFactory))]
pub struct Factory {
    vendor: String,
    url: String,
    email: String,
    table: Vec<ClassData>,
}

impl Factory {
    pub fn new(vendor: &str, url: &str, email: &str) -> Box<Self> {
        Self::allocate(vendor.to_owned(), url.to_owned(), email.to_owned(), vec![])
    }

    pub fn register_class<F>(
        &mut self,
        name: &str,
        category: Option<&str>,
        cid: IID,
        constructor: F,
    ) where
        F: Fn() -> *mut c_void + 'static,
    {
        let class_data = ClassData {
            name: name.to_owned(),
            category: category.unwrap_or("Audio Module Class").to_owned(),
            cid,
            constructor: Box::new(constructor),
        };
        self.table.push(class_data);
    }
}

fn string_cast(s: &str, dst: &mut [char8]) {
    let len = dst.len();
    let string = if s.len() >= len {
        let mut s = s.to_owned();
        s.truncate(len);
        s
    } else {
        let padding_bytes = len - s.len();
        let mut s = s.to_owned();
        s.extend((0..padding_bytes).map(|_| '\0'));
        s
    };
    for (u, i) in string.bytes().zip(dst.iter_mut()) {
        *i = (0x7f & u) as char8; // maybe we'll get garbage, who knows.
    }
}

impl IPluginFactory for Factory {
    unsafe fn get_factory_info(&self, out_info: *mut PFactoryInfo) -> i32 {
        if out_info.is_null() {
            eprintln!("Null pointer passed into Factory::get_factory_info");
            return kInvalidArgument;
        }

        let mut vendor = [0; 64];
        let mut url = [0; 256];
        let mut email = [0; 128];

        string_cast(&self.vendor, &mut vendor);
        string_cast(&self.url, &mut url);
        string_cast(&self.email, &mut email);

        let info = PFactoryInfo {
            vendor,
            url,
            email,
            flags: 1 << 4,
        };

        ptr::write(out_info, info);
        kResultOk
    }

    unsafe fn count_classes(&self) -> i32 {
        let c = self.table.len() as i32;
        c
    }

    unsafe fn get_class_info(&self, index: i32, out_info: *mut PClassInfo) -> i32 {
        let index = index as usize;
        if out_info.is_null() {
            eprintln!("Null pointer passed into Factory::get_class_info");
            return kInvalidArgument;
        }

        if let Some(obj) = self.table.get(index) {
            let mut name = [0; 64];
            let mut category = [0; 32];
            string_cast(&obj.name, &mut name);
            string_cast(&obj.category, &mut category);
            
            let info = PClassInfo {
                cid: obj.cid,
                name,
                category,
                cardinality: 0x7FFF_FFFF,
            };

            ptr::write(out_info, info);
            kResultOk
        } else {
            eprintln!("No class information for given index");
            kInvalidArgument
        }
    }

    unsafe fn create_instance(
        &self,
        cid: *const IID,
        _iid: *const IID,
        obj: *mut *mut c_void,
    ) -> i32 {
        if cid.is_null() || obj.is_null() {
            eprintln!("Nullptr passed into Factory::create_instance()");
            return kInvalidArgument;
        }
        if let Some(class) = self.table.iter().find(|c| c.cid == *cid) {
            *obj = (class.constructor)();
            kResultOk
        } else {
            eprintln!("Class not registered with factory or invalid CID");
            kResultFalse
        }
    }
}

#[macro_export]
macro_rules! vst3 {
    ($plugin:ident, $url:expr, $email:expr, $iid:expr) => {
        #[doc(hidden)]
        pub mod vst3 {
        use super::*;
        use baseplug::api::vst3::prelude::*;
        type WrappedPlugin = baseplug::wrapper::WrappedPlugin<$plugin>;
        
        #[doc(hidden)]
        #[VST3(implements(IComponent, IPluginBase, IAudioProcessor, IEditController))]
        pub struct Vst3Wrapper {
            wrapped: std::cell::UnsafeCell<WrappedPlugin>,
        }

        impl Vst3Wrapper {
            fn new() -> Box<Vst3Wrapper> {
                Vst3Wrapper::allocate(std::cell::UnsafeCell::new(
                    baseplug::wrapper::WrappedPlugin::new(),
                ))
            }
            unsafe fn plugin<'a>(&'a self) -> &'a mut WrappedPlugin {
                &mut *self.wrapped.get()
            }

            unsafe fn param<'a>(&'a self, id:u32) -> Option<&'static baseplug::Param<<<$plugin as baseplug::Plugin>::Model as baseplug::Model>::Smooth>> {
                baseplug::api::vst3::param_for_vst3_id::<<$plugin as Plugin>::Model>(id)
            }
        }

        impl IPluginBase for Vst3Wrapper {
            unsafe fn initialize(&self, _: *mut std::os::raw::c_void) -> i32 {
                0
            }
            unsafe fn terminate(&self) -> i32 {
                0
            }
        }

        impl IUnitInfo for Vst3Wrapper {
            unsafe fn get_unit_count(&self) -> i32 { 
                1
            }
            unsafe fn get_unit_info(&self, unit_index: i32, out_info: *mut UnitInfo) -> i32 {
                if unit_index != 0 || out_info.is_null() {
                    eprintln!("Invalid argument passed to IUnitInfo::get_unit_info");
                    return 2; 
                }
                let info = UnitInfo {
                    id: 0,
                    parent_unit_id: 0,
                    name: baseplug::api::vst3::utf8_to_String128("Root"),
                    program_list_id: -1,
                };
                0
            }
            unsafe fn get_program_list_count(&self) -> i32 { 0 }
            unsafe fn get_program_list_info(&self, _list_index: i32, _out_info: *mut ProgramListInfo) -> i32 {
                0
            }
            unsafe fn get_program_name(&self, _list_id: i32, _program_index: i32, _name: *mut u16) -> i32 {
                0
            }
            unsafe fn get_program_info(
                &self,
                _list_id: i32,
                _program_index: i32,
                _attribute_id: *const u8,
                _attribute_value: *mut u16,
            ) -> i32 {
                0
            }
            unsafe fn has_program_pitch_names(&self, _id: i32, _index: i32) -> i32 {
                0
            }
            unsafe fn get_program_pitch_name(
                &self,
                _id: i32,
                _index: i32,
                _pitch: i16,
                _name: *mut u16,
            ) -> i32 {
                0
            }
            unsafe fn get_selected_unit(&self) -> i32 { 1 }
            unsafe fn select_unit(&self, _id: i32) -> i32 { 0 }
            unsafe fn get_unit_by_bus(
                &self,
                _type_: i32,
                _dir: i32,
                _bus_index: i32,
                _channel: i32,
                unit_id: *mut i32,
            ) -> i32 {
                *unit_id = 1;
                0
            }
            unsafe fn set_unit_program_data(
                &self,
                _list_or_unit: i32,
                _program_idx: i32,
                _data: VstPtr<dyn IBStream>,
            ) -> i32 {
                0
            }
        }

        impl IEditController for Vst3Wrapper {
            unsafe fn set_component_state(&self, _state: VstPtr<IBStream>) -> i32 {
                // leave this unimplemented. That seems to be OK for single component effects
                0
            }

            unsafe fn set_state(&self, state: VstPtr<dyn IBStream>) -> i32 {
                <Self as IComponent>::set_state(self, state)
            }

            unsafe fn get_state(&self, state: VstPtr<dyn IBStream>) -> i32 {
                <Self as IComponent>::get_state(self, state)
            }

            unsafe fn get_parameter_count(&self) -> i32 {
                baseplug::api::vst3::num_params_for::<<$plugin as Plugin>::Model>() as i32
            }

            unsafe fn get_parameter_info(&self, param_index: i32, out_info: *mut ParameterInfo) -> i32 {
                let param = self.param(param_index as u32); //todo: hash ids instead of using indices
                if let None = param { 
                    return 1;
                }
                let param = param.unwrap();
                use baseplug::api::vst3::utf8_to_String128;
                let info = ParameterInfo {
                    id: param_index as u32, 
                    title: utf8_to_String128(param.name), 
                    short_title: utf8_to_String128(param.short_name.unwrap_or(param.name)), 
                    units: utf8_to_String128(param.get_label()), 
                    step_count: 0,                 //todo: discrete units
                    default_normalized_value: 0.0, //todo: default normalized value
                    unit_id: 1,                    //todo: unit ids, probably only use 1 
                    flags: 0,                      //todo: parameter flags
                }; // todo: bypass parameter
                std::ptr::write(out_info, info);
                0
            }

            unsafe fn get_param_string_by_value(
                &self,
                id: u32,
                value_normalized: f64,
                string: *mut TChar,
            ) -> i32 {
                0
            }

            unsafe fn get_param_value_by_string(
                &self,
                id: u32,
                string: *const TChar,
                value_normalized: *mut f64,
            ) -> i32 {
                0
            }
            
            unsafe fn normalized_param_to_plain(&self, id: u32, value_normalized: f64) -> f64 {
                if let Some(param) = self.param(id) {
                    self.plugin().denormalize(param, value_normalized as f32).into()
                } else {
                    0.0
                }
            }
            
            unsafe fn plain_param_to_normalized(&self, id: u32, plain_value: f64) -> f64 {
                if let Some(param) = self.param(id) {
                    self.plugin().normalize(param, plain_value as f32).into()
                } else {
                    0.0
                }
            }

            unsafe fn get_param_normalized(&self, id: u32) -> f64 {
                if let Some(param) = self.param(id) {
                    self.plugin().get_parameter(param) as f64
                } else {
                    std::f64::NAN
                }
            }

            unsafe fn set_param_normalized(&self, id: u32, value: f64) -> i32 {
                if let Some(param) = self.param(id) {
                    self.plugin().set_parameter(param, value as f32);
                    0
                } else {
                    1
                }
            }

            unsafe fn set_component_handler(&self, _handler: VstPtr<IComponentHandler>) -> i32 {
                // we'll need to implement this once we have GUI integration, the _handler
                // arg is used to begin/perform/complete edits.
                0
            }
            unsafe fn create_view(&self, _name: FIDString) -> *mut std::os::raw::c_void {
                std::ptr::null_mut()
            }
        }

        impl IComponent for Vst3Wrapper {
            unsafe fn get_controller_class_id(&self, tuid: *mut IID) -> i32 {
                std::ptr::write(tuid, $iid);
                0
            }
            
            unsafe fn set_io_mode(&self, _mode: IoMode) -> i32 {
                0
            }

            unsafe fn get_bus_count(&self, type_: MediaType, dir: BusDirection) -> i32 {
                if type_ == 0 {
                    // audio
                    1 // single input/output
                } else if type_ == 1 {
                    if dir == 0 {
                        // input
                        1
                    } else {
                        // output, todo: midi output
                        0
                    }
                } else {
                    eprintln!("invalid argument passed to IComponent::get_bus_count");
                    2
                }
            }

            unsafe fn get_bus_info(
                &self,
                type_: MediaType,
                dir: BusDirection,
                index: i32,
                out_info: *mut BusInfo,
            ) -> i32 {
                if index != 0 || out_info.is_null() || dir < 0 || dir > 1 || type_ < 0 || type_ > 1
                {
                    eprintln!("invallid argument passed to IComponent::get_bus_info");
                    return 2; // kInvalidArgument
                }
                let is_input = dir == 0;
                let is_event = type_ == 0;
                let (name, channel_count) = match (is_input, is_event) {
                    (true, false) => ("Audio Input", 2),
                    (false, false) => ("Audio Output", 2),
                    (true, true) => ("Event Input", 0),
                    (false, true) => ("Event Output", 0),
                };
                let info = BusInfo {
                    media_type: type_,
                    direction: dir,
                    channel_count,
                    name: baseplug::api::vst3::utf8_to_String128(name),
                    bus_type: 0,
                    flags: 1,
                };
                std::ptr::write(out_info, info);
                0
            }

            unsafe fn get_routing_info(
                &self,
                _in_info: *mut RoutingInfo,
                _out_info: *mut RoutingInfo,
            ) -> i32 {
                //todo: does this matter? find out next time on dragon ball z
                0
            }

            unsafe fn activate_bus(
                &self,
                _type_: MediaType,
                _dir: BusDirection,
                _index: i32,
                _state: TBool,
            ) -> i32 {
                0
            }

            unsafe fn set_active(&self, _state: TBool) -> i32 {
                0
            }

            unsafe fn set_state(&self, state: baseplug::api::vst3::IBStreamPtr) -> i32 {
                if let Some(buf) = baseplug::api::vst3::drain_ibstream(state) {
                    self.plugin().deserialise(&buf);
                    0
                } else {
                    eprintln!("Failed to drain IBStream in IComponent::set_state");
                    -1
                }
            }

            unsafe fn get_state(&self, state: baseplug::api::vst3::IBStreamPtr) -> i32 {
                let state = if let Some(state) = state.upgrade() {
                    state
                } else {
                    return 1;
                };
                if let Some(mut buf) = self.plugin().serialise() {
                    let mut written = 0i32;
                    while (written as usize) < buf.len() {
                        let result = state.write(
                            &mut buf[written as usize] as *mut u8 as *mut _,
                            (buf.len() - (written as usize)) as i32,
                            &mut written as *mut _,
                        );
                        if result != 0 {
                            return result;
                        }
                    }
                    0
                } else {
                    1
                }
            }
        }

        impl IAudioProcessor for Vst3Wrapper {
            unsafe fn set_bus_arrangements(
                &self,
                inputs: *mut SpeakerArrangement,
                num_ins: i32,
                outputs: *mut SpeakerArrangement,
                num_outs: i32,
            ) -> i32 {
                if (num_ins != 1) || (num_outs != 1) {
                    eprintln!("missing input or output bus");
                    return 2;
                }
                *inputs = 3; // stereo
                *outputs = 3;
                0
            }
            unsafe fn get_bus_arrangement(
                &self,
                _dir: BusDirection,
                _index: i32,
                arr: *mut SpeakerArrangement,
            ) -> i32 {
                *arr = 3;
                0
            }
            unsafe fn can_process_sample_size(&self, symbolic_sample_size: i32) -> i32 {
                if symbolic_sample_size == 0 {
                    // 32 bit floats {
                    0
                } else {
                    1
                }
            }
            unsafe fn get_latency_samples(&self) -> u32 {
                //todo: latency reporting
                0
            }
            unsafe fn setup_processing(&self, setup: *const ProcessSetup) -> i32 {
                if setup.is_null() {
                    eprintln!("Null pointer passed to IAudioProcessor::setup_processing");
                    return 1;
                }
                let setup = &*setup;
                let plugin = self.plugin();
                plugin.set_sample_rate(setup.sample_rate as f32);
                // todo: symbolic_sample_size
                // todo: max_samples_per_block
                // todo: process_mode
                0
            }
            unsafe fn set_processing(&self, _state: TBool) -> i32 {
                0
            }
            unsafe fn process(&self, data: *mut ProcessData) -> i32 {
                use std::slice;
                if data.is_null() {
                    return 0;
                }
                let plugin = self.plugin();
                let data = &mut *data;
                // drain the events
                if let Some(input_events) = data.input_events.upgrade() {
                    for i in 0..input_events.get_event_count() {
                        let mut event = std::mem::MaybeUninit::<Vst3Event>::uninit();
                        if input_events.get_event(i, event.as_mut_ptr()) == 0 {
                            let _event = event.assume_init();
                            // todo: parse into nearest midi notes.
                        }
                        // todo: error condition
                    }
                }
                // drain parameter changes
                // todo: interleave with events for sample accurate automation
                if let Some(input_param_changes) = data.input_param_changes.upgrade() {
                    for i in 0..input_param_changes.get_parameter_count() {
                        if let Some(queue) = input_param_changes.get_parameter_data(i).upgrade() {
                            let id = queue.get_parameter_id();
                            for p in 0..queue.get_point_count() {
                                let mut offset = 0;
                                let mut value = 0.0;
                                if queue.get_point(p, &mut offset as *mut _, &mut value as *mut _)
                                    != 0
                                {
                                    break;
                                }
                                if let Some(param) = baseplug::api::vst3::param_for_vst3_id::<
                                    <$plugin as Plugin>::Model,
                                >(id)
                                {
                                    self.plugin().set_parameter(param, value as f32);
                                }
                            }
                        }
                    }
                }
                if data.inputs.is_null()
                    || data.outputs.is_null()
                    || data.num_inputs != data.num_outputs
                    || (*data.inputs).buffers.is_null()
                    || (*data.outputs).buffers.is_null()
                {
                    return 0;
                }
                let buffer_size = data.num_samples as usize;
                if buffer_size == 0 {
                    return 0;
                }
                let inputs = {
                    let buffers = (&*data.inputs).buffers;
                    [
                        std::slice::from_raw_parts(buffers.offset(0) as *mut f32, buffer_size),
                        std::slice::from_raw_parts(buffers.offset(1) as *mut f32, buffer_size),
                    ]
                };
                let outputs = {
                    let buffers = (&mut *data.outputs).buffers;
                    [
                        std::slice::from_raw_parts_mut(buffers.offset(0) as *mut f32, buffer_size),
                        std::slice::from_raw_parts_mut(buffers.offset(1) as *mut f32, buffer_size),
                    ]
                };
                let time = {
                    if data.context.is_null() {
                        // todo: figure this out
                        baseplug::MusicalTime {
                            bpm: 0.0,
                            beat: 0.0,
                        }
                    } else {
                        let context = &*data.context;
                        baseplug::MusicalTime {
                            bpm: context.tempo,
                            beat: context.project_time_music,
                        }
                    }
                };
                plugin.process(time, inputs, outputs, buffer_size);
                0
            }

            unsafe fn get_tail_samples(&self) -> u32 {
                // todo: tail samples reporting
                0
            }
        }

        #[no_mangle]
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub extern "system" fn InitDll() -> bool {
            true
        }

        #[no_mangle]
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub extern "system" fn ExitDll() -> bool {
            true
        }

        #[cfg(target_os = "linux")]
        #[no_mangle]
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub extern "system" fn ModuleEntry(_: *mut std::os::raw::c_void) -> bool {
            true
        }

        #[cfg(target_os = "linux")]
        #[no_mangle]
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub extern "system" fn ModuleExit() -> bool {
            info!("Module exited");
            true
        }

        #[no_mangle]
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub extern "system" fn bundleEntry(_: *mut std::os::raw::c_void) -> bool {
            true
        }

        #[no_mangle]
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub extern "system" fn bundleExit() -> bool {
            true
        }

        #[no_mangle]
        #[doc(hidden)]
        #[allow(non_snake_case, clippy::missing_safety_doc)]
        pub unsafe extern "system" fn GetPluginFactory() -> *mut std::os::raw::c_void {
            let mut factory = baseplug::api::vst3::Factory::new($plugin::VENDOR, $url, $email);
            let constructor = || Box::into_raw(Vst3Wrapper::new()) as *mut std::os::raw::c_void;
            factory.register_class($plugin::NAME, None, $iid, constructor);
            // todo: get rid of memory leak here
            Box::into_raw(factory) as *mut _
        }
    }
    };
}
