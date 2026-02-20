# Cellarium: API and Language Specification

**Version 0.2 — Draft**

> This document defines the public API of the `cellarium` crate, the
> subset of Rust accepted by the `#[cell]` proc macro, and the exact
> semantics of cell execution. It is intended to be sufficient for both
> users of the library and implementors of the proc macro.

---

## 1. Getting Started

### 1.1 Cargo Dependency

```toml
[dependencies]
cellarium = "0.1"
```

### 1.2 Minimal Example

```rust
use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Life {
    alive: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Life {
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
    Simulation::<Life>::new(512, 512).run();
}
```

### 1.3 The Prelude

`use cellarium::prelude::*` imports:

| Name              | Kind          | Purpose                        |
|-------------------|---------------|--------------------------------|
| `CellState`       | derive macro  | Generates texture layout       |
| `Cell`            | trait         | Marker trait for cell impls    |
| `cell`            | attribute macro | Cross-compiles impl to WGSL |
| `Neighbors`       | type          | Handle for spatial operators   |
| `Color`           | type          | RGBA color (alias for Vec4)    |
| `Vec2`            | type          | 2D vector                      |
| `Vec3`            | type          | 3D vector                      |
| `Vec4`            | type          | 4D vector                      |
| `Simulation`      | type          | Runtime entry point            |
| `mix`             | function      | Linear interpolation           |
| `step`            | function      | Step function                  |
| `smoothstep`      | function      | Smooth Hermite interpolation   |
| `atan2`           | function      | Two-argument arctangent        |
| `vec2`            | function      | Construct Vec2                 |
| `vec3`            | function      | Construct Vec3                 |
| `vec4`            | function      | Construct Vec4                 |
| `PI`              | constant      | 3.14159265...                  |
| `TAU`             | constant      | 6.28318530...                  |

---

## 2. Defining Cell State

### 2.1 The `CellState` Struct

Cell state is a plain Rust struct with `#[derive(CellState)]`:

```rust
#[derive(CellState, Default)]
struct Fluid {
    density: f32,
    velocity: Vec2,
    temperature: f32,
}
```

**Rules:**

1. All fields must be `f32`, `Vec2`, `Vec3`, or `Vec4`.
2. Total state must not exceed 32 scalar floats (8 textures × 4 channels).
3. Fields are packed into RGBA textures in declaration order.
4. The struct should implement `Default` to provide initial values.

### 2.2 Allowed Field Types

| Type   | Size (floats) | GPU Representation      |
|--------|---------------|-------------------------|
| `f32`  | 1             | Single texture channel  |
| `Vec2` | 2             | Two texture channels    |
| `Vec3` | 3             | Three texture channels  |
| `Vec4` | 4             | Four texture channels   |

### 2.3 Texture Packing

Fields are assigned to textures greedily. A field never spans two
textures. Example:

```rust
#[derive(CellState)]
struct Example {
    a: f32,       // tex0.r
    b: Vec2,      // tex0.gb
    c: f32,       // tex0.a
    d: Vec3,      // tex1.rgb  (won't fit in tex0's remaining 0 channels)
    e: f32,       // tex1.a
}
```

This allocates 2 textures.

---

## 3. Defining Cell Behavior

### 3.1 The `#[cell]` Attribute

Applied to an `impl Cell for T` block:

```rust
#[cell(neighborhood = moore)]
impl Cell for MyCell {
    // ...
}
```

**Parameters:**

| Parameter       | Type                                | Default |
|-----------------|-------------------------------------|---------|
| `neighborhood`  | `moore`, `von_neumann`, `radius(N)` | `moore` |

### 3.2 Neighborhoods

| Specifier      | Neighbors | Description                        |
|----------------|-----------|------------------------------------|
| `moore`        | 8         | All 8 surrounding cells            |
| `von_neumann`  | 4         | 4 cardinal directions only         |
| `radius(N)`    | (2N+1)²-1 | All cells within Chebyshev distance N |

The cell itself is never included in its own neighborhood.

### 3.3 Required Methods

#### `fn update(self, nb: Neighbors) -> Self`

Computes the cell's next-tick state given its current state and
neighbors.

- `self` is the cell's current state (all fields available as `self.field`).
- `nb` is a `Neighbors` handle for spatial operations.
- Must return `Self { field: value, ... }` with every field assigned.
- Must be a pure function: no side effects, no external state.

#### `fn view(self) -> Color`

