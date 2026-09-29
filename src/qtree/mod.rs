use std::ops::DerefMut;
use std::io::{self, Read, Write};

pub use crate::vec2::*;
use crate::util::*;

mod iter;
pub use iter::*;

/// The `Qtree` struct is a quadtree container.
///
/// Quadtrees are a tree structure where each node references up to four children.
pub struct Qtree<T> {
	height: u8,
	root: QtreeNode<T>,
}

impl<T> Qtree<T> {
	/// Creates a new empty `Qtree`.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a new empty `Qtree` with a height of `height`.
	pub fn with_height(height: u8) -> Self {
		let mut out = Self::new();
		out.height = height;

		return out;
	}

	/// Insert `value` at `pos`.
	///
	/// Replaces the value at `pos`, if it already had one.
	pub fn insert(&mut self, pos: Vec2, value: T) {
		let coords = self.to_dirs(pos);
		self.insert_by_dirs(coords, value);
	}

	/// Gets the value at `pos`.
	pub fn get(&mut self, pos: Vec2) -> Option<&mut T> {
		let coords = self.to_dirs(pos);
		self.get_owner(coords, false)
			.and_then(|opt| opt.as_mut())
			.map(|b| match b.deref_mut() {
				QtreeNode::Leaf(value) => value,
				_ => unreachable!(),
			})
	}

	/// Removes the value at `pos`.
	pub fn remove(&mut self, pos: Vec2) -> Option<T> {
		let coords = self.to_dirs(pos);
		self.get_owner(coords, false)
			.and_then(|opt| opt.take())
			.map(|x| match *x {
				QtreeNode::Leaf(value) => value,
				_ => unreachable!(),
			})
	}

	/// Get the height of the `Qtree`.
	pub fn get_height(&self) -> u8 {
		self.height
	}

	/// Insert `value` at the location specified by `dirs`.
	///
	/// [insert][Self::insert] is the more common method of insertion.
	///
	/// # Panics
	///
	/// Panics if the count of `dirs` doesn't equal this `Qtree`'s `height`.
	pub fn insert_by_dirs(&mut self, dirs: impl IntoIterator<Item=Dir>, value: T) {
		let coords = dirs.into_iter().collect::<Vec<_>>();
		assert_eq!(coords.len(), self.height as usize);

		let leaf = self.get_owner(coords, true).unwrap();
		let value = QtreeNode::Leaf(value);

		// only allocate a new box if there isn't one already
		if let Some(leaf) = leaf {
			*leaf.deref_mut() = value;
		} else {
			*leaf = Some(Box::new(value));
		}
	}



	/// Helper function to compute the required height needed to contain the provided dimension.
	fn bounding_height(pos: Vec2) -> u8 {
		fn dim(num: i64) -> u8 {
			let positive = if num < 0 {
				num.abs()
			} else {
				num + 1
			};

			return log2_ceil(positive as u64) as u8 + 1;
		}

		dim(pos.x).max(dim(pos.y))
	}

	/// Increase the height by one.
	///
	/// This method also handles all the book-keeping required to increase the height by one.
	fn grow(&mut self) {
		let children = match &mut self.root {
			QtreeNode::Parent(children) => children,
			_ => panic!(),
		};

		if children.top_right.is_some() {
			let mut top_right = Children::default();
			std::mem::swap(&mut children.top_right, &mut top_right.bot_left);
			children.top_right = Some(Box::new(QtreeNode::Parent(top_right)));
		}

		if children.top_left.is_some() {
			let mut top_left = Children::default();
			std::mem::swap(&mut children.top_left, &mut top_left.bot_right);
			children.top_left = Some(Box::new(QtreeNode::Parent(top_left)));
		}

		if children.bot_left.is_some() {
			let mut bot_left = Children::default();
			std::mem::swap(&mut children.bot_left, &mut bot_left.top_right);
			children.bot_left = Some(Box::new(QtreeNode::Parent(bot_left)));
		}

		if children.bot_right.is_some() {
			let mut bot_right = Children::default();
			std::mem::swap(&mut children.bot_right, &mut bot_right.top_left);
			children.bot_right = Some(Box::new(QtreeNode::Parent(bot_right)));
		}

		self.height += 1;
	}

