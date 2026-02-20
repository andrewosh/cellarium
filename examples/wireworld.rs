use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Wire {
    state: f32, // 0=empty, 1=conductor, 2=electron head, 3=electron tail
}

#[cell(neighborhood = moore)]
impl Cell for Wire {
    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let hash = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        let hash2 = ((x * 63.726 + y * 10.873).sin() * 43758.5453).fract();

        // Wire grid: horizontal every 10px, vertical every 14px, diagonal every 20px
        let on_h = (y / 10.0).fract() < (1.0 / 10.0);
        let on_v = (x / 14.0).fract() < (1.0 / 14.0);
        let on_d1 = ((x + y) / 20.0).fract() < (1.0 / 20.0);
        let on_d2 = ((x - y + 1024.0) / 24.0).fract() < (1.0 / 24.0);

        // Random gaps create circuit-like structure (diodes, gates)
        let has_gap = hash2 > 0.88;

        let is_wire = (on_h || on_v || on_d1 || on_d2) && !has_gap;

        // Scatter electron heads on 3% of wire cells
        let state = if !is_wire { 0.0 }
            else if hash > 0.97 { 2.0 }
            else { 1.0 };

        Self { state }
    }

    fn update(self, nb: Neighbors) -> Self {
        let head_count = nb.count(|c| c.state > 1.5 && c.state < 2.5);

        let state = if self.state < 0.5 {
            0.0 // empty stays empty
        } else if self.state > 1.5 && self.state < 2.5 {
            3.0 // electron head → tail
        } else if self.state > 2.5 {
            1.0 // electron tail → conductor
        } else if head_count > 0.5 && head_count < 2.5 {
            2.0 // conductor → head if exactly 1 or 2 head neighbors
        } else {
            1.0 // conductor stays conductor
        };

        Self { state }
    }

    fn view(self) -> Color {
        let s = self.state;
        if s < 0.5 {
            Color::rgb(0.04, 0.03, 0.06) // empty: near-black
        } else if s > 2.5 {
            Color::rgb(0.8, 0.25, 0.08) // tail: orange
        } else if s > 1.5 {
            Color::rgb(0.2, 0.85, 1.0) // head: bright cyan
        } else {
            Color::rgb(0.18, 0.15, 0.25) // conductor: dim purple
        }
    }
}

fn main() {
    Simulation::<Wire>::new(1024, 1024)
        .title("Wireworld")
        .ticks_per_frame(4)
        .run();
}
