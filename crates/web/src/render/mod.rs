use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

type Am<T> = Arc<Mutex<T>>;

use lari::*;

use crate::util::*;

use crate::dom::*;
use crate::wasm_helpers::*;

mod viewport;
use viewport::*;

const CELL_SIZE: f64 = 50.0;

#[derive(Clone)]
struct Line {
    a: Vec2,
    b: Vec2,
}
impl Line {
    fn parallel(&self, other: &Self) -> bool {
        (self.a.x == self.b.x && other.a.x == other.b.x)
            || (self.a.y == self.b.y && other.a.y == other.b.y)
    }

    fn joint(&self, other: &Self) -> bool {
        self.a == other.a || self.a == other.b || self.b == other.a || self.b == other.b
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
        (self.a == other.a && self.b == other.b) || (self.a == other.b && self.b == other.a)
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
        self.a
            .min(self.b)
            .cmp(&other.a.min(other.b))
            .then(self.a.max(self.b).cmp(&other.a.max(other.b)))
    }
}

fn merge_lines(lines: &mut Vec<Line>) {
    let mut i = 0;
    while i < lines.len() {
        let others = (i + 1..lines.len())
            .rev()
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

        let count = 2 + lines[i + 2..]
            .iter()
            .take_while(|&x| x == &lines[i])
            .count();
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
    fn new(window: Window, callback: F, rate: Option<u64>) -> Am<Self> {
        let out = Arc::new(Mutex::new(Self {
            rate,
            callback,
            window,
            prev: None,
        }));

        Self::repeat(out.clone());

        out
    }

    fn repeat<T: Fn() + 'static>(me: Am<DynamicInterval<T>>) {
        let mut locked = me.lock().unwrap();
        if let Some(rate) = locked.rate {
            let cur = set_timeout(
                &locked.window,
                {
                    let me = me.clone();
                    move |_: Event| {
                        (me.lock().unwrap().callback)();
                        Self::repeat(me.clone());
                    }
                },
                rate as i32,
            )
            .unwrap();
            locked.prev.replace(cur);
        } else {
            locked.prev.take();
        }
    }

    fn set_rate(me: Am<Self>, rate: Option<i32>) {
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
    SetGrid { from: Grid, to: Grid },
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
    let write: Promise = write
        .call(&clip, (&out.into(),))
        .unwrap()
        .dyn_into()
        .unwrap();

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
        "flex-direction": "row";
        "width": "100%";
        "height": "100%";
    });
    body.append_child(&div).unwrap();

    let side = WrappedHtml::new("div").unwrap();
    set_style!((&side){
        "display": "flex";
        "flex-direction": "column";
        "width": "20%";
        "height": "100%";
    });
    div.append_child(&side).unwrap();

    let main = WrappedHtml::new("div").unwrap();
    set_style!((&main){
        "display": "flex";
        "flex-direction": "column";
        "width": "100%";
        "height": "100%";
    });
    div.append_child(&main).unwrap();

    // acorn
    let grid = Grid::from_table(&[
        [0, 1, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 0, 0],
        [1, 1, 0, 0, 1, 1, 1],
    ]);

    let viewer = InteractiveViewer::new(grid);
    let vp = viewer.vp();
    set_style!((&vp.canvas){
        "width": "100%";
        "height": "100%";
    });
    main.append_child(&vp.canvas).unwrap();

    set_style!((&viewer.blueprint_div){
        "display": "flex";
        "flex-direction": "column";
        "width": "100%";
        "height": "100%";
        "overflow": "scroll";
        "direction": "rtl";
    });
    side.append_child(&viewer.blueprint_div).unwrap();

    let viewer = Arc::new(Mutex::new(viewer));
    viewer.lock().unwrap().draw();

    init_canvas(viewer.clone());

    side.append_child(&create_button("Add", {
        let viewer = viewer.clone();
        move |_| {
            let viewer = viewer.clone();
            let _ = futures::future_to_promise(async move {
                let grid = from_clipboard().await;

                InteractiveViewer::add_blueprint(viewer, grid);

                Ok(JsValue::NULL)
            });
        }
    }))
    .unwrap();

    let di = DynamicInterval::new(
        w,
        {
            let viewer = viewer.clone();
            move || {
                let viewer: &mut InteractiveViewer = &mut viewer.lock().unwrap();
                if !viewer.paused {
                    let mut vp = viewer.mut_vp();
                    vp.grid.step(1);
                    viewer.draw();
                }
            }
        },
        None,
    );
    DynamicInterval::set_rate(di.clone(), from_slider(DEFAULT_SLIDE));

    main.append_child(&create_controls(viewer, di)).unwrap();

    window()
        .unwrap()
        .dispatch_event(&Event::new("resize").unwrap())
        .unwrap();
}

