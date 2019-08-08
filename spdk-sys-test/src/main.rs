extern crate spdk_sys;

use std::{
    env,
    ffi::CString,
    ptr,
    os::raw::{c_char, c_int},
};
use spdk_sys::*;

extern "C" fn usage() {
}

/// This is our test function which calls a method from spdk rust bindings.
/// We intentionally avoid calling spdk_app_start() because in order to
/// succeed for this function we would have to run as root, have huge pages
/// configured and that is out of scope of this test utility.
fn main() -> Result<(), ()> {
    // hand over command line args to spdk arg parser
    let args = env::args()
        .map(|arg| CString::new(arg).unwrap())
        .collect::<Vec<CString>>();
    let mut c_args = args
        .iter()
        .map(|arg| arg.as_ptr())
        .collect::<Vec<*const c_char>>();
    c_args.push(ptr::null());

    let mut opts: spdk_app_opts = Default::default();

    let rc = unsafe {
        spdk_app_opts_init(&mut opts as *mut spdk_app_opts);
        spdk_app_parse_args(
            (c_args.len() as c_int) - 1,
            c_args.as_ptr() as *mut *mut i8,
            &mut opts,
            ptr::null_mut(), // extra short options i.e. "f:S:"
            ptr::null_mut(), // extra long options
            None,       // extra options parse callback
            Some(usage),
        )
    };
    if rc != spdk_sys::SPDK_APP_PARSE_ARGS_SUCCESS {
        Err(())
    } else {
        println!("Success!");
        Ok(())
    }
}
