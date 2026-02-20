use cellarium::prelude::*;

#[derive(CellState, Default)]
struct SmoothLife {
    value: f32,
}

#[cell(neighborhood = radius(12))]
impl Cell for SmoothLife {
    const INNER_R: f32 = 4.0;
    const BIRTH_LO: f32 = 0.278;
    const BIRTH_HI: f32 = 0.365;
    const DEATH_LO: f32 = 0.267;
    const DEATH_HI: f32 = 0.445;
    const ALPHA_N: f32 = 0.028;
    const ALPHA_M: f32 = 0.147;
    const DT: f32 = 0.05;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        // Fill the whole grid with pseudo-random noise
        let hash = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        Self {
            value: if hash > 0.5 { 1.0 } else { 0.0 },
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let inner = nb.mean_where(
            |c| c.value,
            |c| c.distance() <= INNER_R,
        );
        let outer = nb.mean_where(
            |c| c.value,
            |c| c.distance() > INNER_R,
        );

        // Smooth interval functions on outer ring density
        let sigma_birth = 1.0 / (1.0 + (-((outer - BIRTH_LO) / ALPHA_N)).exp())
                        - 1.0 / (1.0 + (-((outer - BIRTH_HI) / ALPHA_N)).exp());
        let sigma_death = 1.0 / (1.0 + (-((outer - DEATH_LO) / ALPHA_N)).exp())
                        - 1.0 / (1.0 + (-((outer - DEATH_HI) / ALPHA_N)).exp());

        // Smooth transition based on inner disk density (wider sigmoid)
        let s = 1.0 / (1.0 + (-((inner - 0.5) / ALPHA_M)).exp());
        let target = mix(sigma_birth, sigma_death, s);

        Self {
            value: (self.value + DT * (2.0 * target - 1.0)).clamp(0.0, 1.0),
        }
    }

    fn view(self) -> Color {
        let c = smoothstep(0.0, 1.0, self.value);
        Color::rgb(c * 0.2, c * 0.8, c)
    }
}

fn main() {
    Simulation::<SmoothLife>::new(1024, 1024)
        .title("SmoothLife")
        .ticks_per_frame(1)
        .run();
}
