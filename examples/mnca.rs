use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Mnca {
    state: f32,
}

#[cell(neighborhood = radius(15))]
impl Cell for Mnca {
    const R1: f32 = 4.5;
    const R2: f32 = 9.5;
    const T1: f32 = 0.22;
    const T2: f32 = 0.36;
    const T3: f32 = 0.31;
    const WIDTH: f32 = 0.04;
    const DT: f32 = 0.05;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let h1 = ((x * 12.9898 + y * 78.233).sin() * 43758.5453).fract();
        let low_freq = ((x * 0.008).sin() * (y * 0.011).cos() + 1.0) * 0.5;
        let state = if h1 < low_freq * 0.6 { 0.8 + h1 * 0.2 } else { 0.0 };
        Self { state }
    }

    fn update(self, nb: Neighbors) -> Self {
        let ring1 = nb.mean_where(|c| c.state, |c| c.distance() <= R1);
        let ring2 = nb.mean_where(|c| c.state, |c| c.distance() > R1 && c.distance() <= R2);
        let ring3 = nb.mean_where(|c| c.state, |c| c.distance() > R2);

        let g1 = (-(ring1 - T1) * (ring1 - T1) / (WIDTH * 2.0)).exp() * 2.0 - 1.0;
        let g2 = (-(ring2 - T2) * (ring2 - T2) / (WIDTH * 2.0)).exp() * 2.0 - 1.0;
        let g3 = (-(ring3 - T3) * (ring3 - T3) / (WIDTH * 2.0)).exp() * 2.0 - 1.0;

        let growth = (g1 + g2 + g3) / 3.0;

        Self {
            state: (self.state + DT * growth).clamp(0.0, 1.0),
        }
    }

    fn view(self) -> Color {
        let t = self.state.clamp(0.0, 1.0);
        Color::hsv(0.48 - t * 0.38, 0.5 + t * 0.4, 0.04 + t * 0.92)
    }
}

fn main() {
    Simulation::<Mnca>::new(1024, 1024)
        .title("MNCA")
        .ticks_per_frame(1)
        .run();
}
