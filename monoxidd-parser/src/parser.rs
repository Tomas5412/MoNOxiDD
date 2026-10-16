use nom::character::complete::{char, line_ending, multispace0, space0, space1, u64};
use nom::combinator::{consumed, cut, eof, value, iterator, map, recognize};
use nom::error::{ContextError, ErrorKind, FromExternalError, ParseError, context};
use nom::multi::many0_count;
use nom::sequence::{preceded};
use nom::{Err, IResult};
use nom::branch::alt;
use monoxidd::{Circuit, Literal, VarSet};
use crate::util::{
    self, MAX_CAPACITY, context_loc, fail, line_span, word, word_span, weight_line
};


/// Checks if the format is correctly cnf
fn cnf_format<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
    input: &'a [u8],
) -> IResult<&'a [u8], &'a [u8], E> {
    let inner = |input: &'a [u8]| match input {
        [b'c', b'n', b'f', r @ ..] => Ok((r, &input[..3])),
        _ => Err(Err::Error(E::from_error_kind(input, ErrorKind::Alt))),
    };
    context_loc(|| word_span(input), "format must be 'cnf'", word(inner))(input)
}



/// Parses a problem line, i.e., `p cnf <#vars> <#clauses>`
///
/// Returns the number of vars and the number of
/// clauses together with their spans.
fn problem_line<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
    input: &'a [u8],
) -> IResult<&'a [u8], ((&'a [u8], usize), (&'a [u8], usize)), E> {
    let inner = |input| {
        let (input, _) = context(
            "all lines in the preamble must begin with 'c' or 'p'",
            cut(char('p')),
        )(input)?;
        let (input, _) = space1(input)?;
        let (input, _) = consumed(cnf_format)(input)?;
        let (input, _) = space1(input)?;
        let (input, num_vars) = consumed(u64)(input)?;
        if num_vars.1 > MAX_CAPACITY {
            return fail(num_vars.0, "too many variables");
        }
        let num_vars = (num_vars.0, num_vars.1 as usize);

        let msg = "expected the number of clauses (CNF format)";
        let (input, _) = context(msg, space1)(input)?;
        let (input, num_clauses) = context_loc(|| word_span(input), msg, consumed(u64))(input)?;
        if num_clauses.1 > MAX_CAPACITY {
                return fail(num_clauses.0, "too many clauses");
        }
        let num_clauses = (num_clauses.0, num_clauses.1 as usize);
        let (input, _) = space0(input)?;
        value((num_vars, num_clauses), line_ending)(input)
    };

    context_loc(
        || line_span(input),
        "problem line must have format 'p <format> <#vars> [<#clauses>]'",
        cut(inner),
    )(input)
}


#[derive(Clone, PartialEq, Eq, Debug)]
struct Preamble {
    vars: VarSet,
    num_clauses: usize,
    // clause_tree: Option<Tree<usize>>,

}

/// Parses the preamble, i.e., all `c` and `p` lines at the beginning of the
/// file
fn preamble<'a, E>() -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Preamble, E>
where
    E: ParseError<&'a [u8]> + ContextError<&'a [u8]> + FromExternalError<&'a [u8], String>,
{
    move |mut input| {
            let (input, (num_vars, num_clauses)) =
                preceded(many0_count(util::comment), problem_line)(input)?;
            let preamble = Preamble {
                vars: VarSet::new(num_vars.1),
                num_clauses: num_clauses.1,
            };
            Ok((input, preamble))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
    enum TokenKind {
        Int(u64),
        Neg,
        Weight(i64, f64)
    }

#[derive(Clone, PartialEq, Debug)]
    struct Token<'a> {
        span: &'a [u8],
        kind: TokenKind,
    }

fn lex<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
    input: &'a [u8],
) -> IResult<&'a [u8], Token<'a>, E> {
    let tok = alt((
        map(consumed(weight_line), |(span, (lit, w))| Token {
            span,
            kind: TokenKind::Weight(lit, w),
        }),
        map(consumed(u64), |(span, n)| Token {
            span,
            kind: TokenKind::Int(n),
        }),
        map(recognize(char('-')), |span| Token {
            span,
            kind: TokenKind::Neg,
        }),
    ));

    preceded(multispace0, tok)(input)
}

