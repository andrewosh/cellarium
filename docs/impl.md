# Cellarium: Implementation Specification

**Version 0.2 — Draft**

> Cellarium is a Rust library for programming synchronous cellular
> simulations that execute entirely on the GPU. Users define cell state as
> a Rust struct and cell behavior as trait methods. A procedural macro
> cross-compiles these definitions to WGSL shaders at Rust compile time.
> A wgpu-based runtime handles all GPU resource management and execution.

---

## 1. System Overview

A Cellarium program defines a single **cell type** as a Rust struct. Every
pixel on a two-dimensional grid is an instance of that cell. All cells
share the same update rule. Simulation proceeds in discrete, synchronous
**ticks**: at each tick, every cell reads the previous tick's state of
itself and its neighbors, computes its next state, and writes it. The full
grid state is then rendered to the screen.

The system has three phases:

1. **Compilation** (Rust compile time): the `#[cell]` proc macro parses
   the `impl Cell` block, type-checks it against GPU constraints, and
   emits WGSL shader source code as embedded string constants. The
   `#[derive(CellState)]` macro generates texture layout metadata.

2. **Initialization** (program startup): the runtime creates wgpu
   resources — device, textures, bind groups, pipelines — using the
   generated metadata, and writes initial state to GPU textures.

3. **Execution** (main loop): a loop alternates between simulation ticks
   and rendering passes. No simulation data ever leaves GPU memory.

### 1.1 Crate Structure

```
cellarium/
├── cellarium/              # Main library crate
│   ├── src/
│   │   ├── lib.rs          # Public API, re-exports
│   │   ├── runtime.rs      # wgpu simulation loop
│   │   ├── texture.rs      # Texture allocation and ping-pong
│   │   ├── pipeline.rs     # Render/compute pipeline setup
│   │   ├── window.rs       # winit window management
│   │   └── types.rs        # Color, Vec2, Vec3, Vec4, Neighbors
│   └── Cargo.toml
├── cellarium-macros/       # Proc macro crate
│   ├── src/
│   │   ├── lib.rs          # Macro entry points
│   │   ├── parse.rs        # AST analysis using syn
│   │   ├── check.rs        # GPU constraint validation
│   │   ├── lower.rs        # Rust AST → WGSL emission
│   │   └── layout.rs       # State struct → texture layout
│   └── Cargo.toml
└── examples/
    ├── game_of_life.rs
    ├── gray_scott.rs
    └── smoothlife.rs
```

The proc macro crate (`cellarium-macros`) is a compile-time dependency
only. It produces no runtime code beyond string constants and trait
implementations.

---

## 2. The `#[derive(CellState)]` Macro

### 2.1 Purpose

Derives texture layout metadata from a plain Rust struct. Generates an
implementation of the `CellState` trait, which the runtime uses to
allocate GPU textures and map struct fields to texture channels.

### 2.2 Input

```rust
#[derive(CellState)]
struct GrayScott {
    a: f32,
    b: f32,
}
```

### 2.3 Field Type Restrictions

The macro accepts only the following field types:

| Rust Type | GPU Type | Channels Consumed |
|-----------|----------|-------------------|
| `f32`     | `f32`    | 1                 |
| `Vec2`    | `vec2f`  | 2                 |
| `Vec3`    | `vec3f`  | 3                 |
| `Vec4`    | `vec4f`  | 4                 |

`Vec2`, `Vec3`, `Vec4` are types re-exported from the `cellarium`
prelude (thin wrappers around `[f32; N]` with arithmetic ops).

Any other field type produces a compile error:

```
error: cellarium: `String` is not a GPU-compatible type.
       State fields must be f32, Vec2, Vec3, or Vec4.
  --> src/main.rs:4:5
   |
4  |     name: String,
   |     ^^^^^^^^^^^^
```

### 2.4 Texture Layout Algorithm

Fields are packed into RGBA textures (4 channels each) in declaration
order. A field is never split across textures.

```
For each field in declaration order:
    If the field fits in the remaining channels of the current texture:
        Assign it to those channels.
    Else:
        Start a new texture. Assign the field to its first channels.
```

The total number of textures must not exceed 8 (the guaranteed minimum
for simultaneous render targets in wgpu / WebGPU). Since each texture
holds 4 floats, this gives a maximum of 32 floats of state per cell.
Exceeding this limit is a compile error.