fn init_canvas(viewer: Am<InteractiveViewer>) {
    let window = WrappedHtml::own(window().unwrap());
    let mut viewer_ref = viewer.lock().unwrap();
    let mut canvas = &mut viewer_ref.mut_vp().canvas;

    // TODO: _also_ use "mousewheel" event to support Safari
    //       https://developer.mozilla.org/en-US/docs/Web/API/Element/mousewheel_event
    canvas.add_listener("wheel", {
            let viewer = viewer.clone();
            move |e: WheelEvent| {
                let mut viewer = viewer.lock().unwrap();
                let vp = viewer.mut_vp();

                let mpos: Vec2<f64> = (e.offset_x() as f64, e.offset_y() as f64).into();

                let prev = vp.scale;
                let delta = vp.scale * -0.1 * (e.delta_y()).signum();
                vp.scale += delta;

                let before = vp.camera_pos;
                vp.camera_pos =
                    (mpos + vp.camera_pos) * (vp.scale / prev) - mpos;

                if let Some(mut tmp) = viewer.cursor_pin {
                    tmp -= before - viewer.vp().camera_pos;

                    viewer.cursor_pin = Some(tmp);
                }

                viewer.draw();
            }
        })
        .unwrap();

    canvas
        .add_listener("mousemove", {
            let viewer = viewer.clone();
            move |e: MouseEvent| {
                let viewer = viewer.clone();
                let _ = futures::future_to_promise(async move {
                    let mut viewer = viewer.lock().unwrap();

                    let mpos: Vec2<f64> = (e.offset_x() as f64, e.offset_y() as f64).into();

                    let lmb = (e.buttons() & 1) > 0;
                    let rmb = (e.buttons() & 2) > 0;
                    let shift = e.shift_key();

                    // camera pannning
                    let pan = rmb && !shift;
                    if viewer.cursor_pin.is_none() && pan {
                        viewer.cursor_pin = Some(mpos + viewer.vp().camera_pos);
                    } else if !pan {
                        viewer.cursor_pin = None;
                    }

                    if let Some(pinpoint) = viewer.cursor_pin {
                        let vp = viewer.mut_vp();
                        vp.camera_pos = pinpoint - mpos;
                    }

                    // rectangle selection
                    let pos = viewer.vp().map_from_screen_space(mpos);
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

                    Ok(JsValue::NULL)
                });
            }
        })
        .unwrap();

    // toggle cells
    canvas
        .add_listener("mousedown", {
            let viewer = viewer.clone();
            move |e: MouseEvent| {
                let mut viewer = viewer.lock().unwrap();
                let vp = viewer.mut_vp();

                let pos: Vec2<f64> = (e.offset_x() as f64, e.offset_y() as f64).into();

                let lmb = (e.buttons() & 1) > 0;
                let shift = e.shift_key();

                if lmb && !shift {
                    let pos = vp.map_from_screen_space(pos);
                    let pos = (pos.x.floor() as i64, pos.y.floor() as i64).into();

                    let alive = vp.grid.get(&pos);
                    let action = Action::SetCell(pos, !alive);
                    viewer.do_action(action);

                    viewer.draw();
                }
            }
        })
        .unwrap();

    window
        .add_listener("keydown", {
            let viewer = viewer.clone();
            move |e: Event| {
                let viewer = viewer.clone();
                let _ = futures::future_to_promise(async move {
                    let mut draw_flag = false;

                    let key: String = proto_get(e.as_ref(), "key").unwrap().as_string().unwrap();
                    let ctrl: bool = proto_get(e.as_ref(), "ctrlKey")
                        .unwrap()
                        .dyn_into::<Boolean>()
                        .unwrap()
                        .into();
                    let shift: bool = proto_get(e.as_ref(), "shiftKey")
                        .unwrap()
                        .dyn_into::<Boolean>()
                        .unwrap()
                        .into();

                    let copy = key == "c" && ctrl;
                    let paste = key == "v" && ctrl;
                    let cut = key == "x" && ctrl;
                    let undo = key == "z" && ctrl;
                    let redo = ctrl && (key == "Z" || key == "y");

                    let selection = viewer.lock().unwrap().get_selection();

                    // copy selection
                    if (copy || cut)
                        && let Some(sel) = selection
                    {
                        let grid = {
                            let mut viewer = viewer.lock().unwrap();
                            let vp = viewer.mut_vp();

                            let x_bound = sel.0.x..sel.1.x;
                            let y_bound = sel.0.y..sel.1.y;

                            let sel = vp
                                .grid
                                .alive()
                                .filter(|pos| x_bound.contains(&pos.x) && y_bound.contains(&pos.y))
                                .copied()
                                .collect::<HashSet<_>>();

                            if cut {
                                let action = Action::MultiSetCell(
                                    sel.iter().map(|pos| (*pos, false)).collect(),
                                );
                                viewer.do_action(action);
                            }

                            Grid::from_iter(
                                sel.into_iter().map(|pos| {
                                    (pos.x - x_bound.start, pos.y - y_bound.start).into()
                                }),
                            )
                        };

                        to_clipboard(&grid).await;
                        draw_flag = true;
                    }

                    // paste
                    if paste {
                        let new_grid = from_clipboard().await;

                        let mut viewer = viewer.lock().unwrap();
                        let vp = viewer.mut_vp();
                        let action = Action::SetGrid {
                            from: vp.grid.clone(),
                            to: new_grid,
                        };
                        viewer.do_action(action);

                        viewer.rsel = None;
                        draw_flag = true;
                    }

                    if undo {
                        viewer.lock().unwrap().undo_action();
                        draw_flag = true;
                    }

                    if redo {
                        viewer.lock().unwrap().redo_action();
                        draw_flag = true;
                    }

                    if draw_flag {
                        viewer.lock().unwrap().draw();
                    }

                    Ok(JsValue::NULL)
                });
            }
        })
        .unwrap();

    canvas
        .add_listener("contextmenu", |e: Event| {
            e.prevent_default();
        })
        .unwrap();

    canvas_resize({
        let viewer = viewer.clone();
        move |callback| {
            if let Ok(mut viewer) = viewer.try_lock() {
                let vp = viewer.mut_vp();
                callback(vp);
                viewer.draw();
            }
        }
    });
}

