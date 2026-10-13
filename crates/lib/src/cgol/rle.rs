//! A module for the
//! [Run Length Encoded](https://conwaylife.com/wiki/Run_Length_Encoded) (RLE)
//! format.
//!
//! RLE is a common format for storing Conway's Game of Life patterns.
//!
//! ### Example
//! ```
//! use lari::{Grid, rle};
//!
//! let grid = Grid::soup(100, 100);
//!
//! let mut bytes = vec![];
//! let count = rle::from_grid(&grid, &mut bytes).unwrap();
//! assert_eq!(bytes.len(), count);
//!
//! let out = rle::to_grid(&mut std::io::Cursor::new(bytes)).unwrap();
//! assert_eq!(out, grid);
//! ```

use std::io::{self, Read, Write};
use std::str::FromStr;

use crate::cgol::*;

/// Create a [Grid] from the
/// [RLE](https://conwaylife.com/wiki/Run_Length_Encoded) format.
pub fn to_grid(src: &mut impl Read) -> io::Result<Grid> {
    use regex::Regex;

    // TODO: don't load everything into memory at once
    let mut data = String::new();
    src.read_to_string(&mut data)?;

    // skip comments
    let re = Regex::new(r"(?m)(^#[^\n]*\n)*").unwrap();
    let mut off = re
        .find(&data)
        .ok_or(io::ErrorKind::InvalidData)?
        .range()
        .end;
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
        let count = if !count.is_empty() {
            u64::from_str(count).unwrap()
        } else {
            1
        };

        match value {
            "o" | "b" => {
                for _ in 0..count {
                    out.set(pos, value == "o");
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
            _ => unreachable!(),
        }
    }

    Err(io::ErrorKind::InvalidData.into())
}

/// Save a [Grid] to the
/// [RLE](https://conwaylife.com/wiki/Run_Length_Encoded) format.
pub fn from_grid(grid: &Grid, out: &mut impl Write) -> io::Result<usize> {
    let aabb = grid.aabb();
    let width = aabb.1.x - aabb.0.x;
    let height = aabb.1.y - aabb.0.y;

    let mut cells = Vec::from_iter(grid.cells.iter().copied());
    cells.sort_unstable_by_key(|a| (a.y, a.x));

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

        let x_diff = if diff.y > 0 { pos.x } else { diff.x - 1 };

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

    Ok(count)
}

#[test]
fn rle_save_load() {
    fn assert_match(grid: Grid) {
        let bb = grid.aabb();
        let grid = Grid::from_iter(grid.into_iter().map(|p| p - bb.0));

        let mut raw = vec![];
        let len = from_grid(&grid, &mut raw).unwrap();
        assert_eq!(raw.len(), len);

        let out = to_grid(&mut io::Cursor::new(raw)).unwrap();

        assert_eq!(grid, out);
    }

    gen_test_grids().for_each(|mut g| {
        assert_match(g.clone());
        g.step(100);
        assert_match(g);
    });
}
