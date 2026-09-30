use std::sync::{Arc, Mutex};
use std::str::FromStr;
use std::collections::{HashSet, VecDeque};

use lari::*;

use crate::util::*;

use crate::wasm_helpers::*;
use crate::wasm_helpers::{println, eprintln};
use crate::dom::*;



const CELL_SIZE: f64 = 50.0;

#[derive(Clone)]
struct Line {
	a: Vec2,
	b: Vec2,
}
impl Line {
	fn parallel(&self, other: &Self) -> bool {
		(self.a.x == self.b.x && other.a.x == other.b.x) ||
		(self.a.y == self.b.y && other.a.y == other.b.y)
	}

	fn joint(&self, other: &Self) -> bool {
		self.a == other.a || self.a == other.b ||
		self.b == other.a || self.b == other.b
	}

	fn join(&mut self, other: &Self) {
		if self.a == other.a {
			self.a = other.b;
		} else if self.a == other.b {
			self.a = other.a;
		} else if self.b == other.a {
			self.b = other.b;
		} else if self.b == other.b {
			self.b = other.a;
		} else {
			unreachable!();
		}
	}
}
impl std::fmt::Debug for Line {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		let a = self.a.min(self.b);
		let b = self.a.max(self.b);
		write!(f, "({}, {}) <-> ({}, {})", a.x, a.y, b.x, b.y)
	}
}
impl PartialEq for Line {
	fn eq(&self, other: &Self) -> bool {
		(self.a == other.a && self.b == other.b) ||
		(self.a == other.b && self.b == other.a)
	}
}
impl Eq for Line {}
impl PartialOrd for Line {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}
impl Ord for Line {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.a.min(self.b).cmp(&other.a.min(other.b))
			.then(self.a.max(self.b).cmp(&other.a.max(other.b)))
	}
}

fn merge_lines(lines: &mut Vec<Line>) {
	let mut i = 0;
	while i < lines.len() {
		let others = (i + 1..lines.len()).rev()
			.filter(|&j| lines[i].joint(&lines[j]) && lines[i].parallel(&lines[j]))
			.collect::<Vec<_>>();

		if others.is_empty() {
			i += 1;
		} else {
			for j in others {
				let other = lines.remove(j);
				lines[i].join(&other);
			}
		}
	}
}

fn exact_contour(lines: &mut Vec<Line>) {
	lines.sort_unstable();

	// only retain elements which are distinct
	// (which is not the same as Vec::dedup)
	let mut i = 0;
	while i + 1 < lines.len() {
		if lines[i] != lines[i + 1] {
			i += 1;
			continue;
		}

		let count = 2 + lines[i + 2..].iter().take_while(|&x| x == &lines[i]).count();
		lines.drain(i..i + count);
	}
}



struct DynamicInterval<F: Fn() + 'static> {
	rate: Option<u64>,
	callback: F,
	window: Window,
	prev: Option<i32>,
}
impl<F: Fn() + 'static> DynamicInterval<F> {
	fn new(window: Window, callback: F, rate: Option<u64>) -> Arc<Mutex<Self>> {
		let out = Arc::new(Mutex::new(Self {
			rate,
			callback,
			window,
			prev: None,
		}));

		Self::repeat(out.clone());

		return out;
	}

	fn repeat<T: Fn() + 'static>(me: Arc<Mutex<DynamicInterval<T>>>){
		let mut locked = me.lock().unwrap();
		if let Some(rate) = locked.rate {
			let cur = set_timeout(&locked.window, {
				let me = me.clone();
				move |_: Event| {
					(me.lock().unwrap().callback)();
					Self::repeat(me.clone());
				}
			}, rate as i32).unwrap();
			locked.prev.replace(cur);
		} else {
			locked.prev.take();
		}
	}

	fn set_rate(me: Arc<Mutex<Self>>, rate: Option<i32>) {
		let rate = rate.map(|n| n as u64);

		let mut locked = me.lock().unwrap();
		if locked.rate == rate {
			return;
		} else {
			locked.rate = rate;
		}

		if let Some(prev) = locked.prev {
			locked.window.clear_timeout_with_handle(prev);
		}

		drop(locked);

		Self::repeat(me);
	}
}

