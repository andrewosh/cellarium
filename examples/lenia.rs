use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Lenia {
    state: f32,
}

#[cell(neighborhood = radius(13))]
impl Cell for Lenia {
    const R: f32 = 13.0;
    const BETA: f32 = 0.5;
    const ALPHA: f32 = 0.147;
    // Wider growth band — many configurations semi-stable, constant competition
    const MU: f32 = 0.20;
    const SIGMA: f32 = 0.033;
    const DT: f32 = 0.05;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        // Fine-grain noise
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        // Medium-scale patches (~40px blobs)
        let px = (x * 0.025).sin() * (y * 0.031).cos();
        let py = (x * 0.019 + 1.7).cos() * (y * 0.023 + 0.8).sin();
        let patch = (px + py + 1.0) * 0.25;
        // Large-scale variation (~200px gradients)
        let lx = (x * 0.005 + 2.3).sin();
        let ly = (y * 0.007 + 1.1).cos();
        let large = (lx * ly + 1.0) * 0.5;
        // Combine: patchy soup with density ~20-35%
        let density = patch * large;
        let state = if h1 < density { h1 / density * 0.6 + 0.2 } else { 0.0 };
        Self { state }
    }

    fn update(self, nb: Neighbors) -> Self {
        let weighted = nb.sum(|c| c.state * (-(c.distance() / R - BETA).powf(2.0) / (2.0 * ALPHA * ALPHA)).exp());
        let weights = nb.sum(|c| (-(c.distance() / R - BETA).powf(2.0) / (2.0 * ALPHA * ALPHA)).exp());
        let potential = weighted / weights;
        let growth = 2.0 * (-(potential - MU).powf(2.0) / (2.0 * SIGMA * SIGMA)).exp() - 1.0;
        Self {
            state: (self.state + DT * growth).clamp(0.0, 1.0),
        }
    }

    fn view(self) -> Color {
        let s = self.state;
        Color::hsv(0.55 - s * 0.2, 0.4 + s * 0.5, s * 0.9 + 0.03)
    }
}

fn main() {
    Simulation::<Lenia>::new(2048, 2048)
        .title("Lenia")
        .ticks_per_frame(1)
        .run();
}
