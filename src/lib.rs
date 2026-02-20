pub mod types;
pub mod runtime;
pub mod texture;
pub mod pipeline;

pub use cellarium_macros::{CellState, cell};

pub mod prelude {
    pub use crate::types::{
        Vec2, Vec3, Vec4, Color, Neighbors,
        CellState, Cell, FieldMapping,
        PI, TAU,
        vec2, vec3, vec4,
        mix, step, smoothstep, atan2,
    };
    pub use crate::runtime::Simulation;
    pub use cellarium_macros::{CellState, cell};
}
