use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Ecosystem {
    grass: f32,
    prey: f32,
    predator: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Ecosystem {
    const DT: f32 = 0.15;
    const GROW: f32 = 0.8;      // grass regrowth rate
    const GRAZE: f32 = 2.0;     // prey eating grass
    const HUNT: f32 = 1.5;      // predator eating prey
    const BIRTH_H: f32 = 0.7;   // prey birth rate per food
    const BIRTH_P: f32 = 0.4;   // predator birth rate per food
    const DEATH_H: f32 = 0.04;  // prey natural death
    const DEATH_P: f32 = 0.35;  // predator starvation

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        let h2 = ((x * 63.7264 + y * 10.873).sin() * 43758.5453).fract();
        let h3 = ((x * 36.234 + y * 97.135).sin() * 43758.5453).fract();

        // Grass: everywhere, patchy density
        let gx = (x * 0.015).sin() * (y * 0.021).cos();
        let grass = 0.5 + gx * 0.3 + h1 * 0.2;

        // Prey: scattered herds in ~300px patches
        let px = (x / 300.0).floor();
        let py = (y / 300.0).floor();
        let phash = ((px * 43.17 + py * 91.53).sin() * 43758.5453).fract();
        let prey = if phash < 0.4 { 0.3 + h2 * 0.3 } else { 0.0 };

        // Predator: sparse packs in ~400px patches
        let qx = (x / 400.0).floor();
        let qy = (y / 400.0).floor();
        let qhash = ((qx * 71.31 + qy * 29.87).sin() * 43758.5453).fract();
        let predator = if qhash < 0.25 { 0.2 + h3 * 0.2 } else { 0.0 };

        Self { grass, prey, predator }
    }

    fn update(self, nb: Neighbors) -> Self {
        // Diffuse — prey flee fast, predators slower, grass barely spreads
        let avg_g = nb.mean(|c| c.grass);
        let avg_h = nb.mean(|c| c.prey);
        let avg_p = nb.mean(|c| c.predator);

        let g = self.grass + 0.02 * (avg_g - self.grass);
        let h = self.prey + 0.18 * (avg_h - self.prey);
        let p = self.predator + 0.07 * (avg_p - self.predator);

        // Interactions
        let eaten_grass = GRAZE * g * h;
        let eaten_prey = HUNT * h * p;

        // Grass regrows logistically, consumed by prey
        let new_g = (g + DT * (GROW * g * (1.0 - g) - eaten_grass)).clamp(0.0, 1.0);
        // Prey: born from eating grass, eaten by predators, natural death
        let new_h = (h + DT * (BIRTH_H * eaten_grass - eaten_prey - DEATH_H * h)).clamp(0.0, 1.0);
        // Predators: born from eating prey, starve without food
        let new_p = (p + DT * (BIRTH_P * eaten_prey - DEATH_P * p)).clamp(0.0, 1.0);

        Self { grass: new_g, prey: new_h, predator: new_p }
    }

    fn view(self) -> Color {
        let g = self.grass;
        let h = self.prey;
        let p = self.predator;
        // Nature palette: green grass, white/blue prey, red predators
        Color::rgb(
            p * 0.95 + h * 0.3 + g * 0.05,
            g * 0.6 + h * 0.4 + p * 0.05,
            h * 0.7 + p * 0.15,
        )
    }
}

fn main() {
    Simulation::<Ecosystem>::new(2048, 2048)
        .title("Predator Prey")
        .ticks_per_frame(4)
        .run();
}