fn canvas_resize<B>(mut vp: B)
// something like `for<'a> B: 'static FnMut<'a>() -> &'a mut Viewport` would be better
// but requires generic lifetime parameter for FnMut
where
    B: 'static + FnMut(&mut dyn FnMut(&mut Viewport)),
{
    let window = WrappedHtml::own(window().unwrap());

    let mut onresize = {
        move |_: Event| {
            vp(&mut |vp: &mut Viewport| {
                let canvas = vp.canvas.as_elm::<HtmlCanvasElement>().unwrap();

                let width = canvas.client_width() as u32;
                let height = canvas.client_height() as u32;
                if vp.dim.x == width && vp.dim.y == height {
                    return;
                }

                vp.dim = vec2![width, height];
                canvas.set_width(width);
                canvas.set_height(height);
            });
        }
    };
    onresize(Event::new("resize").unwrap());
    window.add_listener("resize", onresize).unwrap();
}

fn create_button<'a>(
    text: impl Into<Cow<'a, str>>,
    callback: impl 'static + FnMut(MouseEvent),
) -> WrappedHtml {
    let button = WrappedHtml::new("button").unwrap();
    button
        .as_elm::<Node>()
        .unwrap()
        .set_text_content(Some(text.into().as_ref()));

    button.add_listener("click", callback).unwrap();

    button
}