### 2.5 Generated Code

The macro generates:

```rust
impl CellState for GrayScott {
    const TEXTURE_COUNT: u32 = 1;
    const FIELD_LAYOUT: &'static [FieldMapping] = &[
        FieldMapping { name: "a", texture: 0, offset: 0, size: 1 },
        FieldMapping { name: "b", texture: 0, offset: 1, size: 1 },
    ];

    fn defaults() -> Vec<[f32; 4]> {
        // One vec4 per texture, with default values in correct channels
        vec![[0.0, 0.0, 0.0, 0.0]]
    }
}
```

Default values are taken from a `Default` implementation if present, or
zero-initialized otherwise. Users can implement `Default` manually or
derive it:

```rust
#[derive(CellState)]
struct GrayScott {
    a: f32,  // default: 0.0
    b: f32,  // default: 0.0
}

impl Default for GrayScott {
    fn default() -> Self {
        Self { a: 1.0, b: 0.0 }
    }
}
```

---

## 3. The `#[cell]` Proc Macro

### 3.1 Purpose

Cross-compiles Rust method bodies to WGSL shader code. Validates that
the code uses only GPU-compatible constructs. Generates the `Cell` trait
implementation containing embedded WGSL source and pipeline metadata.

### 3.2 Input

```rust
#[cell(neighborhood = moore)]
impl Cell for GrayScott {
    fn update(self, nb: Neighbors) -> Self { ... }
    fn view(self) -> Color { ... }
}
```

The `#[cell]` attribute accepts the following parameters:

| Parameter      | Values                                | Default |
|----------------|---------------------------------------|---------|
| `neighborhood` | `moore`, `von_neumann`, `radius(N)`   | `moore` |

### 3.3 Required Methods

| Method   | Signature                         | Purpose                       |
|----------|-----------------------------------|-------------------------------|
| `update` | `fn update(self, nb: Neighbors) -> Self` | Next-tick state computation |
| `view`   | `fn view(self) -> Color`          | State-to-pixel color mapping  |

