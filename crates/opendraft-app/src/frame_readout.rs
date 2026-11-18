//! The optional readout over the canvas (Settings > Performance > Diagnostics):
//! what painting the drawing costs and what the kept picture is doing. While
//! it is off nothing is timed.

use std::time::Instant;

use eframe::egui;
use opendraft_canvas::CacheStats;
use opendraft_ui::Theme;

/// How much of the newest frame goes into the average shown.
const NEW_WEIGHT: f32 = 0.1;

#[derive(Default)]
pub struct Readout {
    /// A moving average of the time `View::show` took, in milliseconds. That is
    /// building the frame; egui turns it into triangles afterwards.
    paint_ms: Option<f32>,
}

impl Readout {
    /// Start timing a frame, if the readout is on.
    pub fn start(on: bool) -> Option<Instant> {
        on.then(Instant::now)
    }

    /// Add the frame that began at `started` (from [`Readout::start`]).
    pub fn finish(&mut self, started: Option<Instant>) {
        let Some(started) = started else {
            self.paint_ms = None;
            return;
        };
        let ms = started.elapsed().as_secs_f32() * 1e3;
        self.paint_ms = Some(match self.paint_ms {
            Some(old) => old + (ms - old) * NEW_WEIGHT,
            None => ms,
        });
    }

    /// The lines to show, if there is anything measured yet.
    pub fn lines(&self, frame_dt: f32, stats: CacheStats, objects: usize) -> Vec<String> {
        let Some(paint) = self.paint_ms else {
            return Vec::new();
        };
        let fps = if frame_dt > 0.0 { 1.0 / frame_dt } else { 0.0 };
        let mut lines = vec![
            format!(
                "paint {paint:.1} ms   frame {:.1} ms ({fps:.0} fps)",
                frame_dt * 1e3
            ),
            format!(
                "cache {:.1} MB, {}, {}, {}",
                stats.bytes as f64 / 1e6,
                crate::text::plural(stats.builds as usize, "build"),
                crate::text::plural(stats.variant_builds as usize, "chunk rebuild"),
                crate::text::plural(stats.hits as usize, "frame"),
            ),
            crate::text::plural(objects, "object"),
        ];
        if let Some(percent) = stats.rebuilding {
            lines.push(format!("making the new picture: {percent}%"));
        }
        lines
    }

    /// Draw the readout in the top right corner of `canvas`.
    pub fn draw(
        &self,
        ui: &egui::Ui,
        canvas: egui::Rect,
        theme: &Theme,
        stats: CacheStats,
        objects: usize,
    ) {
        let dt = ui.ctx().input(|i| i.stable_dt);
        let lines = self.lines(dt, stats, objects);
        if lines.is_empty() {
            return;
        }
        let painter = ui.painter_at(canvas);
        let font = egui::FontId::monospace(theme.font_size_small);
        let galley = painter.layout(
            lines.join("\n"),
            font,
            theme.colors.text,
            canvas.width().max(100.0),
        );
        let size = galley.size() + egui::vec2(12.0, 8.0);
        let at = egui::pos2(canvas.right() - size.x - 8.0, canvas.top() + 8.0);
        let panel = egui::Rect::from_min_size(at, size);
        painter.rect_filled(panel, 4.0, theme.colors.surface.gamma_multiply(0.85));
        painter.galley(at + egui::vec2(6.0, 4.0), galley, theme.colors.text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_timed_or_shown_while_it_is_off() {
        assert!(Readout::start(false).is_none());
        let mut readout = Readout::default();
        readout.finish(None);
        assert!(readout.lines(0.016, CacheStats::default(), 10).is_empty());
    }

    #[test]
    fn a_measured_frame_shows_its_cost_and_the_cache() {
        let mut readout = Readout::default();
        readout.finish(Readout::start(true));
        let stats = CacheStats {
            builds: 1,
            variant_builds: 3,
            hits: 40,
            bytes: 26_000_000,
            ..CacheStats::default()
        };
        let lines = readout.lines(0.01, stats, 1);
        assert!(lines[0].contains("ms") && lines[0].contains("100 fps"));
        assert_eq!(
            lines[1],
            "cache 26.0 MB, 1 build, 3 chunk rebuilds, 40 frames"
        );
        assert_eq!(lines[2], "1 object");
        assert_eq!(lines.len(), 3);
        let making = CacheStats {
            rebuilding: Some(40),
            ..stats
        };
        let lines = readout.lines(0.01, making, 1);
        assert_eq!(lines[3], "making the new picture: 40%");
    }

    #[test]
    fn turning_it_off_forgets_the_average() {
        let mut readout = Readout::default();
        readout.finish(Readout::start(true));
        readout.finish(None);
        assert!(readout.lines(0.016, CacheStats::default(), 0).is_empty());
    }
}
