#![no_std]

use core::num::IntErrorKind;
use core::fmt::{Display, Formatter, Result as DisplayResult};
use ezscn_tokens::{Span, Token, TokenKind};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Span,
    pub line: usize,
}

impl ParseError {
    #[inline]
    pub const fn new(kind: ParseErrorKind, span: Span, line: usize) -> Self {
        ParseError { kind, span, line }
    }

    #[inline]
    pub const fn kind(&self) -> ParseErrorKind {
        self.kind
    }

    #[inline]
    pub const fn span(&self) -> Span {
        self.span
    }

    #[inline]
    pub const fn line(&self) -> usize {
        self.line
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ParseErrorKind {
    InvalidToken(TokenKind, TokenKind),
    UnexpectedToken(TokenKind),
    ExpectedToken(TokenKind),
    InvalidReturnTypeToken(TokenKind),
    ExpectedReturnType,
    ExpectedIntegerFoundFloating,
    LiteralExpected(LiteralKind, Option<TokenKind>),
    LiteralsExpected(Option<TokenKind>),
    EmptyChar,
    EmptyEscapeSequence,
    UnknownEscapeSequence,
    CharOutOfRange,
    UnterminatedChar,
    UnterminatedString(usize),
    InvalidTokenForNewExpr(TokenKind),
    ExpectedIdentifierForNewExpr,
    IntError(IntErrorKind),
    UnexpectedLiteralKind(LiteralKind),
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
    Boolean,
}

impl Display for ParseErrorKind {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        todo!()
    }
}
