use std::ops::{Add, Sub, Mul, Div, Neg};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const PI: f32 = std::f32::consts::PI;
pub const TAU: f32 = std::f32::consts::TAU;

// ---------------------------------------------------------------------------
// Vec2
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn splat(v: f32) -> Self {
        Self { x: v, y: v }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 { Self::ZERO } else { self / len }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self { Self { x: self.x + rhs.x, y: self.y + rhs.y } }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { Self { x: self.x - rhs.x, y: self.y - rhs.y } }
}

impl Mul for Vec2 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self { Self { x: self.x * rhs.x, y: self.y * rhs.y } }
}

impl Div for Vec2 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self { Self { x: self.x / rhs.x, y: self.y / rhs.y } }
}

impl Neg for Vec2 {
    type Output = Self;
    fn neg(self) -> Self { Self { x: -self.x, y: -self.y } }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self { Self { x: self.x * rhs, y: self.y * rhs } }
}

impl Mul<Vec2> for f32 {
    type Output = Vec2;
    fn mul(self, rhs: Vec2) -> Vec2 { Vec2 { x: self * rhs.x, y: self * rhs.y } }
}

impl Div<f32> for Vec2 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self { Self { x: self.x / rhs, y: self.y / rhs } }
}

impl Add<f32> for Vec2 {
    type Output = Self;
    fn add(self, rhs: f32) -> Self { Self { x: self.x + rhs, y: self.y + rhs } }
}

impl Sub<f32> for Vec2 {
    type Output = Self;
    fn sub(self, rhs: f32) -> Self { Self { x: self.x - rhs, y: self.y - rhs } }
}

// ---------------------------------------------------------------------------
// Vec3
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn splat(v: f32) -> Self {
        Self { x: v, y: v, z: v }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 { Self::ZERO } else { self / len }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self { Self { x: self.x + rhs.x, y: self.y + rhs.y, z: self.z + rhs.z } }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { Self { x: self.x - rhs.x, y: self.y - rhs.y, z: self.z - rhs.z } }
}

impl Mul for Vec3 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self { Self { x: self.x * rhs.x, y: self.y * rhs.y, z: self.z * rhs.z } }
}

impl Div for Vec3 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self { Self { x: self.x / rhs.x, y: self.y / rhs.y, z: self.z / rhs.z } }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self { Self { x: -self.x, y: -self.y, z: -self.z } }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self { Self { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs } }
}

impl Mul<Vec3> for f32 {
    type Output = Vec3;
    fn mul(self, rhs: Vec3) -> Vec3 { Vec3 { x: self * rhs.x, y: self * rhs.y, z: self * rhs.z } }
}

impl Div<f32> for Vec3 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self { Self { x: self.x / rhs, y: self.y / rhs, z: self.z / rhs } }
}

impl Add<f32> for Vec3 {
    type Output = Self;
    fn add(self, rhs: f32) -> Self { Self { x: self.x + rhs, y: self.y + rhs, z: self.z + rhs } }
}

impl Sub<f32> for Vec3 {
    type Output = Self;
    fn sub(self, rhs: f32) -> Self { Self { x: self.x - rhs, y: self.y - rhs, z: self.z - rhs } }
}

// ---------------------------------------------------------------------------
// Vec4
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };

    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn splat(v: f32) -> Self {
        Self { x: v, y: v, z: v, w: v }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 { Self::ZERO } else { self / len }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }

    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }
}

impl Add for Vec4 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self { Self { x: self.x + rhs.x, y: self.y + rhs.y, z: self.z + rhs.z, w: self.w + rhs.w } }
}

impl Sub for Vec4 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { Self { x: self.x - rhs.x, y: self.y - rhs.y, z: self.z - rhs.z, w: self.w - rhs.w } }
}

impl Mul for Vec4 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self { Self { x: self.x * rhs.x, y: self.y * rhs.y, z: self.z * rhs.z, w: self.w * rhs.w } }
}

