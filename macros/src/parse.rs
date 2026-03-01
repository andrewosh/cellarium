use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Ident, ImplItem, ImplItemFn, ItemImpl, Result,
    Type, parse2, spanned::Spanned,
};

use crate::check::validate_method_body;
use crate::lower::{self, Neighborhood, CellImplInfo, ConstInfo};

fn parse_neighborhood(attr: &TokenStream) -> Result<Neighborhood> {
    let s = attr.to_string();
    let s = s.trim();
    if s.is_empty() {
        return Ok(Neighborhood::Moore);
    }
    // Parse "neighborhood = moore" / "neighborhood = von_neumann" / "neighborhood = radius(N)"
    let s = s.replace(' ', "");
    if let Some(rest) = s.strip_prefix("neighborhood=") {
        match rest {
            "moore" => Ok(Neighborhood::Moore),
            "von_neumann" => Ok(Neighborhood::VonNeumann),
            _ if rest.starts_with("radius(") && rest.ends_with(')') => {
                let n_str = &rest[7..rest.len() - 1];
                let n: u32 = n_str.parse().map_err(|_| {
                    syn::Error::new(proc_macro2::Span::call_site(), format!(
                        "cellarium: invalid radius '{}'. Expected a positive integer.", n_str
                    ))
                })?;
                Ok(Neighborhood::Radius(n))
            }
            _ => Err(syn::Error::new(proc_macro2::Span::call_site(), format!(
                "cellarium: unknown neighborhood '{}'. Expected moore, von_neumann, or radius(N).", rest
            ))),
        }
    } else {
        Err(syn::Error::new(proc_macro2::Span::call_site(), format!(
            "cellarium: unknown attribute parameter '{}'. Expected 'neighborhood = ...'.", s
        )))
    }
}

fn extract_self_type_name(item_impl: &ItemImpl) -> Result<Ident> {
    match &*item_impl.self_ty {
        Type::Path(tp) => {
            let seg = tp.path.segments.last().ok_or_else(|| {
                syn::Error::new(item_impl.self_ty.span(), "cellarium: cannot determine struct name")
            })?;
            Ok(seg.ident.clone())
        }
        _ => Err(syn::Error::new(item_impl.self_ty.span(), "cellarium: expected a named type")),
    }
}

pub fn cell_impl(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let neighborhood = parse_neighborhood(&attr)?;
    let item_impl: ItemImpl = parse2(item)?;
    let struct_name = extract_self_type_name(&item_impl)?;

    let mut constants: Vec<ConstInfo> = Vec::new();
    let mut update_method: Option<&ImplItemFn> = None;
    let mut view_method: Option<&ImplItemFn> = None;
    let mut init_method: Option<&ImplItemFn> = None;

    for item in &item_impl.items {
        match item {
            ImplItem::Const(c) => {
                let name = c.ident.to_string();
                // Extract f32 value from the expression
                constants.push(ConstInfo {
                    name,
                    expr: c.expr.clone(),
                });
            }
            ImplItem::Fn(m) => {
                let method_name = m.sig.ident.to_string();
                match method_name.as_str() {
                    "update" => update_method = Some(m),
                    "view" => view_method = Some(m),
                    "init" => init_method = Some(m),
                    other => return Err(syn::Error::new(m.sig.ident.span(), format!(
                        "cellarium C015: '{}' is not a recognized method. Expected update, view, or init.", other
                    ))),
                }
            }
            _ => {}
        }
    }

    let update_fn = update_method.ok_or_else(|| {
        syn::Error::new(struct_name.span(), "cellarium: missing required method `update`")
    })?;
    let view_fn = view_method.ok_or_else(|| {
        syn::Error::new(struct_name.span(), "cellarium: missing required method `view`")
    })?;

    // Validate method bodies
    validate_method_body(&update_fn.block, "update", &neighborhood)?;
    validate_method_body(&view_fn.block, "view", &neighborhood)?;
    if let Some(init_fn) = init_method {
        validate_method_body(&init_fn.block, "init", &neighborhood)?;
    }

    // Discover fields from method bodies
    let mut fields = lower::discover_fields(&update_fn.block);
    if let Some(init_fn) = init_method {
        let init_fields = lower::discover_fields(&init_fn.block);
        for f in init_fields {
            if !fields.iter().any(|existing| existing.name == f.name) {
                fields.push(f);
            }
        }
    }

    let info = CellImplInfo {
        neighborhood,
        constants: constants.clone(),
        fields,
    };

    // Generate WGSL shaders
    let update_wgsl = lower::emit_update_shader(&info, &update_fn.block)?;
    let view_wgsl = lower::emit_view_shader(&info, &view_fn.block)?;
    let (has_init, init_wgsl) = if let Some(init_fn) = init_method {
        (true, lower::emit_init_shader(&info, &init_fn.block)?)
    } else {
        (false, String::new())
    };

    // Compute tile size for the runtime dispatch
    let num_textures = ((info.fields.len() + 3) / 4).max(1) as u32;
    let (tile_size, _use_shared) = lower::compute_tile_config(info.neighborhood.radius(), num_textures);
    let tile_size_lit = tile_size;

    let param_names: Vec<String> = constants.iter().map(|c| c.name.clone()).collect();
    let param_name_lits: Vec<syn::LitStr> = param_names.iter()
        .map(|n| syn::LitStr::new(n, proc_macro2::Span::call_site()))
        .collect();
    let param_defaults: Vec<&syn::Expr> = constants.iter().map(|c| &c.expr).collect();

    Ok(quote! {
        impl cellarium::types::Cell for #struct_name {
            const UPDATE_SHADER: &'static str = #update_wgsl;
            const VIEW_SHADER: &'static str = #view_wgsl;
            const INIT_SHADER: &'static str = #init_wgsl;
            const HAS_INIT: bool = #has_init;
            const PARAM_NAMES: &'static [&'static str] = &[#(#param_name_lits),*];
            const PARAM_DEFAULTS: &'static [f32] = &[#(#param_defaults as f32),*];
            const TILE_SIZE: u32 = #tile_size_lit;
        }
    })
}
