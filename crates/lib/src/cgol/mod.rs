use std::collections::HashSet;
use std::io::{self, Read, Write};

mod moore;
use moore::*;

pub mod rle;

use crate::qtree::*;
pub use crate::vec2::*;

/// A logically infinite grid of cells to simulate Conway's Game of Life.
///
///
/// ### Conway's Game of Life's Rules
/// Every cell is either alive or dead.
///
/// At every step of simulation, the following pseudocode is applied to every cell:
/// ```ignore
/// fn step_cell(cell: Cell, neighbor_count: u8) -> Cell {
///     if cell.is_alive() {
///         if neighbor_count == 2 || neighbor_count == 3 {
///             return Cell::alive();
///         } else {
///             return Cell::dead();
///         }
///     } else {
///         if neighbor_count == 3 {
///             return Cell::alive();
///         } else {
///             return Cell::dead();
///         }
///     }
/// }
/// ```
///
/// ### Examples
/// A still life that will always remain the same, forever.
/// ```
/// use lari::Grid;
///
/// let mut grid = Grid::from_table(&[
///     [1, 1],
///     [1, 1],
/// ]);
/// let init = grid.clone();
///
/// assert_eq!(grid, init);
///
/// grid.step(1);
/// assert_eq!(grid, init);
///
/// grid.step(100);
/// assert_eq!(grid, init);
/// ```
///
/// A glider is a 'spaceship', which is just something that moves.
/// ```
/// use lari::Grid;
///
/// let mut glider = Grid::from_table(&[
///     [0, 0, 1],
///     [1, 0, 1],
///     [0, 1, 1],
/// ]);
/// let after = Grid::from_iter(glider.alive().map(|&p| p + (1, 1)));
///
/// glider.step(4);
///
/// assert_eq!(glider, after);
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Grid {
    cells: MooreGrid,
}

impl Grid {
    /// Creates an empty grid.
    ///
    /// # Example
    /// ```
    /// use lari::Grid;
    ///
    /// let grid = Grid::new();
    ///
    /// assert_eq!(grid.alive().count(), 0);
    /// ```
    pub fn new() -> Self {
        Default::default()
    }

    /// Step the simulation by `count` steps.
    ///
    /// # Example
    /// ```
    /// use lari::Grid;
    ///
    /// let mut a = Grid::soup(100, 100);
    /// let mut b = a.clone();
    ///
    /// a.step(10);
    /// b.step(5);
    /// b.step(5);
    ///
    /// assert_eq!(a, b);
    /// ```
    pub fn step(&mut self, count: usize) {
        let mut dirty = HashSet::<Vec2>::from_iter(self.cells.iter_all().map(|all| all.0));
        let mut queue: Vec<Vec2> = vec![];

        for _ in 0..count {
            queue.clear();
            queue.extend(dirty.iter());

            dirty.clear();

            let mut scratch = self.cells.clone();
            while let Some(cur) = queue.pop() {
                let (alive, acount) = self.cells.get_with_count(&cur);

                let changed = scratch.set(cur, acount == 3 || (alive && acount == 2));
                if changed {
                    dirty.insert(cur);
                    for n in MooreGrid::neighbors(cur) {
                        if dirty.insert(n) {
                            queue.push(n);
                        }
                    }
                }
            }

            self.cells = scratch;
        }
    }

    /// Set the value of the cell at `pos` to alive if `value` is true.
    ///
    /// # Example
    /// ```
    /// use lari::{Grid, Vec2};
    ///
    /// let mut grid = Grid::new();
    ///
    /// let p = Vec2::from((0, 0));
    ///
    /// assert_eq!(grid.get(&p), false);
    ///
    /// grid.set(p, true);
    /// assert_eq!(grid.get(&p), true);
    ///
    /// grid.set(p, false);
    /// assert_eq!(grid.get(&p), false);
    ///
    /// grid.set(p, true);
    /// grid.step(1);
    /// assert_eq!(grid.get(&p), false);
    /// ```
    pub fn set(&mut self, pos: impl Into<Vec2>, value: bool) {
        self.cells.set(pos, value);
    }

    /// Returns true if the cell at `pos` is alive.
    ///
    /// # Example
    /// See [set][Grid::set].
    pub fn get(&self, pos: &Vec2) -> bool {
        self.cells.get(pos)
    }

    /// Get the positions of all of the cells that are alive.
    ///
    /// # Example
    /// ```
    /// use lari::Grid;
    ///
    /// let mut grid = Grid::from_table(&[
    ///     [1, 1, 0],
    ///     [1, 0, 1],
    ///     [0, 1, 1],
    /// ]);
    ///
    /// assert_eq!(grid.alive().count(), 6);
    ///
    /// grid.step(100);
    ///
    /// assert_eq!(grid.alive().count(), 6);
    /// ```
    pub fn alive(&self) -> impl Iterator<Item = &Vec2> {
        self.cells.iter()
    }

