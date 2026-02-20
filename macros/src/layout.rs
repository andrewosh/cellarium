use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Result, Type, spanned::Spanned};

struct FieldInfo {
    name: String,
    size: u32,
}

fn type_size(ty: &Type) -> std::result::Result<u32, syn::Error> {
    let path = match ty {
        Type::Path(tp) => tp,
        _ => return Err(syn::Error::new(ty.span(), format!(
            "cellarium C001: '{}' is not a GPU-compatible type. State fields must be f32, Vec2, Vec3, or Vec4.",
            quote!(#ty),
        ))),
    };
    let ident = path.path.segments.last()
        .ok_or_else(|| syn::Error::new(ty.span(), "cellarium C001: empty type path"))?
        .ident.to_string();
    match ident.as_str() {
        "f32" => Ok(1),
        "Vec2" => Ok(2),
        "Vec3" => Ok(3),
        "Vec4" => Ok(4),
        other => Err(syn::Error::new(ty.span(), format!(
            "cellarium C001: '{}' is not a GPU-compatible type. State fields must be f32, Vec2, Vec3, or Vec4.",
            other,
        ))),
    }
}

pub fn derive_cell_state_impl(input: TokenStream) -> Result<TokenStream> {
    let input: DeriveInput = syn::parse2(input)?;
    let struct_name = &input.ident;

    let fields = match &input.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(f) => &f.named,
            _ => return Err(syn::Error::new(input.ident.span(), "cellarium: CellState requires named fields")),
        },
        _ => return Err(syn::Error::new(input.ident.span(), "cellarium: CellState can only be derived on structs")),
    };

    let mut field_infos = Vec::new();
    let mut total_floats: u32 = 0;
    for f in fields.iter() {
        let name = f.ident.as_ref().unwrap().to_string();
        let size = type_size(&f.ty)?;
        total_floats += size;
        field_infos.push(FieldInfo { name, size });
    }

    if total_floats > 32 {
        return Err(syn::Error::new(input.ident.span(), format!(
            "cellarium C002: State exceeds maximum of 32 floats ({} declared). Reduce the number of state fields.",
            total_floats,
        )));
    }

    // Greedy bin-packing into RGBA (4-channel) textures
    let mut mappings: Vec<(String, u32, u32, u32)> = Vec::new(); // (name, tex, offset, size)
    let mut current_tex: u32 = 0;
    let mut current_offset: u32 = 0;

    for fi in &field_infos {
        if current_offset + fi.size > 4 {
            current_tex += 1;
            current_offset = 0;
        }
        mappings.push((fi.name.clone(), current_tex, current_offset, fi.size));
        current_offset += fi.size;
    }

    let texture_count = if mappings.is_empty() { 0 } else { current_tex + 1 };

    let mapping_tokens: Vec<TokenStream> = mappings.iter().map(|(name, tex, off, sz)| {
        quote! {
            cellarium::types::FieldMapping {
                name: #name,
                texture: #tex,
                offset: #off,
                size: #sz,
            }
        }
    }).collect();

    let tex_count_lit = texture_count;
    let num_textures = texture_count as usize;

    // Generate defaults by constructing a Default instance and reading its fields
    let field_names: Vec<&syn::Ident> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();
    let field_sizes: Vec<u32> = field_infos.iter().map(|fi| fi.size).collect();

    // Build code that writes field values into the texture arrays
    let mut default_writes = Vec::new();
    for (i, ((name, tex, off, _sz), size)) in mappings.iter().zip(field_sizes.iter()).enumerate() {
        let field_ident = &field_names[i];
        let tex_idx = *tex as usize;
        let off_idx = *off as usize;
        let _ = name; // used in mapping_tokens above
        match size {
            1 => {
                default_writes.push(quote! {
                    textures[#tex_idx][#off_idx] = defaults.#field_ident;
                });
            }
            2 => {
                default_writes.push(quote! {
                    textures[#tex_idx][#off_idx] = defaults.#field_ident.x;
                    textures[#tex_idx][#off_idx + 1] = defaults.#field_ident.y;
                });
            }
            3 => {
                default_writes.push(quote! {
                    textures[#tex_idx][#off_idx] = defaults.#field_ident.x;
                    textures[#tex_idx][#off_idx + 1] = defaults.#field_ident.y;
                    textures[#tex_idx][#off_idx + 2] = defaults.#field_ident.z;
                });
            }
            4 => {
                default_writes.push(quote! {
                    textures[#tex_idx][#off_idx] = defaults.#field_ident.x;
                    textures[#tex_idx][#off_idx + 1] = defaults.#field_ident.y;
                    textures[#tex_idx][#off_idx + 2] = defaults.#field_ident.z;
                    textures[#tex_idx][#off_idx + 3] = defaults.#field_ident.w;
                });
            }
            _ => unreachable!(),
        }
    }

    Ok(quote! {
        impl cellarium::types::CellState for #struct_name {
            const TEXTURE_COUNT: u32 = #tex_count_lit;
            const FIELD_LAYOUT: &'static [cellarium::types::FieldMapping] = &[
                #(#mapping_tokens),*
            ];

            fn defaults() -> Vec<[f32; 4]> {
                let defaults = <#struct_name as Default>::default();
                let mut textures = vec![[0.0f32; 4]; #num_textures];
                #(#default_writes)*
                textures
            }
        }
    })
}