#[derive(Debug)]
enum Action {
	SetCell(Vec2, bool),
	MultiSetCell(Vec<(Vec2, bool)>),
	SetGrid{from: Grid, to: Grid},
}

struct Viewer {
	grid: Grid,
	ctx: CanvasRenderingContext2d,
	paused: bool,

	viewport_dim: (u32, u32),
	camera_pos: (f64, f64),
	scale: f64,

	// NOTE: these are in viewer because
	//       panning and zooming both need to modify them
	cursor_pin: Option<(f64, f64)>,
	rsel: Option<((f64, f64), (f64, f64))>,

	stopped_sel: bool,
	action_log: VecDeque<Action>,
	action_idx: usize,
}
impl Viewer {
	fn new(grid: Grid, ctx: CanvasRenderingContext2d) -> Self {
		Self {
			grid,
			ctx,
			paused: true,
			viewport_dim: (0, 0),
			camera_pos: (0.0, 0.0),
			scale: 1.0,
			cursor_pin: None,
			rsel: None,
			stopped_sel: true,
			action_log: Default::default(),
			action_idx: 0,
		}
	}

	fn apply_action(&mut self) {
		match self.action_log[self.action_idx] {
			Action::SetCell(pos, value) => self.grid.set_cell(pos, value),
			Action::MultiSetCell(ref delta) => delta.iter().for_each(|&(pos, value)| self.grid.set_cell(pos, value)),
			Action::SetGrid{ref to, ..} => self.grid = to.clone(),
		}
		self.action_idx += 1;
	}
	fn unapply_action(&mut self) {
		self.action_idx -= 1;
		match self.action_log[self.action_idx] {
			Action::SetCell(pos, value) => self.grid.set_cell(pos, !value),
			Action::MultiSetCell(ref delta) => delta.iter().for_each(|&(pos, value)| self.grid.set_cell(pos, !value)),
			Action::SetGrid{ref from, ..} => self.grid = from.clone(),
		}
	}

	const MAX_UNDOS: usize = 100;
	fn do_action(&mut self, action: Action) {
		self.action_log.drain(self.action_idx..);

		let next_len = self.action_log.len() + 1;
		let overflow = next_len - next_len.min(Self::MAX_UNDOS);
		self.action_log.drain(..overflow);
		self.action_idx -= overflow;

		self.action_log.push_back(action);
		self.apply_action();
	}
	fn undo_action(&mut self) {
		if self.action_idx == 0 {
			// we're already at the start of the undo list
			// so do nothing
			return;
		}

		self.unapply_action();
	}
	fn redo_action(&mut self) {
		if self.action_idx >= self.action_log.len() {
			// we're already at the end of the undo list
			// so do nothing
			return;
		}

		self.apply_action();
	}

	fn from_screen_space(&self, xy: (f64, f64)) -> (f64, f64) {
		(
			(xy.0 + self.camera_pos.0) / self.scale / CELL_SIZE,
			(xy.1 + self.camera_pos.1) / self.scale / CELL_SIZE,
		)
	}

	fn to_screen_space(&self, xy: (i64, i64)) -> (f64, f64) {
		(
			xy.0 as f64 * self.scale * CELL_SIZE - self.camera_pos.0,
			xy.1 as f64 * self.scale * CELL_SIZE - self.camera_pos.1,
		)
	}

	fn get_selection(&self) -> Option<((i64, i64), (i64, i64))> {
		self.rsel.map(|(start, end)| (
			(
				start.0.min(end.0).floor() as i64,
				start.1.min(end.1).floor() as i64,
			),
			(
				start.0.max(end.0).ceil() as i64,
				start.1.max(end.1).ceil() as i64,
			),
		))
	}

