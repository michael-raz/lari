use super::*;

pub struct Viewport {
    pub grid: Grid,
    pub canvas: WrappedHtml,
    pub ctx: CanvasRenderingContext2d,

    pub dim: Vec2<u32>,
    pub camera_pos: Vec2<f64>,
    pub scale: f64,
}
impl Viewport {
    pub fn new(grid: Grid) -> Self {
        let canvas = WrappedHtml::new("canvas").unwrap();
        let ctx = canvas
            .as_elm::<HtmlCanvasElement>()
            .unwrap()
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();
        Self {
            grid,
            canvas,
            ctx,
            dim: Default::default(),
            camera_pos: Default::default(),
            scale: 1.0,
        }
    }

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
    pub vp: Viewport,

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
        Self {
            vp: Viewport::new(grid),
            paused: true,
            blueprint_div: WrappedHtml::new("div").unwrap(),
            cursor_pin: None,
            rsel: None,
            stopped_sel: true,
            action_log: Default::default(),
            action_idx: 0,
        }
    }

    fn apply_action(&mut self) {
        match self.action_log[self.action_idx] {
            Action::SetCell(pos, value) => self.vp.grid.set(pos, value),
            Action::MultiSetCell(ref delta) => delta
                .iter()
                .for_each(|&(pos, value)| self.vp.grid.set(pos, value)),
            Action::SetGrid { ref to, .. } => self.vp.grid = to.clone(),
        }
        self.action_idx += 1;
    }
    fn unapply_action(&mut self) {
        self.action_idx -= 1;
        match self.action_log[self.action_idx] {
            Action::SetCell(pos, value) => self.vp.grid.set(pos, !value),
            Action::MultiSetCell(ref delta) => delta
                .iter()
                .for_each(|&(pos, value)| self.vp.grid.set(pos, !value)),
            Action::SetGrid { ref from, .. } => self.vp.grid = from.clone(),
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
        self.vp.draw();

        const RSEL_COLOR: &str = "#ff0000";

        // draw rectangle selection
        if let Some((start, end)) = self.get_selection() {
            let start = self.vp.map_to_screen_space(start);
            let end = self.vp.map_to_screen_space(end);

            self.vp.ctx.set_stroke_style_str(RSEL_COLOR);
            self.vp
                .ctx
                .stroke_rect(start.x, start.y, end.x - start.x, end.y - start.y);
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

        let bp = Viewport::new(grid);
        set_style!((&bp.canvas){
            "width": "100%";
            "aspect-ratio": "1";
        });
        div.append_child(&bp.canvas).unwrap();

        let buttons = WrappedHtml::new("div").unwrap();
        set_style!((&buttons){
            "width": "100%";
            "direction": "initial";
        });
        div.append_child(&buttons).unwrap();

        let bp = Arc::new(Mutex::new(bp));

        let load = create_button("Load", {
            let this = this.clone();
            let bp = bp.clone();
            move |_| {
                let mut this = this.lock().unwrap();
                let bp = bp.lock().unwrap();

                let prev = this.vp.grid.clone();
                this.do_action(Action::SetGrid {
                    from: prev,
                    to: bp.grid.clone(),
                });
                this.draw();
            }
        });
        buttons.append_child(&load).unwrap();

        let copy = create_button("Copy", {
            let bp = bp.clone();
            move |_| {
                let bp = bp.clone();
                let _ = futures::future_to_promise(async move {
                    let grid = bp.lock().unwrap().grid.clone();
                    to_clipboard(&grid).await;

                    Ok(JsValue::NULL)
                });
            }
        });
        buttons.append_child(&copy).unwrap();

        canvas_resize({
            let bp = bp.clone();
            move |callback| {
                if let Ok(mut bp) = bp.try_lock() {
                    callback(&mut bp);

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
