use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub author: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct PluginRequest {
    pub command: String,
    pub data: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct PluginResponse {
    pub success: bool,
    pub data: String,
}

#[macro_export]
macro_rules! export_plugin {
    ($init:expr, $execute:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_init() -> *mut u8 {
            let info = $init();
            let json = serde_json::to_string(&info).unwrap();
            let bytes = json.into_bytes();
            let ptr = bytes.as_ptr() as *mut u8;
            let len = bytes.len();

            unsafe {
                LAST_RESULT_LEN = len;
            }
            std::mem::forget(bytes);
            ptr
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_execute(ptr: *const u8, len: usize) -> *mut u8 {
            let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
            let request: plugin_api::PluginRequest =
                serde_json::from_slice(bytes).unwrap();

            let response = $execute(request);
            let json = serde_json::to_string(&response).unwrap();
            let bytes = json.into_bytes();
            let ptr = bytes.as_ptr() as *mut u8;
            let len = bytes.len();

            unsafe {
                LAST_RESULT_LEN = len;
            }
            std::mem::forget(bytes);
            ptr
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn get_result_len() -> usize {
            unsafe { LAST_RESULT_LEN }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn plugin_cleanup(ptr: *mut u8, len: usize) {
            unsafe {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }
        }
        static mut LAST_RESULT_LEN: usize = 0;
    };
}
