#![no_std]

use bitflags::bitflags;
use core::fmt::{Display, Formatter, Result as DisplayResult};
use core::range::Range;

pub type Span = Range<usize>;
trait Seal {}
#[allow(private_bounds, reason = "This is a sealed trait which would be only used for SpanImpl.")]
pub trait SpanImpl: Seal + Eq + PartialEq {
    type Item: Eq + PartialEq;
    
    fn new(start: Self::Item, end: Self::Item) -> Self;
    fn empty_from_start(start: Self::Item) -> Self;
    fn new_spanned(start_span: Self, end_span: Self) -> Self;
    fn start(&self) -> Self::Item;
    fn end(&self) -> Self::Item;
    fn shift_start_right(self, l: Self::Item) -> Self;
    fn shift_end_left(self, r: Self::Item) -> Self;
    
    #[inline]
    fn is_empty(&self) -> bool {
        self.start() == self.end()
    }
}

impl Seal for Span {}
impl SpanImpl for Span {
    type Item = usize;

    #[inline]
    fn new(start: Self::Item, end: Self::Item) -> Self {
        Self { start, end }
    }

    #[inline]
    fn empty_from_start(start: Self::Item) -> Self {
        Self { start, end: start }
    }

    #[inline]
    fn new_spanned(start_span: Self, end_span: Self) -> Self {
        Self { start: start_span.start, end: end_span.end }
    }

    #[inline]
    fn start(&self) -> Self::Item {
        self.start
    }

    #[inline]
    fn end(&self) -> Self::Item {
        self.end
    }

    #[inline]
    fn shift_start_right(self, l: Self::Item) -> Self {
        Self {
            start: self.start + l,
            end: self.end
        }
    }

    fn shift_end_left(self, r: Self::Item) -> Self {
        Self {
            start: self.start,
            end: self.end - r
        }
    }
}

#[derive(Debug)]
pub struct Spanned<T> {
    pub data: T,
    pub span: Span
}

impl<T: Clone> Clone for Spanned<T> {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), span: self.span }
    }
}

impl<T: Copy> Copy for Spanned<T> {}
impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data && self.span == other.span
    }
}

impl<T: Eq> Eq for Spanned<T> {}

impl<T> Spanned<T> {
    pub const fn new(data: T, span: Span) -> Self {
        Self { data, span }
    }

    pub const fn data(&self) -> &T {
        &self.data
    }

    #[inline]
    pub const fn span(&self) -> Span {
        self.span
    }
}


#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span
}

impl Token {
    #[inline]
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    #[inline]
    pub fn kind(&self) -> &TokenKind {
        &self.kind
    }

