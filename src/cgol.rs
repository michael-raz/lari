//! # Rules
//! Let `n` be the number of live neighbors.
//! 1. If alive and `n != 2 && n != 3` then die; otherwise remain alive
//! 2. If dead and `n == 3` then become alive



use std::collections::HashSet;
use std::io::{self, Read, Write};

pub use crate::vec2::*;
use crate::qtree::*;


/// A grid where Conway's Game of Life is played.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Grid {
	cells: HashSet<Vec2>,
}

impl Grid {
	/// Creates an empty grid.
	pub fn new() -> Self {
		Default::default()
	}

	/// Step the simulation by `count` steps.
	pub fn step(&mut self, count: usize) {
		let offsets: [(i64, i64); 8] = [
			(-1, -1), ( 0, -1), ( 1, -1),
			(-1,  0),           ( 1,  0),
			(-1,  1), ( 0,  1), ( 1,  1),
		];

		let mut visited = HashSet::new();
		let mut queue: Vec<Vec2> = vec![];
		let mut scratch = HashSet::new();

		for _ in 0..count {
			visited.reserve(self.cells.len() * 8);
			queue.reserve(self.cells.len() * 8);
			scratch.reserve(self.cells.len() * 8);

			visited.clear();
			queue.clear();
			scratch.clear();

			queue.extend(self.cells.iter());
			visited.extend(queue.iter());

			while let Some(cur) = queue.pop() {
				let alive = self.cells.contains(&cur);
				let neighbors = offsets.iter()
					.map(|o| cur.add(Vec2::from(o)));

				let mut acount = 0;
				for n in neighbors {
					if self.cells.contains(&n) {
						acount += 1;
					}

					if alive && !visited.contains(&n) {
						queue.push(n);
						visited.insert(n);
					}
				}

				if acount == 3 || (alive && acount == 2) {
					scratch.insert(cur);
				}
			}

			std::mem::swap(&mut self.cells, &mut scratch);
		}
	}

	/// Returns true if the cell at `pos` is alive.
	pub fn get_cell(&self, pos: &Vec2) -> bool {
		self.cells.contains(pos)
	}

	/// Get the positions of cells that are alive.
	pub fn get_alive(&self) -> impl Iterator<Item=&'_ Vec2> {
		self.cells.iter()
	}

	/// Set the value at cell `pos` to alive if `value` is true.
	pub fn set_cell(&mut self, pos: Vec2, value: bool) {
		if value {
			self.cells.insert(pos);
		} else {
			self.cells.remove(&pos);
		}
	}

	/// Get the bound box of the grid.
	pub fn get_bounding_box(&self) -> (Vec2, Vec2) {
		if self.cells.len() <= 0 {
			return ((0, 0).into(), (0, 0).into())
		}

		let mut x_min = i64::MAX;
		let mut x_max = i64::MIN;
		let mut y_min = i64::MAX;
		let mut y_max = i64::MIN;
		for pos in self.cells.iter() {
			x_min = x_min.min(pos.x);
			x_max = x_max.max(pos.x);
			y_min = y_min.min(pos.y);
			y_max = y_max.max(pos.y);
		}

		return (
			(x_min, y_min).into(),
			(x_max, y_max).into(),
		);
	}

	/// Initialize a grid using a table of 1's and 0's.
	pub fn from_bits<const N: usize>(src: &[[u8; N]]) -> Self {
		let alive = src.into_iter().flat_map(|row| row.into_iter()).enumerate()
			.filter_map(|(i, &x)| (x > 0).then_some(((i % N) as i64, (i / N) as i64)));

		alive.fold(Default::default(), |mut out, (x, y)| {
			out.set_cell((x, y).into(), true);
			out
		})
	}

	/// Writes the state of this grid into `out`.
	pub fn save(&self, out: &mut impl Write) -> io::Result<()> {
		let mut q = Qtree::new();
		for &p in self.get_alive() {
			q.insert(p, ());
		}
		q.save(out)
	}

	/// Reads from `src` to create a [Grid].
	pub fn load(src: &mut impl Read) -> io::Result<Self> {
		let q = Qtree::load(src)?.into_iter().map(|(p, _)| p);

		return Ok(Self::from_iter(q));
	}
}

