#![cfg_attr(docsrs, feature(doc_cfg))]

//! # Lari
//! Lari is both a library and binary for simulating
//! [Conway's Game of Life](https://en.wikipedia.org/wiki/Conway%27s_Game_of_Life).
//!
//! The primary interface is through the [Grid] struct.
//! ```
//! use lari::Grid;
//!
//! let mut grid = Grid::from_table(&[
//!     [0, 1, 0],
//!     [0, 1, 0],
//!     [0, 1, 0],
//! ]);
//! grid.step(1);
//!
//! let after = Grid::from_table(&[
//!     [0, 0, 0],
//!     [1, 1, 1],
//!     [0, 0, 0],
//! ]);
//! assert_eq!(grid, after);
//! ```

pub(crate) mod qtree;
pub(crate) mod util;
pub(crate) mod vec2;

mod cgol;
pub use cgol::*;
