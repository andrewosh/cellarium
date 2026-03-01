use std::collections::HashSet;
use syn::{Block, BinOp, Expr, ExprBinary, ExprCall, ExprField, ExprIf, ExprLit,
          ExprMethodCall, ExprPath, ExprStruct, ExprUnary, Lit, Member, Result, Stmt,
          UnOp, spanned::Spanned};

// Prefix all user-defined variable/constant names to avoid WGSL keyword collisions.
fn user_var(name: &str) -> String {
    format!("v_{}", name)
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Neighborhood {
    Moore,
    VonNeumann,
    Radius(u32),
}

impl Neighborhood {
    pub fn radius(&self) -> u32 {
        match self {
            Neighborhood::Moore => 1,
            Neighborhood::VonNeumann => 1,
            Neighborhood::Radius(n) => *n,
        }
    }

    fn skip_condition(&self) -> &str {
        match self {
            Neighborhood::VonNeumann => "if (abs(_dx) + abs(_dy) > 1 || (_dx == 0 && _dy == 0)) { continue; }",
            _ => "if (_dx == 0 && _dy == 0) { continue; }",
        }
    }

    fn neighbor_count(&self) -> u32 {
        match self {
            Neighborhood::Moore => 8,
            Neighborhood::VonNeumann => 4,
            Neighborhood::Radius(n) => (2 * n + 1) * (2 * n + 1) - 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FieldDef {
    pub name: String,
}

#[derive(Clone)]
pub struct ConstInfo {
    pub name: String,
    pub expr: Expr,
}

impl std::fmt::Debug for ConstInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConstInfo").field("name", &self.name).finish()
    }
}

#[derive(Clone, Debug)]
pub struct CellImplInfo {
    pub neighborhood: Neighborhood,
    pub constants: Vec<ConstInfo>,
    pub fields: Vec<FieldDef>,
}

// ---------------------------------------------------------------------------
// Field discovery — find all self.X references in method bodies
// ---------------------------------------------------------------------------

pub fn discover_fields(block: &Block) -> Vec<FieldDef> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    discover_fields_in_block(block, &mut names, &mut seen);
    names
}

fn discover_fields_in_block(block: &Block, names: &mut Vec<FieldDef>, seen: &mut HashSet<String>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    discover_fields_in_expr(&init.expr, names, seen);
                }
            }
            Stmt::Expr(expr, _) => discover_fields_in_expr(expr, names, seen),
            _ => {}
        }
    }
}

fn discover_fields_in_expr(expr: &Expr, names: &mut Vec<FieldDef>, seen: &mut HashSet<String>) {
    match expr {
        Expr::Field(f) => {
            if let Expr::Path(p) = &*f.base {
                if let Some(ident) = p.path.get_ident() {
                    if ident == "self" {
                        if let Member::Named(field_name) = &f.member {
                            let name = field_name.to_string();
                            if !seen.contains(&name) {
                                seen.insert(name.clone());
                                names.push(FieldDef { name });
                            }
                        }
                    }
                }
            }
            discover_fields_in_expr(&f.base, names, seen);
        }
        Expr::Struct(s) => {
            for field in &s.fields {
                if let Member::Named(field_name) = &field.member {
                    let name = field_name.to_string();
                    if !seen.contains(&name) {
                        seen.insert(name.clone());
                        names.push(FieldDef { name });
                    }
                }
                discover_fields_in_expr(&field.expr, names, seen);
            }
        }
        Expr::Binary(b) => {
            discover_fields_in_expr(&b.left, names, seen);
            discover_fields_in_expr(&b.right, names, seen);
        }
        Expr::Unary(u) => discover_fields_in_expr(&u.expr, names, seen),
        Expr::Paren(p) => discover_fields_in_expr(&p.expr, names, seen),
        Expr::Group(g) => discover_fields_in_expr(&g.expr, names, seen),
        Expr::MethodCall(mc) => {
            discover_fields_in_expr(&mc.receiver, names, seen);
            for arg in &mc.args {
                discover_fields_in_expr(arg, names, seen);
            }
        }
        Expr::Call(c) => {
            discover_fields_in_expr(&c.func, names, seen);
            for arg in &c.args {
                discover_fields_in_expr(arg, names, seen);
            }
        }
        Expr::If(i) => {
            discover_fields_in_expr(&i.cond, names, seen);
            discover_fields_in_block(&i.then_branch, names, seen);
            if let Some((_, else_branch)) = &i.else_branch {
                discover_fields_in_expr(else_branch, names, seen);
            }
        }
        Expr::Block(b) => discover_fields_in_block(&b.block, names, seen),
        Expr::Closure(c) => discover_fields_in_expr(&c.body, names, seen),
        Expr::Return(r) => {
            if let Some(expr) = &r.expr {
                discover_fields_in_expr(expr, names, seen);
            }
        }
        _ => {}
    }
}

// Discover fields accessed via c.X in closure bodies (neighbor fields)
fn discover_neighbor_fields_in_expr(expr: &Expr, names: &mut Vec<String>, seen: &mut HashSet<String>, closure_param: &str) {
    match expr {
        Expr::Field(f) => {
            if let Expr::Path(p) = &*f.base {
                if let Some(ident) = p.path.get_ident() {
                    if ident.to_string() == closure_param {
                        if let Member::Named(field_name) = &f.member {
                            let name = field_name.to_string();
                            // Skip spatial accessors
                            if name != "offset" && name != "direction" && name != "distance" {
                                if !seen.contains(&name) {
                                    seen.insert(name.clone());
                                    names.push(name);
                                }
                            }
                        }
                    }
                }
            }
            discover_neighbor_fields_in_expr(&f.base, names, seen, closure_param);
        }
        Expr::Binary(b) => {
            discover_neighbor_fields_in_expr(&b.left, names, seen, closure_param);
            discover_neighbor_fields_in_expr(&b.right, names, seen, closure_param);
        }
        Expr::Unary(u) => discover_neighbor_fields_in_expr(&u.expr, names, seen, closure_param),
        Expr::Paren(p) => discover_neighbor_fields_in_expr(&p.expr, names, seen, closure_param),
        Expr::Group(g) => discover_neighbor_fields_in_expr(&g.expr, names, seen, closure_param),
        Expr::MethodCall(mc) => {
            discover_neighbor_fields_in_expr(&mc.receiver, names, seen, closure_param);
            for arg in &mc.args {
                discover_neighbor_fields_in_expr(arg, names, seen, closure_param);
            }
        }
        Expr::Call(c) => {
            discover_neighbor_fields_in_expr(&c.func, names, seen, closure_param);
            for arg in &c.args {
                discover_neighbor_fields_in_expr(arg, names, seen, closure_param);
            }
        }
        Expr::If(i) => {
            discover_neighbor_fields_in_expr(&i.cond, names, seen, closure_param);
            for stmt in &i.then_branch.stmts {
                if let Stmt::Expr(e, _) = stmt {
                    discover_neighbor_fields_in_expr(e, names, seen, closure_param);
                }
            }
            if let Some((_, else_branch)) = &i.else_branch {
                discover_neighbor_fields_in_expr(else_branch, names, seen, closure_param);
            }
        }
        Expr::Lit(_) | Expr::Path(_) => {}
        _ => {}
    }
}