	/// Helper method that gives a list of directions to arrive at `pos`.
	///
	/// Note that this method will increase the height (if needed) such that
	/// `self` is large enough to contain `pos`.
	fn to_dirs(&mut self, mut pos: Vec2) -> Vec<Dir> {
		let upper = Self::bounding_height(pos);

		// grow height to be less than or equal to upper
		for _ in self.height..upper {
			self.grow();
		}

		let mut out = vec![];
		for i in 0..self.height {
			let dir = vec2_to_dir(&pos);

			let r = shift(1i64, self.height as i8 - i as i8 - 2);
			pos += dir_to_vec2(&dir) * -r;

			out.push(dir);
		}

		return out;
	}

	fn to_pos(dirs: &[Dir]) -> Vec2 {
		let mut iter = dirs.into_iter().rev();
		let mut out = (dir_to_vec2(iter.next().unwrap()) - 1) / 2;
		for (i, dir) in iter.enumerate() {
			out += dir_to_vec2(dir) * (1i64 << i);
		}

		return out;
	}

	/// Helper method to return the 'owner' of the value at `pos`.
	///
	/// The `Box<QtreeNode>` will _always_ contain the [Leaf][QtreeNode::Leaf] variant.
	fn get_owner<I>(&mut self, coords: I, create: bool)
		-> Option<&'_ mut Option<Box<QtreeNode<T>>>>
		where
			I: IntoIterator<Item=Dir>,
			<I as IntoIterator>::IntoIter: DoubleEndedIterator,
	{
		let mut coords = coords.into_iter();

		let last = coords.next_back().unwrap();

		let mut p: &mut QtreeNode<T> = &mut self.root;
		for dir in coords {
			let child = p.children_mut().get_mut(dir);
			if let Some(child) = child {
				p = child;
			} else if create {
				p = child.insert(Box::new(QtreeNode::Parent(Default::default())));
			} else {
				return None;
			}
		}

		let out = p.children_mut().get_mut(last);

		return Some(out);
	}
}
impl Qtree<()> {
	pub fn save(&self, out: &mut impl Write) -> io::Result<()> {
		fn write<T>(q: &QtreeNode<T>, out: &mut BitWriter<impl Write>) -> io::Result<()> {
			if q.is_leaf() {
				return Ok(());
			}

			let children = q.children();
			for dir in Dir::ALL {
				let child = children.get(dir);

				out.write(child.is_some())?;
				if let Some(d) = child {
					write(d, out)?;
				}
			}

			return Ok(());
		}


		out.write_all(&(self.height as u64).to_le_bytes())?;

		let mut out = BitWriter::new(out);
		write(&self.root, &mut out)?;

		let out = out.into_inner()?;

		return Ok(());
	}

	pub fn load(src: &mut impl Read) -> io::Result<Self> {
		let height = read_le!(src, u64) as usize;

		// let mut iter = src.bytes().map(|res| res.map(|b| b > 0));
		let src = io::BufReader::new(src);
		let mut iter = read_bits(src);

		let mut stack: Vec<u8> = vec![];
		let mut out = Qtree::<()>::with_height(height as u8);

		let mut counter = 0;
		'main: while let Some(Ok(value)) = iter.next() {
			if value {
				stack.push(counter);
				counter = 0;
			} else {
				counter += 1;
			}

			if stack.len() == height {
				let dirs = stack.iter().map(|&c| &Dir::ALL[c as usize]);
				out.insert_by_dirs(dirs.copied(), ());

				counter = stack.pop().unwrap() + 1;
			}

			while counter as usize == Dir::ALL.len() {
				if stack.is_empty() {
					break 'main;
				}
				counter = stack.pop().unwrap() + 1;
			}
		};

