use proc_macro::TokenStream;

mod layout;
mod parse;
mod check;
mod lower;

#[proc_macro_derive(CellState)]
pub fn derive_cell_state(input: TokenStream) -> TokenStream {
    layout::derive_cell_state_impl(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn cell(attr: TokenStream, item: TokenStream) -> TokenStream {
    parse::cell_impl(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
