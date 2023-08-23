
use chumsky::Parser;
use chumsky_derive::Chumsky;

type Input<'a> = char;
type Error<'a> = chumsky::error::Simple<Input<'a>>;

#[derive(Debug)]
struct Ident(String);

impl Ident {
    fn parser<'a>() -> impl Parser<Input<'a>, Ident, Error = Error<'a>> + Clone {
        chumsky::text::ident()
            .map(|i| Ident(i))
    }
}

#[derive(Debug, Chumsky)]
struct Ast {
    a: Ident,
    #[chumsky(delimited_by("'('", "')'"))]
    b: Ident,
}

fn main() {
    dbg!(Ast::parser()
        .parse("a(b)")
        .unwrap());
}
