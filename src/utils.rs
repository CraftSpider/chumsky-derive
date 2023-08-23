use std::ops;
use darling::{Error, FromMeta};
use syn::{Lit, ExprArray, Expr, NestedMeta};

pub struct MetaArray<const N: usize>([Expr; N]);

impl<const N: usize> FromMeta for MetaArray<N> {
    fn from_value(value: &Lit) -> darling::Result<Self> {
        if let Lit::Str(str) = value {
            let array = str.parse::<ExprArray>()?;
            <[Expr; N]>::try_from(array.elems.into_iter().collect::<Vec<_>>())
                .map(MetaArray)
                .map_err(|v| if v.len() > N {
                    Error::too_many_items(v.len())
                } else {
                    Error::too_few_items(v.len())
                })
        } else {
            Err(Error::unexpected_lit_type(value))
        }
    }

    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        let v = items.iter()
            .map(|meta| if let NestedMeta::Lit(Lit::Str(str)) = meta {
                Ok(str.parse::<Expr>()?)
            } else {
                Err(Error::unexpected_type("NestedMeta::Meta"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        <[Expr; N]>::try_from(v)
            .map(MetaArray)
            .map_err(|v| if v.len() > N {
                Error::too_many_items(v.len())
            } else {
                Error::too_few_items(v.len())
            })
    }
}

impl<const N: usize> ops::Deref for MetaArray<N> {
    type Target = [Expr; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