impl IntoIterator for Grid {
	type Item = Vec2;
	type IntoIter = <HashSet<Self::Item> as IntoIterator>::IntoIter;
	fn into_iter(self) -> Self::IntoIter {
		self.cells.into_iter()
	}
}
impl FromIterator<Vec2> for Grid {
	fn from_iter<T: IntoIterator<Item=Vec2>>(iter: T) -> Self {
		Self{cells: HashSet::from_iter(iter)}
	}
}


/// A module for the
/// [Run Length Encoded][https://conwaylife.com/wiki/Run_Length_Encoded] (RLE)
/// format.
///
/// RLE is a common format for storing Conway's Game of Life patterns.
pub mod rle {
	use std::io::{self, Read, Write};
	use std::str::FromStr;

	use crate::cgol::*;

	/// Create a [Grid] from the
	/// [RLE][https://conwaylife.com/wiki/Run_Length_Encoded] format.
	pub fn to_grid(src: &mut impl Read) -> io::Result<Grid> {
		use regex::Regex;

		// TODO: don't load everything into memory at once
		let mut data = String::new();
		src.read_to_string(&mut data)?;

		// skip comments
		let re = Regex::new(r"(?m)(^#[^\n]*\n)*").unwrap();
		let mut off = re.find(&data).ok_or(io::ErrorKind::InvalidData)?.range().end;
		let data = &data[off..];
		let end = data.find('\n').ok_or(io::ErrorKind::InvalidData)?;

		use std::collections::HashMap;

		// read flags
		let re = Regex::new(r"\s*([^\s]+)\s*=\s*([^\s]+)\s*,?").unwrap();
		let mut map = HashMap::<&str, &str>::new();
		for (_, [key, value]) in re.captures_iter(&data[..end]).map(|c| c.extract()) {
			if map.insert(key, value).is_some() {
				// duplicate key
				return Err(io::ErrorKind::InvalidData.into());
			}
		}

		// let width = map.get("x").ok_or(io::ErrorKind::InvalidData)?;
		// let height = map.get("y").ok_or(io::ErrorKind::InvalidData)?;
		off = end + 1;
		if off >= data.len() {
			return Err(io::ErrorKind::UnexpectedEof.into());
		}

		// actual data
		let mut pos = vec2![0, 0];
		let mut out = Grid::new();
		let re = Regex::new(r"(\d*)(b|o|\$|!)").unwrap();
		for (_, [count, value]) in re.captures_iter(&data[off..]).map(|c| c.extract()) {
			let count = if count.len() > 0 {
				u64::from_str(count).unwrap()
			} else {
				1
			};

			match value {
				"o" | "b" => {
					for _ in 0..count {
						out.set_cell(pos, value == "o");
						pos.x += 1
					}
				}
				"$" => {
					pos.x = 0;
					pos.y += count as i64;
				}
				"!" => {
					return Ok(out);
				}
				_ => unreachable!()
			}

		}

		return Err(io::ErrorKind::InvalidData.into());
	}

