use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Wave {
    height: f32,
    velocity: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Wave {
    const SPEED: f32 = 0.3;
    const DAMPING: f32 = 0.9999;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        // Large parabolic bumps — clearly visible at 2048x2048
        let dx1 = x - w * 0.5;
        let dy1 = y - h * 0.5;
        let d1 = (dx1 * dx1 + dy1 * dy1).sqrt();
        let t1 = (1.0 - d1 / 200.0).clamp(0.0, 1.0);
        let b1 = t1 * t1;

        let dx2 = x - w * 0.28;
        let dy2 = y - h * 0.3;
        let d2 = (dx2 * dx2 + dy2 * dy2).sqrt();
        let t2 = (1.0 - d2 / 140.0).clamp(0.0, 1.0);
        let b2 = t2 * t2 * 0.7;

        let dx3 = x - w * 0.72;
        let dy3 = y - h * 0.65;
        let d3 = (dx3 * dx3 + dy3 * dy3).sqrt();
        let t3 = (1.0 - d3 / 160.0).clamp(0.0, 1.0);
        let b3 = t3 * t3 * -0.5;

        Self {
            height: b1 + b2 + b3,
            velocity: 0.0,
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let lap = nb.laplacian(|c| c.height);
        let new_vel = (self.velocity + SPEED * lap) * DAMPING;
        Self {
            height: self.height + new_vel,
            velocity: new_vel,
        }
    }

    fn view(self) -> Color {
        // Bright wavefronts on dark background — peaks are warm, troughs are cool
        let amp = (self.height * 4.0).clamp(-1.0, 1.0);
        Color::hsv(0.6 - amp * 0.15, 0.7, 0.1 + amp * amp * 0.9)
    }
}

fn main() {
    Simulation::<Wave>::new(2048, 2048)
        .title("Wave Equation")
        .ticks_per_frame(8)
        .run();
}