Maps the cell's current state to a display color.

- `self` is the cell's current state.
- Must return a `Color` value.
- Cannot access neighbors.

### 3.4 Optional Methods

#### `fn init(x: f32, y: f32, w: f32, h: f32) -> Self`

Programmatic initialization. Called once per cell at startup.

- `x`, `y`: cell grid coordinates (0-based).
- `w`, `h`: grid dimensions.
- Must return `Self { ... }`.
- Cannot access neighbors (no state exists yet).

**Example:**

```rust
fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
    let cx = w / 2.0;
    let cy = h / 2.0;
    let dist = ((x - cx) * (x - cx) + (y - cy) * (y - cy)).sqrt();
    Self {
        a: 1.0,
        b: if dist < 10.0 { 1.0 } else { 0.0 },
    }
}
```

### 3.5 Associated Constants

```rust
#[cell(neighborhood = moore)]
impl Cell for GrayScott {
    const FEED: f32 = 0.055;
    const KILL: f32 = 0.062;
    // ...
}
```

- Only `f32` constants are supported.
- Available by name in all method bodies.
- Compile to WGSL `const` declarations.

---

## 4. The Neighbors API

`Neighbors` is the handle for all spatial operations. It is only
available in the `update` method.

### 4.1 Aggregation Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `nb.sum(\|c\| expr)` | `Fn(C) -> T` → `T` | Sum over all neighbors |
| `nb.mean(\|c\| expr)` | `Fn(C) -> T` → `T` | Average over all neighbors |
| `nb.min(\|c\| expr)` | `Fn(C) -> f32` → `f32` | Minimum value |
| `nb.max(\|c\| expr)` | `Fn(C) -> f32` → `f32` | Maximum value |
| `nb.count(\|c\| expr)` | `Fn(C) -> bool` → `f32` | Count where true |

Where `T` is `f32`, `Vec2`, `Vec3`, or `Vec4` — inferred from the
closure return type.

### 4.2 Filtered Aggregation Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `nb.sum_where(\|c\| val, \|c\| cond)` | value closure + filter closure | Conditional sum |
| `nb.mean_where(\|c\| val, \|c\| cond)` | value closure + filter closure | Conditional mean |
| `nb.min_where(\|c\| val, \|c\| cond)` | value closure + filter closure | Conditional min |
| `nb.max_where(\|c\| val, \|c\| cond)` | value closure + filter closure | Conditional max |

`mean_where` divides by the count of neighbors that pass the filter,
not the total neighborhood size. If no neighbors pass the filter,
`mean_where` returns zero (`0.0` or the zero vector). The emitted WGSL
guards against division by zero.

### 4.3 Differential Operators

| Method | Signature | Description |
|--------|-----------|-------------|
| `nb.laplacian(\|c\| expr)` | `Fn(C) -> T` → `T` | Discrete isotropic Laplacian |
| `nb.gradient(\|c\| expr)` | `Fn(C) -> f32` → `Vec2` | Central difference gradient |
| `nb.divergence(\|c\| expr)` | `Fn(C) -> Vec2` → `f32` | Central difference divergence |

**Restrictions:**

- `gradient` requires `f32`-valued closure. Returns `Vec2`.
- `divergence` requires `Vec2`-valued closure. Returns `f32`.
- Both require `moore` or `radius(N)` neighborhood (need diagonal
  neighbors for isotropic computation). Using them with `von_neumann`
  is a compile error.
- `laplacian` accepts any numeric type.

**Derived fields work naturally:**

```rust
// Laplacian of a derived quantity — works because the closure
// is evaluated at each neighbor position independently
let pressure_lap = nb.laplacian(|c| c.density * c.temperature);
```

### 4.4 Spatial Accessors

Inside any neighbor closure, `c` provides:

| Accessor        | Type   | Description                           |
|-----------------|--------|---------------------------------------|
| `c.field_name`  | varies | State field value at this neighbor    |
| `c.offset()`    | `Vec2` | Grid offset from self `(dx, dy)`     |
| `c.direction()`  | `Vec2` | Normalized direction to neighbor     |
| `c.distance()`   | `f32`  | Euclidean distance in grid units     |

**Example using spatial accessors:**

```rust
// Boids-like separation force
let separation = nb.sum_where(
    |c| -c.direction() / c.distance().max(0.01),
    |c| c.occupied > 0.5
);
```

### 4.5 Self-Access in Neighbor Closures

