pub use crate::branding::palette::*;
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2, pos2, vec2,
};
use linkshop::model::{Pair, ensure_blank_row};

pub const ROW: f32 = 32.0;

pub fn strong_font(size: f32) -> FontId {
    #[cfg(target_os = "windows")]
    {
        FontId::new(size, egui::FontFamily::Name("LinkShop-Semibold".into()))
    }
    #[cfg(not(target_os = "windows"))]
    {
        FontId::proportional(size)
    }
}

pub fn ellipsis_label(ui: &Ui, rect: Rect, value: &str, font: FontId, color: Color32) {
    if rect.width() <= 0. {
        return;
    }
    let mut job = egui::text::LayoutJob::simple(value.into(), font, color, rect.width());
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    ui.painter().with_clip_rect(rect).galley(
        pos2(rect.left(), rect.center().y - galley.size().y / 2.),
        galley,
        color,
    );
}

pub fn method_color(method: &str) -> Color32 {
    match method {
        "GET" | "HEAD" => GREEN,
        "POST" => Color32::from_rgb(167, 133, 27),
        "PUT" => ACCENT,
        "PATCH" => Color32::from_rgb(104, 61, 144),
        "DELETE" => DANGER,
        "OPTIONS" => Color32::from_rgb(163, 45, 115),
        _ => MUTED,
    }
}

pub fn row(ui: &mut Ui, height: f32) -> Rect {
    ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover())
        .0
}

pub fn label(ui: &Ui, rect: Rect, value: &str, size: f32, color: Color32) {
    ui.painter().with_clip_rect(rect).text(
        pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        value,
        FontId::proportional(size),
        color,
    );
}

pub fn hline(ui: &Ui, x1: f32, x2: f32, y: f32) {
    ui.painter()
        .line_segment([pos2(x1, y), pos2(x2, y)], Stroke::new(1.0, LINE));
}

pub fn vline(ui: &Ui, x: f32, y1: f32, y2: f32) {
    ui.painter()
        .line_segment([pos2(x, y1), pos2(x, y2)], Stroke::new(1.0, LINE));
}

#[derive(Clone, Copy)]
pub enum Icon {
    Search,
    Plus,
    Close,
    Down,
    Right,
    Collapse,
    More,
    Trash,
    Duplicate,
    Save,
    Folder,
    History,
    Collection,
    Code,
    Check,
    Grip,
}

