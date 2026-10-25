use crate::vec2::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    TopRight,
    TopLeft,
    BotLeft,
    BotRight,
}
pub use Dir::*;
impl Dir {
    pub const ALL: [Dir; 4] = [TopRight, TopLeft, BotLeft, BotRight];

    pub const fn inc(&self) -> Option<Dir> {
        match self {
            TopRight => Some(TopLeft),
            TopLeft => Some(BotLeft),
            BotLeft => Some(BotRight),
            BotRight => None,
        }
    }

    const fn to_num(&self) -> u8 {
        match self {
            TopRight => 0b00,
            TopLeft => 0b01,
            BotLeft => 0b10,
            BotRight => 0b11,
        }
    }

    const fn from_num(num: u8) -> Self {
        match num {
            0b00 => TopRight,
            0b01 => TopLeft,
            0b10 => BotLeft,
            0b11 => BotRight,
            _ => panic!(),
        }
    }
}
impl Default for Dir {
    fn default() -> Self {
        TopRight
    }
}
impl AsRef<Dir> for Dir {
    fn as_ref(&self) -> &Dir {
        self
    }
}

impl From<Vec2> for Dir {
    fn from(pos: Vec2) -> Self {
        let left = pos.x < 0;
        let bot = pos.y < 0;
        match (bot, left) {
            (true, true) => BotLeft,
            (true, false) => BotRight,
            (false, true) => TopLeft,
            (false, false) => TopRight,
        }
    }
}
impl From<Dir> for Vec2 {
    fn from(dir: Dir) -> Self {
        match dir {
            TopRight => vec2![1, 1],
            TopLeft => vec2![-1, 1],
            BotLeft => vec2![-1, -1],
            BotRight => vec2![1, -1],
        }
    }
}

#[macro_export]
macro_rules! qpath {
    ($($dir:expr),* $(,)?) => {{
        let mut out = Path::new();
        $(
            out.push($dir);
        )*

        out
    }}
}
pub use qpath;

#[derive(Default, Clone)]
pub struct Path {
    dirs: u128,
    count: u8,
}
impl Path {
    pub const fn new() -> Self {
        Self { dirs: 0, count: 0 }
    }

    pub const fn len(&self) -> u8 {
        self.count
    }
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub const fn push(&mut self, dir: Dir) {
        debug_assert!(self.count < 64);

        let count = self.count * 2;

        // const `Option::unwrap_or` isn't stable yet, so we manually do it
        let mask = if let Some(mask) = 1u128.overflowing_shl(count as u32).0.checked_sub(1) {
            mask
        } else {
            0
        };

        // SAFETY: self.count will _never_ be greater than or equal to 64
        self.dirs =
            (self.dirs & mask) | unsafe { (dir.to_num() as u128).unchecked_shl(count as u32) };
        self.count += 1;
    }

    pub const fn get(&self, idx: u8) -> Dir {
        debug_assert!(idx < self.count);

        let count = idx * 2;
        Dir::from_num(((self.dirs >> count) & 0b11) as u8)
    }

    pub const fn pop(&mut self) -> Option<Dir> {
        if self.count > 0 {
            let out = self.get(self.count - 1);
            self.count -= 1;

            Some(out)
        } else {
            None
        }
    }

    pub const fn split_at(mut self, idx: u8) -> (Path, Path) {
        debug_assert!(idx <= self.count);

        let last = Self {
            dirs: self.dirs.overflowing_shr((idx * 2) as u32).0,
            count: self.count - idx,
        };

        self.count = idx;

        (self, last)
    }

    pub fn iter(&self) -> impl Iterator<Item = Dir> {
        PathRefIter {
            path: self,
            index: 0,
        }
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        if self.count != other.count {
            false
        } else {
            let mask = 1_u128
                .checked_shl(self.count as u32 * 2)
                .unwrap_or(0)
                .overflowing_sub(1)
                .0;
            ((self.dirs ^ other.dirs) & mask) == 0
        }
    }
}
impl Eq for Path {}

use std::fmt::{self, Debug, Formatter};
impl Debug for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "({}, none)", self.count)?;
        } else {
            write!(f, "({}, {:?}", self.count, self.get(0))?;
            for i in 1..self.len() {
                write!(f, " -> {:?}", self.get(i))?;
            }
            write!(f, ")")?;
        }

        Ok(())
    }
}

impl FromIterator<Dir> for Path {
    fn from_iter<I: IntoIterator<Item = Dir>>(iter: I) -> Self {
        let mut out = qpath![];
        for dir in iter {
            out.push(dir);
        }

        out
    }
}