Within a neighbor closure, `self.field` still refers to the current
cell's own state. This enables relative computations:

```rust
let pressure_diff = nb.sum(|c| c.pressure - self.pressure);
```

---

## 5. Accepted Rust Subset

The `#[cell]` proc macro accepts a restricted subset of Rust syntax. This
section defines exactly what is and is not permitted in method bodies.

### 5.1 Accepted Constructs

**Bindings:**

```rust
let x = expr;
let x: f32 = expr;
```

All bindings are immutable. `let mut` is rejected.

**Conditionals:**

```rust
if condition { expr } else { expr }
if condition { expr } else if condition { expr } else { expr }
```

Both branches must produce the same type. `if` is an expression
(has a value), not a statement.

**Arithmetic operators:**

```rust
a + b    a - b    a * b    a / b    -a
```

Operand types: `f32 ⊕ f32 → f32`, `VecN ⊕ VecN → VecN`,
`f32 ⊕ VecN → VecN` (scalar broadcast).

**Comparison operators:**

```rust
a == b    a != b    a < b    a > b    a <= b    a >= b
```

Operands must both be `f32`. Result is `bool`.

**Logical operators:**

```rust
a && b    a || b    !a
```

Operands must be `bool`. Result is `bool`.

**Field access:**

```rust
self.field_name     // current cell state
v.x  v.y  v.z  v.w // vector component access
```

**Struct construction (return value only):**

```rust
Self { field1: expr1, field2: expr2, ... }
```

Every field must be assigned. This is the only way to produce the
return value of `update` and `init`.

**Method calls (translated to WGSL built-ins):**

```rust
x.sin()  x.cos()  x.abs()  x.sqrt()  x.floor()  x.ceil()
x.round()  x.exp()  x.ln()  x.log2()  x.signum()  x.fract()
x.powf(y)  x.clamp(lo, hi)  x.min(y)  x.max(y)
v.length()  v.normalize()  v.dot(w)  v.distance(w)
```

**Free function calls:**

```rust
mix(a, b, t)  step(edge, x)  smoothstep(lo, hi, x)
atan2(y, x)   vec2(x, y)     vec3(x, y, z)   vec4(x, y, z, w)
Color::rgb(r, g, b)   Color::hsv(h, s, v)   Color::rgba(r, g, b, a)
```

**Neighbor closures (in spatial operators only):**

```rust
|c| c.field_name
|c| c.field_name * c.other_field
|c| c.field_name > 0.5
|c| expr_using_c_and_self
```

**Literals:**

```rust
0.5       // f32
3         // auto-converted to f32 (becomes 3.0)
true      // bool
false     // bool
```

**Parenthesized expressions:**

```rust
(a + b) * c
```

**Early return (rewritten to `if/else`):**

```rust
if condition {
    return Self { ... };
}
// rest of body
```

The macro rewrites this to an equivalent `if/else` wrapping the
remainder of the method body.

### 5.2 Rejected Constructs

These produce compile-time errors from the proc macro:

| Construct | Reason |
|-----------|--------|
| `let mut x = ...` | GPU shaders have no mutable locals in this model |
| `x = new_value` (reassignment) | Same |
| `loop { }`, `while cond { }` | Unbounded loops |
| `for x in iter { }` | General iteration |
| `match expr { }` | Use `if/else` chains |
| `return expr` | Accepted; rewritten to wrapping `if/else` by the macro |
| `fn helper() { }` | No local function definitions |
| `struct`, `enum`, `type` | No type definitions |
| `&x`, `&mut x`, `*x` | No references or pointers |
| `Box`, `Vec`, `String`, `HashMap`, ... | No heap-allocated types |
| `println!()`, any macro call | No macro expansion inside cell code |
| `unsafe { }` | No unsafe code |
| `use`, `mod` | No module-level items |
| `async`, `.await` | No async code |
| Trait method calls (non-recognized) | Only recognized methods translate |
| Closures outside spatial operators | Closures only for `nb.*` methods |

### 5.3 Type Inference

The proc macro performs basic type inference:

1. State field types are known from the struct definition.
2. Arithmetic follows the rules in §5.1.
3. `let` binding types are inferred from the right-hand side.
4. Optional explicit type annotations are accepted and checked.
5. Spatial operator return types follow the rules in §4.

Unresolvable types produce a compile error.

---

## 6. Execution Semantics

### 6.1 Tick Model

