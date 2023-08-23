use proc_macro2::{Span, TokenStream};
use quote::quote_spanned;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    span: Option<Span>,
    kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    Syn(syn::Error),
    Config(crate::config::ConfigError),
}

impl Error {
    pub fn no_span(kind: ErrorKind) -> Error {
        Error {
            span: None,
            kind,
        }
    }

    pub fn with_span(span: Span, kind: ErrorKind) -> Error {
        Error {
            span: Some(span),
            kind,
        }
    }

    pub fn to_compile_error(self) -> TokenStream {
        // TODO: Make this display, not debug
        let msg = format!("{:?}", self.kind);

        let span = self.span.unwrap_or(Span::mixed_site());

        quote_spanned!(span =>
            compile_error!(concat!("Couldn't generate chumsky parser: ", #msg));
        )
    }
}

impl From<syn::Error> for Error {
    fn from(err: syn::Error) -> Self {
        Error::no_span(ErrorKind::Syn(err))
    }
}

impl From<darling::Error> for Error {
    fn from(err: darling::Error) -> Self {
        Error::no_span(ErrorKind::Darling(err))
    }
}
