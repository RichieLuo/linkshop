use super::*;

struct RequestDrag(Request);

pub(super) fn request_drag_source(hit: &egui::Response, request: &Request) {
    if hit.drag_started_by(egui::PointerButton::Primary) {
        egui::DragAndDrop::set_payload(&hit.ctx, RequestDrag(request.clone()));
    }
}

pub(super) fn folder_drop_target(
    ui: &egui::Ui,
    hit: &egui::Response,
    folder: &FolderTarget,
    action: &mut Option<SidebarAction>,
) -> bool {
    let timer = hit.id.with("drop_hover_start");
    let Some(drag) = hit.dnd_hover_payload::<RequestDrag>() else {
        ui.ctx().data_mut(|data| data.remove::<f64>(timer));
        return false;
    };
    if FolderTarget::parent(&drag.0).key == folder.key {
        ui.ctx().set_cursor_icon(egui::CursorIcon::NotAllowed);
        return false;
    }
    let highlight = hit.rect.shrink2(vec2(4., 1.));
    ui.painter().rect_filled(highlight, 4., SELECTED);
    ui.painter()
        .rect_stroke(highlight, 4., Stroke::new(1., ACCENT), StrokeKind::Inside);
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    let now = ui.input(|input| input.time);
    let since = ui
        .ctx()
        .data_mut(|data| *data.get_temp_mut_or_insert_with(timer, || now));
    let released = hit.dnd_release_payload::<RequestDrag>();
    let expand = released.is_some() || now - since >= 0.6;
    if let Some(drag) = released {
        *action = Some(SidebarAction::MoveRequest(
            Box::new(drag.0.clone()),
            folder.clone(),
        ));
    }
    if expand {
        reveal_folder(ui.ctx(), folder);
    } else {
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64((0.6 - (now - since)).max(0.)));
    }
    expand
}

fn reveal_folder(ctx: &egui::Context, folder: &FolderTarget) {
    for depth in 1..=folder.key.len() {
        ctx.data_mut(|data| {
            data.insert_persisted(
                egui::Id::new(("collection_tree_expansion_v3", &folder.key[..depth])),
                true,
            );
        });
    }
}

pub(super) fn scroll_while_dragging(ui: &egui::Ui) {
    if !egui::DragAndDrop::has_payload_of_type::<RequestDrag>(ui.ctx()) {
        return;
    }
    let viewport = ui.clip_rect().intersect(ui.max_rect());
    if let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
        && viewport.contains(pointer)
    {
        let speed = ((viewport.top() + 28. - pointer.y) / 28.).clamp(0., 1.)
            - ((pointer.y - (viewport.bottom() - 28.)) / 28.).clamp(0., 1.);
        if speed != 0. {
            let delta = speed * 420. * ui.input(|input| input.stable_dt.min(0.05));
            ui.scroll_with_delta_animation(vec2(0., delta), egui::style::ScrollAnimation::none());
            ui.ctx().request_repaint();
        }
    }
}

pub(super) fn drag_preview(ctx: &egui::Context) {
    let Some(drag) = egui::DragAndDrop::payload::<RequestDrag>(ctx) else {
        return;
    };
    let Some(pointer) = ctx.pointer_hover_pos() else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("collection_request_drag"),
    ));
    let text = painter.layout(
        format!("移动请求：{}", drag.0.name),
        FontId::proportional(12.),
        TEXT,
        230.,
    );
    let rect = Rect::from_min_size(pointer + vec2(14., 16.), text.size() + vec2(20., 14.));
    painter.rect_filled(rect, 5., WHITE);
    painter.rect_stroke(rect, 5., Stroke::new(1., ACCENT), StrokeKind::Inside);
    painter.galley(rect.min + vec2(10., 7.), text, TEXT);
}

impl App {
    pub(super) fn move_request(&mut self, request: &Request, folder: &FolderTarget) {
        let Some(index) = self
            .saved
            .iter()
            .position(|saved| same_saved_request(&saved.request, request))
        else {
            return;
        };
        let original = self.saved[index].request.clone();
        let parent = FolderTarget::parent(&original);
        if parent.key == folder.key {
            return;
        }
        let mut source = original.source.clone().unwrap_or_else(|| ImportSource {
            complete: true,
            ..Default::default()
        });
        // Different imported collections can contain identical request IDs.
        if source.id.is_empty()
            || self
                .saved
                .iter()
                .map(|saved| &saved.request)
                .chain(
                    self.tabs
                        .iter()
                        .flat_map(|tab| [&tab.request, &tab.baseline]),
                )
                .any(|request| {
                    !same_saved_request(request, &original)
                        && request.source.as_ref().is_some_and(|other| {
                            other.collection_id == folder.key[0] && other.id == source.id
                        })
                })
        {
            source.id = self.fresh_request_id();
        }
        source.collection_id = folder.key[0].clone();
        source.path = folder.path.clone();
        self.saved[index].request.source = Some(source.clone());
        for tab in &mut self.tabs {
            if same_saved_request(&tab.request, &original)
                || same_saved_request(&tab.baseline, &original)
            {
                // Updating both preserves unrelated unsaved edits and clean tabs.
                tab.request.source = Some(source.clone());
                tab.baseline.source = Some(source.clone());
            }
        }
        self.remember_folder(&parent);
        self.remember_folder(folder);
        self.notice = Some((
            format!("已移动到 {}", folder.path.join(" / ")),
            Instant::now(),
        ));
    }
}
