use eframe::egui;
use module_engine_core::{FrameTimings, GpuTimings};

/// The overlay belongs to this Game pane and takes pointer ownership only
/// inside its bounds. The Game toolbar owns opening and closing it.
pub(super) fn show(ui: &egui::Ui, viewport: egui::Rect, timings: Option<&FrameTimings>) -> bool {
    if viewport.width() < 160. || viewport.height() < 72. {
        return false;
    }
    let bounds = viewport.shrink(4.);
    let width = bounds.width().min(240.);
    let origin = egui::pos2(bounds.right() - width, bounds.top());
    let response = egui::Area::new(ui.id().with("game-frame-timings"))
        .fixed_pos(origin)
        .constrain_to(bounds)
        .movable(false)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            ui.set_width(width);
            // Area starts with the previous content size. Allow scroll content
            // to grow when the first measured frame replaces the waiting text.
            ui.set_max_height(bounds.height());
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.spacing_mut().interact_size.y = ui.text_style_height(&egui::TextStyle::Body);
                ui.spacing_mut().item_spacing.y = 4.;
                ui.strong("Frame timings");
                egui::ScrollArea::vertical()
                    .max_height((bounds.height() - 38.).max(24.))
                    .auto_shrink([false, true])
                    .show(ui, |ui| match timings {
                        Some(timings) => rows(ui, timings),
                        None => {
                            ui.label("Waiting for a measured frame…");
                        }
                    });
            });
        })
        .response;
    ui.input(|input| {
        input
            .pointer
            .hover_pos()
            .is_some_and(|pointer| bounds.contains(pointer) && response.rect.contains(pointer))
    })
}

fn rows(ui: &mut egui::Ui, timings: &FrameTimings) {
    for (name, ms) in [
        ("CPU tick", timings.cpu_tick_ms),
        ("CPU extraction", timings.cpu_extract_ms),
        ("CPU recording", timings.cpu_record_ms),
        ("CPU submission", timings.cpu_submit_ms),
    ] {
        timing_row(ui, name, ms);
    }
    ui.separator();
    match &timings.gpu {
        Some(GpuTimings::Available { graph_ms, passes }) => {
            ui.strong(format!("GPU graph  {graph_ms:.3} ms"));
            for pass in passes {
                let suffix = if pass.bypassed { " (bypass)" } else { "" };
                timing_row(ui, &format!("{}{suffix}", pass.id), Some(pass.ms));
            }
        }
        Some(GpuTimings::Unavailable { reason }) => {
            ui.label("GPU timings unavailable").on_hover_text(reason);
        }
        None => {
            ui.label("GPU timings unavailable");
        }
    }
}

fn timing_row(ui: &mut egui::Ui, label: &str, ms: Option<f64>) {
    ui.horizontal(|ui| {
        let value = ms.map_or_else(|| "—".into(), |ms| format!("{ms:.3} ms"));
        let style = egui::TextStyle::Monospace;
        let value_width = ui
            .painter()
            .layout_no_wrap(
                value.clone(),
                style.resolve(ui.style()),
                ui.visuals().text_color(),
            )
            .size()
            .x;
        let label_width =
            (ui.available_width() - value_width - ui.spacing().item_spacing.x).max(1.);
        let label_size = egui::vec2(label_width, ui.text_style_height(&style));
        ui.allocate_ui_with_layout(
            label_size,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(label_width);
                ui.add(egui::Label::new(label).truncate())
            },
        )
        .inner
        .on_hover_text(label);
        ui.monospace(value);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_core::GpuPassTiming;

    #[test]
    fn game_timing_overlay_keeps_values_in_a_narrow_pane() {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640., 480.));
        let viewport = egui::Rect::from_min_size(egui::pos2(20., 40.), egui::vec2(174., 250.));
        let timings = FrameTimings {
            cpu_tick_ms: Some(0.42),
            gpu: Some(GpuTimings::Available {
                graph_ms: 3.5,
                passes: vec![GpuPassTiming {
                    id: "geometry_pass_with_a_long_user_supplied_name".into(),
                    bypassed: false,
                    ms: 1.1,
                }],
            }),
            ..Default::default()
        };
        let run = |position: egui::Pos2, timings: Option<&FrameTimings>, expected| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![egui::Event::PointerMoved(position)],
                    ..Default::default()
                },
                |ui| {
                    let blocked = show(ui, viewport, timings);
                    if let Some(expected) = expected {
                        assert_eq!(blocked, expected);
                    }
                },
            )
        };
        run(viewport.center(), None, None);
        run(viewport.center(), None, None);
        let inside = viewport.right_top() + egui::vec2(-32., 26.);
        run(inside, Some(&timings), Some(true));
        let output = run(inside, Some(&timings), Some(true));
        for value in ["0.420 ms", "1.100 ms"] {
            assert!(
                output.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.text() == value
                        && viewport.contains_rect(text.visual_bounding_rect())
                        && shape.clip_rect.contains_rect(text.visual_bounding_rect()))
                }),
                "{value} must fit inside Game"
            );
        }
        run(
            viewport.right_top() + egui::vec2(10., 26.),
            Some(&timings),
            Some(false),
        );
    }
}
