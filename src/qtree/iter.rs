use super::*;

pub struct Qiter<T> {
	src: Qtree<T>,
	stack: Vec<Dir>,
}
impl<T> Qiter<T> {
	fn get(&self) -> &'_ QtreeNode<T> {
		let mut out = &self.src.root;

		for &d in self.stack.iter() {
			let children = out.children();
			out = children.get(d).as_ref().unwrap();
		}

		return out;
	}

	fn get_mut(&mut self) -> &'_ mut QtreeNode<T> {
		let mut out = &mut self.src.root;

		for &d in self.stack.iter() {
			let children = out.children_mut();
			out = children.get_mut(d).as_mut().unwrap();
		}

		return out;
	}

	fn pop(&mut self) -> QtreeNode<T> {
		let dir = self.stack.pop().unwrap();
		let parent: &mut QtreeNode<_> = self.get_mut();
		parent.children_mut().get_mut(dir).take().map(|x| *x).unwrap()
	}

	fn set_stack(&mut self) -> bool {
		// the stack is already set
		if self.get().is_leaf() {
			return true;
		}

		// set the stack such that
		// it's referencing a leaf node
		// (via depth-first search)
		loop {
			let cur: &QtreeNode<_> = self.get();
			let children: &Children<_> = cur.children();
			let next_dir = Dir::ALL
				.into_iter()
				.map(|d| (d, children.get(d)))
				.filter(|(_, c)| c.is_some())
				.next();

			if let Some((d, Some(c))) = next_dir {
				let is_leaf = c.is_leaf();
				self.stack.push(d);

				if is_leaf {
					return true;
				}
			} else {
				// the stack is referencing a parent with no children

				// if the stack is referencing the root node
				// then self.src _must_ be empty; so we give up
				if self.stack.is_empty() {
					return false;
				}

				// remove the childless parent
				self.pop();
			}
		}
	}

	fn new(src: Qtree<T>) -> Self {
		let mut out = Self{src, stack: vec![]};

		// try to initialize the stack
		out.set_stack();

		return out;
	}
}
impl<T> Iterator for Qiter<T> {
	type Item = (Vec2, T);

	fn next(&mut self) -> Option<Self::Item> {
		if !self.set_stack() {
			return None;
		}
		let pos = Qtree::<T>::to_pos(&self.stack);
		return Some((pos, self.pop().into_value()));
	}
}
impl<T> IntoIterator for Qtree<T> {
	type Item = (Vec2, T);
	type IntoIter = Qiter<T>;

	fn into_iter(self) -> Self::IntoIter {
		Qiter::new(self)
	}
}



#[test]
fn qiter() {
	use std::collections::HashMap;

	macro_rules! assert_iter_eq {
		($points:expr) => {{
			let mut counter = 0;
			let (q, verify) = qfrom($points.into_iter().map(|p| p.into()), |_| {
				let out = counter;
				counter += 1;
				out
			});

			let out = HashMap::from_iter(q.into_iter());
			assert_eq!(out, verify);
		}};
	}

	assert_iter_eq!(Vec::<Vec2>::new());
	assert_iter_eq!([( 0,  0)]);
	assert_iter_eq!([(-1,  0)]);
	assert_iter_eq!([( 0, -1)]);
	assert_iter_eq!([(-1, -1)]);

	assert_iter_eq!([( 0,  0), ( 0, -1)]);
	assert_iter_eq!([( 0,  0), (-1,  0), ( 0, -1), (-1, -1)]);


	use rand::random;
	let points = std::iter::from_fn(|| Some((
		random::<i64>(),
		random::<i64>(),
	))).take(256);
	assert_iter_eq!(points);
}