pub fn parse_input<'a, E>(
        preamble: Preamble,
    ) -> impl FnOnce(&'a [u8]) -> IResult<&'a [u8], Circuit, E>
    where
        E: ParseError<&'a [u8]> + ContextError<&'a [u8]> + FromExternalError<&'a [u8], String>,
    {
        move |input| {
            let Preamble {
                vars,
                num_clauses,
                ..
            } = preamble;
            let num_vars = vars.len();
            let mut circuit = Circuit::new(vars);
            circuit.add_clause();

            let mut neg = false;

            let mut it = iterator(input, lex::<E>);
            for token in &mut it {
                match token.kind {
                    TokenKind::Int(0) => {
                        circuit.add_clause();
                    }
                    TokenKind::Int(n) => {
                        if n > num_vars as u64 {
                            return fail(token.span, "variables must be in range [1, #vars]");
                        }
                        circuit.add_literal(Literal::from_input(neg, (n - 1) as usize));
                        neg = false;
                    }
                    TokenKind::Neg if !neg => neg = true,
                    TokenKind::Neg => return fail(token.span, "expected a variable"),
                    TokenKind::Weight(lit, w) => {
                        let var = lit.unsigned_abs();
                        if lit == 0 || var > num_vars as u64 {
                            return fail(token.span, "weight literal must be in range [1, #vars]");
                        }
                        circuit.add_weight(Literal::from_input(lit < 0, (var - 1) as usize), w);
                    }
                }
            }

            let (input, ()) = it.finish()?;
            let (input, _) = multispace0(input)?;
            let (input, _) = context("expected a literal or '0'", eof)(input)?;

            // The last clause may or may not be terminated by 0. If it was, `add_clause()`
            // created an extra, empty clause at the end; drop it before counting.
            if circuit.last_clause().is_some_and(|c| c.is_empty()) {
                circuit.pop_clause();
            }
            
            let num_found = circuit.num_clauses();
            if num_found != num_clauses {
                return Err(Err::Failure(E::from_external_error(
                    input,
                    ErrorKind::Fail,
                    format!("expected {num_clauses} clauses, got {num_found}"),
                )));
            }
            let num_gates = circuit.num_clauses(); // may have been decremented above

            Ok((input, circuit))
        }
    }

pub fn parse<'a, E>() -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Circuit, E>
where
    E: ParseError<&'a [u8]> + ContextError<&'a [u8]> + FromExternalError<&'a [u8], String>,
{
    move |input| {
        let (input, preamble) = preamble()(input)?;
        parse_input(preamble)(input)
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    type Err<'a> = nom::error::Error<&'a [u8]>;

    #[test]
    fn parses_clauses_and_weights() {
        let input = b"c hi\np cnf 3 2\nc p weight 1 0.1 0\nc p weight -1 0.9 0\n1 -2 0\n2 3 0\n";
        let (rest, circuit) = parse::<Err>()(input).expect("parse failed");
        assert!(rest.is_empty());
        assert_eq!(circuit.num_clauses(), 2);
        assert_eq!(circuit.num_weights(),3);
        assert_eq!(circuit.get_weights()[&0].get_weights(), (Some(0.9),Some(0.1)));
        assert_eq!(circuit.get_weights()[&1].get_weights(), (None, None));
        assert_eq!(circuit.get_weights()[&2].get_weights(), (None, None));
    }

    #[test]
    fn rejects_out_of_range_variable() {
        let input = b"p cnf 3 1\n5 0\n";
        assert!(parse::<Err>()(input).is_err());
    }

    #[test]
    fn rejects_wrong_clause_count() {
        let input = b"p cnf 3 2\n1 0\n";
        assert!(parse::<Err>()(input).is_err());
    }

    #[test]
    fn accepts_terminated_last_clause() {
        assert!(parse::<Err>()(b"p cnf 3 2\n1 0\n2 0\n").is_ok());
    }
    
    #[test]
    fn accepts_unterminated_last_clause() {
        assert!(parse::<Err>()(b"p cnf 3 2\n1 0\n2").is_ok());
    }
    
    #[test]
    fn rejects_too_many_clauses() {
        assert!(parse::<Err>()(b"p cnf 3 1\n1 0\n2 0\n").is_err());
    }
    
    #[test]
    fn accepts_zero_clauses() {
        assert!(parse::<Err>()(b"p cnf 3 0\n").is_ok());
    }
}