//! parser helping functions

use std::mem::size_of;
use std::ops::{Range, RangeFrom, RangeTo};
use nom::bytes::complete::tag;
use nom::character::complete::{char, i64 as signed, line_ending, space0, space1, u64};
use nom::number::complete::double;
use nom::branch::alt;
use nom::combinator::{cut, value, not, eof};
use nom::error::{ContextError, ErrorKind, ParseError, context};
use nom::sequence::preceded;
use nom::{
    AsBytes, AsChar, Compare, Err, IResult, InputIter, InputLength, InputTakeAtPosition, Parser,
    Slice,
};


pub const MAX_CAPACITY: u64 = (usize::MAX / 2 / size_of::<usize>()) as u64;

#[warn(unused)] //TODO: Remove?
pub fn usize<I, E>(input: I) -> IResult<I, usize, E>
where
    I: InputIter + Slice<RangeFrom<usize>> + Slice<RangeTo<usize>> + InputLength + AsBytes + Clone,
    I::Item: AsChar,
    E: ParseError<I> + ContextError<I>,
{
    let (remaining, res) = u64(input.clone())?;
    if res > MAX_CAPACITY {
        let span = word_span(input);
        fail(span, "number too large")
    } else {
        Ok((remaining, res as usize))
    }
}


#[warn(unused)] //TODO: Remove?
pub fn eol<I, E>(input: I) -> IResult<I, (), E>
where
    I: Slice<Range<usize>>
        + Slice<RangeFrom<usize>>
        + Slice<RangeTo<usize>>
        + InputIter
        + InputLength
        + InputTakeAtPosition
        + Compare<&'static str>,
    <I as InputTakeAtPosition>::Item: AsChar + Clone,
    E: ParseError<I>,
{
    preceded(space0, value((), line_ending))(input)
}

pub fn word<I, O, E, F>(mut parser: F) -> impl FnMut(I) -> IResult<I, O, E>
where
    I: Clone + Slice<RangeTo<usize>> + AsBytes,
    E: ParseError<I>,
    F: Parser<I, O, E>,
{
    move |i1| {
        let (i2, o) = parser.parse(i1.clone())?;
        match i2.as_bytes().first() {
            Some(c) if !c.is_ascii_alphanumeric() => Ok((i2, o)),
            None => Ok((i2, o)),
            _ => Err(Err::Error(E::from_error_kind(i1, ErrorKind::Fail))),
        }
    }
}

pub fn word_span<I: Slice<RangeTo<usize>> + AsBytes>(input: I) -> I {
    let bytes = input.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => return input.slice(..i),
            _ => {}
        }
    }
    input
}

#[inline]
pub fn line_span<I: Slice<RangeTo<usize>> + AsBytes>(input: I) -> I {
    match memchr::memchr2(b'\n', b'\r', input.as_bytes()) {
        Some(i) => input.slice(..i),
        None => input,
    }
}

pub fn context_loc<I, E, F, O>(
    span: impl Fn() -> I,
    msg: &'static str,
    mut f: F,
) -> impl FnMut(I) -> IResult<I, O, E>
where
    E: ContextError<I>,
    F: Parser<I, O, E>,
{
    move |input| match f.parse(input) {
        Ok(o) => Ok(o),
        Err(Err::Incomplete(i)) => Err(Err::Incomplete(i)),
        Err(Err::Error(e)) => Err(Err::Error(E::add_context(span(), msg, e))),
        Err(Err::Failure(e)) => Err(Err::Failure(E::add_context(span(), msg, e))),
    }
}

#[inline]
pub fn fail<I: Clone, O, E: ParseError<I> + ContextError<I>>(
    span: I,
    msg: &'static str,
) -> IResult<I, O, E> {
    Err(Err::Failure(E::add_context(
        span.clone(),
        msg,
        E::from_error_kind(span, ErrorKind::Fail),
    )))
}

#[warn(unused)] //TODO: Remove?
pub fn fail_with_contexts<I, O, E, It>(ctxs: It) -> IResult<I, O, E>
where
    I: Clone,
    E: ParseError<I> + ContextError<I>,
    It: IntoIterator<Item = (I, &'static str)>,
{
    let mut iter = ctxs.into_iter();
    let (span, msg) = iter.next().expect("At least one context required");
    let mut err = E::add_context(span.clone(), msg, E::from_error_kind(span, ErrorKind::Fail));
    for (span, msg) in iter {
        err = E::add_context(span, msg, err);
    }
    Err(Err::Failure(err))
}



// A comment line starting with 'c'. Consumes everything until the next '\n'
// (inclusive).
// Does not remove lines starting with the "c p " prefix.
pub fn comment<'a, E: ParseError<&'a [u8]>>(input: &'a [u8]) -> IResult<&'a [u8], (), E> {
    let (input, _) = not(tag("c p "))(input)?;
    let input = char('c')(input)?.0;
    let input = match memchr::memchr(b'\n', input) {
        Some(i) => &input[i + 1..],
        None => &input[input.len()..],
    };
    Ok((input, ()))
}



// A line starting with 'c p weight'
pub fn weight_line<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
    input: &'a [u8],
) -> IResult<&'a [u8], (i64, f64), E> {
    // Only the prefix is allowed to fail softly, so other `c p ...` lines can fall through.
    let (input, _) = tag("c p weight")(input)?;

    let inner = |input| {
        let (input, _) = space1(input)?;
        let (input, lit) = signed(input)?;
        let (input, _) = space1(input)?;
        let (input, w) = double(input)?;
        let (input, _) = space1(input)?;
        let (input, _) = tag("0")(input)?;
        let (input, _) = space0(input)?;
        let (input, _) = alt((line_ending, eof))(input)?;
        Ok((input, (lit, w)))
    };

    context(
        "weight line must have format 'c p weight <literal> <weight> 0'",
        cut(inner),
    )(input)
}