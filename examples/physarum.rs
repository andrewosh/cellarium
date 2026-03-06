use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Physarum {
    trail: f32,
    density: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Physarum {
    const DT: f32 = 0.05;
    const D_TRAIL: f32 = 0.15;
    const D_DENS: f32 = 0.01;
    const DECAY: f32 = 0.4;
    const CHEMO: f32 = 8.0;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        let h2 = ((x * 63.7264 + y * 10.873).sin() * 43758.5453).fract();

        let px = (x / 40.0).floor();
        let py = (y / 40.0).floor();
        let phash = ((px * 43.17 + py * 91.53).sin() * 43758.5453).fract();
        let lx = x - (px + 0.5) * 40.0;
        let ly = y - (py + 0.5) * 40.0;
        let dist = (lx * lx + ly * ly).sqrt();

        let seeded = if phash < 0.3 && dist < 8.0 { 1.0 } else { 0.0 };
        let density = seeded * (0.3 + h1 * 0.5);
        let trail = density * 0.2 + h2 * 0.002;

        Self { trail, density }
    }

    fn update(self, nb: Neighbors) -> Self {
        let lap_trail = nb.laplacian(|c| c.trail);
        let lap_density = nb.laplacian(|c| c.density);
        let grad_trail = nb.gradient(|c| c.trail);
        let grad_density = nb.gradient(|c| c.density);

        // Saturating deposit prevents runaway feedback
        let deposit = self.density / (self.density + 0.3);

        // Trail: diffuse + deposit - decay
        let new_trail = (self.trail
            + DT * D_TRAIL * lap_trail
            + DT * deposit
            - DT * DECAY * self.trail
        ).clamp(0.0, 1.0);

        // Chemotaxis: density climbs trail gradients
        let chemo = self.density * lap_trail
            + grad_density.x * grad_trail.x
            + grad_density.y * grad_trail.y;

        // Logistic growth coupled to trail, keeps density dynamic
        let growth = self.density * (1.0 - self.density) * self.trail;

        let new_density = (self.density
            + DT * D_DENS * lap_density
            + DT * CHEMO * chemo
            + DT * growth
        ).clamp(0.0, 1.0);

        Self { trail: new_trail, density: new_density }
    }

    fn view(self) -> Color {
        let t = self.trail.clamp(0.0, 1.0);
        let d = self.density.clamp(0.0, 1.0);
        let brightness = (t * 2.0 + d * 0.5).clamp(0.0, 1.0);
        Color::hsv(
            0.12 - t * 0.1 + d * 0.05,
            0.3 + d * 0.6,
            brightness * brightness,
        )
    }
}

fn main() {
    Simulation::<Physarum>::new(1024, 1024)
        .title("Physarum")
        .ticks_per_frame(6)
        .run();
}
