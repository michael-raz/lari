use std::ops::DerefMut;

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

	/// Insert `value` at `xy`.
	///
	/// Replaces the value at `xy`, if it already had one.
	pub fn insert(&mut self, xy: (isize, isize), value: T) {
		let leaf = self.get_owner(xy, true).unwrap();
		let value = QtreeNode::Leaf(value);

		// only allocate a new box if there isn't one already
		if let Some(leaf) = leaf {
			*leaf.deref_mut() = value;
		} else {
			*leaf = Some(Box::new(value));
		}
	}

	/// Gets the value at `xy`.
	pub fn get(&mut self, xy: (isize, isize)) -> Option<&mut T> {
		self.get_owner(xy, false)
			.and_then(|opt| opt.as_mut())
			.map(|b| match b.deref_mut() {
				QtreeNode::Leaf(value) => value,
				_ => unreachable!(),
			})
	}

	/// Removes the value at `xy`.
	pub fn remove(&mut self, xy: (isize, isize)) -> Option<T> {
		self.get_owner(xy, false)
			.and_then(|opt| opt.take())
			.map(|x| match *x {
				QtreeNode::Leaf(value) => value,
				_ => unreachable!(),
			})
	}



	/// Helper function to compute the required height needed to contain the provided dimension.
	fn bounding_height(num: isize) -> u8 {
		let positive = if num < 0 {
			num.abs()
		} else {
			num + 1
		};

		return log2_ceil(positive as u64) as u8 + 1;
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

	/// Helper method that gives a list of directions to arrive at `xy`.
	///
	/// Note that this method will increase the height (if needed) such that
	/// `self` is large enough to contain `xy`.
	fn coord_chain(&mut self, mut xy: (isize, isize)) -> Vec<Dir> {
		let upper = Self::bounding_height(xy.0).max(Self::bounding_height(xy.1));

		// grow height to be less than or equal to upper
		for _ in self.height..upper {
			self.grow();
		}

		let mut out = vec![];
		for i in 0..self.height {
			let bot  = xy.1 < 0;
			let left = xy.0 < 0;

			let r = shift(1isize, self.height as i8 - i as i8 - 2);

			if left {
				xy.0 += r;
			} else {
				xy.0 -= r;
			}

			if bot {
				xy.1 += r;
			} else {
				xy.1 -= r;
			}

			let dir = match (bot, left) {
				(true,  true)  => Dir::BotLeft,
				(true,  false) => Dir::BotRight,
				(false, true)  => Dir::TopLeft,
				(false, false) => Dir::TopRight,
			};

			out.push(dir);
		}

		return out;
	}

	/// Helper method to return the 'owner' of the value at `xy`.
	///
	/// The `Box<QtreeNode>` will _always_ contain the [Leaf][QtreeNode::Leaf] variant.
	fn get_owner(&mut self, xy: (isize, isize), create: bool) ->
		Option<&'_ mut Option<Box<QtreeNode<T>>>>
	{
		let mut coords = self.coord_chain(xy);

		assert!(self.height as usize >= coords.len());

		let last = coords.pop().unwrap();

		let mut p: &mut QtreeNode<T> = &mut self.root;
		for dir in coords {
			let child = p.get_child(dir);
			if let Some(child) = child {
				p = child;
			} else if create {
				p = child.insert(Box::new(QtreeNode::Parent(Default::default())));
			} else {
				return None;
			}
		}

		let out = p.get_child(last);

		return Some(out);
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



enum QtreeNode<T> {
	Leaf(T),
	Parent(Children<T>)
}
impl<T> QtreeNode<T> {
	/// Helper method to get the child of this parent.
	///
	/// # Panics
	///
	/// Panics if `self` isn't the [Parent][Qtree::Parent] variant.
	fn get_child(&mut self, dir: Dir) -> &'_ mut Option<Box<Self>> {
		let children = match self {
			QtreeNode::Parent(children) => children,
			_ => panic!(),
		};

		match dir {
			Dir::TopRight => &mut children.top_right,
			Dir::TopLeft => &mut children.top_left,
			Dir::BotRight => &mut children.bot_right,
			Dir::BotLeft => &mut children.bot_left,
		}
	}
}

#[derive(Debug, PartialEq, Eq)]
enum Dir {
	TopRight,
	TopLeft,
	BotLeft,
	BotRight,
}

struct Children<T> {
	top_left:  Option<Box<QtreeNode<T>>>,
	top_right: Option<Box<QtreeNode<T>>>,
	bot_left:  Option<Box<QtreeNode<T>>>,
	bot_right: Option<Box<QtreeNode<T>>>,
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



/// Returns `log2(num)` rounded up.
///
/// Note that if `num` is zero, then this function will return zero.
fn log2_ceil(num: u64) -> u32 {
	let mut m = num.ilog2();
	if (1u64.unbounded_shl(m) ^ num) != 0 {
		m += 1;
	};

	return m;
}

use std::ops::{Shl, Shr};
fn shift<T: Shl<Output=T> + Shr<Output=T> + From<i8>>(num: T, shift_by: i8) -> T {
	if shift_by > 0 {
		return num << shift_by.into();
	} else if shift_by < 0 {
		return num >> (-shift_by).into();
	} else {
		return num;
	}
}






#[test]
fn qchain() {
	let mut x = Qtree::<()>::new();

	macro_rules! assert_chain {
		($coords:expr, $chain:expr) => {
			let mut x = Qtree::<()>::new();
			let out = x.coord_chain($coords);
			assert_eq!(out, $chain);
		};
		(h = $h:expr, $coords:expr, $chain:expr) => {
			let mut x = Qtree::<()>::new();

			for _ in 1..$h {
				x.grow();
			}
			assert_eq!(x.height, $h);

			let out = x.coord_chain($coords);
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
fn qtree() {
	let points = [(-1, -1), (0, 0), (0, 2), (1, 1), (2, 2)];

	let mut x = Qtree::new();
	for p in points {
		x.insert(p, ());
	}

	for p in points {
		assert!(x.get(p).is_some());
	}
}

#[test]
fn qtree_set_get() {
	use std::collections::HashMap;
	use rand::random;

	let mut qtree = Qtree::new();
	let mut verify = HashMap::new();

	for i in 0..256 {
		let x = random::<i64>() as isize;
		let y = random::<i64>() as isize;
		let value = i;

		let xy = (x, y);

		verify.insert(xy, value);
		qtree.insert(xy, value);

		for (k, v) in &verify {
			assert_eq!(qtree.get(*k).as_deref(), Some(v));
		}
	}
}