// Check if an expression tree contains spatial accessor calls (offset/direction/distance)
fn expr_needs_spatial_accessors(expr: &Expr, closure_param: &str) -> bool {
    match expr {
        Expr::MethodCall(mc) => {
            let method = mc.method.to_string();
            if let Expr::Path(p) = &*mc.receiver {
                if let Some(ident) = p.path.get_ident() {
                    if ident.to_string() == closure_param {
                        if method == "offset" || method == "direction" || method == "distance" {
                            return true;
                        }
                    }
                }
            }
            if expr_needs_spatial_accessors(&mc.receiver, closure_param) {
                return true;
            }
            mc.args.iter().any(|a| expr_needs_spatial_accessors(a, closure_param))
        }
        Expr::Field(f) => {
            if let Expr::Path(p) = &*f.base {
                if let Some(ident) = p.path.get_ident() {
                    if ident.to_string() == closure_param {
                        let name = match &f.member {
                            Member::Named(n) => n.to_string(),
                            _ => String::new(),
                        };
                        if name == "offset" || name == "direction" || name == "distance" {
                            return true;
                        }
                    }
                }
            }
            expr_needs_spatial_accessors(&f.base, closure_param)
        }
        Expr::Binary(b) => {
            expr_needs_spatial_accessors(&b.left, closure_param)
                || expr_needs_spatial_accessors(&b.right, closure_param)
        }
        Expr::Unary(u) => expr_needs_spatial_accessors(&u.expr, closure_param),
        Expr::Paren(p) => expr_needs_spatial_accessors(&p.expr, closure_param),
        Expr::Group(g) => expr_needs_spatial_accessors(&g.expr, closure_param),
        Expr::Call(c) => {
            expr_needs_spatial_accessors(&c.func, closure_param)
                || c.args.iter().any(|a| expr_needs_spatial_accessors(a, closure_param))
        }
        Expr::If(i) => {
            expr_needs_spatial_accessors(&i.cond, closure_param)
                || i.then_branch.stmts.iter().any(|s| {
                    if let Stmt::Expr(e, _) = s { expr_needs_spatial_accessors(e, closure_param) } else { false }
                })
                || i.else_branch.as_ref().map_or(false, |(_, e)| expr_needs_spatial_accessors(e, closure_param))
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// WGSL emission context
// ---------------------------------------------------------------------------

struct EmitCtx<'a> {
    info: &'a CellImplInfo,
    counter: u32,
    preamble: Vec<String>,
    body: Vec<String>,
    output_lines: Vec<String>,
    self_fields_fetched: HashSet<String>,
    compute_mode: bool,
    use_shared: bool,
}

impl<'a> EmitCtx<'a> {
    fn new(info: &'a CellImplInfo) -> Self {
        Self {
            info,
            counter: 0,
            preamble: Vec::new(),
            body: Vec::new(),
            output_lines: Vec::new(),
            self_fields_fetched: HashSet::new(),
            compute_mode: false,
            use_shared: false,
        }
    }

    fn next_id(&mut self) -> u32 {
        let id = self.counter;
        self.counter += 1;
        id
    }

    fn ensure_self_field(&mut self, field_name: &str) -> String {
        let var_name = format!("_self_{}", field_name);
        if !self.self_fields_fetched.contains(field_name) {
            self.self_fields_fetched.insert(field_name.to_string());
            let (tex, swizzle) = self.field_texture_swizzle(field_name);
            if self.compute_mode && self.use_shared {
                self.preamble.push(format!(
                    "    let {} = shared_tex{}[ly * PADDED + lx].{};",
                    var_name, tex, swizzle
                ));
            } else {
                self.preamble.push(format!(
                    "    let {} = textureLoad(state_tex{}, cell_coord, 0).{};",
                    var_name, tex, swizzle
                ));
            }
        }
        var_name
    }

    fn field_texture_swizzle(&self, field_name: &str) -> (u32, String) {
        let swizzle_chars = ['r', 'g', 'b', 'a'];
        let mut current_tex: u32 = 0;
        let mut current_offset: u32 = 0;

        for field in &self.info.fields {
            let size = 1u32;
            if current_offset + size > 4 {
                current_tex += 1;
                current_offset = 0;
            }
            if field.name == field_name {
                let swizzle: String = (current_offset..current_offset + size)
                    .map(|i| swizzle_chars[i as usize])
                    .collect();
                return (current_tex, swizzle);
            }
            current_offset += size;
        }
        (0, "r".to_string())
    }

    fn neighbor_field_fetch(&self, field_name: &str) -> (u32, String) {
        self.field_texture_swizzle(field_name)
    }
}

// ---------------------------------------------------------------------------
// Expression emission — Rust Expr → WGSL string
// ---------------------------------------------------------------------------

fn emit_expr(expr: &Expr, ctx: &mut EmitCtx, closure_param: Option<&str>) -> Result<String> {
    match expr {
        Expr::Lit(lit) => emit_lit(lit),
        Expr::Path(path) => emit_path(path, ctx),
        Expr::Field(field) => emit_field(field, ctx, closure_param),
        Expr::Binary(bin) => emit_binary(bin, ctx, closure_param),
        Expr::Unary(un) => emit_unary(un, ctx, closure_param),
        Expr::Paren(p) => {
            let inner = emit_expr(&p.expr, ctx, closure_param)?;
            Ok(format!("({})", inner))
        }
        Expr::Group(g) => emit_expr(&g.expr, ctx, closure_param),
        Expr::MethodCall(mc) => emit_method_call(mc, ctx, closure_param),
        Expr::Call(call) => emit_call(call, ctx, closure_param),
        Expr::If(if_expr) => emit_if(if_expr, ctx, closure_param),
        Expr::Block(b) => {
            if let Some(Stmt::Expr(e, None)) = b.block.stmts.last() {
                emit_expr(e, ctx, closure_param)
            } else {
                Ok("/* empty block */".to_string())
            }
        }
        Expr::Struct(s) => {
            Err(syn::Error::new(s.span(), "cellarium: struct literals should only appear as return values"))
        }
        _ => Err(syn::Error::new(expr.span(), format!(
            "cellarium C019: '{}' is not supported in cell code.",
            quote::quote!(#expr),
        ))),
    }
}

fn emit_lit(lit: &ExprLit) -> Result<String> {
    match &lit.lit {
        Lit::Float(f) => Ok(f.to_string()),
        Lit::Int(i) => Ok(format!("{}.0", i.base10_digits())),
        Lit::Bool(b) => Ok(if b.value { "true".to_string() } else { "false".to_string() }),
        _ => Err(syn::Error::new(lit.span(), "cellarium: unsupported literal type")),
    }
}

fn emit_path(path: &ExprPath, ctx: &mut EmitCtx) -> Result<String> {
    let s = path.path.segments.iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");

    if s == "Color::WHITE" { return Ok("vec4f(1.0, 1.0, 1.0, 1.0)".to_string()); }
    if s == "Color::BLACK" { return Ok("vec4f(0.0, 0.0, 0.0, 1.0)".to_string()); }
    if s == "PI" { return Ok("3.14159265".to_string()); }
    if s == "TAU" { return Ok("6.28318530".to_string()); }

    if ctx.info.constants.iter().any(|c| c.name == s) {
        return Ok(user_var(&s));
    }

    match s.as_str() {
        "tick" => Ok("f32(uniforms.tick)".to_string()),
        "cell_x" => Ok("f32(cell_coord.x)".to_string()),
        "cell_y" => Ok("f32(cell_coord.y)".to_string()),
        "grid_width" => Ok("uniforms.resolution.x".to_string()),
        "grid_height" => Ok("uniforms.resolution.y".to_string()),
        "true" => Ok("true".to_string()),
        "false" => Ok("false".to_string()),
        _ => Ok(user_var(&s)),
    }
}

fn emit_field(field: &ExprField, ctx: &mut EmitCtx, closure_param: Option<&str>) -> Result<String> {
    let member_name = match &field.member {
        Member::Named(ident) => ident.to_string(),
        Member::Unnamed(idx) => return Err(syn::Error::new(idx.span(), "cellarium: tuple indexing not supported")),
    };

    if let Expr::Path(p) = &*field.base {
        if let Some(ident) = p.path.get_ident() {
            if ident == "self" {
                return Ok(format!("_self_{}", member_name));
            }
            if let Some(cp) = closure_param {
                if ident.to_string() == cp {
                    return Ok(format!("_n_{}", member_name));
                }
            }
        }
    }

    let base = emit_expr(&field.base, ctx, closure_param)?;
    Ok(format!("{}.{}", base, member_name))
}

fn emit_binary(bin: &ExprBinary, ctx: &mut EmitCtx, closure_param: Option<&str>) -> Result<String> {
    let left = emit_expr(&bin.left, ctx, closure_param)?;
    let right = emit_expr(&bin.right, ctx, closure_param)?;
    let op = match &bin.op {
        BinOp::Add(_) => "+",
        BinOp::Sub(_) => "-",
        BinOp::Mul(_) => "*",
        BinOp::Div(_) => "/",
        BinOp::Eq(_) => "==",
        BinOp::Ne(_) => "!=",
        BinOp::Lt(_) => "<",
        BinOp::Gt(_) => ">",
        BinOp::Le(_) => "<=",
        BinOp::Ge(_) => ">=",
        BinOp::And(_) => "&&",
        BinOp::Or(_) => "||",
        _ => return Err(syn::Error::new(bin.span(), "cellarium: unsupported operator")),
    };
    Ok(format!("({} {} {})", left, op, right))
}

fn emit_unary(un: &ExprUnary, ctx: &mut EmitCtx, closure_param: Option<&str>) -> Result<String> {
    let operand = emit_expr(&un.expr, ctx, closure_param)?;
    match &un.op {
        UnOp::Neg(_) => Ok(format!("(-{})", operand)),
        UnOp::Not(_) => Ok(format!("(!{})", operand)),
        _ => Err(syn::Error::new(un.span(), "cellarium: unsupported unary operator")),
    }
}

fn emit_method_call(mc: &ExprMethodCall, ctx: &mut EmitCtx, closure_param: Option<&str>) -> Result<String> {
    let method_name = mc.method.to_string();

    // Check if receiver is `nb` (Neighbors) — hoist spatial op into a temp variable
    if let Expr::Path(p) = &*mc.receiver {
        if let Some(ident) = p.path.get_ident() {
            if ident == "nb" {
                if let Some(spatial_op) = parse_spatial_op(mc) {
                    let id = ctx.next_id();
                    let var_name = format!("_spatial_{}", id);
                    let op_code = emit_spatial_op(&spatial_op, ctx, &var_name)?;
                    ctx.body.push(op_code);
                    return Ok(var_name);
                }
                return Err(syn::Error::new(mc.span(), format!(
                    "cellarium: unrecognized spatial operator '{}'", method_name
                )));
            }
        }
    }

    // Check if this is a spatial accessor method on the closure parameter (c.distance(), c.offset(), c.direction())
    if let Some(cp) = closure_param {
        if let Expr::Path(p) = &*mc.receiver {
            if let Some(ident) = p.path.get_ident() {
                if ident.to_string() == cp {
                    match method_name.as_str() {
                        "distance" if mc.args.is_empty() => return Ok("_n_distance".to_string()),
                        "offset" if mc.args.is_empty() => return Ok("_n_offset".to_string()),
                        "direction" if mc.args.is_empty() => return Ok("_n_direction".to_string()),
                        _ => {}
                    }
                }
            }
        }
    }

    let receiver = emit_expr(&mc.receiver, ctx, closure_param)?;

    match method_name.as_str() {
        "sin" => Ok(format!("sin({})", receiver)),
        "cos" => Ok(format!("cos({})", receiver)),
        "tan" => Ok(format!("tan({})", receiver)),
        "sqrt" => Ok(format!("sqrt({})", receiver)),
        "abs" => Ok(format!("abs({})", receiver)),
        "floor" => Ok(format!("floor({})", receiver)),
        "ceil" => Ok(format!("ceil({})", receiver)),
        "round" => Ok(format!("round({})", receiver)),
        "signum" => Ok(format!("sign({})", receiver)),
        "exp" => Ok(format!("exp({})", receiver)),
        "ln" => Ok(format!("log({})", receiver)),
        "log2" => Ok(format!("log2({})", receiver)),
        "fract" => Ok(format!("fract({})", receiver)),
        "powf" => {
            let arg = emit_expr(&mc.args[0], ctx, closure_param)?;
            Ok(format!("pow({}, {})", receiver, arg))
        }
        "clamp" => {
            let lo = emit_expr(&mc.args[0], ctx, closure_param)?;
            let hi = emit_expr(&mc.args[1], ctx, closure_param)?;
            Ok(format!("clamp({}, {}, {})", receiver, lo, hi))
        }
        "min" => {
            let arg = emit_expr(&mc.args[0], ctx, closure_param)?;
            Ok(format!("min({}, {})", receiver, arg))
        }
        "max" => {
            let arg = emit_expr(&mc.args[0], ctx, closure_param)?;
            Ok(format!("max({}, {})", receiver, arg))
        }
        "length" => Ok(format!("length({})", receiver)),
        "normalize" => Ok(format!("normalize({})", receiver)),
        "dot" => {
            let arg = emit_expr(&mc.args[0], ctx, closure_param)?;
            Ok(format!("dot({}, {})", receiver, arg))
        }
        "distance" => {
            if mc.args.is_empty() {
                Ok(format!("distance({})", receiver))
            } else {
                let arg = emit_expr(&mc.args[0], ctx, closure_param)?;
                Ok(format!("distance({}, {})", receiver, arg))
            }
        }
        "cross" => {
            let arg = emit_expr(&mc.args[0], ctx, closure_param)?;
            Ok(format!("cross({}, {})", receiver, arg))
        }
        other => Err(syn::Error::new(mc.method.span(), format!(
            "cellarium C015: '{}' is not a recognized method. See cellarium docs for supported operations.", other
        ))),
    }
}

fn emit_call(call: &ExprCall, ctx: &mut EmitCtx, closure_param: Option<&str>) -> Result<String> {
    let func_name = match &*call.func {
        Expr::Path(p) => {
            p.path.segments.iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
        }
        _ => return Err(syn::Error::new(call.func.span(), "cellarium: unsupported function call syntax")),
    };

    let args: Vec<String> = call.args.iter()
        .map(|a| emit_expr(a, ctx, closure_param))
        .collect::<Result<_>>()?;

    match func_name.as_str() {
        "mix" => Ok(format!("mix({}, {}, {})", args[0], args[1], args[2])),
        "step" => Ok(format!("step({}, {})", args[0], args[1])),
        "smoothstep" => Ok(format!("smoothstep({}, {}, {})", args[0], args[1], args[2])),
        "atan2" => Ok(format!("atan2({}, {})", args[0], args[1])),
        "vec2" => Ok(format!("vec2f({}, {})", args[0], args[1])),
        "vec3" => Ok(format!("vec3f({}, {}, {})", args[0], args[1], args[2])),
        "vec4" => Ok(format!("vec4f({}, {}, {}, {})", args[0], args[1], args[2], args[3])),
        "Color::rgb" => Ok(format!("vec4f({}, {}, {}, 1.0)", args[0], args[1], args[2])),
        "Color::rgba" => Ok(format!("vec4f({}, {}, {}, {})", args[0], args[1], args[2], args[3])),
        "Color::hsv" => Ok(format!("hsv_to_rgb({}, {}, {})", args[0], args[1], args[2])),
        _ => Err(syn::Error::new(call.func.span(), format!(
            "cellarium C015: '{}' is not a recognized function.", func_name
        ))),
    }
}

fn emit_if(if_expr: &ExprIf, ctx: &mut EmitCtx, closure_param: Option<&str>) -> Result<String> {
    let cond = emit_expr(&if_expr.cond, ctx, closure_param)?;

    let then_expr = if let Some(Stmt::Expr(e, None)) = if_expr.then_branch.stmts.last() {
        emit_expr(&e, ctx, closure_param)?
    } else {
        return Err(syn::Error::new(if_expr.span(), "cellarium: if branch must produce a value"));
    };

    if let Some((_, else_branch)) = &if_expr.else_branch {
        let else_expr = emit_expr(else_branch, ctx, closure_param)?;
        Ok(format!("select({}, {}, {})", else_expr, then_expr, cond))
    } else {
        Err(syn::Error::new(if_expr.span(), "cellarium: if expression must have an else branch"))
    }
}

// ---------------------------------------------------------------------------
// Spatial operator emission
// ---------------------------------------------------------------------------

struct SpatialOp {
    kind: SpatialKind,
    closure_param: String,
    closure_body: Expr,
    filter_closure: Option<(String, Expr)>,
}

enum SpatialKind {
    Sum,
    Mean,
    Min,
    Max,
    Count,
    Laplacian,
    Gradient,
    Divergence,
    SumWhere,
    MeanWhere,
    MinWhere,
    MaxWhere,
}

fn parse_spatial_op(mc: &ExprMethodCall) -> Option<SpatialOp> {
    let method_name = mc.method.to_string();

    if let Expr::Path(p) = &*mc.receiver {
        if let Some(ident) = p.path.get_ident() {
            if ident != "nb" { return None; }
        } else { return None; }
    } else { return None; }

    let kind = match method_name.as_str() {
        "sum" => SpatialKind::Sum,
        "mean" => SpatialKind::Mean,
        "min" => SpatialKind::Min,
        "max" => SpatialKind::Max,
        "count" => SpatialKind::Count,
        "laplacian" => SpatialKind::Laplacian,
        "gradient" => SpatialKind::Gradient,
        "divergence" => SpatialKind::Divergence,
        "sum_where" => SpatialKind::SumWhere,
        "mean_where" => SpatialKind::MeanWhere,
        "min_where" => SpatialKind::MinWhere,
        "max_where" => SpatialKind::MaxWhere,
        _ => return None,
    };

    let closure = match mc.args.first() {
        Some(Expr::Closure(c)) => c,
        _ => return None,
    };

    let closure_param = if let Some(pat) = closure.inputs.first() {
        if let syn::Pat::Ident(pi) = pat { pi.ident.to_string() } else { "c".to_string() }
    } else { "c".to_string() };

    let filter_closure = if mc.args.len() > 1 {
        if let Some(Expr::Closure(fc)) = mc.args.get(1) {
            let fp = if let Some(pat) = fc.inputs.first() {
                if let syn::Pat::Ident(pi) = pat { pi.ident.to_string() } else { "c".to_string() }
            } else { "c".to_string() };
            Some((fp, (*fc.body).clone()))
        } else { None }
    } else { None };

    Some(SpatialOp {
        kind,
        closure_param,
        closure_body: (*closure.body).clone(),
        filter_closure,
    })
}

fn emit_spatial_op(op: &SpatialOp, ctx: &mut EmitCtx, var_prefix: &str) -> Result<String> {
    let neighborhood = &ctx.info.neighborhood;
    let r = neighborhood.radius() as i32;
    let skip = neighborhood.skip_condition();
    let n_count = neighborhood.neighbor_count();

    // Discover neighbor fields accessed in the closure
    let mut n_fields = Vec::new();
    let mut n_seen = HashSet::new();
    discover_neighbor_fields_in_expr(&op.closure_body, &mut n_fields, &mut n_seen, &op.closure_param);
    if let Some((ref fp, ref filter_body)) = op.filter_closure {
        discover_neighbor_fields_in_expr(filter_body, &mut n_fields, &mut n_seen, fp);
    }

    // Build neighbor field fetch lines
    let mut fetch_lines = Vec::new();
    for field_name in &n_fields {
        let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
        if ctx.compute_mode && ctx.use_shared {
            fetch_lines.push(format!(
                "            let _n_{} = shared_tex{}[_nly * PADDED + _nlx].{};",
                field_name, tex, swizzle
            ));
        } else {
            fetch_lines.push(format!(
                "            let _n_{} = textureLoad(state_tex{}, _nc, 0).{};",
                field_name, tex, swizzle
            ));
        }
    }

    // Check if spatial accessors are needed by walking the AST
    let needs_offset = expr_needs_spatial_accessors(&op.closure_body, &op.closure_param)
        || op.filter_closure.as_ref().map_or(false, |(fp, fb)| expr_needs_spatial_accessors(fb, fp));

    let mut spatial_lines = Vec::new();
    if needs_offset {
        spatial_lines.push("            let _n_offset = vec2f(f32(_dx), f32(_dy));".to_string());
        spatial_lines.push("            let _n_distance = length(_n_offset);".to_string());
        spatial_lines.push("            let _n_direction = select(vec2f(0.0, 0.0), normalize(_n_offset), _n_distance > 0.0);".to_string());
    }

    let fetch_block = [fetch_lines.join("\n"), spatial_lines.join("\n")].join("\n");

    let closure_wgsl = emit_expr(&op.closure_body, ctx, Some(&op.closure_param))?;

    // Coordinate calculation differs between fragment (textureLoad) and compute (shared memory)
    let coord_line = if ctx.compute_mode && ctx.use_shared {
        "                let _nlx = u32(i32(lx) + _dx);\n                let _nly = u32(i32(ly) + _dy);\n"
    } else {
        "                let _nc = (cell_coord + vec2i(_dx, _dy) + vec2i(uniforms.resolution)) % vec2i(uniforms.resolution);\n"
    };

    let mut code = String::new();

    match &op.kind {
        SpatialKind::Sum | SpatialKind::Mean => {
            let acc_var = format!("{}_acc", var_prefix);
            code += &format!("        var {}: f32 = 0.0;\n", acc_var);
            code += &format!("        for (var _dy: i32 = {}; _dy <= {}; _dy++) {{\n", -r, r);
            code += &format!("            for (var _dx: i32 = {}; _dx <= {}; _dx++) {{\n", -r, r);
            code += &format!("                {}\n", skip);
            code += coord_line;
            code += &fetch_block;
            code += &format!("                {} += {};\n", acc_var, closure_wgsl);
            code += "            }\n";
            code += "        }\n";
            if matches!(op.kind, SpatialKind::Mean) {
                code += &format!("        let {} = {} / {}.0;\n", var_prefix, acc_var, n_count);
            } else {
                code += &format!("        let {} = {};\n", var_prefix, acc_var);
            }
        }
        SpatialKind::Count => {
            let acc_var = format!("{}_acc", var_prefix);
            code += &format!("        var {}: f32 = 0.0;\n", acc_var);
            code += &format!("        for (var _dy: i32 = {}; _dy <= {}; _dy++) {{\n", -r, r);
            code += &format!("            for (var _dx: i32 = {}; _dx <= {}; _dx++) {{\n", -r, r);
            code += &format!("                {}\n", skip);
            code += coord_line;
            code += &fetch_block;
            code += &format!("                if ({}) {{ {} += 1.0; }}\n", closure_wgsl, acc_var);
            code += "            }\n";
            code += "        }\n";
            code += &format!("        let {} = {};\n", var_prefix, acc_var);
        }
        SpatialKind::Min => {
            let acc_var = format!("{}_acc", var_prefix);
            code += &format!("        var {}: f32 = 999999.0;\n", acc_var);
            code += &format!("        for (var _dy: i32 = {}; _dy <= {}; _dy++) {{\n", -r, r);
            code += &format!("            for (var _dx: i32 = {}; _dx <= {}; _dx++) {{\n", -r, r);
            code += &format!("                {}\n", skip);
            code += coord_line;
            code += &fetch_block;
            code += &format!("                {} = min({}, {});\n", acc_var, acc_var, closure_wgsl);
            code += "            }\n";
            code += "        }\n";
            code += &format!("        let {} = {};\n", var_prefix, acc_var);
        }
        SpatialKind::Max => {
            let acc_var = format!("{}_acc", var_prefix);
            code += &format!("        var {}: f32 = -999999.0;\n", acc_var);
            code += &format!("        for (var _dy: i32 = {}; _dy <= {}; _dy++) {{\n", -r, r);
            code += &format!("            for (var _dx: i32 = {}; _dx <= {}; _dx++) {{\n", -r, r);
            code += &format!("                {}\n", skip);
            code += coord_line;
            code += &fetch_block;
            code += &format!("                {} = max({}, {});\n", acc_var, acc_var, closure_wgsl);
            code += "            }\n";
            code += "        }\n";
            code += &format!("        let {} = {};\n", var_prefix, acc_var);
        }
        SpatialKind::Laplacian => {
            let acc_var = format!("{}_acc", var_prefix);
            let kernel_var = format!("{}_kern", var_prefix);
            code += &format!("        var {}: f32 = 0.0;\n", acc_var);
            code += &format!("        let {} = array<array<f32, 3>, 3>(\n", kernel_var);
            code += "            array<f32, 3>(0.25, 0.5, 0.25),\n";
            code += "            array<f32, 3>(0.5, -3.0, 0.5),\n";
            code += "            array<f32, 3>(0.25, 0.5, 0.25),\n";
            code += "        );\n";
            code += "        for (var _dy: i32 = -1; _dy <= 1; _dy++) {\n";
            code += "            for (var _dx: i32 = -1; _dx <= 1; _dx++) {\n";
            code += coord_line;
            code += &fetch_block;
            code += &format!("                {} += ({}) * {}[_dy + 1][_dx + 1];\n", acc_var, closure_wgsl, kernel_var);
            code += "            }\n";
            code += "        }\n";
            code += &format!("        let {} = {};\n", var_prefix, acc_var);
        }
        SpatialKind::Gradient => {
            let mut grad_code = String::new();
            if ctx.compute_mode && ctx.use_shared {
                // Shared memory: directional reads via local coordinates
                for field_name in &n_fields {
                    let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                    grad_code += &format!("        let _nr_{} = shared_tex{}[ly * PADDED + (lx + 1u)].{};\n", field_name, tex, swizzle);
                }
                for field_name in &n_fields {
                    let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                    grad_code += &format!("        let _nl_{} = shared_tex{}[ly * PADDED + (lx - 1u)].{};\n", field_name, tex, swizzle);
                }
                for field_name in &n_fields {
                    let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                    grad_code += &format!("        let _nt_{} = shared_tex{}[(ly + 1u) * PADDED + lx].{};\n", field_name, tex, swizzle);
                }
                for field_name in &n_fields {
                    let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                    grad_code += &format!("        let _nb_{} = shared_tex{}[(ly - 1u) * PADDED + lx].{};\n", field_name, tex, swizzle);
                }
            } else {
                // Fragment shader / compute without shared: textureLoad
                grad_code += "        let _nc_r = (cell_coord + vec2i(1, 0) + vec2i(uniforms.resolution)) % vec2i(uniforms.resolution);\n";
                for field_name in &n_fields {
                    let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                    grad_code += &format!("        let _nr_{} = textureLoad(state_tex{}, _nc_r, 0).{};\n", field_name, tex, swizzle);
                }
                grad_code += "        let _nc_l = (cell_coord + vec2i(-1, 0) + vec2i(uniforms.resolution)) % vec2i(uniforms.resolution);\n";
                for field_name in &n_fields {
                    let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                    grad_code += &format!("        let _nl_{} = textureLoad(state_tex{}, _nc_l, 0).{};\n", field_name, tex, swizzle);
                }
                grad_code += "        let _nc_t = (cell_coord + vec2i(0, 1) + vec2i(uniforms.resolution)) % vec2i(uniforms.resolution);\n";
                for field_name in &n_fields {
                    let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                    grad_code += &format!("        let _nt_{} = textureLoad(state_tex{}, _nc_t, 0).{};\n", field_name, tex, swizzle);
                }
                grad_code += "        let _nc_b = (cell_coord + vec2i(0, -1) + vec2i(uniforms.resolution)) % vec2i(uniforms.resolution);\n";
                for field_name in &n_fields {
                    let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                    grad_code += &format!("        let _nb_{} = textureLoad(state_tex{}, _nc_b, 0).{};\n", field_name, tex, swizzle);
                }
            }

            let body_right = emit_expr_with_prefix(&op.closure_body, ctx, &op.closure_param, "_nr_")?;
            let body_left = emit_expr_with_prefix(&op.closure_body, ctx, &op.closure_param, "_nl_")?;
            let body_top = emit_expr_with_prefix(&op.closure_body, ctx, &op.closure_param, "_nt_")?;
            let body_bottom = emit_expr_with_prefix(&op.closure_body, ctx, &op.closure_param, "_nb_")?;

            grad_code += &format!("        let {} = vec2f(({} - {}) * 0.5, ({} - {}) * 0.5);\n",
                var_prefix, body_right, body_left, body_top, body_bottom);

            code += &grad_code;
        }
        SpatialKind::Divergence => {
            let mut div_code = String::new();
            if ctx.compute_mode && ctx.use_shared {
                let dirs: [(&str, &str, &str); 4] = [
                    ("r", "lx + 1u", "ly"),
                    ("l", "lx - 1u", "ly"),
                    ("t", "lx", "ly + 1u"),
                    ("b", "lx", "ly - 1u"),
                ];
                for (dir, lx_expr, ly_expr) in &dirs {
                    for field_name in &n_fields {
                        let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                        div_code += &format!("        let _n{}_{} = shared_tex{}[({}) * PADDED + ({})].{};\n",
                            dir, field_name, tex, ly_expr, lx_expr, swizzle);
                    }
                }
            } else {
                for (dir, dx, dy) in &[("r", 1, 0), ("l", -1, 0), ("t", 0, 1), ("b", 0, -1)] {
                    div_code += &format!("        let _nc_{} = (cell_coord + vec2i({}, {}) + vec2i(uniforms.resolution)) % vec2i(uniforms.resolution);\n", dir, dx, dy);
                    for field_name in &n_fields {
                        let (tex, swizzle) = ctx.neighbor_field_fetch(field_name);
                        div_code += &format!("        let _n{}_{} = textureLoad(state_tex{}, _nc_{}, 0).{};\n", dir, field_name, tex, dir, swizzle);
                    }
                }
            }

            let body_right = emit_expr_with_prefix(&op.closure_body, ctx, &op.closure_param, "_nr_")?;
            let body_left = emit_expr_with_prefix(&op.closure_body, ctx, &op.closure_param, "_nl_")?;
            let body_top = emit_expr_with_prefix(&op.closure_body, ctx, &op.closure_param, "_nt_")?;
            let body_bottom = emit_expr_with_prefix(&op.closure_body, ctx, &op.closure_param, "_nb_")?;

            div_code += &format!("        let {} = (({}).x - ({}).x) * 0.5 + (({}).y - ({}).y) * 0.5;\n",
                var_prefix, body_right, body_left, body_top, body_bottom);

            code += &div_code;
        }
        SpatialKind::SumWhere | SpatialKind::MeanWhere => {
            let acc_var = format!("{}_acc", var_prefix);
            let cnt_var = format!("{}_cnt", var_prefix);
            code += &format!("        var {}: f32 = 0.0;\n", acc_var);
            code += &format!("        var {}: f32 = 0.0;\n", cnt_var);
            code += &format!("        for (var _dy: i32 = {}; _dy <= {}; _dy++) {{\n", -r, r);
            code += &format!("            for (var _dx: i32 = {}; _dx <= {}; _dx++) {{\n", -r, r);
            code += &format!("                {}\n", skip);
            code += coord_line;
            code += &fetch_block;
            if let Some((ref fp, ref filter_body)) = op.filter_closure {
                let filter_wgsl = emit_expr(filter_body, ctx, Some(fp))?;
                code += &format!("                if ({}) {{\n", filter_wgsl);
                code += &format!("                    {} += {};\n", acc_var, closure_wgsl);
                code += &format!("                    {} += 1.0;\n", cnt_var);
                code += "                }\n";
            }
            code += "            }\n";
            code += "        }\n";
            if matches!(op.kind, SpatialKind::MeanWhere) {
                code += &format!("        let {} = select({} / {}, 0.0, {} == 0.0);\n", var_prefix, acc_var, cnt_var, cnt_var);
            } else {
                code += &format!("        let {} = {};\n", var_prefix, acc_var);
            }
        }
        SpatialKind::MinWhere => {
            let acc_var = format!("{}_acc", var_prefix);
            code += &format!("        var {}: f32 = 999999.0;\n", acc_var);
            code += &format!("        for (var _dy: i32 = {}; _dy <= {}; _dy++) {{\n", -r, r);
            code += &format!("            for (var _dx: i32 = {}; _dx <= {}; _dx++) {{\n", -r, r);
            code += &format!("                {}\n", skip);
            code += coord_line;
            code += &fetch_block;
            if let Some((ref fp, ref filter_body)) = op.filter_closure {
                let filter_wgsl = emit_expr(filter_body, ctx, Some(fp))?;
                code += &format!("                if ({}) {{ {} = min({}, {}); }}\n", filter_wgsl, acc_var, acc_var, closure_wgsl);
            }
            code += "            }\n";
            code += "        }\n";
            code += &format!("        let {} = {};\n", var_prefix, acc_var);
        }
        SpatialKind::MaxWhere => {
            let acc_var = format!("{}_acc", var_prefix);
            code += &format!("        var {}: f32 = -999999.0;\n", acc_var);
            code += &format!("        for (var _dy: i32 = {}; _dy <= {}; _dy++) {{\n", -r, r);
            code += &format!("            for (var _dx: i32 = {}; _dx <= {}; _dx++) {{\n", -r, r);
            code += &format!("                {}\n", skip);
            code += coord_line;
            code += &fetch_block;
            if let Some((ref fp, ref filter_body)) = op.filter_closure {
                let filter_wgsl = emit_expr(filter_body, ctx, Some(fp))?;
                code += &format!("                if ({}) {{ {} = max({}, {}); }}\n", filter_wgsl, acc_var, acc_var, closure_wgsl);
            }
            code += "            }\n";
            code += "        }\n";
            code += &format!("        let {} = {};\n", var_prefix, acc_var);
        }
    }

    Ok(code)
}

fn emit_expr_with_prefix(expr: &Expr, ctx: &mut EmitCtx, closure_param: &str, prefix: &str) -> Result<String> {
    match expr {
        Expr::Field(f) => {
            if let Expr::Path(p) = &*f.base {
                if let Some(ident) = p.path.get_ident() {
                    if ident.to_string() == closure_param {
                        if let Member::Named(field_name) = &f.member {
                            return Ok(format!("{}{}", prefix, field_name));
                        }
                    }
                }
            }
            let base = emit_expr_with_prefix(&f.base, ctx, closure_param, prefix)?;
            if let Member::Named(name) = &f.member {
                Ok(format!("{}.{}", base, name))
            } else {
                Err(syn::Error::new(f.span(), "cellarium: unsupported field access"))
            }
        }
        Expr::Binary(b) => {
            let left = emit_expr_with_prefix(&b.left, ctx, closure_param, prefix)?;
            let right = emit_expr_with_prefix(&b.right, ctx, closure_param, prefix)?;
            let op = match &b.op {
                BinOp::Add(_) => "+", BinOp::Sub(_) => "-",
                BinOp::Mul(_) => "*", BinOp::Div(_) => "/",
                _ => return emit_expr(expr, ctx, Some(closure_param)),
            };
            Ok(format!("({} {} {})", left, op, right))
        }
        Expr::Unary(u) => {
            let operand = emit_expr_with_prefix(&u.expr, ctx, closure_param, prefix)?;
            match &u.op {
                UnOp::Neg(_) => Ok(format!("(-{})", operand)),
                _ => emit_expr(expr, ctx, Some(closure_param)),
            }
        }
        Expr::MethodCall(mc) => {
            let receiver = emit_expr_with_prefix(&mc.receiver, ctx, closure_param, prefix)?;
            let method_name = mc.method.to_string();
            match method_name.as_str() {
                "sin" => Ok(format!("sin({})", receiver)),
                "cos" => Ok(format!("cos({})", receiver)),
                "abs" => Ok(format!("abs({})", receiver)),
                "sqrt" => Ok(format!("sqrt({})", receiver)),
                _ => emit_expr(expr, ctx, Some(closure_param)),
            }
        }
        _ => emit_expr(expr, ctx, Some(closure_param)),
    }
}

// ---------------------------------------------------------------------------
// Statement-level emission
// ---------------------------------------------------------------------------

fn emit_stmt(stmt: &Stmt, ctx: &mut EmitCtx) -> Result<()> {
    match stmt {
        Stmt::Local(local) => {
            let name = if let syn::Pat::Ident(pi) = &local.pat {
                pi.ident.to_string()
            } else if let syn::Pat::Type(pt) = &local.pat {
                if let syn::Pat::Ident(pi) = &*pt.pat {
                    pi.ident.to_string()
                } else {
                    return Err(syn::Error::new(local.pat.span(), "cellarium: unsupported pattern"));
                }
            } else {
                return Err(syn::Error::new(local.pat.span(), "cellarium: unsupported pattern"));
            };

            if let Some(init) = &local.init {
                // Check if this is a spatial operator assignment
                if let Some(spatial_op) = try_parse_spatial_assignment(&init.expr) {
                    let op_code = emit_spatial_op(&spatial_op, ctx, &user_var(&name))?;
                    ctx.body.push(op_code);
                    return Ok(());
                }

                let val = emit_expr(&init.expr, ctx, None)?;
                ctx.body.push(format!("        let {} = {};", user_var(&name), val));
            }
            Ok(())
        }
        Stmt::Expr(expr, semi) => {
            match expr {
                Expr::Return(ret) => {
                    if let Some(ret_expr) = &ret.expr {
                        let output = emit_return_struct(ret_expr, ctx)?;
                        ctx.body.push(output);
                    }
                    Ok(())
                }
                Expr::Struct(s) => {
                    let output = emit_struct_output(s, ctx)?;
                    ctx.output_lines.push(output);
                    Ok(())
                }
                _ => {
                    let val = emit_expr(expr, ctx, None)?;
                    if semi.is_some() {
                        ctx.body.push(format!("        {};", val));
                    } else {
                        let output = emit_return_struct(expr, ctx)?;
                        ctx.output_lines.push(output);
                    }
                    Ok(())
                }
            }
        }
        _ => Ok(()),
    }
}

fn try_parse_spatial_assignment(expr: &Expr) -> Option<SpatialOp> {
    if let Expr::MethodCall(mc) = expr {
        return parse_spatial_op(mc);
    }
    None
}

fn emit_return_struct(expr: &Expr, ctx: &mut EmitCtx) -> Result<String> {
    match expr {
        Expr::Struct(s) => emit_struct_output(s, ctx),
        Expr::If(if_expr) => {
            let cond = emit_expr(&if_expr.cond, ctx, None)?;
            let then_val = if let Some(Stmt::Expr(e, None)) = if_expr.then_branch.stmts.last() {
                emit_return_struct(e, ctx)?
            } else {
                return Err(syn::Error::new(if_expr.span(), "cellarium: if branch must return a struct"));
            };
            let else_val = if let Some((_, else_branch)) = &if_expr.else_branch {
                emit_return_struct(else_branch, ctx)?
            } else {
                return Err(syn::Error::new(if_expr.span(), "cellarium: if/else must have both branches when returning a struct"));
            };
            Ok(format!("        // if/else return\n        if ({}) {{\n    {}\n        }} else {{\n    {}\n        }}", cond, then_val, else_val))
        }
        _ => {
            let val = emit_expr(expr, ctx, None)?;
            Ok(format!("        // return value\n        {}", val))
        }
    }
}

fn emit_struct_output(s: &ExprStruct, ctx: &mut EmitCtx) -> Result<String> {
    let num_textures = ((ctx.info.fields.len() + 3) / 4).max(1);
    let mut tex_values: Vec<Vec<String>> = (0..num_textures)
        .map(|_| vec!["0.0".to_string(); 4])
        .collect();

    for field in &s.fields {
        let field_name = match &field.member {
            Member::Named(name) => name.to_string(),
            _ => return Err(syn::Error::new(field.span(), "cellarium: expected named field")),
        };

        let val = emit_expr(&field.expr, ctx, None)?;
        // Look up the correct texture/channel for this field by name
        let (tex_idx, swizzle) = ctx.field_texture_swizzle(&field_name);
        let offset = match swizzle.chars().next().unwrap_or('r') {
            'r' => 0, 'g' => 1, 'b' => 2, 'a' => 3,
            _ => 0,
        };
        tex_values[tex_idx as usize][offset] = val;
    }

    let mut output = String::new();
    if ctx.compute_mode {
        for i in 0..num_textures {
            output += &format!("        textureStore(out_tex{}, cell_coord, vec4f({}, {}, {}, {}));\n",
                i, tex_values[i][0], tex_values[i][1], tex_values[i][2], tex_values[i][3]);
        }
    } else if num_textures == 1 {
        output += &format!("        return Output(vec4f({}, {}, {}, {}));",
            tex_values[0][0], tex_values[0][1], tex_values[0][2], tex_values[0][3]);
    } else {
        let tex_args: Vec<String> = (0..num_textures)
            .map(|i| format!("vec4f({}, {}, {}, {})",
                tex_values[i][0], tex_values[i][1], tex_values[i][2], tex_values[i][3]))
            .collect();
        output += &format!("        return Output({});", tex_args.join(", "));
    }

    Ok(output)
}

// ---------------------------------------------------------------------------
// Full shader emission
// ---------------------------------------------------------------------------

fn emit_shader_header(info: &CellImplInfo, num_textures: usize) -> String {
    let mut header = String::new();
    header += "// --- Auto-generated by cellarium ---\n\n";
    let param_vec4s = (info.constants.len() + 3) / 4;
    header += "struct Uniforms {\n";
    header += "    tick: u32,\n";
    header += "    zoom: f32,\n";
    header += "    resolution: vec2f,\n";
    header += "    camera: vec2f,\n";
    header += "    viewport: vec2f,\n";
    if param_vec4s > 0 {
        header += &format!("    params: array<vec4f, {}>,\n", param_vec4s);
    }
    header += "}\n\n";

    for i in 0..num_textures {
        header += &format!("@group(0) @binding({}) var state_tex{}: texture_2d<f32>;\n", i, i);
    }
    header += &format!("@group(0) @binding({}) var<uniform> uniforms: Uniforms;\n\n", num_textures);

    header += "fn hsv_to_rgb(h: f32, s: f32, v: f32) -> vec4f {\n";
    header += "    let hh = ((h % 1.0) + 1.0) % 1.0;\n";
    header += "    let c = v * s;\n";
    header += "    let h6 = hh * 6.0;\n";
    header += "    let x = c * (1.0 - abs(h6 % 2.0 - 1.0));\n";
    header += "    let m = v - c;\n";
    header += "    var rgb: vec3f;\n";
    header += "    if (h6 < 1.0) { rgb = vec3f(c, x, 0.0); }\n";
    header += "    else if (h6 < 2.0) { rgb = vec3f(x, c, 0.0); }\n";
    header += "    else if (h6 < 3.0) { rgb = vec3f(0.0, c, x); }\n";
    header += "    else if (h6 < 4.0) { rgb = vec3f(0.0, x, c); }\n";
    header += "    else if (h6 < 5.0) { rgb = vec3f(x, 0.0, c); }\n";
    header += "    else { rgb = vec3f(c, 0.0, x); }\n";
    header += "    return vec4f(rgb + m, 1.0);\n";
    header += "}\n\n";

    header
}

fn emit_param_reads(info: &CellImplInfo) -> String {
    let mut code = String::new();
    for (i, c) in info.constants.iter().enumerate() {
        code += &format!("    let {} = uniforms.params[{}][{}];\n", user_var(&c.name), i / 4, i % 4);
    }
    if !info.constants.is_empty() {
        code += "\n";
    }
    code
}

fn emit_vertex_shader() -> &'static str {
    "@vertex\nfn vs_main(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4f {\n    let x = f32(vid & 1u) * 4.0 - 1.0;\n    let y = f32((vid >> 1u) & 1u) * 4.0 - 1.0;\n    return vec4f(x, y, 0.0, 1.0);\n}\n\n"
}

pub fn compute_tile_config(radius: u32, num_textures: u32) -> (u32, bool) {
    // Try 16x16 tiles first
    let padded_16 = 16 + 2 * radius;
    let bytes_16 = padded_16 * padded_16 * 16 * num_textures;
    if bytes_16 <= 16384 {
        return (16, true);
    }
    // Try 8x8 tiles
    let padded_8 = 8 + 2 * radius;
    let bytes_8 = padded_8 * padded_8 * 16 * num_textures;
    if bytes_8 <= 16384 {
        return (8, true);
    }
    // Shared memory doesn't fit — compute shader without shared memory
    (16, false)
}

fn emit_compute_shader_header(info: &CellImplInfo, num_textures: usize, tile_size: u32, radius: u32, use_shared: bool) -> String {
    let mut header = String::new();
    header += "// --- Auto-generated by cellarium (compute) ---\n\n";

    let param_vec4s = (info.constants.len() + 3) / 4;
    header += "struct Uniforms {\n";
    header += "    tick: u32,\n";
    header += "    zoom: f32,\n";
    header += "    resolution: vec2f,\n";
    header += "    camera: vec2f,\n";
    header += "    viewport: vec2f,\n";
    if param_vec4s > 0 {
        header += &format!("    params: array<vec4f, {}>,\n", param_vec4s);
    }
    header += "}\n\n";

    // Read textures
    for i in 0..num_textures {
        header += &format!("@group(0) @binding({}) var state_tex{}: texture_2d<f32>;\n", i, i);
    }
    // Uniform buffer
    header += &format!("@group(0) @binding({}) var<uniform> uniforms: Uniforms;\n", num_textures);
    // Write textures (storage)
    for i in 0..num_textures {
        header += &format!("@group(0) @binding({}) var out_tex{}: texture_storage_2d<rgba32float, write>;\n",
            num_textures + 1 + i, i);
    }
    header += "\n";

    // Shared memory declarations
    if use_shared {
        let padded = tile_size + 2 * radius;
        header += &format!("const TILE_SIZE: u32 = {}u;\n", tile_size);
        header += &format!("const RADIUS: u32 = {}u;\n", radius);
        header += &format!("const PADDED: u32 = {}u;\n\n", padded);

        for i in 0..num_textures {
            header += &format!("var<workgroup> shared_tex{}: array<vec4f, {}>;\n", i, padded * padded);
        }
        header += "\n";
    }

    // HSV helper
    header += "fn hsv_to_rgb(h: f32, s: f32, v: f32) -> vec4f {\n";
    header += "    let hh = ((h % 1.0) + 1.0) % 1.0;\n";
    header += "    let c = v * s;\n";
    header += "    let h6 = hh * 6.0;\n";
    header += "    let x = c * (1.0 - abs(h6 % 2.0 - 1.0));\n";
    header += "    let m = v - c;\n";
    header += "    var rgb: vec3f;\n";
    header += "    if (h6 < 1.0) { rgb = vec3f(c, x, 0.0); }\n";
    header += "    else if (h6 < 2.0) { rgb = vec3f(x, c, 0.0); }\n";
    header += "    else if (h6 < 3.0) { rgb = vec3f(0.0, c, x); }\n";
    header += "    else if (h6 < 4.0) { rgb = vec3f(0.0, x, c); }\n";
    header += "    else if (h6 < 5.0) { rgb = vec3f(x, 0.0, c); }\n";
    header += "    else { rgb = vec3f(c, 0.0, x); }\n";
    header += "    return vec4f(rgb + m, 1.0);\n";
    header += "}\n\n";

    header
}

pub fn emit_update_shader(info: &CellImplInfo, block: &Block) -> Result<String> {
    let num_textures = ((info.fields.len() + 3) / 4).max(1);
    let radius = info.neighborhood.radius();
    let (tile_size, use_shared) = compute_tile_config(radius, num_textures as u32);

    let mut shader = emit_compute_shader_header(info, num_textures, tile_size, radius, use_shared);

    // Compute entry point
    shader += &format!("@compute @workgroup_size({}, {}, 1)\n", tile_size, tile_size);
    shader += "fn cs_main(\n";
    shader += "    @builtin(global_invocation_id) global_id: vec3u,\n";
    shader += "    @builtin(local_invocation_id) local_id: vec3u,\n";
    shader += "    @builtin(workgroup_id) wg_id: vec3u,\n";
    shader += ") {\n";
    shader += "    let cell_coord = vec2i(global_id.xy);\n";
    shader += "    let res = vec2i(uniforms.resolution);\n\n";

    // Cooperative tile loading into shared memory
    if use_shared {
        let padded = tile_size + 2 * radius;
        let total = padded * padded;
        let threads = tile_size * tile_size;

        shader += &format!("    let tile_origin = vec2i(wg_id.xy) * vec2i({}i) - vec2i({}i);\n", tile_size, radius);
        shader += &format!("    let tid = local_id.y * {}u + local_id.x;\n\n", tile_size);

        shader += &format!("    for (var i = tid; i < {}u; i += {}u) {{\n", total, threads);
        shader += &format!("        let sy = i / {}u;\n", padded);
        shader += &format!("        let sx = i % {}u;\n", padded);
        shader += "        let gc = (tile_origin + vec2i(i32(sx), i32(sy)) + res) % res;\n";
        for i in 0..num_textures {
            shader += &format!("        shared_tex{}[i] = textureLoad(state_tex{}, gc, 0);\n", i, i);
        }
        shader += "    }\n\n";
        shader += "    workgroupBarrier();\n\n";
    }

    // Bounds check (after barrier so all threads participate in loading)
    shader += "    if (global_id.x >= u32(res.x) || global_id.y >= u32(res.y)) {\n";
    shader += "        return;\n";
    shader += "    }\n\n";

    // Local coordinates within shared tile (for shared memory indexing)
    if use_shared {
        shader += &format!("    let lx = local_id.x + {}u;\n", radius);
        shader += &format!("    let ly = local_id.y + {}u;\n\n", radius);
    }

    shader += &emit_param_reads(info);

    let mut ctx = EmitCtx::new(info);
    ctx.compute_mode = true;
    ctx.use_shared = use_shared;

    for field in &info.fields {
        ctx.ensure_self_field(&field.name);
    }

    for line in &ctx.preamble {
        shader += line;
        shader += "\n";
    }
    shader += "\n";

    for stmt in &block.stmts {
        emit_stmt(stmt, &mut ctx)?;
    }

    for line in &ctx.body {
        shader += line;
        shader += "\n";
    }

    for line in &ctx.output_lines {
        shader += line;
        shader += "\n";
    }

    shader += "}\n";

    Ok(shader)
}

pub fn emit_view_shader(info: &CellImplInfo, block: &Block) -> Result<String> {
    let num_textures = ((info.fields.len() + 3) / 4).max(1);
    let mut shader = emit_shader_header(info, num_textures);
    shader += emit_vertex_shader();

    shader += "@fragment\nfn fs_main(@builtin(position) frag_pos: vec4f) -> @location(0) vec4f {\n";
    shader += "    let center = uniforms.viewport * 0.5;\n";
    shader += "    let view_pos = (frag_pos.xy - center) / uniforms.zoom + uniforms.camera;\n";
    shader += "    let cell_coord_raw = vec2i(floor(view_pos));\n";
    shader += "    let cell_coord = (cell_coord_raw % vec2i(uniforms.resolution) + vec2i(uniforms.resolution)) % vec2i(uniforms.resolution);\n\n";
    shader += &emit_param_reads(info);

    let mut ctx = EmitCtx::new(info);

    for field in &info.fields {
        ctx.ensure_self_field(&field.name);
    }
    for line in &ctx.preamble {
        shader += line;
        shader += "\n";
    }
    shader += "\n";

    let stmts = &block.stmts;
    for (i, stmt) in stmts.iter().enumerate() {
        let is_last = i == stmts.len() - 1;
        match stmt {
            Stmt::Local(local) => {
                let name = if let syn::Pat::Ident(pi) = &local.pat {
                    pi.ident.to_string()
                } else if let syn::Pat::Type(pt) = &local.pat {
                    if let syn::Pat::Ident(pi) = &*pt.pat {
                        pi.ident.to_string()
                    } else {
                        return Err(syn::Error::new(local.pat.span(), "cellarium: unsupported pattern"));
                    }
                } else {
                    return Err(syn::Error::new(local.pat.span(), "cellarium: unsupported pattern"));
                };
                if let Some(init) = &local.init {
                    let val = emit_expr(&init.expr, &mut ctx, None)?;
                    shader += &format!("    let {} = {};\n", user_var(&name), val);
                }
            }
            Stmt::Expr(expr, semi) => {
                if is_last && semi.is_none() {
                    let val = emit_view_return(expr, &mut ctx)?;
                    shader += &format!("    return {};\n", val);
                } else {
                    let val = emit_expr(expr, &mut ctx, None)?;
                    shader += &format!("    {};\n", val);
                }
            }
            _ => {}
        }
    }

    shader += "}\n";
    Ok(shader)
}

fn emit_view_return(expr: &Expr, ctx: &mut EmitCtx) -> Result<String> {
    match expr {
        Expr::If(if_expr) => {
            let cond = emit_expr(&if_expr.cond, ctx, None)?;
            let then_val = if let Some(Stmt::Expr(e, None)) = if_expr.then_branch.stmts.last() {
                emit_view_return(e, ctx)?
            } else {
                return Err(syn::Error::new(if_expr.span(), "cellarium: if branch must return a Color"));
            };
            let else_val = if let Some((_, else_branch)) = &if_expr.else_branch {
                emit_view_return(else_branch, ctx)?
            } else {
                return Err(syn::Error::new(if_expr.span(), "cellarium: view if/else must have both branches"));
            };
            Ok(format!("select({}, {}, {})", else_val, then_val, cond))
        }
        _ => emit_expr(expr, ctx, None),
    }
}

pub fn emit_init_shader(info: &CellImplInfo, block: &Block) -> Result<String> {
    let num_textures = ((info.fields.len() + 3) / 4).max(1);

    // Init shader has its own header: only uniform buffer at binding 0, no state textures.
    let mut shader = String::new();
    shader += "// --- Auto-generated by cellarium ---\n\n";
    let param_vec4s = (info.constants.len() + 3) / 4;
    shader += "struct Uniforms {\n";
    shader += "    tick: u32,\n";
    shader += "    zoom: f32,\n";
    shader += "    resolution: vec2f,\n";
    shader += "    camera: vec2f,\n";
    shader += "    viewport: vec2f,\n";
    if param_vec4s > 0 {
        shader += &format!("    params: array<vec4f, {}>,\n", param_vec4s);
    }
    shader += "}\n\n";
    shader += "@group(0) @binding(0) var<uniform> uniforms: Uniforms;\n\n";

    shader += emit_vertex_shader();

    shader += "struct Output {\n";
    for i in 0..num_textures {
        shader += &format!("    @location({}) tex{}: vec4f,\n", i, i);
    }
    shader += "}\n\n";

    shader += "@fragment\nfn fs_main(@builtin(position) frag_pos: vec4f) -> Output {\n";
    shader += "    let cell_coord = vec2i(frag_pos.xy);\n";
    shader += "    let v_x = f32(cell_coord.x);\n";
    shader += "    let v_y = f32(cell_coord.y);\n";
    shader += "    let v_w = uniforms.resolution.x;\n";
    shader += "    let v_h = uniforms.resolution.y;\n\n";
    shader += &emit_param_reads(info);

    let mut ctx = EmitCtx::new(info);

    for stmt in &block.stmts {
        emit_stmt(stmt, &mut ctx)?;
    }

    for line in &ctx.body {
        shader += line;
        shader += "\n";
    }
    for line in &ctx.output_lines {
        shader += line;
        shader += "\n";
    }

    shader += "}\n";
    Ok(shader)
}
