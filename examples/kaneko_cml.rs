use cellarium::prelude::*;

#[derive(CellState, Default)]
struct KanekoCml {
    x: f32,
    avg: f32,
}

#[cell(neighborhood = moore)]
impl Cell for KanekoCml {
    const R: f32 = 3.62;
    const EPS: f32 = 0.2;
    const SMOOTH: f32 = 0.03;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let s = seed + 1.0;
        let h1 = ((x * (12.99 + s * 3.17) + y * (78.23 + s * 7.91)).sin() * 43758.5453).fract();
        let px = (x * (0.015 + s * 0.004)).sin() * (y * (0.021 + s * 0.003)).cos();
        let val = (h1 * 0.5 + 0.25 + px * 0.15).clamp(0.01, 0.99);

        Self { x: val, avg: val }
    }

    fn update(self, nb: Neighbors) -> Self {
        let local = R * self.x * (1.0 - self.x);
        let neighbor_avg = nb.mean(|c| R * c.x * (1.0 - c.x));
        let new_x = ((1.0 - EPS) * local + EPS * neighbor_avg).clamp(0.001, 0.999);
        let new_avg = self.avg + SMOOTH * (new_x - self.avg);

        Self { x: new_x, avg: new_avg }
    }

    fn view(self) -> Color {
        let v = self.avg.clamp(0.0, 1.0);
        let activity = ((self.x - self.avg).abs() * 12.0).clamp(0.0, 1.0);
        Color::hsv(
            0.62 - v * 0.55,
            0.15 + activity * 0.75,
            v * 0.8 + 0.15,
        )
    }
}

fn main() {
    Simulation::<KanekoCml>::new(1024, 1024)
        .title("Kaneko CML")
        .ticks_per_frame(4)
        .run();
}
