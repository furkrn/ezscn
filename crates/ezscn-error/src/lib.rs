#![no_std]

use core::num::IntErrorKind;
use core::fmt::{Display, Formatter, Result as DisplayResult};
use ezscn_tokens::{Span, Token, TokenKind};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Span,
}

impl ParseError {
    #[inline]
    pub const fn new(kind: ParseErrorKind, span: Span) -> Self {
        ParseError { kind, span }
    }

    #[inline]
    pub const fn kind(&self) -> ParseErrorKind {
        self.kind
    }

    #[inline]
    pub const fn span(&self) -> Span {
        self.span
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ParseErrorKind {
    UnexpectedToken(TokenKind, TokenKind),
    ExpectedToken(TokenKind),
    InvalidReturnTypeToken(TokenKind),
    ExpectedReturnType,
    LiteralExpected(Option<TokenKind>),
    EmptyChar,
    EmptyEscapeSequence,
    UnknownEscapeSequence,
    CharOutOfRange,
    UnterminatedChar,
    UnterminatedString,
    InvalidTokenForNewExpr(TokenKind),
    ExpectedIdentifierForNewExpr,
    IntError(IntErrorKind),
    UnexpectedLiteralKind,
    InvalidStructToken(TokenKind),
    UnterminatedStruct,
    FloatError,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LiteralKind {
    String,
    Char,
    Integer,
    Float,
}

impl Display for ParseErrorKind {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        todo!()
    }
}