	fn draw(&self) {
		let (width, height) = self.viewport_dim;

		const DEAD_COLOR:  &str = "#0f0f0f";
		const ALIVE_COLOR: &str = "#f0f0f0";
		const RSEL_COLOR:  &str = "#ff0000";
		let size = CELL_SIZE * self.scale;

		self.ctx.set_fill_style_str(DEAD_COLOR);
		self.ctx.fill_rect(0.0, 0.0, width as f64, height as f64);

		// draw rectangle selection
		if let Some((start, end)) = self.get_selection() {
			let start = self.to_screen_space(start);
			let end = self.to_screen_space(end);

			self.ctx.set_stroke_style_str(RSEL_COLOR);
			self.ctx.stroke_rect(
				start.0,
				start.1,
				end.0 - start.0,
				end.1 - start.1,
			);
		}

		// group cells together and draw them as one
		self.ctx.set_fill_style_str(ALIVE_COLOR);
		let mut unsued_cells = HashSet::<&Vec2>::from_iter(self.grid.get_alive());
		for pos in self.grid.get_alive() {
			if !unsued_cells.remove(pos) {
				continue;
			}

			// get taxicab flood fill of pos
			let mut group = vec![pos];
			let mut i = 0;
			while i < group.len() {
				let p = group[i];

				group.extend([
					vec2![-1,  0],
					vec2![ 1,  0],
					vec2![ 0, -1],
					vec2![ 0,  1],
				].into_iter().filter_map(|offset| {
					unsued_cells.take(&(offset + *p))
				}));

				i += 1;
			}

			// convert cells into lines
			let mut lines = group.into_iter()
				.flat_map(|pos: &Vec2| {
					[
						Line{a: pos.add(vec2![0, 0]), b: pos.add(vec2![1, 0])},
						Line{a: pos.add(vec2![1, 0]), b: pos.add(vec2![1, 1])},
						Line{a: pos.add(vec2![1, 1]), b: pos.add(vec2![0, 1])},
						Line{a: pos.add(vec2![0, 1]), b: pos.add(vec2![0, 0])},
					].into_iter()
				})
				.collect::<Vec<_>>();

			exact_contour(&mut lines);
			merge_lines(&mut lines);

			// draw cells
			while let Some(line) = lines.pop() {
				macro_rules! with_screen_pos {
					($($func:tt).*($pos:expr)) => {
						$($func).*(($pos.x as f64 * size) - self.camera_pos.0, ($pos.y as f64 * size) - self.camera_pos.1)
					};
				}

				self.ctx.begin_path();
				with_screen_pos!(self.ctx.move_to(line.a));
				with_screen_pos!(self.ctx.line_to(line.b));

				let mut prev = line.b;
				loop {
					let curr = lines.extract_if(.., |other| other.a == prev || other.b == prev).next();
					if let Some(curr) = curr {
						let curr = if curr.a == prev { curr.b } else { curr.a };
						with_screen_pos!(self.ctx.line_to(curr));

						prev = curr;
					} else {
						break;
					}
				}

				with_screen_pos!(self.ctx.line_to(line.a));
				self.ctx.close_path();
				self.ctx.fill();
			}
		}
	}
}



async fn to_clipboard(grid: &Grid) {
	let mut text = vec![];
	grid.save(&mut text).unwrap();

	let out = bytes_to_text(text);

	let w = window().unwrap();
	let nav = proto_get(&w, "navigator").unwrap();
	let clip = proto_get(nav.dyn_ref().unwrap(), "clipboard").unwrap();
	let write = proto_get(clip.dyn_ref().unwrap(), "writeText").unwrap();
	let write = write.dyn_into::<Function>().unwrap();
	let write: Promise = write.call(&clip, (&out.into(),)).unwrap().dyn_into().unwrap();

	write.await.unwrap();
}
async fn from_clipboard() -> Grid {
	let w = window().unwrap();
	let nav = proto_get(&w, "navigator").unwrap();
	let clip = proto_get(nav.dyn_ref().unwrap(), "clipboard").unwrap();
	let read = proto_get(clip.dyn_ref().unwrap(), "readText").unwrap();
	let read = read.dyn_into::<Function>().unwrap();
	let text: Promise = read.call(&clip, ()).unwrap().dyn_into().unwrap();

	let text = text.await.unwrap();

	let data = text_to_bytes(text.as_string().unwrap()).unwrap();

	Grid::load(&mut data.as_slice()).unwrap()
}