Simulation proceeds in discrete ticks numbered 0, 1, 2, ...

At each tick:

1. **All cells read** the state from tick N (the same snapshot).
2. **All cells compute** their update rule independently.
3. **All cells write** their new state as tick N+1.
4. **All cells compute** their view function from tick N+1 state.
5. **View output** is presented to the screen.

There is no observable order among cells within a single tick. The update
function for cell (x, y) is a pure function of the tick-N state. This
is enforced by the double-buffered GPU execution model.

### 6.2 Neighbor Access Semantics

When `update` runs for cell at position `(x, y)` during tick N→N+1:

- `self.field` reads cell `(x, y)` at tick N.
- `c.field` in a neighbor closure reads cell `(x+dx, y+dy)` at tick N.
- The output `Self { ... }` writes to cell `(x, y)` at tick N+1.

A cell **cannot** read its own tick N+1 state or any other cell's
tick N+1 state. This is not merely a language restriction — it is
physically impossible in the execution model.

### 6.3 Boundary Conditions

The grid wraps toroidally. A cell at position `(0, y)` with a neighbor
offset of `(-1, 0)` reads from `(W-1, y)`. This is implemented via
texture repeat addressing and requires no special code in the shader.

### 6.4 Floating-Point Behavior

All computation uses 32-bit IEEE 754 floats. The usual GPU caveats
apply: operations are not guaranteed to be bit-exact across hardware,
`NaN` and `Inf` propagate silently, and denormals may be flushed to
zero.

Users should use `clamp` defensively to prevent state from diverging.

### 6.5 View Semantics

The `view` method runs after the simulation tick. It has access to the
newly computed state but **not** to `Neighbors`. It is a pure function
from cell state to pixel color.

The `Color` type is `Vec4` with components `(r, g, b, a)`, each in the
range `[0.0, 1.0]`. Values outside this range are clamped by the GPU
before display.

---

## 7. The `Simulation` Runtime API

### 7.1 Construction

```rust
let sim = Simulation::<MyCell>::new(width, height);
```

Creates a simulation with the given grid dimensions.

### 7.2 Configuration (Builder Pattern)

```rust
Simulation::<MyCell>::new(1024, 1024)
    .title("My Simulation")          // Window title
    .ticks_per_frame(1)              // Simulation ticks per render frame
    .paused(false)                   // Start paused?
    .run();
```

| Method              | Default         | Description                      |
|---------------------|-----------------|----------------------------------|
| `.title(s)`         | `"Cellarium"`   | Window title                     |
| `.ticks_per_frame(n)` | `1`           | Ticks per displayed frame        |
| `.paused(b)`        | `false`         | Initial pause state              |
| `.run()`            | —               | Opens window, enters main loop   |

### 7.3 Runtime Controls

While running, the following keyboard controls are available:

| Key       | Action                                    |
|-----------|-------------------------------------------|
| `Space`   | Pause / resume simulation                 |
| `→`       | Step one tick (while paused)              |
| `+` / `-` | Increase / decrease ticks per frame       |
| `R`       | Reset to initial state                    |
| `Escape`  | Quit                                      |

### 7.4 Mouse Interaction (Future Extension)

```rust
#[cell(neighborhood = moore)]
impl Cell for MyCell {
    fn update(self, nb: Neighbors) -> Self {
        // mouse_pos: Vec2 in grid coordinates (NaN if not hovering)
        // mouse_down: f32 (1.0 if pressed, 0.0 if not)
        // Available as built-in values in update
        ...
    }
}
```

Mouse state would be passed as additional uniforms, enabling interactive
painting or perturbation of the simulation.

---

## 8. Complete Examples

### 8.1 Gray-Scott Reaction Diffusion

