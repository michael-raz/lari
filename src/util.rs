macro_rules! read_le {
	($src:expr, $t:ty) => {{
		let mut buf = [0u8; std::mem::size_of::<$t>()];
		$src.read_exact(&mut buf)?;

		<$t>::from_le_bytes(buf)
	}};
}
pub(crate) use read_le;

/// Returns `log2(num)` rounded up.
///
/// Note that if `num` is zero, then this function will return zero.
pub(crate) fn log2_ceil(num: u64) -> u32 {
	let mut m = num.ilog2();
	if (1u64.unbounded_shl(m) ^ num) != 0 {
		m += 1;
	};

	return m;
}

use std::ops::{Shl, Shr};
pub(crate) fn shift<T: Shl<Output=T> + Shr<Output=T> + From<i8>>(num: T, shift_by: i8) -> T {
	if shift_by > 0 {
		return num << shift_by.into();
	} else if shift_by < 0 {
		return num >> (-shift_by).into();
	} else {
		return num;
	}
}

use std::io::{self, Read, Write, BufRead};
pub(crate) struct BitWriter<T: Write> {
	inner: T,
	buffer: u8,
	bit_offset: u8,
}
impl<T: Write> BitWriter<T> {
	pub fn new(src: T) -> Self {
		Self {
			inner: src,
			buffer: 0,
			bit_offset: 0,
		}
	}

	pub fn into_inner(mut self) -> io::Result<T> {
		if self.bit_offset > 0 {
			self.flush()?;
		}

		return Ok(self.inner);
	}

	pub fn write(&mut self, value: bool) -> io::Result<()> {
		if value {
			self.buffer |= 1 << self.bit_offset;
		}

		self.bit_offset += 1;
		if self.bit_offset == 8 {
			self.flush()?;
		}

		return Ok(());
	}

	fn flush(&mut self) -> io::Result<()> {
		self.inner.write_all(&[self.buffer])?;
		self.buffer = 0;
		self.bit_offset = 0;

		return Ok(());
	}

}



struct BitIter<T> {
	inner: T,
	buffer: u8,
	bit_offset: u8,
}
impl<T: BufRead> Iterator for BitIter<T> {
	type Item = io::Result<bool>;
	fn next(&mut self) -> Option<Self::Item> {
		if self.bit_offset == 0 {
			let buf = self.inner.fill_buf();
			match buf {
				Err(err) => return Some(Err(err)),
				Ok(buf) => {
					if buf.is_empty() {
						return None;
					}

					self.buffer = *buf.first().unwrap();
					self.inner.consume(1);
				},
			}
		}

		let out = (self.buffer.unbounded_shr(self.bit_offset as u32) & 1) > 0;
		self.bit_offset = (self.bit_offset + 1) % 8;

		return Some(Ok(out));
	}
}
pub(crate) fn read_bits(src: impl BufRead) -> impl Iterator<Item=io::Result<bool>> {
	BitIter {
		inner: src,
		buffer: 0,
		bit_offset: 0,
	}
}
