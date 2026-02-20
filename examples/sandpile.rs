use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Sandpile {
    grains: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Sandpile {
    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let hash = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();

        // Near-critical background: most cells at 7 (one below threshold of 8)
        // Some at 5-6 act as "firebreaks" that slow the cascade
        let bg = if hash < 0.65 { 7.0 } else if hash < 0.80 { 6.0 } else if hash < 0.92 { 5.0 } else { (hash * 8.0).floor() };

        // Tall piles that trigger massive cascading avalanches
        let d0 = ((x - w * 0.5) * (x - w * 0.5) + (y - h * 0.5) * (y - h * 0.5)).sqrt();
        let d1 = ((x - w * 0.2) * (x - w * 0.2) + (y - h * 0.25) * (y - h * 0.25)).sqrt();
        let d2 = ((x - w * 0.8) * (x - w * 0.8) + (y - h * 0.75) * (y - h * 0.75)).sqrt();
        let d3 = ((x - w * 0.25) * (x - w * 0.25) + (y - h * 0.8) * (y - h * 0.8)).sqrt();
        let d4 = ((x - w * 0.75) * (x - w * 0.75) + (y - h * 0.2) * (y - h * 0.2)).sqrt();

        let pile = if d0 < 3.0 { 800.0 }
            else if d1 < 2.0 { 500.0 }
            else if d2 < 2.0 { 600.0 }
            else if d3 < 2.0 { 400.0 }
            else if d4 < 2.0 { 450.0 }
            else { 0.0 };

        Self { grains: bg + pile }
    }

    fn update(self, nb: Neighbors) -> Self {
        // Each toppling neighbor (>= 8 grains) sends us 1 grain
        let incoming = nb.count(|c| c.grains > 7.5);
        // We lose 8 grains if we topple
        let topple = if self.grains > 7.5 { 8.0 } else { 0.0 };
        Self { grains: self.grains - topple + incoming }
    }

    fn view(self) -> Color {
        let g = self.grains;
        let i = (g / 30.0).clamp(0.5, 1.0);
        let t = g / 7.0;
        if g > 7.5 {
            // Active toppling: bright flash
            Color::rgb(i, i * 0.95, i * 0.8)
        } else {
            // Stable: purple(0) → blue → cyan → green → yellow → orange → red(7)
            Color::hsv(0.75 - t * 0.6, 0.5 + t * 0.4, 0.06 + t * 0.85)
        }
    }
}

fn main() {
    Simulation::<Sandpile>::new(1024, 1024)
        .title("Abelian Sandpile")
        .ticks_per_frame(16)
        .run();
}