Both methods receive `self` by value (conceptually copying the current
cell's state). `update` additionally receives a `Neighbors` handle for
accessing neighbor state.

### 3.4 Optional Methods

| Method | Signature                        | Purpose                        |
|--------|----------------------------------|--------------------------------|
| `init` | `fn init(x: f32, y: f32, w: f32, h: f32) -> Self` | Programmatic initial state |

If `init` is provided, it compiles to a third shader run once at startup.
`x`, `y` are the cell's grid coordinates; `w`, `h` are the grid
dimensions. If absent, cells are initialized from `Default`.

### 3.5 Optional Associated Constants

```rust
#[cell(neighborhood = moore)]
impl Cell for GrayScott {
    const FEED: f32 = 0.055;
    const KILL: f32 = 0.062;

    fn update(self, nb: Neighbors) -> Self { ... }
    fn view(self) -> Color { ... }
}
```

Associated `f32` constants are emitted as WGSL `const` declarations.
They are available in all method bodies.

---

## 4. Rust-to-WGSL Cross-Compilation

### 4.1 Overview

The proc macro operates on the `syn` AST of each method body. It walks
the expression tree and emits corresponding WGSL code. This is a
syntax-directed translation, not an evaluation — no Rust code in the
method bodies is ever executed on the host.

### 4.2 Supported Rust Constructs

The following Rust constructs are accepted and translated:

| Rust Construct              | WGSL Output                        |
|-----------------------------|------------------------------------|
| `let x = expr;`            | `let x = expr;`                    |
| `let x: f32 = expr;`       | `let x: f32 = expr;`              |
| `if cond { a } else { b }` | `if (cond) { a } else { b }`      |
| `if cond { a } else if ..` | Chained `if/else if/else`          |
| Arithmetic: `+ - * /`      | Same operators                     |
| Comparison: `== != < > <= >=` | Same operators                  |
| Logical: `&& \|\| !`        | `&&  \|\|  !`                      |
| Field access: `self.a`     | Texel fetch from state texture     |
| Struct literal: `Self { a: x, b: y }` | Fragment output writes  |
| Method calls: `x.clamp(0.0, 1.0)` | `clamp(x, 0.0, 1.0)`       |
| Method calls: `x.sin()`    | `sin(x)`                           |
| Function calls: `f(x, y)`  | `f(x, y)`                          |
| Tuple construction: not used — vectors use constructors             |
| Closures: `\|c\| c.field`  | Inline expansion (see §4.5)       |
| Unary minus: `-x`          | `-x`                               |
| Grouping: `(expr)`         | `(expr)`                           |
| Constants: `FEED`          | Reference to emitted `const`       |
| Literal floats: `0.5`      | `0.5`                              |
| Literal integers: `3`      | `3.0` (auto-converted to f32)      |

### 4.3 Rejected Constructs

The following produce compile errors with clear messages:

| Construct              | Error Message                                     |
|------------------------|---------------------------------------------------|
| `loop`, `while`        | "cellarium: unbounded loops are not GPU-compatible" |
| `for` (general)        | "cellarium: general `for` loops are not supported" |
| `match`                | "cellarium: use `if/else` chains instead of `match`" |
| `return expr` (early)  | Rewritten to wrapping `if/else` by the macro. Accepted but discouraged. |
| Mutable variables      | "cellarium: `let mut` is not supported; all bindings are immutable" |
| References/borrowing   | "cellarium: references are not used in cell code"  |
| Heap types             | "cellarium: `{type}` is not a GPU-compatible type" |
| Closures (non-spatial) | "cellarium: closures are only valid in spatial operators" |
| Function definitions   | "cellarium: define helpers as associated constants or inline" |
| Recursion              | Not possible (no function definitions)             |
| Trait methods          | "cellarium: only `Cell` trait methods are supported" |
| `unsafe`               | "cellarium: `unsafe` is not supported"             |

### 4.4 `self.field` — State Field Access

When the macro encounters `self.a` where `a` is a field of the
`CellState` struct, it emits a WGSL texel fetch:

```wgsl
let _self_a: f32 = textureLoad(state_tex0, cell_coord, 0).r;
```

The texture index and swizzle are determined by the field layout from
`#[derive(CellState)]`. All self-field reads are hoisted to the top of
the generated `main()` function, before any user code, to ensure each
field is fetched exactly once.

### 4.5 `Self { ... }` — State Output

The return value of `update` must be a `Self` struct literal. Every field
must be assigned. The macro collects the field expressions and packs them
into texture output writes:

```wgsl
// Self { a: new_a, b: new_b }
output_tex0 = vec4f(new_a, new_b, 0.0, 0.0);
```

Unused channels are written as `0.0`.

### 4.6 Closures in Spatial Operators

Closures appear only as arguments to `Neighbors` methods. The macro
recognizes this pattern structurally:

```rust
nb.laplacian(|c| c.a)
nb.laplacian(|c| c.density * c.temperature)
nb.mean(|c| c.velocity)
nb.sum(|c| -c.heading.direction_to(self.pos))
nb.count(|c| c.alive > 0.5)
```

The closure parameter `c` represents a neighbor. Within the closure body:

- `c.field` emits a texel fetch at the neighbor's coordinates.
- Arithmetic on fetched values is inlined into the loop body.
- `self.field` remains a reference to this cell's own state (already
  fetched and available as a local variable).

The macro extracts the closure body, replaces `c.field` references with
neighbor texel fetches, and wraps the result in the appropriate
accumulation loop.

**Example:** `nb.laplacian(|c| c.density * c.temperature)` emits:

```wgsl
var _lap: f32 = 0.0;
for (var dy: i32 = -1; dy <= 1; dy++) {
    for (var dx: i32 = -1; dx <= 1; dx++) {
        let nc = cell_coord + vec2i(dx, dy);
        let _n_density = textureLoad(state_tex0, nc, 0).r;
        let _n_temperature = textureLoad(state_tex0, nc, 0).a;
        let _n_val = _n_density * _n_temperature;
        _lap += _n_val * LAPLACIAN_KERNEL[dy + 1][dx + 1];
    }
}
```

This naturally solves the "derived field" problem — the closure body is
the derived field expression, evaluated at each neighbor position.

### 4.7 Method-Call Translation

Certain method calls on `f32` and vector types are recognized and
translated to WGSL built-in functions:

| Rust Method Call         | WGSL Output                |
|--------------------------|----------------------------|
| `x.sin()`               | `sin(x)`                   |
| `x.cos()`               | `cos(x)`                   |
| `x.tan()`               | `tan(x)`                   |
| `x.sqrt()`              | `sqrt(x)`                  |
| `x.abs()`               | `abs(x)`                   |
| `x.floor()`             | `floor(x)`                 |
| `x.ceil()`              | `ceil(x)`                  |
| `x.round()`             | `round(x)`                 |
| `x.signum()`            | `sign(x)`                  |
| `x.exp()`               | `exp(x)`                   |
| `x.ln()`                | `log(x)`                   |
| `x.log2()`              | `log2(x)`                  |
| `x.powf(y)`             | `pow(x, y)`                |
| `x.clamp(lo, hi)`       | `clamp(x, lo, hi)`         |
| `x.min(y)`              | `min(x, y)`                |
| `x.max(y)`              | `max(x, y)`                |
| `x.fract()`             | `fract(x)`                 |
| `v.length()`            | `length(v)`                |
| `v.normalize()`         | `normalize(v)`             |
| `v.dot(w)`              | `dot(v, w)`                |
| `v.distance(w)`         | `distance(v, w)`           |

These are the standard `f32` methods from Rust's standard library where
applicable, plus vector methods from cellarium's `Vec2`/`Vec3`/`Vec4`
types.

### 4.8 Free Function Translation

The following free functions from the cellarium prelude are recognized:

| Rust Function                 | WGSL Output                    |
|-------------------------------|--------------------------------|
| `mix(a, b, t)`               | `mix(a, b, t)`                 |
| `step(edge, x)`              | `step(edge, x)`                |
| `smoothstep(lo, hi, x)`      | `smoothstep(lo, hi, x)`       |
| `atan2(y, x)`                | `atan2(y, x)`                  |
| `vec2(x, y)`                 | `vec2f(x, y)`                  |
| `vec3(x, y, z)`              | `vec3f(x, y, z)`               |
| `vec4(x, y, z, w)`           | `vec4f(x, y, z, w)`            |
| `Color::rgb(r, g, b)`        | `vec4f(r, g, b, 1.0)`          |
| `Color::hsv(h, s, v)`        | HSV-to-RGB conversion inlined  |
| `Color::rgba(r, g, b, a)`    | `vec4f(r, g, b, a)`            |

---

## 5. Neighbors API and Spatial Operators

### 5.1 The `Neighbors` Type

`Neighbors` is an opaque handle passed to `update`. It cannot be stored,
returned, or passed to other functions. It exists only as a target for
method calls that the proc macro translates to spatial operations.

### 5.2 Aggregation Methods

All methods take a closure `|c| -> T` where `c` represents a neighbor
cell. The closure body may access `c.field` for any state field.

| Method                          | Return Type | Semantics                      |
|---------------------------------|-------------|--------------------------------|
| `nb.sum(\|c\| expr)`           | `T`         | Sum of expr over all neighbors |
| `nb.mean(\|c\| expr)`          | `T`         | Mean of expr over all neighbors |
| `nb.min(\|c\| expr)`           | `f32`       | Minimum (f32 only)             |
| `nb.max(\|c\| expr)`           | `f32`       | Maximum (f32 only)             |
| `nb.count(\|c\| bool_expr)`    | `f32`       | Count where condition holds    |

`T` is the return type of the closure body: `f32`, `Vec2`, `Vec3`, or
`Vec4`.

### 5.3 Differential Operators

| Method                          | Return Type | Semantics                   |
|---------------------------------|-------------|-----------------------------|
| `nb.laplacian(\|c\| expr)`     | `T`         | Discrete Laplacian          |
| `nb.gradient(\|c\| expr)`      | `Vec2`      | Central differences (f32 input only) |
| `nb.divergence(\|c\| expr)`    | `f32`       | Divergence (Vec2 input only) |

The Laplacian uses a weighted 3×3 isotropic kernel:

```
0.25  0.5   0.25
0.5   -3.0  0.5
0.25  0.5   0.25
```

`gradient` and `divergence` use central differences and require the
`moore` neighborhood (they need the cardinal neighbors at minimum). The
macro emits a compile error if used with `von_neumann`.

### 5.4 Filtered Aggregation

All aggregation and differential methods accept an optional second
closure for filtering:

