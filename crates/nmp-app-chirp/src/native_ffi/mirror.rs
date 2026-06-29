use std::mem::ManuallyDrop;

use nmp_native_runtime::app_mirror::{error, error_bytes};
use nmp_native_runtime::NmpApp;

use super::const_app_ref;

#[repr(C)]
pub struct NmpMirrorBytes {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

#[no_mangle]
pub extern "C" fn nmp_mirror_pull_page(
    app: *const NmpApp,
    cursor_id: u64,
    max_entries: u32,
    max_total_raw_bytes: u32,
) -> NmpMirrorBytes {
    let bytes = const_app_ref(app)
        .map(|app| {
            app.mirror_pull_page_raw_bytes(cursor_id, max_entries, max_total_raw_bytes as usize)
        })
        .unwrap_or_else(|| error_bytes(error::NULL_APP));
    into_owned_bytes(bytes)
}

#[no_mangle]
pub extern "C" fn nmp_mirror_free_bytes(bytes: NmpMirrorBytes) {
    if bytes.ptr.is_null() {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(bytes.ptr, bytes.len, bytes.cap));
    }
}

fn into_owned_bytes(bytes: Vec<u8>) -> NmpMirrorBytes {
    let mut bytes = ManuallyDrop::new(bytes);
    NmpMirrorBytes {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
        cap: bytes.capacity(),
    }
}
