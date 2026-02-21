use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Brusselator {
    u: f32,
    v: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Brusselator {
    const DU: f32 = 0.16;
    const DV: f32 = 0.8;
    const A: f32 = 4.5;
    const B: f32 = 7.5;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        let h2 = ((x * 63.7264 + y * 10.873).sin() * 43758.5453).fract();

        // Steady state is u=A, v=B/A, perturb to trigger instability
        Self {
            u: A + (h1 - 0.5) * 0.5,
            v: B / A + (h2 - 0.5) * 0.5,
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let lap_u = nb.laplacian(|c| c.u);
        let lap_v = nb.laplacian(|c| c.v);

        let u2v = self.u * self.u * self.v;
        let du = A - (B + 1.0) * self.u + u2v + DU * lap_u;
        let dv = B * self.u - u2v + DV * lap_v;

        Self {
            u: (self.u + du * 0.01).max(0.0),
            v: (self.v + dv * 0.01).max(0.0),
        }
    }

    fn view(self) -> Color {
        let t = ((self.u - 2.0) / 6.0).clamp(0.0, 1.0);
        Color::hsv(0.08 + (1.0 - t) * 0.6, 0.5 + t * 0.5, 0.05 + t * 0.95)
    }
}

fn main() {
    Simulation::<Brusselator>::new(1024, 1024)
        .title("Brusselator")
        .ticks_per_frame(32)
        .run();
}