```rust
nb.sum_where(
    |c| c.velocity,                    // what to aggregate
    |c| c.density > 0.1               // which neighbors to include
)

nb.mean_where(
    |c| c.temperature,
    |c| c.alive > 0.5
)

nb.count(|c| c.alive > 0.5)  // count is inherently filtered
```

| Method                                          | Semantics                  |
|-------------------------------------------------|----------------------------|
| `nb.sum_where(\|c\| val, \|c\| cond)`          | Sum where condition holds  |
| `nb.mean_where(\|c\| val, \|c\| cond)`         | Mean where condition holds |
| `nb.min_where(\|c\| val, \|c\| cond)`          | Min where condition holds  |
| `nb.max_where(\|c\| val, \|c\| cond)`          | Max where condition holds  |

`mean_where` divides by the dynamic count of neighbors passing the
filter, not by the total neighborhood size. If no neighbors pass the
filter, `mean_where` returns zero (scalar `0.0` or the zero vector).
The generated WGSL uses `select(sum / count, zero, count == 0.0)` to
avoid division by zero.

### 5.5 Spatial Accessors

Within any neighbor closure, the following methods are available on `c`:

| Accessor            | Type   | Meaning                              |
|---------------------|--------|--------------------------------------|
| `c.offset()`        | `Vec2` | Integer grid offset from self        |
| `c.direction()`     | `Vec2` | `normalize(offset)`                  |
| `c.distance()`      | `f32`  | `length(offset)` (Euclidean)         |
| `c.field_name`      | varies | Value of named field at this neighbor |

These are only valid inside closures passed to `Neighbors` methods.
The macro rejects their use elsewhere.

### 5.6 Neighborhoods

| Specifier          | Cells Visited    | Loop Bounds             |
|--------------------|------------------|-------------------------|
| `moore`            | 8 (Chebyshev 1)  | `dx,dy ∈ [-1,1]`, skip (0,0) |
| `von_neumann`      | 4 (Manhattan 1)  | (±1,0) and (0,±1)     |
| `radius(N)`        | (2N+1)²−1        | `dx,dy ∈ [-N,N]`, skip (0,0) |

The neighborhood does **not** include the cell itself.

### 5.7 WGSL Emission for Spatial Operators

All spatial operators emit inline loops. No WGSL functions are generated
for them — the loop body contains the translated closure expression.

For `moore` neighborhood, `nb.sum(|c| c.a)`:

```wgsl
var _sum: f32 = 0.0;
for (var dy: i32 = -1; dy <= 1; dy++) {
    for (var dx: i32 = -1; dx <= 1; dx++) {
        if (dx == 0 && dy == 0) { continue; }
        let nc = vec2i(cell_coord) + vec2i(dx, dy);
        let _c_a = textureLoad(state_tex0, nc, 0).r;
        _sum += _c_a;
    }
}
```

For `radius(N)`, the loop bounds expand to `[-N, N]`.

For filtered methods, an `if` wraps the accumulation:

```wgsl
if (_c_density > 0.1) {
    _sum += _c_velocity;
    _filter_count += 1.0;
}
```

---

## 6. State Texture Management

### 6.1 Texture Format

All state textures use format `rgba32float` — four 32-bit floats per
texel. This provides full precision for all state fields.

Filtering mode: `nearest` (no interpolation). Address mode: `repeat`
(toroidal wrapping at grid edges).

### 6.2 Ping-Pong Double Buffering

For each logical state texture, two physical textures exist: `A` and `B`.
On even ticks, the simulation reads from `A` and writes to `B`. On odd
ticks, it reads from `B` and writes to `A`. The swap is a pointer swap
in the bind group — no data is copied.

For a cell type with N state textures, the runtime allocates 2N physical
texture objects.

### 6.3 Bind Group Layout

The simulation shader's bind group contains:

| Binding | Resource                      | Access     |
|---------|-------------------------------|------------|
| 0       | State texture 0 (read side)   | texture    |
| 1       | State texture 1 (read side)   | texture    |
| ...     | ...                           | texture    |
| N       | Uniform buffer (tick, resolution) | uniform |

The output textures are bound as render targets (color attachments on
the render pass), not as bind group entries.

The view shader's bind group is identical in structure, but reads from
the just-written textures.

---

## 7. Runtime Execution

