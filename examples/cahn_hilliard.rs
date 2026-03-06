use cellarium::prelude::*;

#[derive(CellState)]
struct CahnHilliard {
    phi: f32,
    mu: f32,
}

impl Default for CahnHilliard {
    fn default() -> Self {
        Self { phi: 0.0, mu: 0.0 }
    }
}

#[cell(neighborhood = moore)]
impl Cell for CahnHilliard {
    const M: f32 = 1.0;
    const EPS2: f32 = 0.5;
    const DT: f32 = 0.01;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        let sine_mod = (x * 0.02).sin() * (y * 0.03).cos() * 0.05;
        let phi = (h1 - 0.5) * 0.1 + sine_mod;
        Self { phi, mu: 0.0 }
    }

    fn update(self, nb: Neighbors) -> Self {
        let lap_phi = nb.laplacian(|c| c.phi);
        let lap_mu = nb.laplacian(|c| c.mu);
        let new_phi = (self.phi + DT * M * lap_mu).clamp(-1.5, 1.5);
        let new_mu = self.phi * self.phi * self.phi - self.phi - EPS2 * lap_phi;
        Self {
            phi: new_phi,
            mu: new_mu,
        }
    }

    fn view(self) -> Color {
        let t = ((self.phi + 1.0) * 0.5).clamp(0.0, 1.0);
        Color::hsv(0.5 - t * 0.42, 0.6 + t * 0.2, 0.15 + t * 0.75)
    }
}

fn main() {
    Simulation::<CahnHilliard>::new(1024, 1024)
        .title("Cahn-Hilliard")
        .ticks_per_frame(32)
        .run();
}