pub fn run() {
	let w = window().unwrap();
	let document = w.document().unwrap();
	let body = WrappedHtml::own(document.body().unwrap().into());

	let div = WrappedHtml::new("div").unwrap();
	set_style!((&div){
		"display": "flex";
		"flex-direction": "column";
		"width": "100%";
		"height": "100%";
	});
	body.append_child(&div).unwrap();

	let canvas = WrappedHtml::new("canvas").unwrap();
	set_style!((&canvas){
		"width": "100%";
		"height": "100%";
	});

	let ctx = canvas.as_elm::<HtmlCanvasElement>().unwrap().get_context("2d").unwrap().unwrap()
		.dyn_into::<CanvasRenderingContext2d>().unwrap();

	// acorn
	let grid = Grid::from_bits(&[
		[0, 1, 0, 0, 0, 0 ,0],
		[0, 0, 0, 1, 0, 0 ,0],
		[1, 1, 0, 0, 1, 1 ,1],
	]);

	let viewer = Viewer::new(grid, ctx);
	let viewer = Arc::new(Mutex::new(viewer));
	viewer.lock().unwrap().draw();

	let canvas = Arc::new(canvas);
	init_canvas(canvas.clone(), viewer.clone());

	let di = DynamicInterval::new(w, {
		let viewer = viewer.clone();
		move || {
			let viewer: &mut Viewer = &mut viewer.lock().unwrap();
			if !viewer.paused {
				viewer.grid.step(1);
				viewer.draw();
			}
		}
	}, None);
	DynamicInterval::set_rate(di.clone(), from_slider(DEFAULT_SLIDE));

	div.append_child(&canvas).unwrap();
	div.append_child(&create_controls(viewer, di)).unwrap();

	window().unwrap().dispatch_event(&Event::new("resize").unwrap()).unwrap();
}