```rust
use cellarium::prelude::*;

#[derive(CellState)]
struct GrayScott {
    a: f32,
    b: f32,
}

impl Default for GrayScott {
    fn default() -> Self {
        Self { a: 1.0, b: 0.0 }
    }
}

#[cell(neighborhood = moore)]
impl Cell for GrayScott {
    const FEED: f32 = 0.055;
    const KILL: f32 = 0.062;
    const DA: f32 = 1.0;
    const DB: f32 = 0.5;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let cx = w / 2.0;
        let cy = h / 2.0;
        let dx = x - cx;
        let dy = y - cy;
        let dist = (dx * dx + dy * dy).sqrt();
        Self {
            a: 1.0,
            b: if dist < 10.0 { 1.0 } else { 0.0 },
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let reaction = self.a * self.b * self.b;
        Self {
            a: (self.a + DA * nb.laplacian(|c| c.a) - reaction
                + FEED * (1.0 - self.a)).clamp(0.0, 1.0),
            b: (self.b + DB * nb.laplacian(|c| c.b) + reaction
                - (KILL + FEED) * self.b).clamp(0.0, 1.0),
        }
    }

    fn view(self) -> Color {
        let v = self.a - self.b;
        Color::rgb(v, v * 0.5, self.b * 0.8)
    }
}

fn main() {
    Simulation::<GrayScott>::new(512, 512)
        .title("Gray-Scott Reaction Diffusion")
        .ticks_per_frame(4)
        .run();
}
```

### 8.2 Wireworld

```rust
use cellarium::prelude::*;

#[derive(CellState, Default)]
struct Wire {
    kind: f32,
}

#[cell(neighborhood = moore)]
impl Cell for Wire {
    const EMPTY: f32 = 0.0;
    const HEAD: f32 = 1.0;
    const TAIL: f32 = 2.0;
    const CONDUCTOR: f32 = 3.0;

    fn update(self, nb: Neighbors) -> Self {
        let heads = nb.count(|c| c.kind == HEAD);
        let kind = if self.kind == EMPTY {
            EMPTY
        } else if self.kind == HEAD {
            TAIL
        } else if self.kind == TAIL {
            CONDUCTOR
        } else if self.kind == CONDUCTOR && (heads == 1.0 || heads == 2.0) {
            HEAD
        } else {
            self.kind
        };
        Self { kind }
    }

    fn view(self) -> Color {
        if self.kind == HEAD {
            Color::rgb(0.2, 0.6, 1.0)
        } else if self.kind == TAIL {
            Color::rgb(1.0, 0.3, 0.1)
        } else if self.kind == CONDUCTOR {
            Color::rgb(1.0, 0.85, 0.2)
        } else {
            Color::BLACK
        }
    }
}

fn main() {
    Simulation::<Wire>::new(256, 256)
        .title("Wireworld")
        .run();
}
```

### 8.3 Continuous SmoothLife

```rust
use cellarium::prelude::*;

#[derive(CellState, Default)]
struct SmoothLife {
    value: f32,
}

#[cell(neighborhood = radius(6))]
impl Cell for SmoothLife {
    const INNER_R: f32 = 3.0;
    const BIRTH_LO: f32 = 0.278;
    const BIRTH_HI: f32 = 0.365;
    const DEATH_LO: f32 = 0.267;
    const DEATH_HI: f32 = 0.445;
    const ALPHA: f32 = 0.028;
    const DT: f32 = 0.1;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let cx = x - w / 2.0;
        let cy = y - h / 2.0;
        let r = (cx * cx + cy * cy).sqrt();
        Self {
            value: if r < 20.0 {
                (1.0 - r / 20.0).clamp(0.0, 1.0)
            } else {
                0.0
            },
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

        let sigma_birth = 1.0 / (1.0 + (-((outer - BIRTH_LO) / ALPHA)).exp())
                        - 1.0 / (1.0 + (-((outer - BIRTH_HI) / ALPHA)).exp());
        let sigma_death = 1.0 / (1.0 + (-((outer - DEATH_LO) / ALPHA)).exp())
                        - 1.0 / (1.0 + (-((outer - DEATH_HI) / ALPHA)).exp());

        let s = 1.0 / (1.0 + (-((inner - 0.5) / ALPHA)).exp());
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
    Simulation::<SmoothLife>::new(256, 256)
        .title("SmoothLife")
        .ticks_per_frame(2)
        .run();
}
```

### 8.4 Wave Equation

