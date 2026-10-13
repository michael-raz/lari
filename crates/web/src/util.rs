use flate2::{Compression, read::{ZlibEncoder, ZlibDecoder}};
use base64::prelude::*;
use base64::DecodeError;
use std::borrow::Cow;
use std::io::{self, Read};

#[derive(Debug)]
pub(crate) enum ZBase64Error {
	Zlib(io::Error),
	Base64(DecodeError),
}
pub(crate) fn bytes_to_text(src: impl AsRef<[u8]>) -> String {
	let mut bytes = vec![];
	ZlibEncoder::new(&mut src.as_ref(), Compression::best())
		.read_to_end(&mut bytes).unwrap();

	let out = BASE64_STANDARD.encode(bytes);
	return out;
}
pub(crate) fn text_to_bytes<'a>(src: impl Into<Cow<'a, str>>) -> Result<Vec<u8>, ZBase64Error> {
	let bytes = BASE64_STANDARD.decode(src.into().as_ref())
		.map_err(|e| ZBase64Error::Base64(e))?;

	let mut out = vec![];
	ZlibDecoder::new(&mut bytes.as_slice())
		.read_to_end(&mut out)
		.map_err(|e| ZBase64Error::Zlib(e))?;

	return Ok(out);
}
