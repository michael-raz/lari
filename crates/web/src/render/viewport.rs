use super::*;

use slotmap::DefaultKey;
use slotmap::basic::SlotMap;

type ViewportKey = DefaultKey;

#[derive(Default)]
struct Viewports {
    map: SlotMap<ViewportKey, Viewport>,
}

impl Viewports {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, grid: Grid) -> ViewportKey {
        let canvas = WrappedHtml::new("canvas").unwrap();
        let ctx = canvas
            .as_elm::<HtmlCanvasElement>()
            .unwrap()
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();

        let out = Viewport {
            grid,
            canvas,
            ctx,
            dim: Default::default(),
            camera_pos: Default::default(),
            scale: 1.0,
        };

        self.map.insert(out)
    }

    fn get(&self, key: ViewportKey) -> Option<&'_ Viewport> {
        self.map.get(key)
    }

    fn get_mut(&mut self, key: ViewportKey) -> Option<&'_ mut Viewport> {
        self.map.get_mut(key)
    }
}

pub struct Viewport {
    pub grid: Grid,
    pub canvas: WrappedHtml,
    pub ctx: CanvasRenderingContext2d,

    pub dim: Vec2<u32>,
    pub camera_pos: Vec2<f64>,
    pub scale: f64,
}

impl Viewport {
    pub fn map_from_screen_space(&self, pos: Vec2<f64>) -> Vec2<f64> {
        (pos + self.camera_pos) / self.scale / CELL_SIZE
    }

    pub fn map_to_screen_space(&self, pos: Vec2) -> Vec2<f64> {
        vec2_cast!(pos => f64) * self.scale * CELL_SIZE - self.camera_pos
    }

    pub fn draw(&self) {
        const DEAD_COLOR: &str = "#0f0f0f";
        const ALIVE_COLOR: &str = "#f0f0f0";
        let size = CELL_SIZE * self.scale;

        self.ctx.set_fill_style_str(DEAD_COLOR);
        self.ctx
            .fill_rect(0.0, 0.0, self.dim.x as f64, self.dim.y as f64);

        // group cells together and draw them as one
        self.ctx.set_fill_style_str(ALIVE_COLOR);
        let mut unsued_cells = HashSet::<&Vec2>::from_iter(self.grid.alive());
        for pos in self.grid.alive() {
            if !unsued_cells.remove(pos) {
                continue;
            }

            // get taxicab flood fill of pos
            let mut group = vec![pos];
            let mut i = 0;
            while i < group.len() {
                let p = group[i];

                group.extend(
                    [vec2![-1, 0], vec2![1, 0], vec2![0, -1], vec2![0, 1]]
                        .into_iter()
                        .filter_map(|offset| unsued_cells.take(&(offset + *p))),
                );

                i += 1;
            }

            // convert cells into lines
            let mut lines = group
                .into_iter()
                .flat_map(|pos: &Vec2| {
                    [
                        Line {
                            a: pos.add(vec2![0, 0]),
                            b: pos.add(vec2![1, 0]),
                        },
                        Line {
                            a: pos.add(vec2![1, 0]),
                            b: pos.add(vec2![1, 1]),
                        },
                        Line {
                            a: pos.add(vec2![1, 1]),
                            b: pos.add(vec2![0, 1]),
                        },
                        Line {
                            a: pos.add(vec2![0, 1]),
                            b: pos.add(vec2![0, 0]),
                        },
                    ]
                    .into_iter()
                })
                .collect::<Vec<_>>();

            exact_contour(&mut lines);
            merge_lines(&mut lines);

            // draw cells
            self.ctx.begin_path();
            while let Some(line) = lines.pop() {
                macro_rules! with_screen_pos {
                    ($($func:tt).*($pos:expr)) => {
                        $($func).*(
                            ($pos.x as f64 * size) - self.camera_pos.x,
                            ($pos.y as f64 * size) - self.camera_pos.y,
                        )
                    };
                }

                with_screen_pos!(self.ctx.move_to(line.a));
                with_screen_pos!(self.ctx.line_to(line.b));

                let mut prev = line.b;
                loop {
                    let curr = lines
                        .extract_if(.., |other| other.a == prev || other.b == prev)
                        .next();
                    if let Some(curr) = curr {
                        let curr = if curr.a == prev { curr.b } else { curr.a };
                        with_screen_pos!(self.ctx.line_to(curr));

                        prev = curr;
                    } else {
                        break;
                    }
                }

                with_screen_pos!(self.ctx.line_to(line.a));
            }
            let fill = proto_get(&self.ctx, "fill").unwrap();
            let fill = fill.dyn_into::<Function>().unwrap();
            fill.call(&self.ctx, (&"evenodd".into(),)).unwrap();
        }
    }
}

pub struct InteractiveViewer {
    vps: Viewports,
    vp: ViewportKey,

    pub paused: bool,

    pub blueprint_div: WrappedHtml,

    // NOTE: these are in viewer because
    //       panning and zooming both need to modify them
    pub cursor_pin: Option<Vec2<f64>>,
    pub rsel: Option<(Vec2<f64>, Vec2<f64>)>,

    pub stopped_sel: bool,
    pub action_log: VecDeque<Action>,
    pub action_idx: usize,
}
impl InteractiveViewer {
    pub fn new(grid: Grid) -> Self {
        let mut vps = Viewports::new();
        let vp = vps.insert(grid);
        Self {
            vps,
            vp,
            paused: true,
            blueprint_div: WrappedHtml::new("div").unwrap(),
            cursor_pin: None,
            rsel: None,
            stopped_sel: true,
            action_log: Default::default(),
            action_idx: 0,
        }
    }

