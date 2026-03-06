use cellarium::prelude::*;

#[derive(CellState, Default)]
struct BelousovZhabotinsky {
    u: f32,
    v: f32,
}

#[cell(neighborhood = moore)]
impl Cell for BelousovZhabotinsky {
    const A: f32 = 0.75;
    const B: f32 = 0.01;
    const EPSILON: f32 = 0.02;
    const DU: f32 = 0.2;
    const DV: f32 = 0.0;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        let h2 = ((x * 63.7264 + y * 10.873).sin() * 43758.5453).fract();
        let px = (x / 120.0).floor();
        let py = (y / 120.0).floor();
        let phash = ((px * 43.17 + py * 91.53).sin() * 43758.5453).fract();
        let lx = x - (px + 0.5) * 120.0;
        let ly = y - (py + 0.5) * 120.0;
        let dist = (lx * lx + ly * ly).sqrt();
        let u = if phash < 0.12 && lx > 0.0 && dist < 40.0 {
            1.0
        } else {
            h1 * 0.05
        };
        let v = if phash < 0.12 && ly > 0.0 && dist < 40.0 {
            0.5
        } else {
            h2 * 0.05
        };
        Self { u, v }
    }

    fn update(self, nb: Neighbors) -> Self {
        let lap_u = nb.laplacian(|c| c.u);
        let reaction = self.u * (1.0 - self.u) * (self.u - (self.v + B) / A) / EPSILON;
        let new_u = (self.u + reaction + DU * lap_u).clamp(0.0, 1.0);
        let new_v = (self.v + 0.005 * (self.u - self.v)).clamp(0.0, 1.0);
        Self {
            u: new_u,
            v: new_v,
        }
    }

    fn view(self) -> Color {
        let t = self.u.clamp(0.0, 1.0);
        let r = self.v.clamp(0.0, 1.0);
        Color::hsv(0.6 - t * 0.45 + r * 0.1, 0.4 + t * 0.5, 0.02 + t * 0.98)
    }
}

fn main() {
    Simulation::<BelousovZhabotinsky>::new(1024, 1024)
        .title("Belousov-Zhabotinsky")
        .ticks_per_frame(8)
        .run();
}
