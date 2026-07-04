#![no_std]

extern crate alloc;

use ezscn_ast::*;
use ezscn_ast::expression::Expression;
use ezscn_ast::statement::IdentifierOrUnderscore;
use ezscn_error::{ParseError, ParseErrorKind};
use ezscn_lexer::TokenStream;
use ezscn_tokens::{Span, SpanImpl, Spanned, Token, TokenKind};
use thin_vec::{thin_vec, ThinVec};

pub mod expression;
pub mod items;
pub mod statement;
pub(crate) mod string;

#[derive(Debug)]
pub struct EndLineInformation {
    pub line: usize,
    pub len: usize,
}

impl EndLineInformation {
    #[inline]
    pub const fn new(line: usize, len: usize) -> Self {
        EndLineInformation { line, len }
    }

    #[inline]
    pub const fn line(&self) -> usize {
        self.line
    }

    #[inline]
    pub const fn input_len(&self) -> usize {
        self.len
    }
}

#[derive(Debug)]
pub struct Parser<'t> {
    pub(crate) token_stream: TokenStream<'t>,
    pub(crate) input: &'t str,
    pub(crate) errors: ThinVec<ParseError>,
}

impl<'t> From<&'t str> for Parser<'t> {
    #[inline]
    fn from(input: &'t str) -> Self {
        let token_stream = TokenStream::from(input);

        Parser { token_stream, input, errors: thin_vec![] }
    }
}

impl<'t> Parser<'t> {
    #[inline]
    pub fn new(input: &'t str) -> Self {
        Self::from(input)
    }

    #[inline]
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    #[inline]
    pub fn reached_eof(&mut self) -> bool {
        self.token_stream.reached_eof()
    }

