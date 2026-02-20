use cellarium::prelude::*;

#[derive(CellState)]
struct GrayScott {
    a: f32,
    b: f32,
}

impl Default for GrayScott {
    fn default() -> Self {
        Self { a: 1.0, b: 0.0 }
    }
}

#[cell(neighborhood = moore)]
impl Cell for GrayScott {
    const DA: f32 = 0.21;
    const DB: f32 = 0.105;
    const FEED: f32 = 0.026;
    const KILL: f32 = 0.052;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        let h2 = ((x * 63.7264 + y * 10.873).sin() * 43758.5453).fract();

        // Tile space into 60px patches, seed ~12% of them with a blob of b
        let px = (x / 60.0).floor();
        let py = (y / 60.0).floor();
        let phash = ((px * 43.17 + py * 91.53).sin() * 43758.5453).fract();

        let lx = x - (px + 0.5) * 60.0;
        let ly = y - (py + 0.5) * 60.0;
        let dist = (lx * lx + ly * ly).sqrt();

        let seeded = if phash < 0.25 && dist < 10.0 { 1.0 } else { 0.0 };
        let noise = h2 * 0.01;

        Self {
            a: 1.0 - seeded * 0.5,
            b: seeded * (0.25 + h1 * 0.1) + noise,
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let lap_a = nb.laplacian(|c| c.a);
        let lap_b = nb.laplacian(|c| c.b);
        let reaction = self.a * self.b * self.b;
        Self {
            a: (self.a + DA * lap_a - reaction + FEED * (1.0 - self.a)).clamp(0.0, 1.0),
            b: (self.b + DB * lap_b + reaction - (KILL + FEED) * self.b).clamp(0.0, 1.0),
        }
    }

    fn view(self) -> Color {
        let b = self.b;
        let t = (b * 3.5).clamp(0.0, 1.0);
        Color::hsv(0.58 - t * 0.25, 0.5 + t * 0.4, 0.04 + t * 0.96)
    }
}

fn main() {
    Simulation::<GrayScott>::new(1024, 1024)
        .title("Gray-Scott Reaction Diffusion")
        .ticks_per_frame(32)
        .run();
}
