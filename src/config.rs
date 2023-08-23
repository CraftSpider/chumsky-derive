use proc_macro2::{Ident, TokenStream};
use syn::{Expr, Lit, LitStr, parenthesized, Path, Token};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

use crate::utils::MetaArray;

trait FromMetaVal {
    fn from_val(val: &MetaValue) -> Result<Self>;
}

impl FromMetaVal for MetaValue {
    fn from_val(val: &MetaValue) -> Result<Self> {
        Ok(val.clone())
    }
}

impl FromMetaVal for Expr {
    fn from_val(val: &MetaValue) -> Result<Self> {
        match val {
            MetaValue::NameVal(nv) => Ok(nv.value.clone()),
            _ => Err(ConfigError::WrongType)
        }
    }
}

impl<const N: usize> FromMetaVal for [Expr; N] {
    fn from_val(val: &MetaValue) -> Result<Self> {
        match val {
            MetaValue::ListVal(nl) => {
                if nl.value.len() == N {
                    Ok(nl.value.iter()
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap())
                } else {
                    Err(ConfigError::WrongNumber)
                }
            }
            _ => Err(ConfigError::WrongType)
        }
    }
}

impl FromMetaVal for Override {
    fn from_val(val: &MetaValue) -> Result<Self> {
        match val {
            MetaValue::Ident(_) => Ok(Override::Flag),
            MetaValue::NameVal(nv) => Ok(Override::Value(nv.value.clone())),
        }
    }
}

impl<T: FromMetaVal> FromMetaVal for Option<T> {
    fn from_val(val: &MetaValue) -> Result<Self> {
        match T::from_val(val) {
            Ok(ret) => Ok(Some(ret)),
            Err(ConfigError::DoesntExist) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

type Result<T> = core::result::Result<T, ConfigError>;

pub enum ConfigError {
    DoesntExist,
    WrongType,
    WrongNumber,
}

enum Override {
    NotPresent,
    Flag,
    Value(Expr),
}

/// A name followed by a value in a macro, such as `seperated_by = a.b`
#[derive(Clone, Debug)]
pub struct NameValue {
    name: Ident,
    _eq: Token![=],
    value: Expr,
}

impl Parse for NameValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(NameValue {
            name: Parse::parse(input)?,
            _eq: Parse::parse(input)?,
            value: Parse::parse(input)?,
        })
    }
}

/// A name followed by a list in a macro, such as `bounds(Clone, Debug)`
#[derive(Clone, Debug)]
pub struct NameList {
    name: Ident,
    _paren: syn::token::Paren,
    value: Punctuated<Expr, Token![,]>,
}

impl Parse for NameList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;

        Ok(NameList {
            name: Parse::parse(input)?,
            _paren: parenthesized!(content in input),
            value: Punctuated::parse_terminated(&content)?,
        })
    }
}

/// A list value in a macro
#[derive(Clone, Debug)]
pub enum ListMetaValue {
    Lit(Lit),
    Path(Path),
    Expr(Expr),
}

impl Parse for ListMetaValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        use syn::parse::discouraged::Speculative;

        let fork = input.fork();
        if let Ok(lit) = fork.parse::<Lit>() {
            input.advance_to(&fork);
            return Ok(ListMetaValue::Lit(lit));
        }

        let fork = input.fork();
        if let Ok(path) = input.fork().parse::<Path>() {
            input.advance_to(&fork);
            Ok(ListMetaValue::Path(path))
        } else {
            Ok(ListMetaValue::Expr(Parse::parse(input)?))
        }
    }
}

/// A meta value in an attribute
#[derive(Clone, Debug)]
pub enum MetaValue {
    /// An identifier, such as `bar`
    Ident(Ident),
    /// A name and a value, such as `foo = blah`
    NameVal(NameValue),
    /// A name and a list of values, such as `delimited_by("a", "b")`
    ListVal(NameList),
}

impl Parse for MetaValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(if input.peek2(Token![=]) {
            MetaValue::NameVal(Parse::parse(input)?)
        } else if input.peek2(syn::token::Paren) {
            MetaValue::ListVal(Parse::parse(input)?)
        } else {
            MetaValue::Ident(Parse::parse(input)?)
        })
    }
}

struct AttrMeta {
    _paren: Option<syn::token::Paren>,
    metas: Punctuated<MetaValue, Token![,]>,
}

impl AttrMeta {
    fn get_name(&self, name: &str) -> Option<&MetaValue> {
        self.metas.iter()
            .find(|m| match m {
                MetaValue::Ident(i) => i == name,
                MetaValue::NameVal(nv) => nv.name == name,
                MetaValue::ListVal(lv) => lv.name == name,
            })
    }

    fn get_flag(&self, name: &str) -> Result<bool> {
        let m = match self.get_name(name) {
            Some(m) => m,
            None => return Ok(false),
        };

        match m {
            MetaValue::Ident(_) => Ok(true),
            _ => Err(ConfigError::WrongType),
        }
    }

    fn get_value(&self, name: &str) -> Result<&Expr> {
        let m = self.get_name(name).ok_or(ConfigError::DoesntExist)?;

        match m {
            MetaValue::NameVal(nv) => Ok(&nv.value),
            _ => Err(ConfigError::WrongType)
        }
    }

    fn get_array<const N: usize>(&self, name: &str) -> Result<[&Expr; N]> {
        let m = self.get_name(name).ok_or(ConfigError::DoesntExist)?;

        match m {
            MetaValue::ListVal(nl) => {
                if nl.value.len() == N {
                    Ok(nl.value.iter()
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap())
                } else {
                    Err(ConfigError::WrongNumber)
                }
            }
            _ => Err(ConfigError::WrongType)
        }
    }
}

impl Parse for AttrMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        let paren = if input.peek(syn::token::Paren) {
            Some(parenthesized!(content in input))
        } else {
            content = input.fork();
            None
        };

        Ok(AttrMeta {
            _paren: paren,
            metas: Punctuated::parse_terminated(&content)?,
        })
    }
}

#[derive(Default)]
pub struct GenericConfig {
    pub delimited_by: Option<[Expr; 2]>,
    pub seperated_by: Option<Expr>,
    pub padded: Option<Path>,
}

impl GenericConfig {
    fn from_attr_meta(attrs: &AttrMeta) -> Result<GenericConfig> {
        let delimited_by = match attrs.get_array("delimited_by") {
            Ok(a) => Some(a.map(Expr::clone)),
            Err(ConfigError::DoesntExist) => None,
            Err(e) => return Err(e),
        };

        let seperated_by = match attrs.get_value("seperated_by") {
            Ok(a) => Some(a.clone()),
            Err(ConfigError::DoesntExist) => None,
            Err(e) => return Err(e),
        };

        let padded = match attrs.get_override("padded") {
            Some(MetaValue::Ident(i)) if i == "padded" => Some()
            None => None,
        };

        Ok(GenericConfig {
            delimited_by,
            seperated_by,
            padded,
        })
    }
}

#[derive(Default)]
pub struct ItemConfig {
    pub generic: GenericConfig,
    pub bounds: Vec<Path>,
}

impl ItemConfig {
    fn from_tokens(tokens: TokenStream) -> Result<ItemConfig> {
        let meta = syn::parse2::<AttrMeta>(tokens)?;
        Ok(ItemConfig {
            generic: GenericConfig::from_meta(meta.metas)?,
            bounds: Vec::<Path>::from_meta(meta.metas)?,
        })
    }
}
