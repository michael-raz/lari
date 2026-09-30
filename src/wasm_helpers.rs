pub use wasm_bindgen::prelude::*;
pub use wasm_bindgen::convert::*;
pub use web_sys::*;
pub use js_sys::*;

use std::borrow::Cow;



#[cfg(target_family="wasm")]
macro_rules! println {
	() => {
		console::log_0();
	};
	($($arg:tt)*) => {
		::web_sys::console::log_1(&format!($($arg)*).into())
	};
}
pub(crate) use println;

#[cfg(target_family="wasm")]
macro_rules! eprintln {
	() => {
		console::error_0();
	};
	($($arg:tt)*) => {
		::web_sys::console::error_1(&format!($($arg)*).into());
	};
}
pub(crate) use eprintln;

#[cfg(target_family="wasm")]
macro_rules! dbg {
	() => {
		println!("[{}:{}]", file!(), line!())
	};
	($($x:expr),* $(,)?) => {
		$(
			println!("[{}:{}] {} = {:?}", file!(), line!(), stringify!($x), $x);
		);*
	};
}
pub(crate) use dbg;


pub fn wrap<T: FromWasmAbi, F: FnMut(T) + 'static>(callback: F) -> ScopedClosure<'static, dyn FnMut(T)> {
	Closure::wrap(Box::new(callback) as Box<dyn FnMut(_)>)
}


pub fn set_interval<E, F>(target: &Window, callback: F, rate: i32)
	where
		E: FromWasmAbi,
		F: FnMut(E) + 'static,
{
	let c = wrap(callback);
	target.set_interval_with_callback_and_timeout_and_arguments_0(
		&c.as_ref().unchecked_ref(), rate,
	).unwrap();
	c.forget();
}
pub fn set_timeout<E, F>(target: &Window, callback: F, rate: i32) -> Result<i32, JsValue>
	where
		E: FromWasmAbi,
		F: FnMut(E) + 'static,
{
	let c = wrap(callback);
	let out = target.set_timeout_with_callback_and_timeout_and_arguments_0(
		&c.as_ref().unchecked_ref(), rate,
	)?;
	c.forget();

	return Ok(out);
}


// https://developer.mozilla.org/en-US/docs/Learn_web_development/Extensions/Advanced_JavaScript_objects/Object_prototypes
/// Traverse prototypes to get `key`.
pub fn proto_get(obj: &Object, key: &str) -> Option<JsValue> {
	let root = obj;
	let mut obj = Cow::Borrowed(obj);

	let key: &JsString = &key.into();

	while !obj.is_null() {
		let has = Object::has_own(&obj, key);
		if has {
			let desc = Object::get_own_property_descriptor_str(&obj, key).unwrap();
			let value = desc.get_value();
			if value.is_some() {
				return value;
			}

			let getter = desc.get_get().unwrap();
			let value = getter.call(&root, ()).unwrap();

			return Some(value);
		}

		obj = Cow::Owned(Object::get_prototype_of(&obj));
	}

	return None;
}
/// Traverse prototypes and set `key` to `value`.
pub fn proto_set(obj: &Object, key: &str, value: &JsValue) -> Option<()> {
	let root = obj;
	let mut obj = Cow::Borrowed(obj);

	let key: &JsString = &key.into();

	while !obj.is_null() {
		let has = Object::has_own(&obj, key);
		if has {
			let desc = Object::get_own_property_descriptor_str(&obj, key).unwrap();

			let getter = desc.get_set().unwrap();
			getter.call(&root, (value,)).unwrap();

			return Some(());
		}

		obj = Cow::Owned(Object::get_prototype_of(&obj));
	}

	return None;
}