pub fn icon(ui: &Ui, center: Pos2, kind: Icon, color: Color32) {
    let p = ui.painter();
    let s = Stroke::new(1.15, color);
    let pt = |x: f32, y: f32| center + vec2(x, y);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], s);
    };
    match kind {
        Icon::Search => {
            p.circle_stroke(pt(-1.5, -1.5), 4.5, s);
            line((2., 2.), (6., 6.));
        }
        Icon::Plus => {
            line((-5., 0.), (5., 0.));
            line((0., -5.), (0., 5.));
        }
        Icon::Close => {
            line((-3.5, -3.5), (3.5, 3.5));
            line((3.5, -3.5), (-3.5, 3.5));
        }
        Icon::Down => {
            line((-3.5, -1.5), (0., 2.));
            line((0., 2.), (3.5, -1.5));
        }
        Icon::Right => {
            line((-1.5, -3.5), (2., 0.));
            line((2., 0.), (-1.5, 3.5));
        }
        Icon::Collapse => {
            line((-5., 2.), (-5., -5.));
            line((-5., -5.), (2., -5.));
            p.rect_stroke(
                Rect::from_min_max(pt(-2., -2.), pt(6., 6.)),
                1.,
                s,
                StrokeKind::Inside,
            );
            line((0., 2.), (4., 2.));
        }
        Icon::More => {
            for x in [-4., 0., 4.] {
                p.circle_filled(pt(x, 0.), 1., color);
            }
        }
        Icon::Check => {
            line((-4., 0.), (-1., 3.));
            line((-1., 3.), (4., -3.));
        }
        Icon::Trash => {
            line((-5., -3.), (5., -3.));
            line((-2., -5.), (2., -5.));
            line((-3.5, -2.), (-3., 5.));
            line((-3., 5.), (3., 5.));
            line((3., 5.), (3.5, -2.));
            line((-1., -1.), (-1., 3.));
            line((1., -1.), (1., 3.));
        }
        Icon::Duplicate => {
            p.rect_stroke(
                Rect::from_min_max(pt(-2., -2.), pt(5., 5.)),
                1.,
                s,
                StrokeKind::Inside,
            );
            line((-5., 2.), (-5., -5.));
            line((-5., -5.), (2., -5.));
        }
        Icon::Save => {
            p.rect_stroke(
                Rect::from_min_max(pt(-6., -6.), pt(6., 6.)),
                1.,
                s,
                StrokeKind::Inside,
            );
            p.rect_stroke(
                Rect::from_min_max(pt(-3., -6.), pt(3., -1.)),
                0.,
                s,
                StrokeKind::Inside,
            );
            p.rect_stroke(
                Rect::from_min_max(pt(-3., 2.), pt(3., 6.)),
                0.,
                s,
                StrokeKind::Inside,
            );
        }
        Icon::Folder => {
            p.add(egui::Shape::closed_line(
                vec![
                    pt(-6., -5.),
                    pt(-1., -5.),
                    pt(1., -3.),
                    pt(6., -3.),
                    pt(6., 5.),
                    pt(-6., 5.),
                ],
                s,
            ));
        }
        Icon::Collection => {
            p.rect_stroke(
                Rect::from_min_max(pt(-6., -4.), pt(6., 6.)),
                1.,
                s,
                StrokeKind::Inside,
            );
            line((-4., -7.), (4., -7.));
            line((-2., 0.), (2., 0.));
        }
        Icon::History => {
            p.circle_stroke(center, 6., s);
            line((0., -4.), (0., 0.));
            line((0., 0.), (3., 2.));
        }
        Icon::Code => {
            line((-4., -4.), (-7., 0.));
            line((-7., 0.), (-4., 4.));
            line((4., -4.), (7., 0.));
            line((7., 0.), (4., 4.));
            line((2., -6.), (-2., 6.));
        }
        Icon::Grip => {
            for x in [-2., 2.] {
                for y in [-4., 0., 4.] {
                    p.circle_filled(pt(x, y), 0.7, color);
                }
            }
        }
    }
}

pub fn icon_button(
    ui: &mut Ui,
    rect: Rect,
    id: impl std::hash::Hash,
    kind: Icon,
    hint: &str,
) -> Response {
    let response = ui
        .interact(rect, ui.id().with(id), Sense::click())
        .on_hover_text(hint)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_filled(rect.shrink(2.), 4., HOVER);
    }
    icon(ui, rect.center(), kind, MUTED);
    response
}

pub fn text_button(ui: &mut Ui, rect: Rect, id: impl std::hash::Hash, value: &str) -> Response {
    let response = ui
        .interact(rect, ui.id().with(id), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_filled(rect, 4., HOVER);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        value,
        FontId::proportional(12.),
        TEXT,
    );
    response
}

pub fn tab_button(
    ui: &mut Ui,
    rect: Rect,
    id: impl std::hash::Hash,
    value: &str,
    selected: bool,
) -> Response {
    let response = ui
        .interact(rect, ui.id().with(id), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    label(
        ui,
        rect.shrink2(vec2(12., 0.)),
        value,
        12.,
        if selected { ACCENT } else { MUTED },
    );
    if selected {
        ui.painter().line_segment(
            [
                pos2(rect.left() + 12., rect.bottom() - 1.),
                pos2(rect.right() - 12., rect.bottom() - 1.),
            ],
            Stroke::new(2., ACCENT),
        );
    }
    response
}

pub fn field(
    ui: &mut Ui,
    rect: Rect,
    id: impl std::hash::Hash,
    value: &mut String,
    hint: &str,
) -> Response {
    let response = ui.place(
        rect,
        egui::TextEdit::singleline(value)
            .id(ui.id().with(id))
            .font(FontId::proportional(12.))
            .text_color(TEXT)
            .hint_text(egui::RichText::new(hint).color(PLACEHOLDER))
            .frame(false)
            .margin(egui::Margin::symmetric(9, 7))
            .vertical_align(Align::Center)
            .desired_width(rect.width())
            .clip_text(true),
    );
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 0., Stroke::new(1., ACCENT), StrokeKind::Inside);
    }
    response
}

