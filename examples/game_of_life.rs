use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Life {
    alive: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Life {
    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let hash = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        Self {
            alive: if hash > 0.6 { 1.0 } else { 0.0 },
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let n = nb.count(|c| c.alive > 0.5);
        let alive = if self.alive > 0.5 && (n == 2.0 || n == 3.0) {
            1.0
        } else if self.alive < 0.5 && n == 3.0 {
            1.0
        } else {
            0.0
        };
        Self { alive }
    }

    fn view(self) -> Color {
        if self.alive > 0.5 { Color::WHITE } else { Color::BLACK }
    }
}

fn main() {
    Simulation::<Life>::new(2048, 2048)
        .title("Game of Life")
        .run();
}
