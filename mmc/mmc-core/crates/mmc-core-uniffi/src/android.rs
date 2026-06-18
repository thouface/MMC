//! Android platform support module.
//!
//! This module provides Android-specific functionality used by the core library.
//! When compiled for non-Android targets this module is empty.

use std::os::raw::c_char;
use std::ffi::{CStr, CString};

use crate::core::MmcCore;

/// Internal helper to convert a null-terminated C string to a Rust String.
#[allow(dead_code)]
unsafe fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    Some(
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    )
}

/// C-style FFI entry points. These are declared with no_mangle and extern "C"
/// so they are visible from dlopen on Android and are also used by
/// our own tests.

#[no_mangle]
pub extern "C" fn mmc_core_create() -> *mut MmcCore {
    let core = Box::new(MmcCore::new());
    Box::into_raw(core)
}

#[no_mangle]
pub extern "C" fn mmc_core_destroy(core: *mut MmcCore) {
    if !core.is_null() {
        // Consume the boxed core
        unsafe {
            let _ = Box::from_raw(core);
        }
    }
}

#[no_mangle]
pub extern "C" fn mmc_core_version() -> *mut c_char {
    let version = CString::new(env!("CARGO_PKG_VERSION")).unwrap();
    version.into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn mmc_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_create_destroy() {
        let core = mmc_core_create();
        assert!(!core.is_null());
        mmc_core_destroy(core);
    }

    #[test]
    fn test_core_version() {
        let version = unsafe {
            let raw = mmc_core_version();
            let s = CStr::from_ptr(raw).to_string_lossy().to_string();
            mmc_free_string(raw);
            s
        };
        assert!(!version.is_empty());
    }

    #[test]
    fn test_destroy_null_safe() {
        mmc_core_destroy(std::ptr::null_mut());
    }

    #[test]
    fn test_free_null_safe() {
        unsafe {
            mmc_free_string(std::ptr::null_mut());
        }
    }

    #[test]
    fn test_cstring_roundtrip() {
        let s = CString::new("test string").unwrap();
        let ptr = s.as_ptr();
        let result = unsafe { cstr_to_string(ptr) };
        assert_eq!(result, Some("test string".to_string()));
    }

    #[test]
    fn test_cstring_null_ptr() {
        let result = unsafe { cstr_to_string(std::ptr::null()) };
        assert!(result.is_none());
    }
}