pub fn checkbox(ui: &mut Ui, rect: Rect, id: impl std::hash::Hash, checked: &mut bool) -> Response {
    let mut response = ui
        .interact(rect, ui.id().with(id), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let box_rect = Rect::from_center_size(rect.center(), vec2(12., 12.));
    ui.painter()
        .rect_filled(box_rect, 2., if *checked { ACCENT } else { WHITE });
    ui.painter().rect_stroke(
        box_rect,
        2.,
        Stroke::new(1., if *checked { ACCENT } else { BORDER }),
        StrokeKind::Inside,
    );
    if *checked {
        icon(ui, box_rect.center(), Icon::Check, WHITE);
    }
    response
}

pub fn method_picker(ui: &mut Ui, rect: Rect, method: &mut String, custom: &mut String) -> bool {
    let response = ui
        .interact(rect, ui.id().with("method"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_filled(rect.shrink(1.), 3., HOVER);
    }
    label(
        ui,
        Rect::from_min_max(rect.min + vec2(12., 0.), rect.max - vec2(23., 0.)),
        method,
        12.,
        method_color(method),
    );
    icon(
        ui,
        pos2(rect.right() - 18., rect.center().y),
        Icon::Down,
        MUTED,
    );
    let mut changed = false;
    egui::Popup::menu(&response)
        .width(154.)
        .frame(
            egui::Frame::popup(ui.style())
                .fill(WHITE)
                .inner_margin(6.)
                .corner_radius(4.),
        )
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for m in ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
                let r = row(ui, 32.);
                let hit = ui
                    .interact(r, ui.id().with(m), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if method == m || hit.hovered() {
                    ui.painter()
                        .rect_filled(r, 3., if method == m { SELECTED } else { HOVER });
                }
                label(ui, r.shrink2(vec2(8., 0.)), m, 12., method_color(m));
                if hit.clicked() {
                    *method = m.into();
                    changed = true;
                    ui.close();
                }
            }
            ui.add_space(6.);
            ui.separator();
            let r = row(ui, 32.);
            let edit = field(ui, r, "custom_method", custom, "输入自定义方法");
            if edit.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && !custom.trim().is_empty()
            {
                let candidate = custom.trim().to_uppercase();
                if reqwest::Method::from_bytes(candidate.as_bytes()).is_ok() {
                    *method = candidate;
                    changed = true;
                    ui.close();
                }
            }
        });
    changed
}

#[derive(Clone)]
pub struct TableState {
    pub columns: [f32; 2],
    pub bulk: bool,
    pub text: String,
}
impl Default for TableState {
    fn default() -> Self {
        Self {
            columns: [0.34, 0.34],
            bulk: false,
            text: String::new(),
        }
    }
}

