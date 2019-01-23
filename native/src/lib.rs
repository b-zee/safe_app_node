#![allow(dead_code)]
#![allow(unused)]

extern crate ffi_utils;
#[macro_use]
extern crate neon;
extern crate safe_app;
extern crate safe_core;

use ffi_utils::FfiResult;
use neon::prelude::*;
use safe_app::App;
use safe_app::test_utils::create_auth_req;
use safe_core::btree_set;
use safe_core::ipc::Permission;
use std::collections::HashMap;
use std::ffi::CString;
use std::ffi::CStr;
use std::os::raw::c_void;

fn app_is_mock(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    Ok(cx.boolean(safe_app::app_is_mock()))
}

fn app_pub_enc_key(mut cx: FunctionContext<'_>) -> JsResult<JsUndefined> {
    let app = cx.argument::<JsArrayBuffer>(0)?;
    let app = cx.borrow(&app, |data| { data.as_slice::<u8>() });
    let app = u64::from_ne_bytes([app[0], app[1], app[2], app[3], app[4], app[5], app[6], app[7]]) as *const App;

    let cx = Box::new(cx);
    let cx = Box::into_raw(cx) as *mut c_void;

    dbg!(cx);

    unsafe {
        safe_app::ffi::crypto::app_pub_enc_key(app, cx, o_cb);
    }
    extern "C" fn o_cb(
        user_data: *mut c_void,
        error: *const FfiResult,
        public_key_h: safe_app::ffi::object_cache::EncryptPubKeyHandle
    ) {
        let mut cx: Box<FunctionContext> = unsafe { Box::from_raw(user_data as *mut _) };

        dbg!(user_data);
        dbg!(cx.len()); // Sometimes correct, sometimes gibberish

        // SEGFAULT
        let f = cx.argument::<JsFunction>(1).unwrap();

        // let arg1 = cx.number(unsafe { (*error).error_code });
        // let arg2: Handle<'_, JsValue> = if unsafe { (*error).description }.is_null() {
        //     JsNull::new().upcast()
        // } else {
        //     cx.string(unsafe { CStr::from_ptr( (*error).description ) }.to_str().unwrap()).upcast()
        // };

        // let ffi_result = JsObject::new(&mut cx);
        // ffi_result.set(&mut cx, "error_code", arg1).unwrap();
        // ffi_result.set(&mut cx, "description", arg2).unwrap();

        // let args: Vec<Handle<JsValue>> = vec![ffi_result.upcast()];

        // f.call(&mut cx, JsNull::new(), vec![JsNull::new()]);
    }


    Ok(JsUndefined::new())
}

fn test_create_app(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app_id = cx.argument::<JsString>(0)?.value();
    let app_id = CString::new(app_id).expect("CString::new failed");

    let cxp = Box::new(cx);
    let cxp = Box::into_raw(cxp) as *mut c_void;

    unsafe {
        safe_app::ffi::test_utils::test_create_app(app_id.as_ptr(), cxp, o_cb);
    }
    extern "C" fn o_cb(user_data: *mut c_void, error: *const FfiResult, o_app: *mut App) {
        let cx: Box<FunctionContext> = unsafe { Box::from_raw(user_data as *mut _) };
        let mut cx = *cx;

        let f = cx.argument::<JsFunction>(1).unwrap();
        let arg1 = cx.number(unsafe { (*error).error_code });
        let arg2: Handle<'_, JsValue> = if unsafe { (*error).description }.is_null() {
            JsNull::new().upcast()
        } else {
            cx.string(unsafe { CStr::from_ptr( (*error).description ) }.to_str().unwrap()).upcast()
        };
        let mut arg3 = JsArrayBuffer::new(&mut cx, std::mem::size_of::<*mut App>() as u32).unwrap();

        cx.borrow_mut(&mut arg3, |data| {
            let slice = (o_app as u64).to_ne_bytes();
            data.as_mut_slice::<u8>()
                .clone_from_slice(&slice);
        });

        let ffi_result = JsObject::new(&mut cx);
        ffi_result.set(&mut cx, "error_code", arg1).unwrap();
        ffi_result.set(&mut cx, "description", arg2).unwrap();
        let args: Vec<Handle<JsValue>> = vec![ffi_result.upcast(), arg3.upcast()];

        f.call(&mut cx, JsNull::new(), args);
    }

    Ok(JsUndefined::new())
}



register_module!(mut cx, {
    cx.export_function("app_is_mock", app_is_mock)?;
    cx.export_function("app_pub_enc_key", app_pub_enc_key)?;
    cx.export_function("test_create_app", test_create_app)?;

    Ok(())
});
