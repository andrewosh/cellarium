use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Terrain {
    height: f32,
    water: f32,
    hardness: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Terrain {
    const UPLIFT: f32 = 0.0005;
    const THERMAL: f32 = 0.1;
    const ERODE: f32 = 0.06;
    const WEATHER: f32 = 0.006;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();

        // Terrain with fine roughness
        let t = 0.45
            + 0.15 * (x * 0.005 + 0.5).sin() * (y * 0.006 + 1.2).cos()
            + 0.10 * (x * 0.015 + 2.1).sin() * (y * 0.012 + 0.7).cos()
            + 0.05 * (x * 0.04 + 3.7).sin() * (y * 0.035 + 2.9).cos()
            + h1 * 0.08;

        // Tectonic hardness zones (uncorrelated with terrain)
        let r = 0.5
            + 0.3 * (x * 0.004 + 2.7).cos() * (y * 0.003 + 0.4).sin()
            + 0.15 * (x * 0.011 + 1.1).cos() * (y * 0.009 + 3.5).sin();

        Self {
            height: t.clamp(0.15, 0.85),
            water: 0.0,
            hardness: r.clamp(0.1, 0.95),
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let avg_h = nb.mean(|c| c.height);

        let diff = avg_h - self.height;
        let slope = diff.abs();
        let softness = 1.1 - self.hardness;

        // TECTONIC UPLIFT: hard rock rises, self-regulating (slows near max)
        let uplift = UPLIFT * self.hardness * self.hardness * (1.0 - self.height);

        // WATER: instantaneous — pools in valleys, non-linear concentration
        // No accumulation or evaporation — just terrain shape
        let valley = diff.max(0.0);
        let water = (valley * valley * 500.0).clamp(0.0, 0.5);

        // THERMAL SMOOTHING: hard rock RESISTS (softness^2)
        let thermal = THERMAL * diff * softness * softness;

        // EROSION: water + slope + soft rock
        let erosion = ERODE * water * slope * softness * self.height;

        // WEATHERING: exposed ridges crumble
        let exposure = (0.0 - diff).max(0.0);
        let weather = WEATHER * exposure * softness;

        Self {
            height: (self.height + uplift + thermal - erosion - weather).clamp(0.05, 0.99),
            water: water,
            hardness: (self.hardness - weather * 0.06).clamp(0.05, 0.95),
        }
    }

    fn view(self) -> Color {
        let h = self.height;
        let w = (self.water * 6.0).clamp(0.0, 1.0);
        let r = self.hardness;

        // Terrain: warm brown (soft rock) to cool gray (hard rock)
        let warm = (1.0 - r) * 0.3;
        let cool = r * 0.35;
        let tr = h * (0.65 + warm);
        let tg = h * (0.50 + cool * 0.15);
        let tb = h * (0.30 + cool);

        // Water: blue overlay
        Color::rgb(
            tr * (1.0 - w * 0.85) + 0.02 * w,
            tg * (1.0 - w * 0.65) + 0.12 * w,
            tb * (1.0 - w * 0.3) + 0.55 * w,
        )
    }
}

fn main() {
    Simulation::<Terrain>::new(1024, 1024)
        .title("Geological Erosion")
        .ticks_per_frame(32)
        .run();
}