pub fn pairs_ui(
    ui: &mut Ui,
    id: &str,
    title: &str,
    pairs: &mut Vec<Pair>,
    state: &mut TableState,
) -> bool {
    let mut changed = false;
    ui.push_id(id, |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let title_rect = row(ui, 34.);
        label(ui, title_rect, title, 12., TEXT);
        let table_width = ui.available_width();
        let head = row(ui, ROW);
        let check_width = 44.;
        let data_width = table_width - check_width;
        let x = [
            head.left(),
            head.left() + check_width,
            head.left() + check_width + data_width * state.columns[0],
            head.left() + check_width + data_width * (state.columns[0] + state.columns[1]),
            head.right(),
        ];
        ui.painter().rect_filled(head, 0., WHITE);
        let mut all = pairs.iter().filter(|p| !p.is_empty()).all(|p| p.enabled);
        if pairs.iter().all(Pair::is_empty) {
            all = false;
        }
        if !state.bulk
            && checkbox(
                ui,
                Rect::from_min_max(head.min, pos2(x[1], head.bottom())),
                "all",
                &mut all,
            )
            .changed()
        {
            for p in pairs.iter_mut().filter(|p| !p.is_empty()) {
                p.enabled = all;
            }
            changed = true;
        }
        if !state.bulk {
            label(
                ui,
                Rect::from_min_max(pos2(x[1] + 9., head.top()), pos2(x[2], head.bottom())),
                "名称",
                12.,
                MUTED,
            );
            label(
                ui,
                Rect::from_min_max(pos2(x[2] + 9., head.top()), pos2(x[3], head.bottom())),
                "值",
                12.,
                MUTED,
            );
            label(
                ui,
                Rect::from_min_max(
                    pos2(x[3] + 9., head.top()),
                    pos2((head.right() - 115.).max(x[3]), head.bottom()),
                ),
                "描述",
                12.,
                MUTED,
            );
        } else {
            label(
                ui,
                head.shrink2(vec2(12., 0.)),
                "每行一组，格式为 名称:值",
                12.,
                MUTED,
            );
        }
        let bulk_rect = Rect::from_min_size(pos2(head.right() - 84., head.top()), vec2(84., ROW));
        ui.painter().rect_filled(bulk_rect, 0., WHITE);
        let more_rect =
            Rect::from_min_size(pos2(bulk_rect.left() - 30., head.top()), vec2(30., ROW));
        ui.painter().rect_filled(more_rect, 0., WHITE);
        if text_button(
            ui,
            bulk_rect,
            "bulk",
            if state.bulk {
                "键值编辑"
            } else {
                "批量编辑"
            },
        )
        .clicked()
        {
            state.bulk = !state.bulk;
            if state.bulk {
                state.text = linkshop::model::bulk_text(pairs);
            }
        }
        let more = icon_button(ui, more_rect, "more", Icon::More, "更多操作");
        egui::Popup::menu(&more).width(160.).show(|ui| {
            if ui.button("添加参数行").clicked() {
                pairs.push(Pair::default());
                ui.close();
            }
            if ui.button("全部启用").clicked() {
                for p in pairs.iter_mut() {
                    p.enabled = true;
                }
                changed = true;
                ui.close();
            }
            if ui.button("全部禁用").clicked() {
                for p in pairs.iter_mut() {
                    p.enabled = false;
                }
                changed = true;
                ui.close();
            }
            ui.separator();
            if ui.button("清空所有行").clicked() {
                *pairs = vec![Pair::default()];
                state.text.clear();
                changed = true;
                ui.close();
            }
        });
        if state.bulk && changed {
            state.text = linkshop::model::bulk_text(pairs);
        }
        if !state.bulk {
            for xx in x {
                vline(ui, xx, head.top(), head.bottom());
            }
        } else {
            vline(ui, head.left(), head.top(), head.bottom());
            vline(ui, head.right(), head.top(), head.bottom());
        }
        hline(ui, head.left(), head.right(), head.top());
        hline(ui, head.left(), head.right(), head.bottom());
        if state.bulk {
            let r = ui.add_sized(
                [table_width, ui.available_height().max(64.)],
                egui::TextEdit::multiline(&mut state.text)
                    .id_salt("bulk_text")
                    .font(egui::TextStyle::Monospace)
                    .margin(10.)
                    .frame(false)
                    .hint_text("name:value\n// disabled:value"),
            );
            if r.changed() {
                *pairs = linkshop::model::parse_bulk(&state.text, pairs);
                changed = true;
            }
            return;
        }
        let max_height = ui.available_height().max(32.);
        let mut remove = None;
        let mut duplicate = None;
        let scroll = egui::ScrollArea::vertical()
            .id_salt("rows")
            .auto_shrink([false, false])
            .max_height(max_height)
            .show(ui, |ui| {
                for (i, pair) in pairs.iter_mut().enumerate() {
                    let r = row(ui, ROW);
                    let hovered = ui.rect_contains_pointer(r);
                    if hovered {
                        ui.painter().rect_filled(r, 0., BG);
                    }
                    let has_content = !pair.is_empty();
                    if has_content || hovered {
                        let cb = Rect::from_min_max(pos2(x[0], r.top()), pos2(x[1], r.bottom()));
                        if checkbox(ui, cb, ("enabled", i), &mut pair.enabled).changed() {
                            changed = true;
                        }
                        if hovered {
                            icon(ui, pos2(x[0] + 7., r.center().y), Icon::Grip, PLACEHOLDER);
                        }
                    }
                    let key_rect = Rect::from_min_max(pos2(x[1], r.top()), pos2(x[2], r.bottom()));
                    let value_rect =
                        Rect::from_min_max(pos2(x[2], r.top()), pos2(x[3], r.bottom()));
                    let desc_rect = Rect::from_min_max(
                        pos2(x[3], r.top()),
                        pos2(x[4] - if hovered { 60. } else { 0. }, r.bottom()),
                    );
                    changed |= field(ui, key_rect, ("key", i), &mut pair.key, "名称").changed();
                    changed |= field(ui, value_rect, ("value", i), &mut pair.value, "值").changed();
                    changed |= field(
                        ui,
                        desc_rect,
                        ("description", i),
                        &mut pair.description,
                        "描述",
                    )
                    .changed();
                    if hovered {
                        if icon_button(
                            ui,
                            Rect::from_min_size(pos2(x[4] - 60., r.top()), vec2(30., ROW)),
                            ("copy", i),
                            Icon::Duplicate,
                            "复制此行",
                        )
                        .clicked()
                        {
                            duplicate = Some(i);
                        }
                        if icon_button(
                            ui,
                            Rect::from_min_size(pos2(x[4] - 30., r.top()), vec2(30., ROW)),
                            ("delete", i),
                            Icon::Trash,
                            "删除此行",
                        )
                        .clicked()
                        {
                            remove = Some(i);
                        }
                    }
                    if !pair.enabled && has_content {
                        ui.painter().rect_filled(
                            Rect::from_min_max(pos2(x[1], r.top()), pos2(x[4], r.bottom())),
                            0.,
                            Color32::from_white_alpha(90),
                        );
                    }
                    for xx in x {
                        vline(ui, xx, r.top(), r.bottom());
                    }
                    hline(ui, r.left(), r.right(), r.bottom());
                }
            });
        for (i, xx) in [x[2], x[3]].into_iter().enumerate() {
            let r = Rect::from_min_max(
                pos2(xx - 3., head.top()),
                pos2(xx + 3., scroll.inner_rect.bottom()),
            );
            let resize = ui
                .interact(r, ui.id().with(("resize", i)), Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if resize.dragged()
                && let Some(pointer) = resize.interact_pointer_pos()
            {
                let fraction = ((pointer.x - x[1]) / data_width).clamp(0.15, 0.85);
                if i == 0 {
                    let total = state.columns[0] + state.columns[1];
                    state.columns[0] = fraction.min(total - 0.15);
                    state.columns[1] = total - state.columns[0];
                } else {
                    state.columns[1] = (fraction - state.columns[0]).max(0.15);
                }
            }
        }
        if let Some(i) = remove {
            pairs.remove(i);
            changed = true;
        }
        if let Some(i) = duplicate {
            pairs.insert(i + 1, pairs[i].clone());
            changed = true;
        }
        ensure_blank_row(pairs);
    });
    changed
}
