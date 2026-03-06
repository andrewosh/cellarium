use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Kuramoto {
    phase: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Kuramoto {
    const OMEGA: f32 = 0.005;
    const K: f32 = 0.08;
    const TAU: f32 = 6.2831853;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let px = (x / 80.0).floor();
        let py = (y / 80.0).floor();
        let base = ((px * 43.17 + py * 91.53).sin() * 43758.5453).fract();
        let noise = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        Self {
            phase: (base + noise * 0.05).fract(),
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let sin_sum = nb.sum(|c| (TAU * c.phase).sin());
        let cos_sum = nb.sum(|c| (TAU * c.phase).cos());
        let self_angle = TAU * self.phase;
        let sin_self = self_angle.sin();
        let cos_self = self_angle.cos();
        let coupling = K * (sin_sum * cos_self - cos_sum * sin_self) / 8.0;
        let new_phase = self.phase + OMEGA + coupling;
        let wrapped = (new_phase - new_phase.floor());
        Self { phase: wrapped }
    }

    fn view(self) -> Color {
        Color::hsv(self.phase, 0.8, 0.85)
    }
}

fn main() {
    Simulation::<Kuramoto>::new(1024, 1024)
        .title("Kuramoto Oscillators")
        .ticks_per_frame(4)
        .run();
}