    pub fn vp(&self) -> &'_ Viewport {
        self.vps.get(self.vp).unwrap()
    }

    pub fn mut_vp(&mut self) -> &'_ mut Viewport {
        self.vps.get_mut(self.vp).unwrap()
    }

    fn apply_action(&mut self) {
        let vp = self.vps.get_mut(self.vp).unwrap();

        match self.action_log[self.action_idx] {
            Action::SetCell(pos, value) => vp.grid.set(pos, value),
            Action::MultiSetCell(ref delta) => delta
                .iter()
                .for_each(|&(pos, value)| vp.grid.set(pos, value)),
            Action::SetGrid { ref to, .. } => vp.grid = to.clone(),
        }
        self.action_idx += 1;
    }
    fn unapply_action(&mut self) {
        let vp = self.vps.get_mut(self.vp).unwrap();

        self.action_idx -= 1;
        match self.action_log[self.action_idx] {
            Action::SetCell(pos, value) => vp.grid.set(pos, !value),
            Action::MultiSetCell(ref delta) => delta
                .iter()
                .for_each(|&(pos, value)| vp.grid.set(pos, !value)),
            Action::SetGrid { ref from, .. } => vp.grid = from.clone(),
        }
    }

    const MAX_UNDOS: usize = 100;
    pub fn do_action(&mut self, action: Action) {
        self.action_log.drain(self.action_idx..);

        let next_len = self.action_log.len() + 1;
        let overflow = next_len - next_len.min(Self::MAX_UNDOS);
        self.action_log.drain(..overflow);
        self.action_idx -= overflow;

        self.action_log.push_back(action);
        self.apply_action();
    }
    pub fn undo_action(&mut self) {
        if self.action_idx == 0 {
            // we're already at the start of the undo list
            // so do nothing
            return;
        }

        self.unapply_action();
    }
    pub fn redo_action(&mut self) {
        if self.action_idx >= self.action_log.len() {
            // we're already at the end of the undo list
            // so do nothing
            return;
        }

        self.apply_action();
    }

    pub fn get_selection(&self) -> Option<(Vec2, Vec2)> {
        self.rsel.map(|(start, end)| {
            (
                (
                    start.x.min(end.x).floor() as i64,
                    start.y.min(end.y).floor() as i64,
                )
                    .into(),
                (
                    start.x.max(end.x).ceil() as i64,
                    start.y.max(end.y).ceil() as i64,
                )
                    .into(),
            )
        })
    }

    pub fn draw(&self) {
        let vp = self.vp();
        vp.draw();

        const RSEL_COLOR: &str = "#ff0000";

        // draw rectangle selection
        if let Some((start, end)) = self.get_selection() {
            let start = vp.map_to_screen_space(start);
            let end = vp.map_to_screen_space(end);

            vp.ctx.set_stroke_style_str(RSEL_COLOR);
            vp.ctx.stroke_rect(start.x, start.y, end.x - start.x, end.y - start.y);
        }
    }

    pub fn add_blueprint(this: Arc<Mutex<Self>>, grid: Grid) {
        // make it so the most top left cell is at (0, 0)
        let min = grid.aabb().0;
        let grid = Grid::from_iter(grid.into_iter().map(|p| p - min));

        let div = WrappedHtml::new("div").unwrap();
        set_style!((&div){
            "display": "flex";
            "flex-direction": "column";
            "width": "100%";
        });
        this.lock()
            .unwrap()
            .blueprint_div
            .append_child(&div)
            .unwrap();

        let vps = &mut this.lock().unwrap().vps;

        let bp = vps.insert(grid);
        {
            let bp = vps.get(bp).unwrap();
            set_style!((&bp.canvas){
                "width": "100%";
                "aspect-ratio": "1";
            });
            div.append_child(&bp.canvas).unwrap();
        }

        let buttons = WrappedHtml::new("div").unwrap();
        set_style!((&buttons){
            "width": "100%";
            "direction": "initial";
        });
        div.append_child(&buttons).unwrap();

        let load = create_button("Load", {
            let this = this.clone();
            move |_| {
                let mut this = this.lock().unwrap();

                let prev = this.vp().grid.clone();
                let next = this.vps.get(bp).unwrap().grid.clone();
                this.do_action(Action::SetGrid {
                    from: prev,
                    to: next,
                });
                this.draw();
            }
        });
        buttons.append_child(&load).unwrap();

        let copy = create_button("Copy", {
            let this = this.clone();
            move |_| {
                let this = this.clone();
                let _ = futures::future_to_promise({
                    async move {
                        let mut this = this.lock().unwrap();

                        let grid = this.vps.get(bp).unwrap().grid.clone();
                        to_clipboard(&grid).await;

                        Ok(JsValue::NULL)
                    }
                });
            }
        });
        buttons.append_child(&copy).unwrap();

        canvas_resize({
            let this = this.clone();
            move |callback| {
                if let Ok(mut this) = this.try_lock() {
                    let bp = this.vps.get_mut(bp).unwrap();
                    callback(bp);

                    let bb = bp.grid.aabb();

                    let g_width = ((bb.1.x - bb.0.x) as f64 + 1.0) * CELL_SIZE;
                    let g_height = ((bb.1.y - bb.0.y) as f64 + 1.0) * CELL_SIZE;
                    let g_max = g_width.max(g_height);

                    let (c_width, c_height) = bp.dim.cast_inner::<f64>().into();
                    let c_max = c_width.max(c_height);

                    // make it so the entire grid is visible
                    bp.scale = c_max / g_max;
                    bp.camera_pos.x = -(c_width - g_width * bp.scale) / 2.0;
                    bp.camera_pos.y = -(c_height - g_height * bp.scale) / 2.0;

                    bp.draw();
                }
            }
        });
    }
}