    /// Get the axis aligned bounding box (aabb) of the grid.
    ///
    /// The first returned `Vec2` is the bottom left,
    /// and the the second is the top right.
    ///
    /// # Example
    /// ```
    /// use lari::Grid;
    ///
    /// let grid = Grid::soup(100, 100);
    ///
    /// let (bot_left, top_right) = grid.aabb();
    /// assert!(bot_left.x <= top_right.x);
    /// assert!(bot_left.y <= top_right.y);
    /// ```
    pub fn aabb(&self) -> (Vec2, Vec2) {
        if self.cells.is_empty() {
            return ((0, 0).into(), (0, 0).into());
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

        ((x_min, y_min).into(), (x_max, y_max).into())
    }

    /// Initialize a grid using a table of 1's and 0's.
    ///
    /// Intended to be a more ergonomic way to hardcode patterns.
    ///
    /// # Example
    /// ```
    /// use lari::Grid;
    ///
    /// let mut a = Grid::new();
    /// a.set((2, 0), true);
    /// a.set((0, 1), true);
    /// a.set((2, 1), true);
    /// a.set((1, 2), true);
    /// a.set((2, 2), true);
    ///
    /// let b = Grid::from_table(&[
    ///     [0, 0, 1],
    ///     [1, 0, 1],
    ///     [0, 1, 1],
    /// ]);
    ///
    /// assert_eq!(a, b);
    /// ```
    pub fn from_table<const N: usize>(src: &[[u8; N]]) -> Self {
        let alive = src
            .iter()
            .flat_map(|row| row.iter())
            .enumerate()
            .filter_map(|(i, &x)| (x > 0).then_some(((i % N) as i64, (i / N) as i64)));

        alive.fold(Default::default(), |mut out, (x, y)| {
            out.set((x, y), true);
            out
        })
    }

    /// Writes the state of this grid into `out`.
    ///
    /// This function is the inverse of [load][Grid::load].
    ///
    /// # Example
    /// ```
    /// use lari::Grid;
    ///
    /// let grid = Grid::soup(100, 100);
    ///
    /// let mut bytes = vec![];
    /// grid.save(&mut bytes).unwrap();
    ///
    /// let other = Grid::load(&mut std::io::Cursor::new(bytes)).unwrap();
    ///
    /// assert_eq!(grid, other);
    /// ```
    pub fn save(&self, out: &mut impl Write) -> io::Result<()> {
        Qtree::from_iter(self.cells.iter().map(|&pos| (pos, ()))).save(out)
    }

    /// Reads from `src` to create a [Grid].
    ///
    /// This function is the inverse of [save][Grid::save].
    ///
    /// # Example
    /// See [save][Grid::save].
    pub fn load(src: &mut impl Read) -> io::Result<Self> {
        Ok(Self::from_iter(
            Qtree::load(src)?.into_iter().map(|(p, _)| p),
        ))
    }

    /// Generate a random grid.
    ///
    /// Random grid meaning random cells are set to alive within the bounds.
    /// The bounds are specified by `width` and `height`.
    /// Note this will only generate cells in the positve `(+x, +y)` quadrant of the grid.
    #[cfg(feature = "rand")]
    pub fn soup(width: u64, height: u64) -> Self {
        if width == 0 || height == 0 {
            return Self::new();
        }

        let count = rand::rng().sample(rand::distr::Uniform::new(0, width * height).unwrap());

        use rand::RngExt;
        let mut width = rand::rng().sample_iter(rand::distr::Uniform::new(0, width).unwrap());
        let mut height = rand::rng().sample_iter(rand::distr::Uniform::new(0, height).unwrap());

        let iter = std::iter::from_fn(|| {
            Some((width.next().unwrap() as i64, height.next().unwrap() as i64).into())
        });

        Grid::from_iter(iter.take(count as usize))
    }
}

impl IntoIterator for Grid {
    type Item = Vec2;
    type IntoIter = <MooreGrid as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.cells.into_iter()
    }
}
impl FromIterator<Vec2> for Grid {
    fn from_iter<T: IntoIterator<Item = Vec2>>(iter: T) -> Self {
        let mut out = Self::default();
        for p in iter {
            out.set(p, true);
        }

        out
    }
}

#[cfg(test)]
pub(crate) fn gen_test_grids() -> impl Iterator<Item = Grid> {
    vec![
        // empty
        Grid::new(),
        // one cell
        Grid::from_table(&[[1]]),
        // block (stable)
        Grid::from_table(&[[1, 1], [1, 1]]),
        // tub (stable)
        Grid::from_table(&[[0, 1, 0], [1, 0, 1], [0, 1, 0]]),
        // blinker (period of 2)
        Grid::from_table(&[[0, 1, 0], [0, 1, 0], [0, 1, 0]]),
        // glider
        Grid::from_table(&[[0, 0, 1], [1, 0, 1], [0, 1, 1]]),
        // acorn
        Grid::from_table(&[
            [0, 1, 0, 0, 0, 0, 0],
            [0, 0, 0, 1, 0, 0, 0],
            [1, 1, 0, 0, 1, 1, 1],
        ]),
    ]
    .into_iter()
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

#[cfg(test)]
macro_rules! assert_steps {
    ($before:expr => $after:expr; after $steps:literal steps) => {
        let mut before = Grid::from_table($before);
        let after = Grid::from_table($after);

        before.step($steps);

        assert_eq!(before, after);
    };
}

#[test]
fn simple() {
    assert_steps!(&[[1]] => &[[1]]; after 0 steps);
    assert_steps!(&[[1]] => &[[0]]; after 1 steps);

    assert_steps!(&[
        [1, 1],
        [1, 1],
    ] => &[
        [1, 1],
        [1, 1],
    ]; after 1 steps);

    assert_steps!(&[
        [1, 1],
        [1, 1],
    ] => &[
        [1, 1],
        [1, 1],
    ]; after 10 steps);
}

#[test]
fn blinker() {
    let init = Grid::from_table(&[[0, 0, 0], [1, 1, 1], [0, 0, 0]]);
    let other = Grid::from_table(&[[0, 1, 0], [0, 1, 0], [0, 1, 0]]);

    let mut grid = init.clone();

    assert_eq!(grid, init);

    grid.step(1);
    assert_eq!(grid, other);

    grid.step(1);
    assert_eq!(grid, init);
}