### 7.1 Initialization

```rust
let sim = Simulation::<GrayScott>::new(1024, 1024);
sim.run();  // Opens window and enters main loop
```

`Simulation::new` performs:

1. Initialize wgpu: adapter, device, queue, surface.
2. Create a window via `winit` at the specified grid resolution.
3. Compile shader modules from the embedded WGSL strings generated by
   the proc macro.
4. Create render pipelines: one for simulation, one for view.
5. Allocate state textures (2N textures of size W×H, format `rgba32float`).
6. Create bind groups for both ping-pong phases.
7. Write initial state:
   - If the `Cell` impl includes `fn init`, run the init shader once.
   - Otherwise, fill textures with `CellState::defaults()`.
8. Create the fullscreen triangle vertex state (no vertex buffer; vertex
   positions are computed from `vertex_index` in the vertex shader).

### 7.2 Main Loop

Each frame:

```
1.  Begin command encoder
2.  --- Simulation Pass (render pass with MRT) ---
    a. Set pipeline: simulation
    b. Set bind group: read textures for current phase
    c. Set render targets: write textures for current phase
    d. Draw 3 vertices (fullscreen triangle)
3.  --- View Pass (render pass to surface) ---
    a. Set pipeline: view
    b. Set bind group: the textures just written
    c. Set render target: surface texture
    d. Draw 3 vertices (fullscreen triangle)
4.  Submit command buffer
5.  Present surface
6.  Swap ping-pong phase (toggle a boolean)
```

Steps 2 and 3 are the only GPU work. The CPU submits commands and
toggles the phase flag. State never touches system memory.

### 7.3 Uniforms

The uniform buffer contains:

| Field         | Type     | Value                           |
|---------------|----------|---------------------------------|
| `tick`        | `u32`    | Current tick number             |
| `resolution`  | `vec2f`  | Grid (width, height)            |

These are available in WGSL as:

```wgsl
struct Uniforms {
    tick: u32,
    resolution: vec2f,
}
@group(0) @binding(N) var<uniform> uniforms: Uniforms;
```

The proc macro translates references to `tick`, `cell_x`, `cell_y`,
`grid_width`, `grid_height` to the appropriate uniform reads and
fragment coordinate accesses.

### 7.4 Fullscreen Vertex Shader

Constant across all Cellarium programs:

```wgsl
@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4f {
    let x = f32(vid & 1u) * 4.0 - 1.0;
    let y = f32((vid >> 1u) & 1u) * 4.0 - 1.0;
    return vec4f(x, y, 0.0, 1.0);
}
```

Three vertices cover the entire viewport. No vertex buffer is needed.

### 7.5 Boundary Conditions

Textures use `address_mode_u: repeat, address_mode_v: repeat`. Neighbor
fetches at grid edges automatically wrap to the opposite side (toroidal
boundary). No bounds checking is emitted in shader code.

### 7.6 Performance Characteristics

For a W×H grid with N state textures and a neighborhood of size K:

- **Simulation pass:** W×H fragment invocations. Each invocation performs
  up to K×N texel loads (one per neighbor per texture accessed). Writes
  N texture outputs via MRT.
- **View pass:** W×H fragment invocations. Each performs N texel loads.
  Writes 1 color output.
- **CPU work per frame:** ~2 draw calls, 1 bind group swap, 1 uniform
  buffer write. Microseconds.

A 1024×1024 grid with `moore` neighborhood (K=8) and 2 state textures
typically completes both passes in under 0.5ms on a modern GPU.

---

## 8. Built-in Values

The following identifiers are available in `update`, `view`, and `init`
method bodies. The proc macro recognizes them and emits the appropriate
WGSL:

| Name           | Type  | WGSL Source                              |
|----------------|-------|------------------------------------------|
| `tick`         | `f32` | `f32(uniforms.tick)`                     |
| `cell_x`       | `f32` | `f32(cell_coord.x)`                     |
| `cell_y`       | `f32` | `f32(cell_coord.y)`                     |
| `grid_width`   | `f32` | `uniforms.resolution.x`                 |
| `grid_height`  | `f32` | `uniforms.resolution.y`                 |
| `PI`           | `f32` | `3.14159265`                             |
| `TAU`          | `f32` | `6.28318530`                             |
| `Color::WHITE` | `Color` | `vec4f(1.0, 1.0, 1.0, 1.0)`           |
| `Color::BLACK` | `Color` | `vec4f(0.0, 0.0, 0.0, 1.0)`           |

