extern crate ffi_utils;
#[macro_use]
extern crate neon;
extern crate safe_app;
extern crate safe_core;

use ffi_utils::FfiResult;
use neon::prelude::*;
use safe_app::ffi::crypto::app_pub_enc_key;
use safe_app::ffi::crypto::enc_pub_key_get;
use safe_app::ffi::test_utils::test_create_app;
use safe_app::App;
use std::ffi::{CStr, CString};
use std::os::raw::c_void;
use std::sync::mpsc;

fn test_create_app_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app = Wrapper::<CString>::from((&mut cx, 0));
    let jsf = cx.argument::<JsFunction>(1)?;

    SafeTask(Box::new(move || {
        join_cb(|ud, cb| unsafe { test_create_app(app.0.as_ptr(), ud, cb) })
    }))
    .schedule(jsf);

    Ok(JsUndefined::new())
}

fn app_is_mock_js(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    Ok(cx.boolean(safe_app::app_is_mock()))
}

fn app_pub_enc_key_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app = Wrapper::<*const App>::from((&mut cx, 0));
    let jsf = cx.argument::<JsFunction>(1)?;

    SafeTask(Box::new(move || {
        join_cb(|ud, cb| unsafe { app_pub_enc_key(app.0, ud, cb) })
    }))
    .schedule(jsf);

    Ok(JsUndefined::new())
}

fn enc_pub_key_get_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app = Wrapper::<*const App>::from((&mut cx, 0));
    let key = Wrapper::<u64>::from((&mut cx, 1));
    let jsf = cx.argument::<JsFunction>(2)?;

    SafeTask(Box::new(move || {
        join_cb(|ud, cb| unsafe { enc_pub_key_get(app.0, key.0, ud, cb) })
    }))
    .schedule(jsf);

    Ok(JsUndefined::new())
}

/// Call FFI and wait for callback to pass back value(s)
fn join_cb<F, T>(f: F) -> Result<T::Primitive, (i32, String)>
where
    F: FnOnce(*mut c_void, extern "C" fn(*mut c_void, *const FfiResult, T)),
    T: RawToPrimitive,
{
    let (tx, rx) = std::sync::mpsc::channel::<Result<T::Primitive, (i32, String)>>();
    let txp = &tx as *const _ as *mut c_void;

    f(txp, cb::<T>);

    rx.recv().unwrap()
}

extern "C" fn cb<T>(user_data: *mut c_void, res: *const FfiResult, o_arg: T)
where
    T: RawToPrimitive,
{
    let tx = user_data as *mut mpsc::Sender<Result<T::Primitive, (i32, String)>>;

    unsafe {
        (*tx)
            .send(match (*res).error_code {
                0 => Ok(o_arg.to_rust()),
                _ => Err((
                    (*res).error_code,
                    String::from(CStr::from_ptr((*res).description).to_str().unwrap()),
                )),
            })
            .unwrap();
    }
}

impl<'a> From<(&mut FunctionContext<'a>, i32)> for Wrapper<*const App> {
    fn from(ci: (&mut FunctionContext<'a>, i32)) -> Wrapper<*const App> {
        let app = ci.0.argument::<JsArrayBuffer>(ci.1).unwrap();
        let app = ci.0.borrow(&app, |b| b.as_slice::<u8>());

        let mut x = [0u8; std::mem::size_of::<usize>()];
        x.copy_from_slice(app);

        Wrapper(usize::from_ne_bytes(x) as *const App)
    }
}

impl<'a> From<(&mut FunctionContext<'a>, i32)> for Wrapper<u64> {
    fn from(ci: (&mut FunctionContext<'a>, i32)) -> Wrapper<u64> {
        let app = ci.0.argument::<JsArrayBuffer>(ci.1).unwrap();
        let app = ci.0.borrow(&app, |b| b.as_slice::<u8>());

        let mut x = [0u8; std::mem::size_of::<u64>()];
        x.copy_from_slice(app);

        Wrapper(u64::from_ne_bytes(x))
    }
}

impl<'a> From<(&mut FunctionContext<'a>, i32)> for Wrapper<CString> {
    fn from(ci: (&mut FunctionContext<'a>, i32)) -> Wrapper<CString> {
        let s = ci.0.argument::<JsString>(ci.1).unwrap().value();
        let s = CString::new(s.clone()).expect("CString::new failed");

        Wrapper(s)
    }
}

struct SafeTask<T>(Box<Fn() -> Result<T, (i32, String)> + Send>);

impl<T: PrimitiveToJs + 'static> Task for SafeTask<T> {
    type Output = Wrapper<T>;
    type Error = (i32, String);
    type JsEvent = T::Js;