```rust
use cellarium::prelude::*;

#[derive(CellState)]
struct Wave {
    height: f32,
    velocity: f32,
}

impl Default for Wave {
    fn default() -> Self {
        Self { height: 0.0, velocity: 0.0 }
    }
}

#[cell(neighborhood = moore)]
impl Cell for Wave {
    const SPEED: f32 = 0.4;
    const DAMPING: f32 = 0.999;

    fn init(x: f32, y: f32, w: f32, h: f32) -> Self {
        let cx = x - w / 2.0;
        let cy = y - h / 2.0;
        let r = (cx * cx + cy * cy).sqrt();
        Self {
            height: if r < 8.0 { (1.0 - r / 8.0).max(0.0) } else { 0.0 },
            velocity: 0.0,
        }
    }

    fn update(self, nb: Neighbors) -> Self {
        let accel = SPEED * SPEED * nb.laplacian(|c| c.height);
        let new_vel = (self.velocity + accel) * DAMPING;
        Self {
            height: self.height + new_vel,
            velocity: new_vel,
        }
    }

    fn view(self) -> Color {
        let v = self.height * 0.5 + 0.5;
        Color::rgb(
            smoothstep(0.5, 0.8, v),
            smoothstep(0.3, 0.6, v) * 0.8,
            smoothstep(0.0, 0.5, v),
        )
    }
}

fn main() {
    Simulation::<Wave>::new(512, 512)
        .title("Wave Equation")
        .ticks_per_frame(2)
        .run();
}
```
```

---

## 9. Error Reference

Compile-time errors produced by the `#[cell]` and `#[derive(CellState)]`
macros:

| Code  | Message | Cause |
|-------|---------|-------|
| C001  | `'{type}' is not a GPU-compatible type. State fields must be f32, Vec2, Vec3, or Vec4.` | Invalid field type in CellState struct |
| C002  | `State exceeds maximum of 32 floats ({n} declared). Reduce the number of state fields.` | Too many state fields |
| C003  | `Missing field '{name}' in return struct. All state fields must be assigned.` | Incomplete struct literal in update return |
| C004  | `Extra field '{name}' in return struct.` | Field not in CellState struct |
| C005  | `Type mismatch for field '{name}': expected {T}, got {U}.` | Wrong type assigned to field |
| C006  | `let mut is not supported. All bindings are immutable in cell code.` | Mutable binding |
| C007  | `Unbounded loops are not GPU-compatible. Use spatial operators for neighbor iteration.` | `loop`, `while`, or `for` |
| C008  | `match is not supported in cell code. Use if/else chains.` | `match` expression |
| C009  | `Early return rewritten to if/else. Consider writing it as an if/else directly.` | `return` keyword (warning, not error) |
| C010  | `Closures are only valid as arguments to Neighbors methods.` | Closure in non-spatial context |
| C011  | `Neighbors is only available in the update method.` | `nb` used in `view` or `init` |
| C012  | `gradient/divergence requires moore or radius(N) neighborhood.` | Differential op with von_neumann |
| C013  | `if branches have different types: {T} vs {U}.` | Type mismatch in conditional |
| C014  | `Cannot compare {type} values. Use .length() for vector magnitude comparison.` | Vector comparison |
| C015  | `'{name}' is not a recognized method. See cellarium docs for supported operations.` | Unknown method call |
| C016  | `References and borrowing are not used in cell code.` | `&` or `&mut` |
| C017  | `'{name}' is not a field of {struct}.` | Nonexistent field access |
| C018  | `Spatial accessor c.offset()/c.direction()/c.distance() used outside a neighbor closure.` | Accessor misuse |
| C019  | `'{construct}' is not supported in cell code.` | Catch-all for other unsupported syntax |

---

## 10. Appendix: Vec2, Vec3, Vec4 API

These types exist both as host-side Rust types (for `Default` impls and
tests) and as proc-macro-recognized types that translate to WGSL vectors.

### 10.1 Construction

```rust
let v = vec2(1.0, 2.0);
let v = Vec2::new(1.0, 2.0);
let v = Vec2::splat(0.0);       // (0.0, 0.0)

// Same patterns for Vec3, Vec4
```

### 10.2 Component Access

```rust
v.x  v.y              // Vec2
v.x  v.y  v.z         // Vec3
v.x  v.y  v.z  v.w    // Vec4
```

### 10.3 Arithmetic

All standard operators work component-wise. Scalar broadcast is
supported: `vec2(1.0, 2.0) * 3.0` produces `vec2(3.0, 6.0)`.

### 10.4 Methods

| Method        | Signature            | Description              |
|---------------|----------------------|--------------------------|
| `.length()`   | `VecN → f32`         | Euclidean magnitude      |
| `.normalize()` | `VecN → VecN`       | Unit vector              |
| `.dot(other)` | `VecN × VecN → f32`  | Dot product              |
| `.distance(other)` | `VecN × VecN → f32` | Euclidean distance    |
| `.cross(other)` | `Vec3 × Vec3 → Vec3` | Cross product (Vec3 only) |
