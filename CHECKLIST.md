# VST2
## FFI
- [ ] `dispatch(opcode: i32, index: i32, value: isize, ptr: *mut c_void, opt: f32) -> isize` - Dispatch an event with an opcode.
  - [ ] `Get VST API Version` - Return the VST API version.
  - [ ] `Shutdown` - Shut down the plugin.
  - [ ] `SetSampleRate` - Set the sample rate to `opt`.
  - [ ] `StateChanged` - (Is this a call to reset the plugin I assume?)
  - [ ] `GetParameterName` - Store the name of the parameter at `index` into `ptr. Return 0 for success.
  - [ ] `GetParameterLabel` - Store the label of the parameter at `index` into `ptr. Return 0 for success.
  - [ ] `GetParameterDisplay` - (Not sure what this does)
  - [ ] `CanBeAutomated` - (Not sure what this does)
  - [ ] `GetEffectName` - Store the effect name into `ptr`. Return 1 for success.
  - [ ] `GetProductName` - Store the product name into `ptr`. Return 1 for success.
  - [ ] `GetVendorName` - Store the vendor name into `ptr`. Return 1 for success.
  - [ ] `GetCurrentPresetName` - (Incomplete I assume?)
  - [ ] `ProcessEvent` - (Not sure what this does)
  - [ ] `GetData` - (Not sure what this does)
  - [ ] `SetData` - (Not sure what this does)
  - [ ] `EditorGetRect` - Store initial plugin window size into `ptr`. The host may call this before opening the plugin editor window. Returning the correct size based on DPI scaling can be acheived by first a VST extension, second from a user-supplied config-file, and third from guessing the DPI scaling of the system.
  - [ ] `EditorOpen` - Open the editor window. (Is `ptr` a handle to the window?)
  - [ ] `EditorClose` - Close the editor window.
  - [ ] `UnhandledOpCode` - Print the unhandled opcode.
- [ ] `get_parameter(index: i32) -> f32` - Retreive the current value of the parameter at `index`.
- [ ] `set_parameter(index: i32, val: f32)` - Set the value of the parameter at `index`.
- [ ] `get_musical_time() -> MusicalTime { bmp: f64, beat: f64 }` - Retreive musical time information.
- [ ] `process_replacing(in_buffers: *const *const f32, out_buffers: *mut *mut f32, nframes: i32)` - Process buffers.

# VST3
## FFI
- [ ] (commands)

# AU
## FFI
- [ ] (commands)

# LV2
## FFI
- [ ] (commands)
