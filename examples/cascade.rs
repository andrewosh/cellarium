use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Cascade {
    state: f32,
    timer: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Cascade {
    const N: f32 = 30.0;
    const PACE: f32 = 400.0;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let hash = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();

        // Dense ignition clusters — enough for threshold=2 propagation
        let d0 = ((x - w * 0.3) * (x - w * 0.3) + (y - h * 0.2) * (y - h * 0.2)).sqrt();
        let d1 = ((x - w * 0.7) * (x - w * 0.7) + (y - h * 0.5) * (y - h * 0.5)).sqrt();
        let d2 = ((x - w * 0.4) * (x - w * 0.4) + (y - h * 0.8) * (y - h * 0.8)).sqrt();
        let d3 = ((x - w * 0.85) * (x - w * 0.85) + (y - h * 0.25) * (y - h * 0.25)).sqrt();
        let d4 = ((x - w * 0.15) * (x - w * 0.15) + (y - h * 0.6) * (y - h * 0.6)).sqrt();

        let near = d0.min(d1).min(d2).min(d3).min(d4);
        // ~50% excited inside 30px clusters → guaranteed threshold=2 at edges
        let state = if near < 30.0 && hash < 0.5 { 1.0 } else { 0.0 };
        let timer = hash * PACE;

        Self { state, timer }
    }

    fn update(self, nb: Neighbors) -> Self {
        let excited = nb.count(|c| c.state > 0.5 && c.state < 1.5);

        // Subthreshold warming: nearby activity heats pacemaker faster
        let timer_inc = 1.0 + excited * 3.0;

        // State transitions
        let new_state = if self.state < 0.5 {
            // Resting: fire if 2+ excited neighbors OR pacemaker triggers
            if excited > 1.5 { 1.0 }
            else if self.timer > PACE { 1.0 }
            else { 0.0 }
        } else if self.state < 1.5 {
            2.0
        } else if self.state > N - 0.5 {
            0.0
        } else {
            self.state + 1.0
        };

        // Timer: counts up while staying resting, resets otherwise
        let still_resting = if self.state < 0.5 && new_state < 0.5 { 1.0 } else { 0.0 };
        let new_timer = still_resting * (self.timer + timer_inc);

        Self { state: new_state, timer: new_timer }
    }

    fn view(self) -> Color {
        let s = self.state;
        let t = (s - 1.0) / (N - 1.0);
        let warmth = (self.timer / PACE).clamp(0.0, 1.0);
        if s < 0.5 {
            // Resting: dim glow shows pacemaker warming
            Color::rgb(warmth * 0.15, 0.0, warmth * 0.08)
        } else if s < 1.5 {
            Color::rgb(1.0, 1.0, 0.95)
        } else {
            Color::hsv(0.08 + t * 0.55, 0.9 - t * 0.3, 0.95 - t * 0.85)
        }
    }
}

fn main() {
    Simulation::<Cascade>::new(2048, 2048)
        .title("Cascade")
        .ticks_per_frame(4)
        .run();
}