`cell_coord` is derived from `@builtin(position)` in the fragment
shader, truncated to integer coordinates.

---

## 9. Generated WGSL Structure

For a `GrayScott` cell with 1 state texture, the generated simulation
shader has the following structure:

```wgsl
// --- Auto-generated by cellarium ---

struct Uniforms {
    tick: u32,
    resolution: vec2f,
}

@group(0) @binding(0) var state_tex0: texture_2d<f32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;

const FEED: f32 = 0.055;
const KILL: f32 = 0.062;
const DA: f32 = 1.0;
const DB: f32 = 0.5;

const LAPLACIAN_KERNEL = array<array<f32, 3>, 3>(
    array<f32, 3>(0.25, 0.5, 0.25),
    array<f32, 3>(0.5, -3.0, 0.5),
    array<f32, 3>(0.25, 0.5, 0.25),
);

struct Output {
    @location(0) tex0: vec4f,
}

@fragment
fn fs_main(@builtin(position) frag_pos: vec4f) -> Output {
    let cell_coord = vec2i(frag_pos.xy);

    // Self-state reads (hoisted)
    let _self_a = textureLoad(state_tex0, cell_coord, 0).r;
    let _self_b = textureLoad(state_tex0, cell_coord, 0).g;

    // User code: let reaction = self.a * self.b * self.b;
    let reaction = _self_a * _self_b * _self_b;

    // User code: nb.laplacian(|c| c.a)
    var _lap_0: f32 = 0.0;
    for (var dy: i32 = -1; dy <= 1; dy++) {
        for (var dx: i32 = -1; dx <= 1; dx++) {
            let nc = cell_coord + vec2i(dx, dy);
            let _c_a = textureLoad(state_tex0, nc, 0).r;
            _lap_0 += _c_a * LAPLACIAN_KERNEL[dy + 1][dx + 1];
        }
    }

    // User code: nb.laplacian(|c| c.b)
    var _lap_1: f32 = 0.0;
    for (var dy: i32 = -1; dy <= 1; dy++) {
        for (var dx: i32 = -1; dx <= 1; dx++) {
            let nc = cell_coord + vec2i(dx, dy);
            let _c_b = textureLoad(state_tex0, nc, 0).g;
            _lap_1 += _c_b * LAPLACIAN_KERNEL[dy + 1][dx + 1];
        }
    }

    // User code: Self { a: ..., b: ... }
    let _out_a = clamp(_self_a + DA * _lap_0 - reaction
                       + FEED * (1.0 - _self_a), 0.0, 1.0);
    let _out_b = clamp(_self_b + DB * _lap_1 + reaction
                       - (KILL + FEED) * _self_b, 0.0, 1.0);

    return Output(vec4f(_out_a, _out_b, 0.0, 0.0));
}
```

---

## 10. Future Extensions

### 10.1 Interactive Parameters

```rust
#[cell(neighborhood = moore)]
impl Cell for GrayScott {
    #[param(range = 0.0..0.1)]
    const FEED: f32 = 0.055;
    ...
}
```

Parameters marked with `#[param]` become runtime-adjustable uniforms.
The runtime generates a UI overlay with sliders. Constant propagation
is replaced with uniform reads.

### 10.2 Hot Reload

Using `notify` (file watcher) and runtime shader recompilation. The proc
macro outputs WGSL to a file alongside the binary. The runtime watches
this file and recompiles the pipeline when it changes, without resetting
state textures.

### 10.3 Multiple Cell Types

Multiple structs implementing `Cell`, each occupying its own set of
textures. Cross-type neighbor access via typed handles.

### 10.4 3D Grids

Replace 2D textures with 3D textures. Neighborhoods generalize to 3D
(e.g., `moore` becomes 26 neighbors). The `Neighbors` API is unchanged;
the macro emits 3-deep loops instead of 2-deep.

### 10.5 Snapshot and Readback

```rust
sim.on_tick(100, |state: &[GrayScott]| {
    // state has been read back from GPU — expensive, use sparingly
});
```

An explicit opt-in for reading state back to the CPU for analysis or
serialization.
