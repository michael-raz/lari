cfg_select! {
	target_family="wasm" => {
		mod wasm_helpers;
		mod dom;
		mod render;
	}
	_ => {}
}

#[cfg(target_family="wasm")]
fn main() {
	use crate::wasm_helpers::{println, eprintln};

	std::panic::set_hook(Box::new(|pinfo| {
		let mut out = String::new();
		if let Some(loc) = pinfo.location() {
			out += format!(
				"panicked at {}:{}:{}:\n",
				loc.file(), loc.line(), loc.column(),
			).as_str();
		} else {
			out += "panicked at an unknown location\n";
		}

		if let Some(msg) = pinfo.payload_as_str() {
			out += msg;
			out += "\n";
		}

		eprintln!("{}", out.trim_end());
	}));

	crate::render::run();
}

pub(crate) mod vec2;
pub(crate) mod util;
pub(crate) mod qtree;

mod cgol;
pub use cgol::*;

#[cfg(not(target_family="wasm"))]
fn main() {
	todo!();
}
