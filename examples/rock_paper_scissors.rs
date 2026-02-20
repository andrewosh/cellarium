use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Rps {
    a: f32,
    b: f32,
    c: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Rps {
    const SIGMA: f32 = 3.5;  // competition strength
    const DIFF: f32 = 0.12;  // diffusion rate
    const DT: f32 = 0.2;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let hash = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        // Assign species in ~200px blocks
        let bx = (x / 200.0).floor();
        let by = (y / 200.0).floor();
        let block = ((bx * 127.1 + by * 311.7).sin() * 43758.5453).fract();
        let species = (block * 3.0).floor();
        let intensity = 0.4 + hash * 0.4;

        let a = if species < 1.0 { intensity } else { 0.02 };
        let b = if species > 0.5 && species < 1.5 { intensity } else { 0.02 };
        let c = if species > 1.5 { intensity } else { 0.02 };
        Self { a, b, c }
    }

    fn update(self, nb: Neighbors) -> Self {
        // Diffuse
        let avg_a = nb.mean(|c| c.a);
        let avg_b = nb.mean(|c| c.b);
        let avg_c = nb.mean(|c| c.c);

        let a = self.a + DIFF * (avg_a - self.a);
        let b = self.b + DIFF * (avg_b - self.b);
        let c = self.c + DIFF * (avg_c - self.c);

        // Logistic growth + cyclic competition
        // A beats B, B beats C, C beats A
        let total = a + b + c;
        let space = (1.0 - total).max(0.0);

        let new_a = (a + DT * (a * space - SIGMA * a * c)).clamp(0.0, 1.0);
        let new_b = (b + DT * (b * space - SIGMA * b * a)).clamp(0.0, 1.0);
        let new_c = (c + DT * (c * space - SIGMA * c * b)).clamp(0.0, 1.0);

        Self { a: new_a, b: new_b, c: new_c }
    }

    fn view(self) -> Color {
        let total = self.a + self.b + self.c + 0.001;
        let ra = self.a / total;
        let rb = self.b / total;
        let rc = self.c / total;
        let bright = total.min(1.0);
        Color::rgb(
            bright * (ra * 1.0 + rb * 0.1 + rc * 0.2),
            bright * (ra * 0.15 + rb * 0.9 + rc * 0.1),
            bright * (ra * 0.1 + rb * 0.2 + rc * 1.0),
        )
    }
}

fn main() {
    Simulation::<Rps>::new(2048, 2048)
        .title("Rock Paper Scissors")
        .ticks_per_frame(4)
        .run();
}