	/// Save a [Grid] to the
	/// [RLE][https://conwaylife.com/wiki/Run_Length_Encoded] format.
	pub fn from_grid(grid: &Grid, out: &mut impl Write) -> io::Result<usize> {
		let aabb = grid.get_bounding_box();
		let width = aabb.1.x - aabb.0.x;
		let height = aabb.1.y - aabb.0.y;

		let mut cells = Vec::from_iter(grid.cells.iter().map(|&p| p - aabb.0));
		cells.sort_unstable_by(|a, b| (a.y, a.x).cmp(&(b.y, b.x)));

		let mut count = 0;

		let header = format!("x = {width}, y = {height}, rule = B3/S23\n");
		out.write_all(header.as_bytes())?;
		count += header.len();

		fn put_tag(out: &mut String, run: u64, tag: &'static str) {
			if run == 1 {
				out.push_str(tag);
			} else if run > 1 {
				out.push_str(&format!("{run}{tag}"));
			}
		}

		let mut iter = cells.into_iter().peekable();
		let mut cur = vec2![-1, 0];
		let mut entry = String::new();
		let mut line_count = 0;
		while let Some(mut pos) = iter.next() {
			entry.clear();

			let diff = pos - cur;

			let x_diff = if diff.y > 0 {
				pos.x
			} else {
				diff.x - 1
			};

			put_tag(&mut entry, diff.y as u64, "$");
			put_tag(&mut entry, x_diff as u64, "b");

			let mut a_count = 1;
			while let Some(p) = iter.next_if(|p| p.x - pos.x == 1 && p.y == pos.y) {
				pos = p;
				a_count += 1;
			}
			put_tag(&mut entry, a_count, "o");

			if line_count + entry.len() > 70 {
				line_count = 0;

				let nl = "\n";
				out.write_all(nl.as_bytes())?;
				count += nl.len();
			}

			cur = pos;
			out.write_all(entry.as_bytes())?;
			count += entry.len();
		}
		let trailer = "!";
		out.write_all(trailer.as_bytes())?;
		count += trailer.len();

		return Ok(count);
	}



	#[test]
	fn rle_save_load() {
		fn assert_match(grid: Grid) {
			let bb = grid.get_bounding_box();
			let grid = Grid::from_iter(grid.into_iter().map(|p| p - bb.0));

			let mut raw = vec![];
			let len = rle::from_grid(&grid, &mut raw).unwrap();
			assert_eq!(raw.len(), len);

			let out = rle::to_grid(&mut io::Cursor::new(raw)).unwrap();

			assert_eq!(grid, out);
		}

		gen_test_grids().for_each(|mut g| {
			assert_match(g.clone());
			g.step(100);
			assert_match(g);
		});
	}
}



#[cfg(test)]
pub(crate) fn gen_test_grids() -> impl Iterator<Item=Grid> {
	vec![
		// empty
		Grid::new(),

		// one cell
		Grid::from_bits(&[
			[1],
		]),

		// block (stable)
		Grid::from_bits(&[
			[1, 1],
			[1, 1],
		]),

		// tub (stable)
		Grid::from_bits(&[
			[0, 1, 0],
			[1, 0, 1],
			[0, 1, 0],
		]),

		// blinker (period of 2)
		Grid::from_bits(&[
			[0, 1, 0],
			[0, 1, 0],
			[0, 1, 0],
		]),

		// glider
		Grid::from_bits(&[
			[0, 0, 1],
			[1, 0, 1],
			[0, 1, 1],
		]),

		// acorn
		Grid::from_bits(&[
			[0, 1, 0, 0, 0, 0 ,0],
			[0, 0, 0, 1, 0, 0 ,0],
			[1, 1, 0, 0, 1, 1 ,1],
		]),
	].into_iter()
}



#[test]
fn save_load() {
	fn assert_match(grid: Grid) {
		let mut raw = vec![];
		grid.save(&mut raw).unwrap();

		let out = Grid::load(&mut io::Cursor::new(raw)).unwrap();

		assert_eq!(grid, out);
	}

	gen_test_grids().for_each(|mut g| {
		assert_match(g.clone());
		g.step(100);
		assert_match(g);
	});
}


#[test]
fn blinker() {
	let init = Grid::from_bits(&[
		[0, 0, 0],
		[1, 1, 1],
		[0, 0, 0],
	]);
	let other = Grid::from_bits(&[
		[0, 1, 0],
		[0, 1, 0],
		[0, 1, 0],
	]);

	let mut grid = init.clone();

	assert_eq!(grid, init);

	grid.step(1);
	assert_eq!(grid, other);

	grid.step(1);
	assert_eq!(grid, init);
}
