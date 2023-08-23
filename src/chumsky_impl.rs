use quote::{quote, quote_spanned};
use proc_macro2::{Span, TokenStream, Ident};
use syn::spanned::Spanned;
use syn::{Attribute, Data, DataEnum, DataStruct, DeriveInput, Field, Index, NestedMeta, Token, Variant};
use syn::punctuated::Punctuated;

use crate::config::ItemConfig;
use crate::error::{Error, ErrorKind, Result};

fn parse_attribute_args(toks: TokenStream) -> Result<Vec<NestedMeta>> {
    use syn::parse::Parse;
    use syn::parse::ParseStream;

    struct Foo(Punctuated<NestedMeta, Token![,]>);

    impl Parse for Foo {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let content;
            syn::parenthesized!(content in input);
            Punctuated::parse_terminated(&content).map(Foo)
        }
    }

    let span = toks.span();

    syn::parse2::<Foo>(toks)
        .map(|f| f.0.into_iter().collect::<Vec<_>>())
        .map_err(|e| Error::with_span(span, ErrorKind::Syn(e)))
}

fn parser_attrs(attrs: &[Attribute]) -> Result<ParserConfig> {
    let attr = attrs.iter()
        .find(|a| {
            a.path.segments.len() == 1 && a.path.segments[0].ident == "chumsky"
        });

    let span = attr.span();

    println!("Raw Attribute: {:?}", attr);

    let attr = match attr {
        Some(a) => parse_attribute_args(a.tokens.clone()),
        None => return Ok(ParserConfig::default()),
    }?;

    println!("MetaList: {:?}", attr);

    ParserConfig::from_list(&attr.into_iter().collect::<Vec<_>>())
        .map_err(|e| Error::with_span(span, ErrorKind::Darling(e)))
}

fn add_config(mut parser: TokenStream, config: ParserConfig) -> TokenStream {
    if let Some(sep) = &config.seperated_by {
        parser = quote!(#parser.seperated_by(::chumsky::primitive::just(#sep)))
    }

    if let Some([a, b]) = config.delimited_by.as_deref() {
        parser = quote!(#parser.delimited_by(::chumsky::primitive::just(#a), ::chumsky::primitive::just(#b)));
    }

    if let Some(padded) = &config.padded {
        match padded {
            darling::util::Override::Explicit(e) => {
                parser = quote!(#e().ignore_then(#parser).then_ignore(#e()))
            }
            darling::util::Override::Inherit => {
                parser = quote!(#parser.padded())
            }
        }
    }

    parser
}

fn parser_for_field((idx, field): (usize, &Field)) -> Result<TokenStream> {
    let config = parser_attrs(&field.attrs)?;

    let ty = &field.ty;

    let parser = quote!(#ty::parser());

    let parser = add_config(parser, config);

    if idx == 0 {
        Ok(parser)
    } else {
        Ok(quote!(.then(#parser)))
    }
}

fn item_map<'a>(parser: TokenStream, item: TokenStream, fields: impl Iterator<Item = &'a Field> + Clone) -> TokenStream {
    let map_input = fields
        .clone()
        .enumerate()
        .fold(TokenStream::new(), |acc, (idx, _)| {
            let ident = Ident::new(&format!("arg{}", idx), Span::call_site());
            if acc.is_empty() {
                quote!(#ident)
            } else {
                quote!((#ident, #acc))
            }
        });

    let (field_names, args): (Vec<_>, Vec<_>) = fields
        .enumerate()
        .map(|(idx, field)| {
            let field_name = match &field.ident {
                Some(ident) => quote!(#ident),
                None => {
                    let idx = Index { index: idx as u32, span: Span::call_site() };
                    quote!(#idx)
                },
            };
            (
                field_name,
                Ident::new(&format!("arg{}", idx), Span::call_site())
            )
        })
        .unzip();

    quote!(
        #parser.map(|#map_input| #item { #( #field_names: #args ),* })
    )
}

fn parser_for_variant((idx, variant): (usize, &Variant)) -> Result<TokenStream> {
    let config = parser_attrs(&variant.attrs)?;

    let ident = &variant.ident;

    let parser = variant.fields.iter()
        .enumerate()
        .map(parser_for_field)
        .collect::<Result<TokenStream>>()?;

    let parser = add_config(parser, config);

    let parser = item_map(parser, quote!(Self::#ident), variant.fields.iter());

    let parser = if idx == 0 {
        parser
    } else {
        quote!(.or(#parser))
    };

    Ok(parser)
}

fn gen_struct(s: DataStruct, attrs: &[Attribute]) -> Result<TokenStream> {
    let config = parser_attrs(attrs)?;

    let parser = s.fields.iter()
        .enumerate()
        .map(parser_for_field)
        .collect::<Result<TokenStream>>()?;

    let parser = item_map(parser, quote!(Self), s.fields.iter());

    Ok(add_config(parser, config))
}

fn gen_enum(e: DataEnum, attrs: &[Attribute]) -> Result<TokenStream> {
    let config = parser_attrs(attrs)?;

    let parser = e.variants.iter()
        .enumerate()
        .map(parser_for_variant)
        .collect::<Result<TokenStream>>()?;

    Ok(add_config(parser, config))
}

pub fn chumsky(item: TokenStream) -> TokenStream {
    let item_span = item.span();
    let item: DeriveInput = match syn::parse2(item) {
        Ok(item) => item,
        Err(_) => return quote_spanned!(item_span => compile_error!("Couldn't parse macro input");),
    };

    let ident = &item.ident;

    let parser_impl = match item.data {
        Data::Struct(s) => gen_struct(s, &item.attrs),
        Data::Enum(e) => gen_enum(e, &item.attrs),
        Data::Union(_) => return quote_spanned!(
            item.ident.span() => compile_error!("Chumsky parser derivation doesn't support unions");
        ),
    };

    let parser_impl = match parser_impl {
        Ok(i) => i,
        Err(e) => {
            return e.to_compile_error();
        },
    };

    quote!(
        impl #ident {
            fn parser<'a>() -> impl Parser<Input<'a>, Self, Error = Error<'a>> {
                use ::chumsky::Parser;
                use ::chumsky::text::TextParser;

                #parser_impl
            }
        }
    )
}