fn init_canvas(canvas: Arc<WrappedHtml>, viewer: Arc<Mutex<Viewer>>) {
	let window = WrappedHtml::own(window().unwrap());

	// TODO: _also_ use "mousewheel" event to support Safari
	//       https://developer.mozilla.org/en-US/docs/Web/API/Element/mousewheel_event
	canvas.add_listener("wheel", {
		let viewer = viewer.clone();
		move |e: WheelEvent|{
			let viewer = &mut viewer.lock().unwrap();

			let mpos = (e.offset_x() as f64, e.offset_y() as f64);

			let prev = viewer.scale;
			let delta = viewer.scale * -0.1 * (e.delta_y()).signum();
			viewer.scale += delta;

			let before = viewer.camera_pos;
			viewer.camera_pos.0 = (viewer.scale / prev) * (mpos.0 + viewer.camera_pos.0) - mpos.0;
			viewer.camera_pos.1 = (viewer.scale / prev) * (mpos.1 + viewer.camera_pos.1) - mpos.1;

			if let Some(mut tmp) = viewer.cursor_pin {
				tmp.0 -= before.0 - viewer.camera_pos.0;
				tmp.1 -= before.1 - viewer.camera_pos.1;

				viewer.cursor_pin = Some(tmp);
			}

			viewer.draw();
		}
	}).unwrap();


	canvas.add_listener("mousemove", {
		let viewer = viewer.clone();
		move |e: MouseEvent| {
			let viewer = viewer.clone();
			let _ = futures::future_to_promise(async move {
				let viewer: &mut Viewer = &mut viewer.lock().unwrap();

				let mpos = (
					e.x() as f64,
					e.y() as f64,
				);

				let lmb = (e.buttons() & 1) > 0;
				let rmb = (e.buttons() & 2) > 0;
				let shift = e.shift_key();

				// camera pannning
				let pan = rmb && !shift;
				if viewer.cursor_pin.is_none() && pan {
					let mut tmp = mpos;
					tmp.0 += viewer.camera_pos.0;
					tmp.1 += viewer.camera_pos.1;

					viewer.cursor_pin = Some(tmp);
				} else if !pan {
					viewer.cursor_pin = None;
				}

				if let Some(pinpoint) = viewer.cursor_pin {
					viewer.camera_pos.0 = pinpoint.0 - mpos.0;
					viewer.camera_pos.1 = pinpoint.1 - mpos.1;
				}


				// rectangle selection
				let pos = viewer.from_screen_space(mpos);
				let rsel = lmb && shift;
				if (viewer.stopped_sel || viewer.rsel.is_none()) && rsel {
					viewer.rsel = Some((pos, pos));
					viewer.stopped_sel = false;
				} else if !rsel {
					if lmb {
						viewer.rsel = None;
					}
					viewer.stopped_sel = true;
				} else if rsel && let Some(rsel) = viewer.rsel.as_mut() {
					rsel.1 = pos;
				}

				viewer.draw();

				return Ok(JsValue::NULL);
			});
		}
	}).unwrap();


	// toggle cells
	canvas.add_listener("mousedown", {
		let viewer = viewer.clone();
		move |e: MouseEvent|{
			let viewer: &mut Viewer = &mut viewer.lock().unwrap();

			let pos = (
				e.offset_x() as f64,
				e.offset_y() as f64,
			);

			let lmb = (e.buttons() & 1) > 0;
			let shift = e.shift_key();

			if lmb && !shift {
				let pos = viewer.from_screen_space(pos);
				let pos = (
					pos.0.floor() as i64,
					pos.1.floor() as i64,
				).into();

				let alive = viewer.grid.get_cell(&pos);
				let action = Action::SetCell(pos, !alive);
				viewer.do_action(action);

				viewer.draw();
			}
		}
	}).unwrap();


	window.add_listener("keydown", {
		let viewer = viewer.clone();
		move |e: Event| {
			let viewer = viewer.clone();
			let _ = futures::future_to_promise(async move {
				let mut viewer = viewer.lock().unwrap();
				let mut draw_flag = false;

				let key: String = proto_get(e.as_ref(), "key").unwrap().as_string().unwrap();
				let ctrl: bool = proto_get(e.as_ref(), "ctrlKey").unwrap().dyn_into::<Boolean>().unwrap().into();
				let shift: bool = proto_get(e.as_ref(), "shiftKey").unwrap().dyn_into::<Boolean>().unwrap().into();

				let copy = key == "c" && ctrl;
				let paste = key == "v" && ctrl;
				let cut = key == "x" && ctrl;
				let undo = key == "z" && ctrl;
				let redo = ctrl && (key == "Z" || key == "y");

				// copy selection
				if (copy || cut) && let Some(sel) = viewer.get_selection() {
					let x_bound = sel.0.0..sel.1.0;
					let y_bound = sel.0.1..sel.1.0;

					let sel = viewer.grid.get_alive()
						.filter(|pos| x_bound.contains(&pos.x) && y_bound.contains(&pos.y))
						.copied()
						.collect::<HashSet<_>>();

					if cut {
						let action = Action::MultiSetCell(sel.iter().map(|pos| (*pos, false)).collect());
						viewer.do_action(action);
					}

					let grid = Grid::from_iter(
						sel.into_iter().map(|pos| (pos.x - x_bound.start, pos.y - y_bound.start).into())
					);

					to_clipboard(&grid).await;
					draw_flag = true;
				}

				// paste
				if paste {
					let new_grid = from_clipboard().await;
					let action = Action::SetGrid{from: viewer.grid.clone(), to: new_grid};
					viewer.do_action(action);

					viewer.rsel = None;
					draw_flag = true;
				}

				if undo {
					viewer.undo_action();
					draw_flag = true;
				}

				if redo {
					viewer.redo_action();
					draw_flag = true;
				}

				if draw_flag {
					viewer.draw();
				}

				return Ok(JsValue::NULL);
			});
		}
	}).unwrap();


	canvas.add_listener("contextmenu", |e: Event| {
		e.prevent_default();
	}).unwrap();


	let onresize = {
		let viewer = viewer.clone();
		let canvas = canvas.clone();
		move |_: Event|{
			let canvas = canvas.as_elm::<HtmlCanvasElement>().unwrap();
			let viewer: &mut Viewer = &mut viewer.lock().unwrap();

			let width = canvas.client_width() as u32;
			let height = canvas.client_height() as u32;
			if viewer.viewport_dim.0 == width && viewer.viewport_dim.1 == height {
				return;
			}

			viewer.viewport_dim = (width, height);
			canvas.set_width(width);
			canvas.set_height(height);

			viewer.draw();
		}
	};
	onresize(Event::new("resize").unwrap());
	window.add_listener("resize", onresize).unwrap();
}



