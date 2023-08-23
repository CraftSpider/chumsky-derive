use proc_macro::TokenStream;

mod utils;
mod error;
mod config;
mod chumsky_impl;

/// Derive a parser for the provided type. This will create a `parser`
/// method on your type, which returns the desired parser.
#[proc_macro_derive(Chumsky, attributes(chumsky))]
pub fn chumsky(item: TokenStream) -> TokenStream {
    chumsky_impl::chumsky(item.into()).into()
}
