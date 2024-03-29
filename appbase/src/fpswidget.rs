use imgui::{WindowFlags, Condition};

pub struct FpsWidget {
    count: usize,
    frames: Vec<f32>
}

impl FpsWidget {
    pub fn new(count: usize) -> Self {
        FpsWidget {
            count,
            frames: Vec::new()
        }
    }

    pub fn push(&mut self, value: f32) {
        self.frames.push(value);
        while self.frames.len() > self.count {
            self.frames.remove(0);
        }
    }

    fn average(&self, last: usize) -> f32 {
        let mut sum = 0.0;
        let mut cnt = 0;
        for value in self.frames.iter() {
            sum += value;
            cnt += 1;
            if cnt >= last {
                break;
            }
        }
        sum / cnt as f32
    }

    pub fn render(&self, ui: &imgui::Ui, position: (f32, f32), size: (f32, f32)) {
        ui.window("frametimewidget")
            .flags(WindowFlags::NO_RESIZE | WindowFlags::NO_MOVE | WindowFlags::NO_TITLE_BAR | WindowFlags::NO_SAVED_SETTINGS | WindowFlags::NO_SCROLLBAR)
            .size([size.0, size.1], Condition::Always)
            .position([position.0, position.1], Condition::Always)
            .build(|| {
                let plotsize = [size.0 - 60.0, size.1 - 35.0];
                let max = self.frames.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&1.0);

                ui.text(format!("msecs per Frame ({0:.1} FPS)", 1.0 / self.average(30)));

                ui.plot_lines("", &self.frames)
                    .graph_size(plotsize)
                    .scale_min(0.0)
                    .scale_max(*max)
                    .build();

                ui.same_line_with_pos(plotsize[0] + 20.0);
                ui.text(format!("{0:.1}\n\n0.0", *max * 1000.0));
            });
    }
}
