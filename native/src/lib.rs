extern crate ffi_utils;
#[macro_use]
extern crate neon;
extern crate safe_app;
extern crate safe_core;

use ffi_utils::test_utils::call_1;
use neon::prelude::*;
use safe_app::ffi::app_container_name;
use safe_app::ffi::crypto::app_pub_enc_key;
use safe_app::ffi::crypto::enc_pub_key_get;
use safe_app::ffi::object_cache::EncryptPubKeyHandle;
use safe_app::ffi::test_utils::test_create_app;
use safe_app::App;
use std::ffi::CString;

fn test_create_app_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app = Wrapper::<CString>::from((&mut cx, 0));
    let jsf = cx.argument::<JsFunction>(1)?;

    SafeTask(Box::new(move || unsafe {
        call_1::<_, _, *mut App>(|ud, cb| test_create_app(app.as_ptr(), ud, cb))
    }))
    .schedule(jsf);

    Ok(JsUndefined::new())
}

fn app_is_mock_js(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    Ok(cx.boolean(safe_app::app_is_mock()))
}

fn app_container_name_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app = Wrapper::<CString>::from((&mut cx, 0));
    let jsf = cx.argument::<JsFunction>(1)?;

    SafeTask(Box::new(move || unsafe {
        call_1::<_, _, String>(|ud, cb| app_container_name(app.as_ptr(), ud, cb))
    }))
    .schedule(jsf);

    Ok(JsUndefined::new())
}

fn app_pub_enc_key_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app = Wrapper::<*const App>::from((&mut cx, 0));
    let jsf = cx.argument::<JsFunction>(1)?;

    SafeTask(Box::new(move || unsafe {
        call_1::<_, _, EncryptPubKeyHandle>(|ud, cb| app_pub_enc_key(app.0, ud, cb))
    }))
    .schedule(jsf);

    Ok(JsUndefined::new())
}

fn enc_pub_key_get_js(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let app = Wrapper::<*const App>::from((&mut cx, 0));
    let key = Wrapper::<u64>::from((&mut cx, 1));
    let jsf = cx.argument::<JsFunction>(2)?;

    SafeTask(Box::new(move || unsafe {
        call_1::<_, _, [u8; 32]>(|ud, cb| enc_pub_key_get(app.0, key.0, ud, cb))
    }))
    .schedule(jsf);

    Ok(JsUndefined::new())
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

struct SafeTask<T>(Box<Fn() -> Result<T, i32> + Send>);

impl<T: PrimitiveToJs + 'static> Task for SafeTask<T> {
    type Output = Wrapper<T>;
    type Error = i32;
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
            Ok(res) => Ok(res.0.to_js(&mut cx)),
            Err(err) => {
                let js_err = cx.error("SAFE API ERROR").unwrap();

                // Add an `error_code` property to Error
                let code = cx.number(err);
                js_err.set(&mut cx, "error_code", code).unwrap();

                cx.throw(js_err)
            }
        }
    }
}

struct Wrapper<T>(T);
unsafe impl<T> Send for Wrapper<T> {}

impl<T> std::ops::Deref for Wrapper<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

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

impl PrimitiveToJs for String {
    type Js = JsString;

    fn to_js<'a>(self, c: &mut TaskContext<'a>) -> Handle<'a, Self::Js> {
        JsString::new(c, self)
    }
}

register_module!(mut cx, {
    cx.export_function("app_is_mock", app_is_mock_js)?;
    cx.export_function("app_container_name", app_container_name_js)?;
    cx.export_function("app_pub_enc_key", app_pub_enc_key_js)?;
    cx.export_function("enc_pub_key_get", enc_pub_key_get_js)?;
    cx.export_function("test_create_app", test_create_app_js)?;

    Ok(())
});