fn create_load_button(viewer: Am<InteractiveViewer>) -> WrappedHtml {
    create_button("Load", move |_| {
        let viewer = viewer.clone();
        let _ = futures::future_to_promise(async move {
            let grid = from_clipboard().await;

            let mut viewer = viewer.lock().unwrap();
            let vp = viewer.mut_vp();

            let action = Action::SetGrid {
                from: vp.grid.clone(),
                to: grid,
            };
            viewer.do_action(action);
            viewer.draw();

            Ok(JsValue::NULL)
        });
    })
}

fn create_save_button(viewer: Am<InteractiveViewer>) -> WrappedHtml {
    create_button("Save", move |_| {
        let viewer = viewer.clone();
        let _ = futures::future_to_promise(async move {
            // NOTE: we clone grid here so that we don't hold onto the lock for
            //       viewer whilst awaiting a promise
            //       as doing so could cause a dead-lock
            let grid = {
                let viewer = viewer.lock().unwrap();
                let vp = viewer.vp();
                vp.grid.clone()
            };

            to_clipboard(&grid).await;

            Ok(JsValue::NULL)
        });
    })
}

fn create_play_button(viewer: Am<InteractiveViewer>) -> WrappedHtml {
    let button = WrappedHtml::new("button").unwrap();
    button
        .as_elm::<Node>()
        .unwrap()
        .set_text_content(Some("Play"));

    button
        .add_listener("click", {
            let viewer = viewer.clone();
            move |e: MouseEvent| {
                let mut viewer = viewer.lock().unwrap();
                viewer.paused = !viewer.paused;

                let node: Node = e.target().unwrap().dyn_into().unwrap();
                node.set_text_content(Some(if viewer.paused { "Play" } else { "Pause" }));
            }
        })
        .unwrap();

    button
}

const DEFAULT_SLIDE: i32 = 10;
fn from_slider(src: i32) -> Option<i32> {
    (src != 0).then(|| 50_000 / src / src)
}
fn create_slider<F: 'static + Fn()>(
    di: Am<DynamicInterval<F>>,
    label: Arc<WrappedHtml>,
) -> WrappedHtml {
    let slider = WrappedHtml::new("input").unwrap();
    slider.set("type", &"range".into()).unwrap();
    slider.set("value", &DEFAULT_SLIDE.into()).unwrap();
    slider
        .add_listener("input", {
            let di = di.clone();
            let label = label.clone();
            move |e: Event| {
                let t = e.target().unwrap();

                let value = proto_get(t.as_ref(), "value").unwrap();
                let value = i32::from_str(&value.as_string().unwrap()).unwrap();
                let value = from_slider(value);

                label.as_elm::<Node>().unwrap().set_text_content(Some(
                    &(if let Some(value) = value {
                        format!("{:>5.2}", 1000.0 / value as f64)
                    } else {
                        "N/A".to_string()
                    }),
                ));

                DynamicInterval::set_rate(di.clone(), value);
            }
        })
        .unwrap();

    slider
}

fn create_label() -> WrappedHtml {
    WrappedHtml::new("span").unwrap()
}

fn create_controls<F>(
    viewer: Am<InteractiveViewer>,
    di: Am<DynamicInterval<F>>,
) -> WrappedHtml
where
    F: 'static + Fn(),
{
    let controls = WrappedHtml::new("div").unwrap();

    let label = create_label();
    let label = Arc::new(label);

    controls
        .append_child(&create_slider(di, label.clone()))
        .unwrap();
    controls.append_child(&label).unwrap();

    controls
        .append_child(&create_play_button(viewer.clone()))
        .unwrap();
    controls
        .append_child(&create_save_button(viewer.clone()))
        .unwrap();
    controls
        .append_child(&create_load_button(viewer.clone()))
        .unwrap();

    controls
}
