#![allow(dead_code)]
#![allow(unused)]

extern crate ffi_utils;
#[macro_use]
extern crate neon;
extern crate safe_app;
extern crate safe_core;

use ffi_utils::test_utils::call_1;
use ffi_utils::FfiResult;
use neon::prelude::*;
use safe_app::ffi::crypto::app_pub_enc_key;
use safe_app::ffi::object_cache::EncryptPubKeyHandle;
use safe_app::ffi::test_utils::test_create_app;
use safe_app::test_utils::create_auth_req;
use safe_app::App;
use safe_core::btree_set;
use safe_core::ipc::Permission;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::sync::mpsc;

fn app_is_mock_js(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    Ok(cx.boolean(safe_app::app_is_mock()))
}

fn app_pub_enc_key_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    // Convert array buffer into App pointer
    let app = cx.argument::<JsArrayBuffer>(0)?;
    let app = cx.borrow(&app, |data| data.as_slice::<u8>());
    let app = u64::from_ne_bytes([
        app[0], app[1], app[2], app[3], app[4], app[5], app[6], app[7],
    ]) as *const App;

    let key: Result<EncryptPubKeyHandle, (i32, String)> =
        unsafe { join_cb(|ud, cb| app_pub_enc_key(app, ud, cb)) };

    Ok(JsUndefined::new())
}

/// Call FFI and wait for callback to pass back value(s)
fn join_cb<F, T>(f: F) -> Result<T, (i32, String)>
where
    F: FnOnce(*mut c_void, extern "C" fn(*mut c_void, *const FfiResult, T)),
{
    let (tx, rx) = std::sync::mpsc::channel::<Result<T, (i32, String)>>();
    let txp = &tx as *const _ as *mut c_void;

    f(txp, cb::<T>);

    rx.recv().unwrap()
}

extern "C" fn cb<T>(user_data: *mut c_void, res: *const FfiResult, app: T) {
    let tx = user_data as *mut mpsc::Sender<Result<T, (i32, String)>>;

    unsafe {
        (*tx).send(match (*res).error_code {
            0 => Ok(app),
            _ => Err((
                (*res).error_code,
                String::from(CStr::from_ptr((*res).description).to_str().unwrap()),
            )),
        });
    }
}

fn test_create_app_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app_id = cx.argument::<JsString>(0)?.value();
    let f = cx.argument::<JsFunction>(1).unwrap();

    let task = SafeTask { app_id };
    task.schedule(f);

    Ok(JsUndefined::new())
}

struct SafeTask {
    app_id: String,
}
impl Task for SafeTask {
    type Output = u64;
    type Error = (i32, String);
    type JsEvent = JsArrayBuffer;

    fn perform(&self) -> Result<u64, (i32, String)> {
        let app_id = CString::new(self.app_id.clone()).expect("CString::new failed");
        let app_id = app_id.as_ptr();

        let app: Result<*mut App, _> = join_cb(|ud, cb| unsafe { test_create_app(app_id, ud, cb) });

        match app {
            Ok(app) => Ok(app as u64),
            Err(err) => Err(err),
        }
    }

    fn complete<'a>(
        self,
        mut cx: TaskContext<'a>,
        result: Result<u64, (i32, String)>,
    ) -> JsResult<JsArrayBuffer> {
        match result {
            Ok(app_h) => {
                let mut buf =
                    JsArrayBuffer::new(&mut cx, std::mem::size_of::<*mut App>() as u32).unwrap();
                cx.borrow_mut(&mut buf, |data| {
                    let slice = (app_h as u64).to_ne_bytes();
                    data.as_mut_slice::<u8>().clone_from_slice(&slice);
                });
                Ok(buf)
            }
            Err(err) => {
                let code = cx.number(err.0);

                let err = cx.error(err.1).unwrap();
                // Add a `error_code` property to Error
                err.set(&mut cx, "error_code", code).unwrap();

                cx.throw(err)
            }
        }
    }
}

register_module!(mut cx, {
    cx.export_function("app_is_mock", app_is_mock_js)?;
    cx.export_function("app_pub_enc_key", app_pub_enc_key_js)?;
    cx.export_function("test_create_app", test_create_app_js)?;
    // cx.export_function("test_create_app_sync", test_create_app_js_sync)?;

    Ok(())
});
