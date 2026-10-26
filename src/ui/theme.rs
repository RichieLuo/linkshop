use super::*;

pub(super) fn set_style(ctx: &egui::Context) {
    // The custom-painted workspace is light; keep native widgets on the same theme
    // when the OS theme arrives after the first frame.
    ctx.set_theme(egui::Theme::Light);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = WHITE;
    style.visuals.window_fill = WHITE;
    style.visuals.window_stroke = Stroke::new(1., LINE);
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1., MUTED);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1., LINE);
    style.visuals.widgets.noninteractive.bg_fill = BG;
    style.visuals.widgets.noninteractive.weak_bg_fill = BG;
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(4);
        widget.bg_stroke = Stroke::new(1., LINE);
        widget.fg_stroke = Stroke::new(1., TEXT);
    }
    style.visuals.widgets.inactive.bg_fill = BG;
    style.visuals.widgets.inactive.weak_bg_fill = BG;
    style.visuals.widgets.hovered.bg_fill = HOVER;
    style.visuals.widgets.hovered.weak_bg_fill = HOVER;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1., ACCENT);
    style.visuals.widgets.active.bg_fill = SELECTED;
    style.visuals.widgets.active.weak_bg_fill = SELECTED;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1., ACCENT);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1., ACCENT);
    style.visuals.widgets.open.bg_fill = SELECTED;
    style.visuals.widgets.open.weak_bg_fill = SELECTED;
    style.visuals.widgets.open.bg_stroke = Stroke::new(1., ACCENT);
    style.visuals.selection.bg_fill = SELECTED;
    style.visuals.selection.stroke = Stroke::new(1., ACCENT);
    style.spacing.item_spacing = vec2(8., 6.);
    style.spacing.button_padding = vec2(12., 6.);
    style.spacing.interact_size = vec2(32., 28.);
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(12.));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(12.));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(12.));
    ctx.set_style(style);
}

#[cfg(target_os = "windows")]
pub(super) fn install_fonts(ctx: &egui::Context) {
    let fonts = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("Fonts");
    let mut definitions = egui::FontDefinitions::default();
    for (name, paths, primary) in [
        ("LinkShop-UI", vec!["segoeui.ttf"], true),
        (
            "LinkShop-CJK",
            vec!["msyh.ttc", "msjh.ttc", "simsun.ttc"],
            false,
        ),
    ] {
        if let Some(bytes) = paths.iter().find_map(|p| std::fs::read(fonts.join(p)).ok()) {
            definitions.font_data.insert(
                name.into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            if primary {
                definitions
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, name.into());
            } else {
                for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    definitions
                        .families
                        .get_mut(&family)
                        .unwrap()
                        .push(name.into());
                }
            }
        }
    }
    let mut semibold = vec![];
    for (name, filename) in [
        ("LinkShop-UI-Semibold", "seguisb.ttf"),
        ("LinkShop-CJK-Bold", "msyhbd.ttc"),
    ] {
        if let Ok(bytes) = std::fs::read(fonts.join(filename)) {
            definitions.font_data.insert(
                name.into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            semibold.push(name.into());
        }
    }
    semibold.extend(
        definitions.families[&egui::FontFamily::Proportional]
            .iter()
            .cloned(),
    );
    definitions
        .families
        .insert(egui::FontFamily::Name("LinkShop-Semibold".into()), semibold);
    ctx.set_fonts(definitions);
}

#[cfg(not(target_os = "windows"))]
pub(super) fn install_fonts(_: &egui::Context) {}