		return Ok(out);
	}
}

impl<T> Default for Qtree<T> {
	fn default() -> Self {
		Self {
			height: 1,
			root: QtreeNode::Parent(Default::default()),
		}
	}
}



pub(crate) enum QtreeNode<T> {
	Leaf(T),
	Parent(Children<T>)
}
impl<T> QtreeNode<T> {
	fn is_parent(&self) -> bool {
		matches!(self, QtreeNode::Parent(..))
	}
	fn is_leaf(&self) -> bool {
		matches!(self, QtreeNode::Leaf(..))
	}

	fn into_value(self) -> T {
		match self {
			QtreeNode::Leaf(value) => value,
			_ => panic!(),
		}
	}

	fn children(&self) -> &Children<T> {
		match self {
			QtreeNode::Parent(children) => children,
			_ => panic!(),
		}
	}
	fn children_mut(&mut self) -> &mut Children<T> {
		match self {
			QtreeNode::Parent(children) => children,
			_ => panic!(),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dir {
	TopRight,
	TopLeft,
	BotLeft,
	BotRight,
}
use Dir::*;
impl Dir{
	const ALL: [Dir; 4] = [
		TopRight,
		TopLeft,
		BotLeft,
		BotRight,
	];
}

struct Children<T> {
	top_left:  Option<Box<QtreeNode<T>>>,
	top_right: Option<Box<QtreeNode<T>>>,
	bot_left:  Option<Box<QtreeNode<T>>>,
	bot_right: Option<Box<QtreeNode<T>>>,
}
impl<T> Children<T> {
	fn get(&self, dir: Dir) -> Option<&QtreeNode<T>> {
		match dir {
			TopLeft  => self.top_left.as_ref().map(|b| b.as_ref()),
			TopRight => self.top_right.as_ref().map(|b| b.as_ref()),
			BotLeft  => self.bot_left.as_ref().map(|b| b.as_ref()),
			BotRight => self.bot_right.as_ref().map(|b| b.as_ref()),
		}
	}

	fn get_mut(&mut self, dir: Dir) -> &mut Option<Box<QtreeNode<T>>> {
		match dir {
			TopLeft  => &mut self.top_left,
			TopRight => &mut self.top_right,
			BotLeft  => &mut self.bot_left,
			BotRight => &mut self.bot_right,
		}
	}
}
impl<T> Default for Children<T> {
	fn default() -> Self {
		Self {
			top_left:  None,
			top_right: None,
			bot_left:  None,
			bot_right: None,
		}
	}
}




fn dir_to_vec2(dir: &Dir) -> Vec2 {
	match dir {
		TopRight => vec2![ 1,  1],
		TopLeft  => vec2![-1,  1],
		BotLeft  => vec2![-1, -1],
		BotRight => vec2![ 1, -1],
	}
}
fn vec2_to_dir(pos: &Vec2) -> Dir {
	let left = pos.x < 0;
	let bot = pos.y < 0;
	match (bot, left) {
		(true,  true)  => BotLeft,
		(true,  false) => BotRight,
		(false, true)  => TopLeft,
		(false, false) => TopRight,
	}
}



#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
fn qfrom<T: Copy>(src: impl IntoIterator<Item=Vec2>, mut f: impl FnMut(Vec2) -> T)
	-> (Qtree<T>, HashMap<Vec2, T>)
{
	let mut verify = HashMap::new();
	let mut q = Qtree::new();
	for p in src.into_iter() {
		let value = f(p);
		verify.insert(p, value);
		q.insert(p, value);
	}

	return (q, verify);
}

#[test]
fn qsave() {
	macro_rules! assert_save_eq {
		($points:expr) => {{
			let (q, verify) = qfrom($points.into_iter().map(|p| p.into()), |_| ());
			dbg!(&verify);

			let mut bytes = vec![];
			q.save(&mut bytes).unwrap();
			dbg!(&bytes);

			let after = Qtree::load(&mut std::io::Cursor::new(&bytes)).unwrap();
			let out = HashMap::from_iter(after.into_iter());

			assert_eq!(out, verify);
		}};
	}

	let empty: Vec<Vec2> = vec![];
	assert_save_eq!(empty);
	assert_save_eq!([( 0,  0)]);
	assert_save_eq!([(-1,  0)]);
	assert_save_eq!([( 0, -1)]);
	assert_save_eq!([(-1, -1)]);

	assert_save_eq!([( 0,  0), ( 0, -1)]);
	assert_save_eq!([( 0,  0), (-1,  0), ( 0, -1), (-1, -1)]);

	assert_save_eq!([( 1,  1)]);
	assert_save_eq!([( 3,  3)]);

	use rand::random;
	let points = std::iter::from_fn(|| Some((
		random::<i64>(),
		random::<i64>(),
	))).take(256);
	assert_save_eq!(points);
}


#[test]
fn qchain() {
	let mut x = Qtree::<()>::new();

	macro_rules! assert_chain {
		($coords:expr, $chain:expr) => {
			let mut x = Qtree::<()>::new();
			let out = x.to_dirs($coords.into());
			assert_eq!(out, $chain);
		};
		(h = $h:expr, $coords:expr, $chain:expr) => {
			let mut x = Qtree::<()>::new();

			for _ in 1..$h {
				x.grow();
			}
			assert_eq!(x.height, $h);

			let out = x.to_dirs($coords.into());
			assert_eq!(out, $chain);
		};
	}

	assert_chain!(( 0,  0), vec![Dir::TopRight]);
	assert_chain!((-1,  0), vec![Dir::TopLeft]);
	assert_chain!((-1, -1), vec![Dir::BotLeft]);
	assert_chain!(( 0, -1), vec![Dir::BotRight]);

	assert_chain!(h=2, ( 0,  0), vec![Dir::TopRight, Dir::BotLeft ]);
	assert_chain!(h=2, (-1,  0), vec![Dir::TopLeft,  Dir::BotRight]);
	assert_chain!(h=2, (-1, -1), vec![Dir::BotLeft,  Dir::TopRight]);
	assert_chain!(h=2, ( 0, -1), vec![Dir::BotRight, Dir::TopLeft ]);

	assert_chain!(h=3, ( 0,  0), vec![Dir::TopRight, Dir::BotLeft,  Dir::BotLeft]);
	assert_chain!(h=3, (-1,  0), vec![Dir::TopLeft,  Dir::BotRight, Dir::BotRight]);
	assert_chain!(h=3, (-1, -1), vec![Dir::BotLeft,  Dir::TopRight, Dir::TopRight]);
	assert_chain!(h=3, ( 0, -1), vec![Dir::BotRight, Dir::TopLeft,  Dir::TopLeft]);
}


#[test]
fn qtree_set_get_basic() {
	let points = [(-1, -1), (0, 0), (0, 2), (1, 1), (2, 2)].into_iter()
		.map(|p| p.into()).collect::<Vec<_>>();

	let mut x = Qtree::new();
	for &p in &points {
		x.insert(p, ());
	}

	for &p in &points {
		assert!(x.get(p).is_some());
	}
}

#[test]
fn qtree_set_get_rand() {
	use std::collections::HashMap;
	use rand::random;

	let mut qtree = Qtree::new();
	let mut verify = HashMap::new();

	for i in 0..256 {
		let x = random::<i64>();
		let y = random::<i64>();
		let value = i;

		let pos = vec2![x, y];

		verify.insert(pos, value);
		qtree.insert(pos, value);

		for (k, v) in &verify {
			assert_eq!(qtree.get(*k).as_deref(), Some(v));
		}
	}
}