fn create_load_button(viewer: Arc<Mutex<Viewer>>) -> WrappedHtml {
	let button = WrappedHtml::new("button").unwrap();
	button.as_elm::<Node>().unwrap().set_text_content(Some("Load"));

	button.add_listener("click", {
		move |_: MouseEvent| {
			let viewer = viewer.clone();
			let _ = futures::future_to_promise(async move {
				let grid = from_clipboard().await;

				let mut viewer = viewer.lock().unwrap();
				let action = Action::SetGrid{from: viewer.grid.clone(), to: grid};
				viewer.do_action(action);
				viewer.draw();

				return Ok(JsValue::NULL);
			});
		}
	}).unwrap();

	button
}

fn create_save_button(viewer: Arc<Mutex<Viewer>>) -> WrappedHtml {
	let button = WrappedHtml::new("button").unwrap();
	button.as_elm::<Node>().unwrap().set_text_content(Some("Save"));

	button.add_listener("click", {
		let viewer = viewer.clone();
		move |_: MouseEvent| {
			let viewer = viewer.clone();
			let _ = futures::future_to_promise(async move {
				// NOTE: we clone grid here so that we don't hold onto the lock for
				//       viewer whilst awaiting a promise
				//       as doing so could cause a dead-lock
				let viewer = viewer.lock().unwrap();
				let grid = viewer.grid.clone();
				drop(viewer);

				to_clipboard(&grid).await;

				return Ok(JsValue::NULL);
			});
		}

	}).unwrap();

	button
}

fn create_play_button(viewer: Arc<Mutex<Viewer>>) -> WrappedHtml {
	let button = WrappedHtml::new("button").unwrap();
	button.as_elm::<Node>().unwrap().set_text_content(Some("Play"));

	button.add_listener("click", {
		let viewer = viewer.clone();
		move |e: MouseEvent| {
			let mut viewer = viewer.lock().unwrap();
			viewer.paused = !viewer.paused;

			let node: Node = e.target().unwrap().dyn_into().unwrap();
			node.set_text_content(Some(if viewer.paused { "Play" } else { "Pause" }));
		}
	}).unwrap();

	button
}

const DEFAULT_SLIDE: i32 = 10;
fn from_slider(src: i32) -> Option<i32> {
	(src != 0).then(|| 50_000 / src / src)
}
fn create_slider<F: 'static + Fn()>(di: Arc<Mutex<DynamicInterval<F>>>, label: Arc<WrappedHtml>) -> WrappedHtml {
	let slider = WrappedHtml::new("input").unwrap();
	slider.set("type", &"range".into()).unwrap();
	slider.set("value", &DEFAULT_SLIDE.into()).unwrap();
	slider.add_listener("input", {
		let di = di.clone();
		let label = label.clone();
		move |e: Event| {
			let t = e.target().unwrap();

			let value = proto_get(t.as_ref(), "value").unwrap();
			let value = i32::from_str(&value.as_string().unwrap()).unwrap();
			let value = from_slider(value);

			label.as_elm::<Node>().unwrap().set_text_content(Some(&(if let Some(value) = value {
				format!("{:>5.2}", 1000.0 / value as f64)
			} else {
				"N/A".to_string()
			})));

			DynamicInterval::set_rate(di.clone(), value);
		}
	}).unwrap();

	slider
}

fn create_label() -> WrappedHtml {
	WrappedHtml::new("span").unwrap()
}

fn create_controls<F>(viewer: Arc<Mutex<Viewer>>, di: Arc<Mutex<DynamicInterval<F>>>) -> WrappedHtml
	where F: 'static + Fn()
{
	let controls = WrappedHtml::new("div").unwrap();

	let label = create_label();
	let label = Arc::new(label);

	controls.append_child(&create_slider(di, label.clone())).unwrap();
	controls.append_child(&label).unwrap();

	controls.append_child(&create_play_button(viewer.clone())).unwrap();
	controls.append_child(&create_save_button(viewer.clone())).unwrap();
	controls.append_child(&create_load_button(viewer.clone())).unwrap();

	controls
}
