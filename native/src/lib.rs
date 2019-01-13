#[macro_use]
extern crate neon;

use neon::prelude::*;

fn app_is_mock(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    Ok(cx.boolean(safe_app::app_is_mock()))
}

register_module!(mut cx, {
    cx.export_function("app_is_mock", app_is_mock)
});
