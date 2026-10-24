#![allow(unused)]

use std::io::{self, Read, Write};
use std::ops::DerefMut;

use crate::util::*;
pub use crate::vec2::*;

mod path;
pub use path::*;

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

        out
    }

    /// Insert `value` at `pos`.
    ///
    /// Replaces the value at `pos`, if it already had one.
    pub fn insert(&mut self, pos: Vec2, value: T) {
        let upper = Self::bounding_height(pos);

        // grow height to be less than or equal to upper
        for _ in self.height..upper {
            self.grow();
        }
        let coords = self.as_dirs(pos);

        self.insert_by_dirs(&coords, value);
    }

    /// Gets the value at `pos`.
    pub fn get(&self, pos: Vec2) -> Option<&T> {
        if Self::bounding_height(pos) > self.height {
            return None;
        }

        let coords = self.as_dirs(pos);
        self.root.get_owner(&coords).map(|b| match b {
            QtreeNode::Leaf(value) => value,
            _ => unreachable!(),
        })
    }

    /// Removes the value at `pos`.
    pub fn remove(&mut self, pos: Vec2) -> Option<T> {
        let upper = Self::bounding_height(pos);

        // grow height to be less than or equal to upper
        for _ in self.height..upper {
            self.grow();
        }
        let coords = self.as_dirs(pos);

        self.root
            .get_owner_mut(&coords, false)
            .and_then(|opt| opt.take())
            .map(|x| match *x {
                QtreeNode::Leaf(value) => value,
                _ => unreachable!(),
            })
    }

    /// Get the height of the `Qtree`.
    pub fn height(&self) -> u8 {
        self.height
    }

    /// Insert `value` at the location specified by `dirs`.
    ///
    /// [insert][Self::insert] is the more common method of insertion.
    ///
    /// # Panics
    ///
    /// Panics if the count of `dirs` doesn't equal this `Qtree`'s `height`.
    pub fn insert_by_dirs(&mut self, path: &Path, value: T) {
        assert_eq!(path.len(), self.height);

        let leaf = self.root.get_owner_mut(path, true).unwrap();
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
            let positive = if num < 0 { num.abs() } else { num + 1 };

            log2_ceil(positive as u64) as u8 + 1
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
    pub fn as_dirs(&self, mut pos: Vec2) -> Path {
        let mut out = Path::new();
        let range = 0..self.height as i8;
        for i in range.clone() {
            let dir = vec2_to_dir(&pos);

            let r = shift(1i64, range.end - i - 2);
            pos += dir_to_vec2(dir) * -r;

            out.push(dir);
        }

        out
    }

    fn to_pos(path: &Path) -> Vec2 {
        let mut iter = (0..path.len()).rev().map(|i| path.get(i));
        let mut out = (dir_to_vec2(iter.next().unwrap()) - 1) / 2;
        for (i, dir) in iter.enumerate() {
            out += dir_to_vec2(dir) * (1i64 << i);
        }

        out
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

            Ok(())
        }

        out.write_all(&(self.height as u64).to_le_bytes())?;

        let mut out = BitWriter::new(out);
        write(&self.root, &mut out)?;

        let out = out.into_inner()?;

        Ok(())
    }

    pub fn load(src: &mut impl Read) -> io::Result<Self> {
        let height = read_le!(src, u64) as usize;

        let src = io::BufReader::new(src);
        let mut iter = read_bits(src);

        let mut stack: Path = qpath![];
        let mut out = Qtree::<()>::with_height(height as u8);

        let mut counter = Dir::default();
        'main: while let Some(Ok(value)) = iter.next() {
            let mut flag = true;
            if value {
                stack.push(counter);
                counter = Dir::default();
                flag = false
            }

            if stack.len() == height as u8 {
                out.insert_by_dirs(&stack, ());

                counter = stack.pop().unwrap();
                flag = true;
            }

            if !flag {
                continue;
            }

            let mut next = counter.inc();
            while next.is_none() {
                if stack.is_empty() {
                    break 'main;
                }
                next = stack.pop().unwrap().inc();
            }
            counter = next.unwrap()
        }

        Ok(out)
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
    Parent(Children<T>),
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
    fn as_value(&self) -> &T {
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

    /// Helper method to return the 'owner' of the value at `pos`.
    ///
    /// The `QtreeNode` will _always_ be the [Leaf][QtreeNode::Leaf] variant.
    pub fn get_owner(&self, path: &Path) -> Option<&'_ QtreeNode<T>> {
        if path.len() > 0 {
            let mut p: &QtreeNode<T> = self;
            for dir in path.iter().take((path.len() - 1) as usize) {
                let child = p.children().get(dir);
                if let Some(child) = child {
                    p = child;
                } else {
                    return None;
                }
            }

            p.children().get(path.get(path.len() - 1))
        } else {
            Some(self)
        }
    }

    fn get_owner_mut(
        &mut self,
        path: &Path,
        create: bool,
    ) -> Option<&'_ mut Option<Box<QtreeNode<T>>>> {
        let mut path = (0..path.len()).map(|i| path.get(i));
        let last = path.next_back().unwrap();

        let mut p: &mut QtreeNode<T> = self;
        for dir in path {
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

        Some(out)
    }

    pub fn insert(&mut self, path: &Path, value: T) {
        let leaf = self.get_owner_mut(path, true).unwrap();
        let value = QtreeNode::Leaf(value);

        // only allocate a new box if there isn't one already
        if let Some(leaf) = leaf {
            *leaf.deref_mut() = value;
        } else {
            *leaf = Some(Box::new(value));
        }
    }
}

