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
use std::mem;
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
    let app = usize::from_ne_bytes([
        app[0], app[1], app[2], app[3], app[4], app[5], app[6], app[7],
    ]);

    let f = cx.argument::<JsFunction>(1).unwrap();

    let task = SafeTask::<EncryptPubKeyHandle> {
        f: Box::new(move || {
            join_cb(|ud, cb| unsafe { app_pub_enc_key(app as *const App, ud, cb) })
        }),
    };
    task.schedule(f);

    Ok(JsUndefined::new())
}

trait MyReprC {
    type C;

    fn from_c(val: Self::C) -> Self;
}
impl MyReprC for u64 {
    type C = u64;

    fn from_c(val: Self::C) -> Self {
        val
    }
}
impl<T> MyReprC for *mut T {
    type C = *mut T;
    fn from_c(val: Self::C) -> Self {
        val
    }
}

/// Call FFI and wait for callback to pass back value(s)
fn join_cb<F, T>(f: F) -> Result<T, (i32, String)>
where
    F: FnOnce(*mut c_void, extern "C" fn(*mut c_void, *const FfiResult, T::C)),
    T: MyReprC,
{
    let (tx, rx) = std::sync::mpsc::channel::<Result<T, (i32, String)>>();
    let txp = &tx as *const _ as *mut c_void;

    f(txp, cb::<T>);

    rx.recv().unwrap()
}

extern "C" fn cb<T>(user_data: *mut c_void, res: *const FfiResult, o_arg: T::C)
where
    T: MyReprC,
{
    let tx = user_data as *mut mpsc::Sender<Result<T, (i32, String)>>;

    unsafe {
        (*tx).send(match (*res).error_code {
            0 => Ok(T::from_c(o_arg)),
            _ => Err((
                (*res).error_code,
                String::from(CStr::from_ptr((*res).description).to_str().unwrap()),
            )),
        });
    }
}

fn test_create_app_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app_id = cx.argument::<JsString>(0)?.value();
    let app_id = CString::new(app_id.clone()).expect("CString::new failed");

    let f = cx.argument::<JsFunction>(1).unwrap();

    let task = SafeTask::<*mut App> {
        f: Box::new(move || join_cb(|ud, cb| unsafe { test_create_app(app_id.as_ptr(), ud, cb) })),
    };
    task.schedule(f);

    Ok(JsUndefined::new())
}

/// A type to aid converting between foreign types.
struct NewType<T>(T);

impl From<[u8; mem::size_of::<usize>()]> for NewType<*mut App> {
    fn from(bytes: [u8; mem::size_of::<usize>()]) -> Self {
        NewType(usize::from_ne_bytes(bytes) as *mut App)
    }
}

impl From<*mut App> for NewType<[u8; mem::size_of::<usize>()]> {
    fn from(app: *mut App) -> Self {
        NewType((app as usize).to_ne_bytes())
    }
}

impl<'a> From<(&mut TaskContext<'a>, [u8; mem::size_of::<usize>()])>
    for NewType<Handle<'a, JsArrayBuffer>>
{
    fn from(thing: (&mut TaskContext<'a>, [u8; mem::size_of::<usize>()])) -> Self {
        let mut b = JsArrayBuffer::new(thing.0, mem::size_of::<usize>() as u32).unwrap();

        thing.0.borrow_mut(&mut b, |data| {
            data.as_mut_slice::<u8>().clone_from_slice(&thing.1);
        });

        NewType(b)
    }
}

struct SafeTask<T> {
    f: Box<Fn() -> Result<T, (i32, String)> + Send + 'static>,
}

enum MyResult {
    Pointer(usize),
    U64([u8; 8]),
}
impl From<*mut App> for MyResult {
    fn from(app: *mut App) -> Self {
        MyResult::Pointer(app as usize)
    }
}
impl From<u64> for MyResult {
    fn from(u: u64) -> Self {
        MyResult::U64(u.to_ne_bytes())
    }
}

impl<T: Into<MyResult> + 'static> Task for SafeTask<T> {
    type Output = MyResult;
    type Error = (i32, String);
    type JsEvent = JsArrayBuffer;

    fn perform(&self) -> Result<Self::Output, Self::Error> {
        let app = (self.f)();

        app.map(|v| v.into())
    }

    fn complete<'a>(
        self,
        mut cx: TaskContext<'a>,
        result: Result<Self::Output, Self::Error>,
    ) -> JsResult<Self::JsEvent> {
        match result {
            Ok(app) => match app {
                MyResult::Pointer(ptr) => Ok(NewType::from((&mut cx, ptr.to_ne_bytes())).0),
                MyResult::U64(u) => Ok(NewType::from((&mut cx, u)).0),
            },
            Err(err) => {
                let js_err = cx.error(err.1).unwrap();

                // Add an `error_code` property to Error
                let code = cx.number(err.0);
                js_err.set(&mut cx, "error_code", code).unwrap();

                cx.throw(js_err)
            }
        }
    }
}

register_module!(mut cx, {
    cx.export_function("app_is_mock", app_is_mock_js)?;
    cx.export_function("app_pub_enc_key", app_pub_enc_key_js)?;
    cx.export_function("test_create_app", test_create_app_js)?;

    Ok(())
});
