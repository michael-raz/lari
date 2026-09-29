//! # Rules
//! Let `n` be the number of live neighbors.
//! 1. If alive and `n != 2 && n != 3` then die; otherwise remain alive
//! 2. If dead and `n == 3` then become alive



use std::collections::HashSet;
use std::io::{self, Read, Write};

pub use crate::vec2::*;


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
	///
	/// The format is a `u64` (all numbers are in little endian) that denotes the length of the
	/// following array which; contains positions `(i64, i64)` for each living cell.
	pub fn save(&self, out: &mut impl Write) -> io::Result<()> {
		out.write_all(&(self.cells.len() as u64).to_le_bytes())?;
		for pos in self.cells.iter() {
			out.write_all(&(pos.x as i64).to_le_bytes())?;
			out.write_all(&(pos.y as i64).to_le_bytes())?;
		}

		return Ok(());
	}

	/// Reads from `src` to create a [Grid].
	///
	/// See [save][Grid::save] for details on the format.
	pub fn load(src: &mut impl Read) -> io::Result<Self> {
		macro_rules! read_le {
			($t:ty) => {{
				let mut buf = [0u8; std::mem::size_of::<$t>()];
				src.read_exact(&mut buf)?;

				<$t>::from_le_bytes(buf)
			}};
		}

		let count = read_le!(u64);
		let mut out = Grid::new();
		for _ in 0..count {
			let x = read_le!(i64);
			let y = read_le!(i64);

			out.cells.insert((x, y).into());
		}

		return Ok(out);
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



#[cfg(test)]
fn gen_test_grids() -> impl Iterator<Item=Grid> {
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