pub(crate) struct Children<T> {
    top_left: Option<Box<QtreeNode<T>>>,
    top_right: Option<Box<QtreeNode<T>>>,
    bot_left: Option<Box<QtreeNode<T>>>,
    bot_right: Option<Box<QtreeNode<T>>>,
}
impl<T> Children<T> {
    fn get(&self, dir: Dir) -> Option<&QtreeNode<T>> {
        match dir {
            TopLeft => self.top_left.as_ref().map(|b| b.as_ref()),
            TopRight => self.top_right.as_ref().map(|b| b.as_ref()),
            BotLeft => self.bot_left.as_ref().map(|b| b.as_ref()),
            BotRight => self.bot_right.as_ref().map(|b| b.as_ref()),
        }
    }

    fn get_mut(&mut self, dir: Dir) -> &mut Option<Box<QtreeNode<T>>> {
        match dir {
            TopLeft => &mut self.top_left,
            TopRight => &mut self.top_right,
            BotLeft => &mut self.bot_left,
            BotRight => &mut self.bot_right,
        }
    }
}
impl<T> Default for Children<T> {
    fn default() -> Self {
        Self {
            top_left: None,
            top_right: None,
            bot_left: None,
            bot_right: None,
        }
    }
}

fn dir_to_vec2(dir: Dir) -> Vec2 {
    match dir {
        TopRight => vec2![1, 1],
        TopLeft => vec2![-1, 1],
        BotLeft => vec2![-1, -1],
        BotRight => vec2![1, -1],
    }
}
fn vec2_to_dir(pos: &Vec2) -> Dir {
    let left = pos.x < 0;
    let bot = pos.y < 0;
    match (bot, left) {
        (true, true) => BotLeft,
        (true, false) => BotRight,
        (false, true) => TopLeft,
        (false, false) => TopRight,
    }
}

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
fn qfrom<T: Copy>(
    src: impl IntoIterator<Item = Vec2>,
    mut f: impl FnMut(Vec2) -> T,
) -> (Qtree<T>, HashMap<Vec2, T>) {
    let mut verify = HashMap::new();
    let mut q = Qtree::new();
    for p in src.into_iter() {
        let value = f(p);
        verify.insert(p, value);
        q.insert(p, value);
    }

    (q, verify)
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
    assert_save_eq!([(0, 0)]);
    assert_save_eq!([(-1, 0)]);
    assert_save_eq!([(0, -1)]);
    assert_save_eq!([(-1, -1)]);

    assert_save_eq!([(0, 0), (0, -1)]);
    assert_save_eq!([(0, 0), (-1, 0), (0, -1), (-1, -1)]);

    assert_save_eq!([(1, 1)]);
    assert_save_eq!([(3, 3)]);

    use rand::random;
    let points = std::iter::from_fn(|| Some((random::<i64>(), random::<i64>()))).take(256);
    assert_save_eq!(points);
}

#[test]
fn qchain() {
    let mut x = Qtree::<()>::new();

    macro_rules! assert_chain {
        ($coords:expr, $chain:expr) => {
            let mut x = Qtree::<()>::new();
            let out = x.as_dirs($coords.into());
            assert_eq!(out, $chain);
        };
        (h = $h:expr, $coords:expr, $chain:expr) => {
            let mut x = Qtree::<()>::new();

            for _ in 1..$h {
                x.grow();
            }
            assert_eq!(x.height, $h);

            let out = x.as_dirs($coords.into());
            assert_eq!(out, $chain);
        };
    }

    assert_chain!((0, 0), qpath![TopRight]);
    assert_chain!((-1, 0), qpath![TopLeft]);
    assert_chain!((-1, -1), qpath![BotLeft]);
    assert_chain!((0, -1), qpath![BotRight]);

    assert_chain!(h = 2, (0, 0), qpath![TopRight, BotLeft]);
    assert_chain!(h = 2, (-1, 0), qpath![TopLeft, BotRight]);
    assert_chain!(h = 2, (-1, -1), qpath![BotLeft, TopRight]);
    assert_chain!(h = 2, (0, -1), qpath![BotRight, TopLeft]);

    assert_chain!(h = 3, (0, 0), qpath![TopRight, BotLeft, BotLeft]);
    assert_chain!(h = 3, (-1, 0), qpath![TopLeft, BotRight, BotRight]);
    assert_chain!(h = 3, (-1, -1), qpath![BotLeft, TopRight, TopRight]);
    assert_chain!(h = 3, (0, -1), qpath![BotRight, TopLeft, TopLeft]);
}

#[cfg(test)]
fn showq<T: std::fmt::Debug>(q: &Qtree<T>) {
    let mut counter = Path::from_iter(std::iter::repeat(TopRight).take(q.height() as usize));
    'main: loop {
        eprintln!(
            "{:?} -> {:?} -> {:?}",
            &counter,
            Qtree::<()>::to_pos(&counter),
            q.root.get_owner(&counter).map(|node| node.as_value())
        );

        // increment
        let mut next = counter.pop().unwrap().inc();
        while next.is_none() {
            if counter.is_empty() {
                break 'main;
            }
            next = counter.pop().unwrap().inc();
        }
        counter.push(next.unwrap());
        while counter.len() < q.height() {
            counter.push(TopRight);
        }
    }
}

#[test]
fn qtree_set_get_basic() {
    let points = [(-1, -1), (0, 0), (0, 2), (1, 1), (2, 2)]
        .into_iter()
        .map(|p| p.into())
        .collect::<Vec<_>>();

    let mut x = Qtree::new();
    for &p in &points {
        dbg!(p);
        x.insert(p, ());

        showq(&x);
        assert!(x.contains(&p));
    }

    for p in &points {
        dbg!(p);
        assert!(x.contains(&p));
    }
}

#[test]
fn qtree_set_get_rand() {
    use rand::random;
    use std::collections::HashMap;

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
