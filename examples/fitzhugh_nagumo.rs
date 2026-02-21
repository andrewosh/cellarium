use cellarium::prelude::*;

#[derive(CellState, Default)]
struct FitzhughNagumo {
    u: f32,
    v: f32,
}

#[cell(neighborhood = moore)]
impl Cell for FitzhughNagumo {
    const DU: f32 = 1.0;
    const DV: f32 = 0.3;
    const A: f32 = 0.7;
    const EPSILON: f32 = 0.08;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        let h2 = ((x * 63.7264 + y * 10.873).sin() * 43758.5453).fract();

        // Seed a few spiral nucleation sites: opposing u/v regions
        let px = (x / 80.0).floor();
        let py = (y / 80.0).floor();
        let phash = ((px * 43.17 + py * 91.53).sin() * 43758.5453).fract();

        let lx = x - (px + 0.5) * 80.0;
        let ly = y - (py + 0.5) * 80.0;

        // Break symmetry with a half-plane offset to encourage spiral formation
        let u = if phash < 0.15 && lx > 0.0 && (lx * lx + ly * ly).sqrt() < 25.0 {
            1.0
        } else {
            -1.0 + h1 * 0.1
        };

        let v = if phash < 0.15 && ly > 0.0 && (lx * lx + ly * ly).sqrt() < 25.0 {
            0.5
        } else {
            -0.5 + h2 * 0.1
        };

        Self { u, v }
    }

    fn update(self, nb: Neighbors) -> Self {
        let lap_u = nb.laplacian(|c| c.u);
        let lap_v = nb.laplacian(|c| c.v);

        // FHN dynamics: cubic nullcline for u, linear recovery for v
        let du = self.u - self.u * self.u * self.u / 3.0 - self.v + DU * lap_u;
        let dv = EPSILON * (self.u + A - 0.5 * self.v) + DV * lap_v;

        Self {
            u: self.u + du * 0.05,
            v: self.v + dv * 0.05,
        }
    }

    fn view(self) -> Color {
        let t = ((self.u + 1.5) / 3.0).clamp(0.0, 1.0);
        Color::hsv(0.55 + t * 0.4, 0.6 + t * 0.3, 0.05 + t * 0.95)
    }
}

fn main() {
    Simulation::<FitzhughNagumo>::new(1024, 1024)
        .title("Fitzhugh-Nagumo")
        .ticks_per_frame(16)
        .run();
}
