use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Brain {
    state: f32, // 0=off, 1=on, 2=dying
}

#[cell(neighborhood = moore)]
impl Cell for Brain {
    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let hash = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        Self {
            state: if hash > 0.85 { 1.0 } else { 0.0 },
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let on_count = nb.count(|c| c.state > 0.5 && c.state < 1.5);
        let state = if self.state < 0.5 {
            if on_count > 1.2 && on_count < 2.5 { 1.0 } else { 0.0 }
        } else if self.state < 1.5 {
            2.0
        } else {
            0.0
        };
        Self { state }
    }

    fn view(self) -> Color {
        if self.state > 1.5 {
            Color::rgb(0.6, 0.2, 0.05)
        } else if self.state > 0.5 {
            Color::rgb(0.4, 0.8, 1.0)
        } else {
            Color::BLACK
        }
    }
}

fn main() {
    Simulation::<Brain>::new(2048, 2048)
        .title("Brian's Brain")
        .run();
}