impl Div for Vec4 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self { Self { x: self.x / rhs.x, y: self.y / rhs.y, z: self.z / rhs.z, w: self.w / rhs.w } }
}

impl Neg for Vec4 {
    type Output = Self;
    fn neg(self) -> Self { Self { x: -self.x, y: -self.y, z: -self.z, w: -self.w } }
}

impl Mul<f32> for Vec4 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self { Self { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs, w: self.w * rhs } }
}

impl Mul<Vec4> for f32 {
    type Output = Vec4;
    fn mul(self, rhs: Vec4) -> Vec4 { Vec4 { x: self * rhs.x, y: self * rhs.y, z: self * rhs.z, w: self * rhs.w } }
}

impl Div<f32> for Vec4 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self { Self { x: self.x / rhs, y: self.y / rhs, z: self.z / rhs, w: self.w / rhs } }
}

impl Add<f32> for Vec4 {
    type Output = Self;
    fn add(self, rhs: f32) -> Self { Self { x: self.x + rhs, y: self.y + rhs, z: self.z + rhs, w: self.w + rhs } }
}

impl Sub<f32> for Vec4 {
    type Output = Self;
    fn sub(self, rhs: f32) -> Self { Self { x: self.x - rhs, y: self.y - rhs, z: self.z - rhs, w: self.w - rhs } }
}

// ---------------------------------------------------------------------------
// Color (alias for Vec4 with convenience constructors)
// ---------------------------------------------------------------------------

pub type Color = Vec4;

impl Color {
    pub const WHITE: Self = Self { x: 1.0, y: 1.0, z: 1.0, w: 1.0 };
    pub const BLACK: Self = Self { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { x: r, y: g, z: b, w: 1.0 }
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { x: r, y: g, z: b, w: a }
    }

    pub fn hsv(h: f32, s: f32, v: f32) -> Self {
        let h = ((h % 1.0) + 1.0) % 1.0;
        let c = v * s;
        let h6 = h * 6.0;
        let x = c * (1.0 - ((h6 % 2.0) - 1.0).abs());
        let m = v - c;
        let (r, g, b) = if h6 < 1.0 {
            (c, x, 0.0)
        } else if h6 < 2.0 {
            (x, c, 0.0)
        } else if h6 < 3.0 {
            (0.0, c, x)
        } else if h6 < 4.0 {
            (0.0, x, c)
        } else if h6 < 5.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };
        Self { x: r + m, y: g + m, z: b + m, w: 1.0 }
    }
}

// ---------------------------------------------------------------------------
// Free functions (WGSL-equivalent math)
// ---------------------------------------------------------------------------

pub fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

pub fn vec3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

pub fn vec4(x: f32, y: f32, z: f32, w: f32) -> Vec4 {
    Vec4::new(x, y, z, w)
}

pub fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

pub fn step(edge: f32, x: f32) -> f32 {
    if x < edge { 0.0 } else { 1.0 }
}

pub fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn atan2(y: f32, x: f32) -> f32 {
    y.atan2(x)
}

// ---------------------------------------------------------------------------
// Neighbors (marker type — only meaningful to the proc macro)
// ---------------------------------------------------------------------------

pub struct Neighbors {
    _private: (),
}

// ---------------------------------------------------------------------------
// FieldMapping — texture layout metadata
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct FieldMapping {
    pub name: &'static str,
    pub texture: u32,
    pub offset: u32,
    pub size: u32,
}

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

pub trait CellState: Default + 'static {
    const TEXTURE_COUNT: u32;
    const FIELD_LAYOUT: &'static [FieldMapping];

    fn defaults() -> Vec<[f32; 4]>;
}

pub trait Cell: CellState {
    const UPDATE_SHADER: &'static str;
    const VIEW_SHADER: &'static str;
    const INIT_SHADER: &'static str;
    const HAS_INIT: bool;
    const PARAM_NAMES: &'static [&'static str];
    const PARAM_DEFAULTS: &'static [f32];
}