    fn perform(&self) -> Result<Self::Output, Self::Error> {
        // Call the blocking closure
        let result = (self.0)();

        // Wrap value to allow sending !Send types (e.g. pointers)
        result.map(|v| Wrapper(v))
    }

    fn complete<'a>(
        self,
        mut cx: TaskContext<'a>,
        result: Result<Self::Output, Self::Error>,
    ) -> JsResult<Self::JsEvent> {
        match result {
            Ok(app) => Ok(app.0.to_js(&mut cx)),
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

trait RawToPrimitive {
    type Primitive;

    fn to_rust(self) -> Self::Primitive;
}
impl RawToPrimitive for u64 {
    type Primitive = u64;

    fn to_rust(self) -> Self::Primitive {
        self
    }
}
impl<T> RawToPrimitive for *mut T {
    type Primitive = *mut T;

    fn to_rust(self) -> Self::Primitive {
        self
    }
}
impl RawToPrimitive for *const [u8; 32] {
    type Primitive = [u8; 32];

    fn to_rust(self) -> Self::Primitive {
        (unsafe { *self }) as [u8; 32]
    }
}

struct Wrapper<T>(T);
unsafe impl<T> Send for Wrapper<T> {}

trait PrimitiveToJs {
    type Js: Value;

    fn to_js<'a>(self, c: &mut TaskContext<'a>) -> Handle<'a, Self::Js>;

    /// Helper function for converting slices to ArrayBuffer
    fn slice_to_array<'a>(c: &mut TaskContext<'a>, s: &[u8]) -> Handle<'a, JsArrayBuffer> {
        let x = JsArrayBuffer::new(c, s.len() as u32);
        let mut x = x.unwrap();
        c.borrow_mut(&mut x, |data| {
            data.as_mut_slice::<u8>().clone_from_slice(s);
        });

        x
    }
}
impl PrimitiveToJs for u64 {
    type Js = JsArrayBuffer;

    fn to_js<'a>(self, c: &mut TaskContext<'a>) -> Handle<'a, Self::Js> {
        let bytes = self.to_ne_bytes();
        Self::slice_to_array(c, &bytes)
    }
}

impl<T> PrimitiveToJs for *mut T {
    type Js = JsArrayBuffer;

    fn to_js<'a>(self, c: &mut TaskContext<'a>) -> Handle<'a, Self::Js> {
        let bytes = (self as usize).to_ne_bytes();
        Self::slice_to_array(c, &bytes)
    }
}

impl PrimitiveToJs for [u8; 32] {
    type Js = JsArrayBuffer;

    fn to_js<'a>(self, c: &mut TaskContext<'a>) -> Handle<'a, Self::Js> {
        Self::slice_to_array(c, &self)
    }
}

register_module!(mut cx, {
    cx.export_function("app_is_mock", app_is_mock_js)?;
    cx.export_function("app_pub_enc_key", app_pub_enc_key_js)?;
    cx.export_function("enc_pub_key_get", enc_pub_key_get_js)?;
    cx.export_function("test_create_app", test_create_app_js)?;

    Ok(())
});