pub struct PathIter {
    path: Path,
    index: u8,
}
impl Iterator for PathIter {
    type Item = Dir;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.path.len() {
            None
        } else {
            let out = self.path.get(self.index);
            self.index += 1;

            Some(out)
        }
    }
}
impl IntoIterator for Path {
    type Item = Dir;
    type IntoIter = PathIter;

    fn into_iter(self) -> Self::IntoIter {
        PathIter {
            path: self,
            index: 0,
        }
    }
}

struct PathRefIter<'a> {
    path: &'a Path,
    index: u8,
}
impl<'a> Iterator for PathRefIter<'a> {
    type Item = Dir;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.path.len() {
            None
        } else {
            let out = self.path.get(self.index);
            self.index += 1;

            Some(out)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::random;

    fn rand_dir() -> Dir {
        Dir::from_num(random::<u8>() & 0b11)
    }

    fn rand_path() -> Path {
        let len = (random::<u8>() % (64 - 8)) + 8;
        let mut out = Path::new();
        for _ in 0..len {
            out.push(rand_dir());
        }

        out
    }

    fn rand_path_max() -> Path {
        let mut out = Path::new();
        for _ in 0..64 {
            out.push(rand_dir());
        }

        out
    }

    #[test]
    fn qpath_macro() {
        let p = qpath![TopLeft];
        assert_eq!(p.get(0), TopLeft);
        assert_eq!(p.len(), 1);

        let p = qpath![TopRight];
        assert_eq!(p.get(0), TopRight);
        assert_eq!(p.len(), 1);

        let p = qpath![TopRight, BotLeft];
        assert_eq!(p.get(0), TopRight);
        assert_eq!(p.get(1), BotLeft);
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn iter() {
        let p = rand_path();
        let x = p.clone().into_iter();

        assert_eq!(Path::from_iter(x), p);

        for (i, dir) in p.iter().enumerate() {
            assert_eq!(p.get(i as u8), dir);
        }
    }

    #[test]
    fn stack() {
        let mut p = qpath!();
        assert_eq!(p.pop(), None);
        assert_eq!(p.len(), 0);

        p.push(TopLeft);
        assert_eq!(p.len(), 1);
        assert_eq!(p.pop(), Some(TopLeft));
        assert_eq!(p.len(), 0);

        p.push(TopLeft);
        p.push(TopRight);
        p.push(BotLeft);
        p.push(BotRight);

        assert_eq!(p.len(), 4);
        assert_eq!(p.pop(), Some(BotRight));
        assert_eq!(p.len(), 3);
        assert_eq!(p.pop(), Some(BotLeft));
        assert_eq!(p.len(), 2);
        assert_eq!(p.pop(), Some(TopRight));
        assert_eq!(p.len(), 1);
        assert_eq!(p.pop(), Some(TopLeft));
        assert_eq!(p.len(), 0);
        assert_eq!(p.pop(), None);
        assert_eq!(p.len(), 0);
    }

    #[test]
    fn split_at() {
        let p = rand_path();
        let (a, b) = p.clone().split_at(0);
        assert_eq!(a, qpath![]);
        assert_eq!(b, p);

        let (a, b) = p.clone().split_at(p.len());
        assert_eq!(a, p);
        assert_eq!(b, qpath![]);

        let (a, b) = qpath![].split_at(0);
        assert_eq!(a, qpath![]);
        assert_eq!(b, qpath![]);

        let p = qpath![TopLeft, BotRight, TopRight, BotLeft];
        let (a, b) = p.clone().split_at(2);
        assert_eq!(a, qpath![TopLeft, BotRight]);
        assert_eq!(b, qpath![TopRight, BotLeft]);

        let p = rand_path_max();
        let (a, b) = p.clone().split_at(p.len());
        assert_eq!(a, p);
        assert_eq!(b, qpath![]);
    }

    #[test]
    fn eq() {
        assert_ne!(qpath![TopRight], qpath![BotRight]);
        assert_ne!(qpath![TopRight, TopLeft], qpath![BotRight]);
        assert_ne!(qpath![TopRight], qpath![BotRight, TopLeft]);

        assert_eq!(qpath![], qpath![]);
        assert_ne!(qpath![TopRight], qpath![]);
        assert_ne!(qpath![], qpath![TopRight]);

        let a = rand_path_max();
        assert_eq!(a, a);

        let b = rand_path_max();
        assert_eq!(a == b, a.iter().eq(b.iter()));
    }
}
