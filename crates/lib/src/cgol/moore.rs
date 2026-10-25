use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::vec2::*;

/// A grid to store cells and how many of their Moore neighbors are alive.
///
/// This grid fundamentaly does one thing:
/// every time a cell is changed, update each of its neighbor's count.
/// Which makes repeated lookup of this count more efficient.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MooreGrid {
    inner: HashMap<Vec2, (bool, u8)>,
}

impl MooreGrid {
    /// Creates a new emtpy `MooreGrid`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the value of the cell at `pos`.
    pub fn get(&self, pos: &Vec2) -> bool {
        self.get_with_count(pos).0
    }

    /// Get the value of the cell at `pos`
    /// and the number of it's Moore neighbors that are true.
    pub fn get_with_count(&self, pos: &Vec2) -> (bool, u8) {
        self.inner.get(pos).copied().unwrap_or((false, 0))
    }

    /// Set the value of the cell at `pos`.
    ///
    /// Only returns true if the value changed.
    pub fn set(&mut self, pos: impl Into<Vec2>, value: bool) -> bool {
        let pos = pos.into();
        if value {
            let prev = self.inner.entry(pos).or_insert((false, 0));
            if prev.0 == value {
                return false;
            }

            prev.0 = true;
            self.update_neighbors(pos, true);
        } else {
            match self.inner.entry(pos) {
                Entry::Occupied(mut entry) => {
                    let prev = entry.get_mut();
                    if prev.0 == value {
                        return false;
                    }

                    prev.0 = false;
                    if prev.1 == 0 {
                        entry.remove();
                    }
                }
                _ => return false,
            }
            self.update_neighbors(pos, false);
        }

        true
    }

    fn update_neighbors(&mut self, pos: Vec2, increase: bool) {
        for n in Self::neighbors(pos) {
            let mut entry = match self.inner.entry(n) {
                Entry::Occupied(o) => o,
                entry => entry.insert_entry((false, 0)),
            };

            let value = entry.get_mut();

            if increase {
                value.1 += 1;
            } else {
                value.1 -= 1;
            }

            if value.1 == 0 && !value.0 {
                entry.remove();
            }
        }
    }

    /// Get the Moore neighbors of `pos`.
    pub fn neighbors(pos: Vec2) -> impl Iterator<Item = Vec2> {
        const OFFSETS: [(i64, i64); 8] = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];
        OFFSETS.iter().map(move |o| pos.add(Vec2::from(o)))
    }

    /// Removes all of the cells.
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    /// Returns true if there are no alive cells.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the number tracked cells.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns the number of alive cells.
    pub fn count(&self) -> usize {
        self.iter().count()
    }

    /// Returns an iterator over all of the alive cells.
    pub fn iter(&self) -> impl Iterator<Item = &Vec2> {
        self.inner.iter().filter_map(|(p, (a, _))| a.then_some(p))
    }
}

// workaround for not being able to name the type in the `IntoIterator` impl.
// can be resolved once `type T = impl Trait` is stable
// https://github.com/rust-lang/rust/issues/63063
pub struct MooreIter<I>(I);
impl<I> Iterator for MooreIter<I>
where
    I: Iterator<Item = (Vec2, (bool, u8))>,
{
    type Item = Vec2;
    fn next(&mut self) -> Option<Self::Item> {
        for next in self.0.by_ref() {
            if !next.1.0 {
                continue;
            }

            return Some(next.0);
        }

        None
    }
}
impl IntoIterator for MooreGrid {
    type Item = Vec2;
    type IntoIter = MooreIter<<HashMap<Self::Item, (bool, u8)> as IntoIterator>::IntoIter>;
    fn into_iter(self) -> Self::IntoIter {
        MooreIter(self.inner.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count() {
        let mut m = MooreGrid::new();
        assert_eq!(m.count(), 0);

        m.set(vec2![0, 0], false);
        assert_eq!(m.count(), 0);

        m.set(vec2![1, 0], true);
        assert_eq!(m.count(), 1);

        m.set(vec2![0, 0], true);
        assert_eq!(m.count(), 2);

        m.set(vec2![1, 0], false);
        assert_eq!(m.count(), 1);
    }
}
