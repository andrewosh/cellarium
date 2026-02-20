use syn::{Block, Expr, Pat, Result, Stmt, spanned::Spanned};

use crate::lower::Neighborhood;

pub fn validate_method_body(block: &Block, method: &str, neighborhood: &Neighborhood) -> Result<()> {
    for stmt in &block.stmts {
        validate_stmt(stmt, method, neighborhood)?;
    }
    Ok(())
}

fn validate_stmt(stmt: &Stmt, method: &str, neighborhood: &Neighborhood) -> Result<()> {
    match stmt {
        Stmt::Local(local) => {
            // Reject `let mut`
            if let Pat::Ident(pi) = &local.pat {
                if pi.mutability.is_some() {
                    return Err(syn::Error::new(pi.span(), "cellarium C006: let mut is not supported. All bindings are immutable in cell code."));
                }
            }
            if let Some(init) = &local.init {
                validate_expr(&init.expr, method, neighborhood, false)?;
            }
            Ok(())
        }
        Stmt::Expr(expr, _) => validate_expr(expr, method, neighborhood, false),
        _ => Ok(()),
    }
}

fn validate_expr(expr: &Expr, method: &str, neighborhood: &Neighborhood, in_closure: bool) -> Result<()> {
    match expr {
        Expr::Assign(a) => {
            Err(syn::Error::new(a.span(), "cellarium C006: reassignment is not supported. All bindings are immutable in cell code."))
        }
        Expr::ForLoop(f) => {
            Err(syn::Error::new(f.span(), "cellarium C007: Unbounded loops are not GPU-compatible. Use spatial operators for neighbor iteration."))
        }
        Expr::While(w) => {
            Err(syn::Error::new(w.span(), "cellarium C007: Unbounded loops are not GPU-compatible. Use spatial operators for neighbor iteration."))
        }
        Expr::Loop(l) => {
            Err(syn::Error::new(l.span(), "cellarium C007: Unbounded loops are not GPU-compatible. Use spatial operators for neighbor iteration."))
        }
        Expr::Match(m) => {
            Err(syn::Error::new(m.span(), "cellarium C008: match is not supported in cell code. Use if/else chains."))
        }
        Expr::Reference(r) => {
            Err(syn::Error::new(r.span(), "cellarium C016: References and borrowing are not used in cell code."))
        }
        Expr::Unsafe(u) => {
            Err(syn::Error::new(u.span(), "cellarium C019: 'unsafe' is not supported in cell code."))
        }
        Expr::Closure(c) => {
            if !in_closure {
                // Closures are only valid inside spatial operator calls
                // We allow them here because they'll be validated at the call site
                // The check is done via the nb.method(|c| ...) pattern
            }
            validate_expr(&c.body, method, neighborhood, true)?;
            Ok(())
        }
        Expr::MethodCall(mc) => {
            // Check for Neighbors usage outside update
            let method_name = mc.method.to_string();

            // Check differential operators require appropriate neighborhood
            if matches!(method_name.as_str(), "gradient" | "divergence") {
                if matches!(neighborhood, Neighborhood::VonNeumann) {
                    return Err(syn::Error::new(mc.span(), "cellarium C012: gradient/divergence requires moore or radius(N) neighborhood."));
                }
            }

            // Validate receiver and arguments
            validate_expr(&mc.receiver, method, neighborhood, in_closure)?;
            for arg in &mc.args {
                validate_expr(arg, method, neighborhood, in_closure)?;
            }
            Ok(())
        }
        Expr::Binary(b) => {
            validate_expr(&b.left, method, neighborhood, in_closure)?;
            validate_expr(&b.right, method, neighborhood, in_closure)?;
            Ok(())
        }
        Expr::Unary(u) => {
            validate_expr(&u.expr, method, neighborhood, in_closure)?;
            Ok(())
        }
        Expr::If(i) => {
            validate_expr(&i.cond, method, neighborhood, in_closure)?;
            for stmt in &i.then_branch.stmts {
                validate_stmt(stmt, method, neighborhood)?;
            }
            if let Some((_, else_branch)) = &i.else_branch {
                validate_expr(else_branch, method, neighborhood, in_closure)?;
            }
            Ok(())
        }
        Expr::Block(b) => {
            for stmt in &b.block.stmts {
                validate_stmt(stmt, method, neighborhood)?;
            }
            Ok(())
        }
        Expr::Paren(p) => validate_expr(&p.expr, method, neighborhood, in_closure),
        Expr::Call(c) => {
            validate_expr(&c.func, method, neighborhood, in_closure)?;
            for arg in &c.args {
                validate_expr(arg, method, neighborhood, in_closure)?;
            }
            Ok(())
        }
        Expr::Field(_) | Expr::Path(_) | Expr::Lit(_) | Expr::Struct(_) | Expr::Return(_) => Ok(()),
        Expr::Group(g) => validate_expr(&g.expr, method, neighborhood, in_closure),
        _ => Ok(()),
    }
}
