pub(crate) mod vec2;
pub(crate) mod util;
pub(crate) mod qtree;

cfg_select! {
	target_family="wasm" => {
		pub(crate) mod wasm_helpers;
		pub(crate) mod dom;
		pub(crate) mod render;
	}
	_ => {}
}

mod cgol;
pub use cgol::*;
