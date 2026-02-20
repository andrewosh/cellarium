use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Cyclic {
    state: f32, // integer 0..N-1 stored as f32
}

#[cell(neighborhood = moore)]
impl Cell for Cyclic {
    const N: f32 = 14.0;
    const THRESHOLD: f32 = 1.0;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let hash = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        Self {
            state: (hash * N).floor(),
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let next = if self.state + 1.0 >= N { 0.0 } else { self.state + 1.0 };
        let advance_count = nb.count(|c| (c.state - next).abs() < 0.5);
        let state = if advance_count >= THRESHOLD { next } else { self.state };
        Self { state }
    }

    fn view(self) -> Color {
        Color::hsv(self.state / N, 0.85, 0.9)
    }
}

fn main() {
    Simulation::<Cyclic>::new(2048, 2048)
        .title("Cyclic Cellular Automaton")
        .run();
}