    #[inline]
    pub fn expression(&mut self) -> Option<Expression<'t>> {
        expression::expression(self)
    }

    #[inline]
    pub fn advance_until_path(&mut self) -> Option<Path<'t>> {
        let first_identifier = self.advance_until_identifier_spanned()?;
        let mut identifiers = thin_vec![first_identifier.data];
        let mut last_span = first_identifier.span;

        while self.next_if_kind(TokenKind::ColonColon).is_some() {
            let identifier = self.advance_until_identifier_spanned()?;
            identifiers.push(identifier.data);
            last_span = identifier.span;
        }

        let span = Span::merge(first_identifier.span, last_span);
        Some(Path { identifiers, span })
    }

    #[inline]
    pub fn advance_until_identifier(&mut self) -> Option<&'t str> {
        let token = self.advance_until_kind(TokenKind::Identifier)?;
        Some(&self.input[token.span])
    }

    #[inline]
    pub fn advance_until_identifier_or_underscore(&mut self) -> Option<IdentifierOrUnderscore<'t>> {
        self.advance_until_identifier_or_underscore_spanned().map(|t| t.data)
    }

    #[inline]
    pub fn advance_until_identifier_spanned(&mut self) -> Option<Spanned<&'t str>> {
        let token = self.advance_until_kind(TokenKind::Identifier)?;
        let span = token.span;

        Some(Spanned::new(&self.input[token.span], span))
    }

    #[inline]
    pub fn advance_until_identifier_or_underscore_spanned(&mut self) -> Option<Spanned<IdentifierOrUnderscore<'t>>> {
        self.advance_map(|t| {
            match t {
                Ok(Token { kind: TokenKind::Identifier, span, .. }) => Ok(Spanned::new(IdentifierOrUnderscore::Identifier(&self.input[span]), span)),
                Ok(Token { kind: TokenKind::Underscore, span, .. }) => Ok(Spanned::new(IdentifierOrUnderscore::Underscore, span)),
                Ok(Token { kind, span, line }) => Err(ParseError::new(ParseErrorKind::InvalidToken(TokenKind::Identifier, kind), span, line)),
                Err(EndLineInformation { line, len }) => {
                    let kind = ParseErrorKind::ExpectedToken(TokenKind::Identifier);
                    let span = Span::empty_from_start(len);
                    Err(ParseError { kind, span, line })
                }
            }
        })
    }

    pub(crate) fn comma_seperated_map<T>(&mut self, end_kind: TokenKind, f: impl Fn(&mut Self) -> Option<T>) -> Option<ThinVec<T>> {
        let mut items = thin_vec![];
        loop {
            if self.is_next(end_kind) {
                break
            }

            items.push((f)(self)?);

            if !self.is_next(end_kind) {
                self.advance_until_kind(TokenKind::Comma)?;
            }
        }

        Some(items)
    }

    #[inline]
    pub(crate) fn error(&mut self, error: ParseError) {
        self.errors.push(error)
    }

    #[inline]
    pub(crate) fn peek(&mut self) -> Option<&Token> {
        self.token_stream.peek()
    }

    #[inline]
    pub(crate) fn next(&mut self) -> Option<Token> {
        self.token_stream.next()
    }

    #[inline]
    pub(crate) fn line(&self) -> usize {
        self.token_stream.line()
    }

    #[inline]
    pub(crate) fn is_next(&mut self, kind: TokenKind) -> bool {
        self.token_stream.is_next(kind)
    }

    pub(crate) fn next_if_map<T>(&mut self, f: impl FnOnce(Option<Token>) -> Result<T, Option<Token>>) -> Option<T> {
        self.token_stream.next_if_map(f)
    }

    pub(crate) fn next_if_map_errored<T>(&mut self, f: impl FnOnce(Result<Token, EndLineInformation>) -> Result<T, ParseError>) -> Option<T> {
        let line = self.line();
        let len = self.input.len();
        let errors = &mut self.errors;
        self.token_stream.next_if_map(|token| {
            match (f)(token.ok_or(EndLineInformation { line, len })) {
                Ok(t) => Ok(t),
                Err(e) => {
                    errors.push(e);
                    Err(token)
                },
            }
        })
    }

    pub(crate) fn next_if(&mut self, f: impl FnOnce(&Token) -> bool) -> Option<Token> {
        self.token_stream.next_if(f)
    }

    #[inline]
    pub(crate) fn next_if_kind(&mut self, kind: TokenKind) -> Option<Token> {
        self.next_if(|t| t.kind == kind)
    }

    #[inline]
    pub(crate) fn next_if_kind_errored(&mut self, expected: TokenKind) -> Option<Token> {
        self.next_if_map_errored(|t| {
            match t {
                Ok(token) if token.kind == expected => Ok(token),
                Ok(token) => {
                    let kind = ParseErrorKind::InvalidToken(expected, token.kind);
                    let span = token.span;
                    let line = token.line;
                    Err(ParseError { kind, span, line })
                },
                Err(EndLineInformation { line, len }) => {
                    let kind = ParseErrorKind::ExpectedToken(expected);
                    let span = Span::empty_from_start(len);
                    Err(ParseError { kind, span, line })
                }
            }
        })
    }

    #[inline]
    pub(crate) fn advance_until_kind(&mut self, expected: TokenKind) -> Option<Token> {
        self.advance_map(|t| {
            match t {
                Ok(token) if token.kind == expected => Ok(token),
                Ok(token) => {
                    let kind = ParseErrorKind::InvalidToken(expected, token.kind);
                    let span = token.span;
                    let line = token.line;
                    Err(ParseError { kind, span, line })
                },
                Err(EndLineInformation { line, len }) => {
                    let kind = ParseErrorKind::ExpectedToken(expected);
                    let span = Span::empty_from_start(len);
                    Err(ParseError { kind, span, line })
                }
            }
        })
    }

    pub(crate) fn advance_map<T>(&mut self, f: impl Fn(Result<Token, EndLineInformation>) -> Result<T, ParseError>) -> Option<T> {
        let line = self.line();
        let len = self.input.len();
        let errors = &mut self.errors;
        while !self.token_stream.reached_eof() {
            let t_option = self.token_stream.next_if_map(|token| {
                match (f)(token.ok_or(EndLineInformation { line, len })) {
                    Ok(t) => Ok(t),
                    Err(e) => {
                        errors.push(e);
                        Err(token)
                    }
                }
            });

            if t_option.is_some() {
                return t_option
            }

            self.token_stream.next();
        }

        None
    }
}
