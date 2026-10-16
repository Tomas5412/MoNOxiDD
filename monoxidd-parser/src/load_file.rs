use monoxidd::Circuit;
use crate::parser::parse;

use std::fmt;
use std::path::Path;

use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::ColorChoice;
use codespan_reporting::term::termcolor::{StandardStream, WriteColor};
use codespan_reporting::term::{Config, emit_to_write_style};
use nom::Offset;
use nom::error::{ContextError, ErrorKind, FromExternalError, ParseError};


struct ParserReport<I>(Vec<(I, ParserError)>);

enum ParserError {
    Nom(ErrorKind),
    Char(char),
    Context(&'static str),
    External(String),
}

impl<I> ParseError<I> for ParserReport<I> {
    fn from_error_kind(input: I, kind: ErrorKind) -> Self {
        ParserReport(vec![(input, ParserError::Nom(kind))])
    }

    fn append(input: I, kind: ErrorKind, mut other: Self) -> Self {
        other.0.push((input, ParserError::Nom(kind)));
        other
    }

    fn from_char(input: I, c: char) -> Self {
        ParserReport(vec![(input, ParserError::Char(c))])
    }
}

impl<I> ContextError<I> for ParserReport<I> {
    fn add_context(input: I, ctx: &'static str, mut other: Self) -> Self {
        match other.0[0].1 {
            ParserError::Context(_) => {}
            // Assume that the context is a better description
            _ => other.0.clear(),
        }
        other.0.push((input, ParserError::Context(ctx)));
        other
    }
}

impl<I, S: ToString> FromExternalError<I, S> for ParserReport<I> {
    fn from_external_error(input: I, _kind: ErrorKind, e: S) -> Self {
        Self(vec![(input, ParserError::External(e.to_string()))])
    }
}


pub fn parse_file<S: AsRef<str> + Clone + fmt::Display>(
    input: &[u8],
    file_id: S,
    writer: &mut dyn WriteColor,
    config: &Config,
) -> Option<Circuit> {
    let parse_result = parse::<ParserReport<_>>()(input);

    let errors = match parse_result {
        Ok((_, circuit)) => {
            return Some(circuit);
        }
        Err(nom::Err::Error(e) | nom::Err::Failure(e)) => e.0,
        Err(nom::Err::Incomplete(_)) => unreachable!("only using complete parsers"),
    };

    let range = move |span: &[u8]| {
        let offset = input.offset(span);
        let end = offset + span.len();
        if end >= input.len() {
            offset..offset
        } else {
            offset..end
        }
    };

    let mut labels = Vec::with_capacity(errors.len());
    let mut errors = errors.into_iter().map(|(span, err)| {
        (
            span,
            match err {
                ParserError::Nom(e) => format!("Expected {}", e.description()),
                ParserError::Char(c) => format!("Expected '{c}'"),
                ParserError::Context(msg) => msg.to_string(),
                ParserError::External(s) => s,
            },
        )
    });
    let (span, error) = errors.next().unwrap();
    labels.push(Label::primary((), range(span)).with_message(error.to_string()));
    for (span, error) in errors {
        labels.push(Label::secondary((), range(span)).with_message(error.to_string()));
    }

    let diagnostic = Diagnostic::error()
        .with_message("parsing failed")
        .with_labels(labels);

    let file = SimpleFile::new(file_id, String::from_utf8_lossy(input));
    emit_to_write_style(writer, config, &file, &diagnostic).ok();

    None
}


fn check_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        ext.as_encoded_bytes() == b"cnf"
     }
     else {
        false
     }
    
}

/// Load and parse the file at `path`, reporting errors to stderr
///
/// Returns `Some(problem)` on success, `None` on error.
pub fn load_file(path: impl AsRef<Path>) -> Option<Circuit> {
    let path = path.as_ref();

    if !check_extension(path) {
            eprintln!(
                "error: could not determine file type of '{}'",
                path.display()
            );
            return None;
        }

    let src = match std::fs::read(path) {
        Ok(src) => src,
        Err(err) => {
            eprintln!("error: could not read '{}' ({err})", path.display());
            return None;
        }
    };

    let config = codespan_reporting::term::Config::default();
    let writer = StandardStream::stderr(ColorChoice::Auto);
    let mut write_lock = writer.lock();

    parse_file(
        &src,
        path.to_string_lossy(),
        &mut write_lock,
        &config,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestError<'a> = nom::error::Error<&'a [u8]>;

    #[test]
    fn parses_my_file() {
        let input = std::fs::read("test/data/csd.dft.cnf").unwrap();
        let (rest, circuit) = parse::<TestError>()(&input).expect("parse failed");
        assert!(rest.is_empty());
        println!("{} clauses", circuit.num_clauses());
        println!("{:?}", circuit.get_clauses());
        for (value, weight) in &circuit.get_weights() {
            println!("{:?}", value);
            println!("{:?}",weight.get_weights());
        }
    }

}