    #[inline]
    pub fn lexeme<'s>(&self, str: &'s str) -> Option<&'s str> {
        if str.len() > self.span.end {
            Some(&str[self.span.start..self.span.start])
        } else {
            None
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum TokenKind {
    CurlyBracketLeft,
    CurlyBracketRight,
    ParanthesisLeft,
    ParanthesisRight,
    SquareBracketLeft,
    SquareBracketRight,
    Dot,
    DotDot,
    DotDotDot,
    Semicolon,
    Colon,
    ColonColon,
    Comma,
    LessThan,
    BitwiseLeft,
    BitwiseLeftCompound,
    LessThanEquals,
    GreaterThan,
    BitwiseRight,
    BitwiseRightCompound,
    GreaterThanEquals,
    Equals,
    EqualsEquals,
    Not,
    NotEquals,
    And,
    AndEquals,
    AndAnd,
    Caret,
    CaretEquals,
    Tilde,
    TildeEquals,
    Or,
    OrEquals,
    OrOr,
    Star,
    StarEquals,
    Slash,
    SlashEquals,
    Percent,
    PercentEquals,
    Plus,
    PlusEquals,
    PlusPlus,
    Minus,
    MinusEquals,
    MinusMinus,
    At,
    Backslash,
    QuestionMark,
    Underscore,
    Tag,
    FatArrow,

    EnumKeyword,
    StructKeyword,
    ConfigKeyword,
    ConstKeyword,
    FlagsKeyword,
    SelfKeyword,
    FuncKeyword,
    MatchKeyword,
    LetKeyword,
    IfKeyword,
    ElseKeyword,
    WhileKeyword,
    ForKeyword,
    InKeyword,
    SigKeyword,
    NewKeyword,
    ReturnKeyword,
    TrueKeyword,
    NullKeyword,
    FalseKeyword,
    FeatureKeyword,
    ImportKeyword,
    ContinueKeyword,
    BreakKeyword,

    StringLiteral {
        options: StringOptions,
        quote_start: usize,
        terminated: bool,
    },
    NumberLiteral {
        base: BaseN,
        is_floating: bool,
    },
    CharacterLiteral {
        escape_type: CharacterEscapeType,
        terminated: bool
    },
    Identifier,

    #[default]
    Unknown,
}

impl Display for TokenKind {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        let val = match self {
            Self::CurlyBracketLeft => "{",
            Self::CurlyBracketRight => "}",
            Self::ParanthesisLeft => "(",
            Self::ParanthesisRight => ")",
            Self::SquareBracketLeft => "[",
            Self::SquareBracketRight => "]",
            Self::Dot => ".",
            Self::DotDot => "..",
            Self::DotDotDot => "...",
            Self::Semicolon => ";",
            Self::Colon => ":",
            Self::ColonColon => "::",
            Self::Comma => ",",
            Self::LessThan => "<",
            Self::BitwiseLeft => "<<",
            Self::BitwiseLeftCompound => ">>=",
            Self::LessThanEquals => "<=",
            Self::GreaterThan => ">",
            Self::BitwiseRight => ">>",
            Self::BitwiseRightCompound => ">>=",
            Self::GreaterThanEquals => ">=",
            Self::Equals => "=",
            Self::EqualsEquals => "==",
            Self::Not => "!",
            Self::NotEquals => "!=",
            Self::And => "&",
            Self::AndAnd => "&&",
            Self::AndEquals => "&=",
            Self::Caret => "^",
            Self::CaretEquals => "^=",
            Self::Tilde => "~",
            Self::TildeEquals => "~=",
            Self::Or => "|",
            Self::OrEquals => "|=",
            Self::OrOr => "||",
            Self::Star => "*",
            Self::StarEquals => "*=",
            Self::Slash => "/",
            Self::SlashEquals => "/=",
            Self::Percent => "%",
            Self::PercentEquals => "%=",
            Self::Plus => "+",
            Self::PlusEquals => "+=",
            Self::PlusPlus => "++",
            Self::Minus => "-",
            Self::MinusEquals => "-=",
            Self::MinusMinus => "--",
            Self::At => "@",
            Self::Backslash => r"\",
            Self::QuestionMark => "?",
            Self::Underscore => "_",
            Self::Tag => "#",
            Self::FatArrow => "=>",
            Self::EnumKeyword => "enum",
            Self::StructKeyword => "struct",
            Self::ConfigKeyword => "config",
            Self::ConstKeyword => "const",
            Self::FlagsKeyword => "flags",
            Self::SelfKeyword => "self",
            Self::FuncKeyword => "func",
            Self::MatchKeyword => "match",
            Self::LetKeyword => "let",
            Self::IfKeyword => "if",
            Self::ElseKeyword => "else",
            Self::WhileKeyword => "while",
            Self::ForKeyword => "for",
            Self::InKeyword => "in",
            Self::SigKeyword => "sig",
            Self::NewKeyword => "new",
            Self::ReturnKeyword => "return",
            Self::TrueKeyword => "true",
            Self::NullKeyword => "null",
            Self::FalseKeyword => "false",
            Self::FeatureKeyword => "feature",
            Self::ImportKeyword => "import",
            Self::ContinueKeyword => "continue",
            Self::BreakKeyword => "continue",
            Self::StringLiteral { .. } => "<STRING>",
            Self::NumberLiteral { .. } => "<NUMBER>",
            Self::CharacterLiteral { .. } => "<CHARACTER>",
            Self::Identifier => "<IDENTIFIER>",
            Self::Unknown => "<UNKNOWN>",
        };

        write!(f, "{}", val)
    }
}

impl TokenKind {
    #[inline]
    pub const fn is_string_literal(&self) -> bool {
        matches!(self, Self::StringLiteral{ .. })
    }

    #[inline]
    pub const fn is_number_literal(&self) -> bool {
        matches!(self, Self::NumberLiteral{ .. })
    }

    #[inline]
    pub const fn is_character_literal(&self) -> bool {
        matches!(self, Self::CharacterLiteral{ .. })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum BaseN {
    Bin = 2,
    Octal = 8,
    #[default]
    Decimal = 10,
    Hex = 16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CharacterEscapeType {
    #[default]
    None,
    Simple,
    Hex,
    Utf16,
    Utf32
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct StringOptions: u8 {
        const RAWSTR = 1;
        const MULTILINE_STR = 1 << 1;
    }
}

impl<'c> TryFrom<&'c str> for StringOptions {
    type Error = &'c str;

    #[inline]
    fn try_from(value: &'c str) -> Result<Self, Self::Error> {
        let mut initial = StringOptions::empty();
        for char in value.chars() {
            match char {
                'r' | 'R' => initial |= StringOptions::RAWSTR,
                'm' | 'M' => initial |= StringOptions::MULTILINE_STR,
                _ => return Err(value),
            }
        }

        Ok(initial)
    }
}
