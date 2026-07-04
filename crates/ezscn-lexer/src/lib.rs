#![no_std]

use core::iter::{Iterator, Peekable};
use core::str::CharIndices;
use ezscn_tokens::{BaseN, CharacterEscapeType, Token,
    TokenKind, Span, SpanImpl, StringOptions};
use unicode_ident::{is_xid_continue, is_xid_start};

#[derive(Debug)]
pub struct TokenStream<'t> {
    inner: TokenStreamInner<'t>,
    peek: Option<Option<Token>>,
}

impl Iterator for TokenStream<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        match self.peek.take() {
            Some(t) => t,
            None => self.inner.next()
        }
    }
}

impl<'t> From<&'t str> for TokenStream<'t> {
    #[inline]
    fn from(value: &'t str) -> Self {
        TokenStream { inner: TokenStreamInner::new(value), peek: None }
    }
}

impl<'t> TokenStream<'t> {
    #[inline]
    pub fn new(input: &'t str) -> Self {
        Self::from(input)
    }

    #[inline]
    pub const fn line(&self) -> usize {
        self.inner.line
    }

    #[inline]
    pub fn peek(&mut self) -> Option<&Token> {
        let iter = &mut self.inner;
        self.peek.get_or_insert_with(|| iter.next()).as_ref()
    }

    #[inline]
    pub fn is_next(&mut self, kind: TokenKind) -> bool {
        self.peek().is_some_and(|t| t.kind == kind)
    }

    pub fn next_if(&mut self, f: impl FnOnce(&Token) -> bool) -> Option<Token> {
        match self.next() {
            Some(t) if (f)(&t) => Some(t),
            other => {
                self.peek = Some(other);
                None
            }
        }
    }

    pub fn next_if_map<T>(&mut self, f: impl FnOnce(Option<Token>) -> Result<T, Option<Token>>) -> Option<T> {
        let t = self.next();
        match (f)(t) {
            Ok(t) => Some(t),
            Err(t) => {
                self.peek = Some(t);
                None
            }
        }
    }

    #[inline]
    pub fn reached_eof(&mut self) -> bool {
        self.peek().is_none()
    }
}

#[derive(Debug)]
pub struct TokenStreamInner<'s> {
    cursor: Peekable<CharIndices<'s>>,
    raw: &'s str,
    line: usize,
}

impl Iterator for TokenStreamInner<'_> {
    type Item = Token;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (start, char) = self.consume_char()?;
        let (start, char) = if char.is_whitespace() {
            self.consume_while_procedure(|_, c| c.is_whitespace());
            self.consume_char()?
        } else {
            (start, char)
        };

        let line = self.line;
        let (kind, end) = self.match_token(start, char);
        let end = end.unwrap_or(start + char.len_utf8());

        Some(Token::new(kind, Span::new(start, end), line))
    }
}

impl<'a> TokenStreamInner<'a> {
    #[inline]
    pub(crate) fn new(raw: &'a str) -> Self {
        TokenStreamInner {
            cursor: raw.char_indices().peekable(),
            raw,
            line: 0
        }
    }

    #[inline]
    fn match_token(&mut self, start: usize, current_char: char) -> (TokenKind, Option<usize>) {
        match current_char {
            '{' => (TokenKind::CurlyBracketLeft, None),
            '}' => (TokenKind::CurlyBracketRight, None),
            '(' => (TokenKind::ParanthesisLeft, None),
            ')' => (TokenKind::ParanthesisRight, None),
            '[' => (TokenKind::SquareBracketLeft, None),
            ']' => (TokenKind::SquareBracketRight, None),
            '.' => self.try_dots(start),
            ';' => (TokenKind::Semicolon, None),
            ':' => self.try_colons(start),
            ',' => (TokenKind::Comma, None),
            '<' => self.lt(),
            '>' => self.gt(),
            '=' => self.eq_s(),
            '!' => self.tokenize_if(|mut c| c.take_if(|(_, ch)| *ch == '=').map(|(i, _)| (TokenKind::NotEquals, Some(i + 1))))
                .unwrap_or((TokenKind::Not, None)),
            '&' => self.tokenize_if(|c| {
                match c {
                    Some((i, '=')) => Some((TokenKind::AndEquals, Some(i + 1))),
                    Some((i, '&')) => Some((TokenKind::AndAnd, Some(i + 1))),
                    _ => None,
                }
            }).unwrap_or((TokenKind::And, None)),
            '^' => self.tokenize_if(|mut c| c.take_if(|(_, ch)| *ch == '=').map(|(i, _)| (TokenKind::CaretEquals, Some(i + 1))))
                .unwrap_or((TokenKind::Caret, None)),
            '~' => self.tokenize_if(|mut c| c.take_if(|(_, ch)| *ch == '=').map(|(i, _)| (TokenKind::TildeEquals, Some(i + 1))))
                .unwrap_or((TokenKind::Tilde, None)),
            '|' => self.tokenize_if(|c| {
                match c {
                    Some((i, '=')) => Some((TokenKind::OrEquals, Some(i + 1))),
                    Some((i, '|')) => Some((TokenKind::OrOr, Some(i + 1))),
                    _ => None,
                }
            }).unwrap_or((TokenKind::Or, None)),
            '*' => self.tokenize_if(|mut c| c.take_if(|(_, ch)| *ch == '=').map(|(i, _)| (TokenKind::StarEquals, Some(i + 1))))
                .unwrap_or((TokenKind::Star, None)),
            '/' => self.tokenize_if(|mut c| c.take_if(|(_, ch)| *ch == '=').map(|(i, _)| (TokenKind::SlashEquals, Some(i + 1))))
                .unwrap_or((TokenKind::Slash, None)),
            '%' => self.tokenize_if(|mut c| c.take_if(|(_, ch)| *ch == '=').map(|(i, _)| (TokenKind::PercentEquals, Some(i + 1))))
                .unwrap_or((TokenKind::Percent, None)),
            '+' => self.plus(),
            '-' => self.minus(),
            '#' => (TokenKind::Tag, None),
            '@' => {
                let next = self.peek_char();
                match next {
                    Some((_, ch)) if is_xid_continue(ch) => {
                        let range = self.consume_while_procedure(|_, c| is_xid_continue(c))
                            .expect("Ambiqious operation, peeked a value which is xid_continue but nothing is returned.");

                        (TokenKind::Identifier, Some(range.end))
                    },
                    _ => (TokenKind::At, None)
                }
            },
            '$' => {
                let next = self.peek_char();
                if next.is_some_and(|(_, c)| is_xid_continue(c)) {
                    let range = self.consume_while_procedure(|_, c| is_xid_continue(c))
                        .expect("Ambigious operation, peeked a value which is xid_continue but nothing is returned.");

                    (TokenKind::Identifier, Some(range.end))
                } else {
                    (TokenKind::Dollar, None)
                }
            }
            '\\' => (TokenKind::Backslash, None),
            '?' => (TokenKind::QuestionMark, None),
            '_' => {
                let next = self.peek_char();
                if next.is_some_and(|(_, c)| is_xid_continue(c)) {
                    let range = self.consume_while_procedure(|_, c| is_xid_continue(c))
                        .expect("Ambigious operation, peeked a value which is xid_continue but nothing is returned.");

                    (TokenKind::Identifier, Some(range.end))
                } else {
                    (TokenKind::Underscore, None)
                }
            },
            '"' => self.string_sequence(start, StringOptions::empty()),
            '\'' => {
                if let Some((escape_type, char_range)) = self.char_sequence() {
                    if let Some((quote_end, _)) = self.consume_if(|_, c| c == '\'') {
                        (TokenKind::CharacterLiteral { escape_type, terminated: true }, Some(quote_end + 1))
                    } else {
                        let end = char_range.end;
                        (TokenKind::CharacterLiteral { escape_type, terminated: false }, Some(end))
                    }
                } else {
                    (TokenKind::Unknown, None)
                }
            }
            c if is_xid_start(c) => {
                let range = self.consume_while_procedure(|_, c| is_xid_continue(c))
                    .unwrap_or_else(|| Span { start, end: start + current_char.len_utf8() });

                let str = &self.raw[start..range.end];
                if self.peek_char().is_some_and(|(_, c)| c == '"') {
                    let (quote_start, _) = self.consume_char() //
                        .expect("Peeked a quote but no quote is returned.");

                    match StringOptions::try_from(str) {
                        Ok(options) => self.string_sequence(quote_start, options),
                        Err(_) => (TokenKind::Unknown, Some(quote_start + 1)),
                    }
                } else {
                    let kind = self.keywords(str)
                        .unwrap_or(TokenKind::Identifier);

                    (kind, Some(range.end))
                }
            },
            c if c.is_ascii_digit() => {
                let (base, mut is_floating) = match (c, self.peek_char()) {
                    ('0', Some((_, 'b' | 'B'))) => {
                        self.consume_char();
                        (BaseN::Bin, false)
                    }
                    ('0', Some((_, 'o' | 'O'))) => {
                        self.consume_char();
                        (BaseN::Octal, false)
                    }
                    ('0', Some((_, 'x' | 'X'))) => {
                        self.consume_char();
                        (BaseN::Hex, false)
                    }
                    (_, Some((_, '.'))) => {
                        self.consume_char();
                        (BaseN::Decimal, true)
                    },
                    (_, None) => return (TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: false }, None),
                    (_, _) => (BaseN::Decimal, false)
                };

                let end = self.number_sequence(&base, &mut is_floating);
                (TokenKind::NumberLiteral { base, is_floating }, end)
            },
            _ => (TokenKind::Unknown, None)
        }
    }

    #[inline]
    fn try_dots(&mut self, start: usize) -> (TokenKind, Option<usize>) {
        let dots_range = self.consume_while_procedure(|_, c| c == '.')
            .map(|c| c.end - c.start)
            .unwrap_or(0);

        match dots_range {
            0 => (TokenKind::Dot, None),
            1 => (TokenKind::DotDot, Some(start + 2)),
            2 => (TokenKind::DotDotDot, Some(start + 3)),
            r => (TokenKind::Unknown, Some(start + r + 1)),
        }
    }

    #[inline]
    fn eq_s(&mut self) -> (TokenKind, Option<usize>) {
        match self.peek_char() {
            Some((i, '=')) => { self.consume_char(); (TokenKind::EqualsEquals, Some(i + 1)) },
            Some((i, '>')) => { self.consume_char(); (TokenKind::FatArrow, Some(i + 1)) },
            _ => (TokenKind::Equals, None)
        }
    }

    #[inline]
    fn plus(&mut self) -> (TokenKind, Option<usize>) {
        match self.peek_char() {
            Some((i, '+')) => { self.consume_char(); (TokenKind::PlusPlus, Some(i + 1)) },
            Some((i, '=')) => { self.consume_char(); (TokenKind::PlusEquals, Some(i + 1)) },
            _ => (TokenKind::Plus, None)
        }
    }

    #[inline]
    fn minus(&mut self) -> (TokenKind, Option<usize>) {
        match self.peek_char() {
            Some((i, '-')) => { self.consume_char(); (TokenKind::MinusMinus, Some(i + 1)) },
            Some((i, '=')) => { self.consume_char(); (TokenKind::MinusEquals, Some(i + 1)) },
            _ => (TokenKind::Minus, None)
        }
    }

    #[inline]
    fn try_colons(&mut self, start: usize) -> (TokenKind, Option<usize>) {
        let count = self.consume_while_procedure(|_, c| c == ':')
            .map(|r| r.end - r.start)
            .unwrap_or(0);

        match count {
            0 => (TokenKind::Colon, None),
            1 => (TokenKind::ColonColon, Some(start + 2)),
            r => (TokenKind::Unknown, Some(start + r + 2)),
        }
    }

    #[inline]
    fn keywords<'c>(&self, str: &'c str) -> Result<TokenKind, &'c str> {
        let keyword = match str {
            "enum" => TokenKind::EnumKeyword,
            "struct" => TokenKind::StructKeyword,
            "config" => TokenKind::ConfigKeyword,
            "const" => TokenKind::ConstKeyword,
            "flags" => TokenKind::FlagsKeyword,
            "self" => TokenKind::SelfKeyword,
            "func" => TokenKind::FuncKeyword,
            "match" => TokenKind::MatchKeyword,
            "let" => TokenKind::LetKeyword,
            "if" => TokenKind::IfKeyword,
            "else" => TokenKind::ElseKeyword,
            "while" => TokenKind::WhileKeyword,
            "for" => TokenKind::ForKeyword,
            "in" => TokenKind::InKeyword,
            "sig" => TokenKind::SigKeyword,
            "new" => TokenKind::NewKeyword,
            "return" => TokenKind::ReturnKeyword,
            "true" => TokenKind::TrueKeyword,
            "false" => TokenKind::FalseKeyword,
            "null" => TokenKind::NullKeyword,
            "feature" => TokenKind::FeatureKeyword,
            "import" => TokenKind::ImportKeyword,
            "continue" => TokenKind::ContinueKeyword,
            "break" => TokenKind::BreakKeyword,
            "where" => TokenKind::WhereKeyword,
            "as" => TokenKind::AsKeyword,
            "pub" => TokenKind::PubKeyword,
            "local" => TokenKind::LocalKeyword,
            _ => return Err(str),
        };

        Ok(keyword)
    }

    #[inline]
    fn lt(&mut self) -> (TokenKind, Option<usize>) {
        match self.peek_char() {
            Some((i, '=')) => {
                self.consume_char();
                (TokenKind::LessThanEquals, Some(i + 1))
            },
            Some((i, '<')) => {
                self.consume_char();
                self.tokenize_if(|mut c| c.take_if(|(_, ch)| *ch == '=')
                    .map(|(i, _)| (TokenKind::BitwiseLeftCompound, Some(i + 1))))
                    .unwrap_or((TokenKind::BitwiseLeft, Some(i + 1)))
            }
            _ => (TokenKind::LessThan, None),
        }
    }

    #[inline]
    fn gt(&mut self) -> (TokenKind, Option<usize>) {
        match self.peek_char() {
            Some((i, '=')) => {
                self.consume_char();
                (TokenKind::GreaterThanEquals, Some(i + 1))
            },
            Some((i, '>')) => {
                self.consume_char();
                self.tokenize_if(|mut c| c.take_if(|(_, ch)| *ch == '=')
                    .map(|(i, _)| (TokenKind::BitwiseRightCompound, Some(i + 1))))
                    .unwrap_or((TokenKind::BitwiseRight, Some(i + 1)))
            }
            _ => (TokenKind::GreaterThan, None),
        }
    }

    #[inline]
    fn number_sequence(&mut self, base_n: &BaseN, mantissa: &mut bool) -> Option<usize> {
        #[inline]
         fn consume_pattern(c: char, base_n: &BaseN, consuming_mantissa: bool) -> bool {
             match base_n {
                 BaseN::Bin => matches!(c, '0'..='1'),
                 BaseN::Octal => matches!(c, '0'..='7'),
                 BaseN::Decimal if consuming_mantissa => c.is_ascii_digit() || c == 'E' || c == 'e' || c == '+' || c == '-',
                 BaseN::Decimal => c.is_ascii_digit(),
                 BaseN::Hex => c.is_ascii_hexdigit(),
             }
         }

         let exponent_range = self.consume_while_procedure(|_, c| consume_pattern(c, base_n, *mantissa))?;
         if *mantissa {
             return Some(exponent_range.end)
         }

         if base_n == &BaseN::Decimal && let Some((dot_range, _)) = self.consume_if(|_, c| c == '.') {
             *mantissa = true;
             self.consume_while_procedure(|_, c| consume_pattern(c, base_n, true))
                 .map(|c| c.end)
                 .or(Some(dot_range))
         } else {
             Some(exponent_range.end)
         }
    }

    #[inline]
    fn string_sequence(&mut self, quote_start: usize, options: StringOptions) -> (TokenKind, Option<usize>) {
        let mut end = None;
        let terminated = loop {
            match self.consume_char() {
                Some((i, '"')) => {
                    if options.contains(StringOptions::MULTILINE_STR) {
                        let peek = self.peek_char();
                        if peek.is_none() {
                            end = Some(i + 1);
                            break false
                        }

                        if peek.is_some_and(|(_, c)| c == 'm' || c == 'M') {
                            self.consume_char();
                            end = Some(i + 2);
                            break true
                        }
                    } else {
                        end = Some(i + 1);
                        break true
                    }
                },
                Some((i, '\n')) if !options.contains(StringOptions::MULTILINE_STR) => {
                    end = Some(i);
                    break false
                },
                Some((i, '\\')) if !options.contains(StringOptions::RAWSTR) => {
                    if self.peek_char().is_some_and(|(_, c)| c == '"') {
                        self.consume_char();
                        end = Some(i + 2);
                    } else {
                        end = Some(i + 1);
                    }
                },
                Some((i, c)) => {
                    end = Some(i + c.len_utf8())
                },
                None => {
                    break false
                },
            }
        };

        let ending_line = self.line;
        (TokenKind::StringLiteral { options, quote_start, terminated, ending_line }, end)
    }

    #[inline]
    fn char_sequence(&mut self) -> Option<(CharacterEscapeType, Span)> {
        #[inline]
        fn escaped<'t>(ts: &mut TokenStreamInner<'t>) -> Option<(CharacterEscapeType, usize)> {
            match ts.consume_char()? {
                (i, 'x') => {
                    let mut current = 0;
                    ts.consume_while_procedure(move |_, c| {
                        current += 1;
                        current <= 4 && c.is_ascii_hexdigit()
                    }).map(|c| (CharacterEscapeType::Hex, c.end))
                        .or_else(|| Some((CharacterEscapeType::Hex, i + 1)))
                },
                (i, 'u') => ts.consume_n_times(4).map(|c| (CharacterEscapeType::Utf16, c.end))
                    .or_else(|| Some((CharacterEscapeType::Utf16, i + 1))),
                (i, 'U') => ts.consume_n_times(8).map(|c| (CharacterEscapeType::Utf32, c.end))
                    .or_else(|| Some((CharacterEscapeType::Utf32, i + 1))),
                (i, c) => Some((CharacterEscapeType::Simple, i + c.len_utf8())),
            }
        }

        let (start, char) = self.consume_char()?;
        let (char_type, end) = match char {
            '\\' => escaped(self)
                .unwrap_or_else(|| (CharacterEscapeType::Simple, start + 1)),
            c => (CharacterEscapeType::None, start + c.len_utf8()),
        };

        Some((char_type, Span { start, end }))
    }

    #[inline]
    pub(crate) fn consume_char(&mut self) -> Option<(usize, char)> {
        let line = &mut self.line;
        self.cursor.next()
            .inspect(|(_, c)| {
                if *c == '\n' {
                    *line += 1;
                }
            })
    }

    pub(crate) fn consume_if<F>(&mut self, f: F) -> Option<(usize, char)>
        where F: Fn(usize, char) -> bool {
            self.cursor.next_if(|(i, c)| (f)(*i, *c))
    }

    #[inline]
    pub(crate) fn peek_char(&mut self) -> Option<(usize, char)> {
        self.cursor.peek()
            .copied()
    }

    pub(crate) fn tokenize_if<F>(&mut self, f: F) -> Option<(TokenKind, Option<usize>)>
        where F: FnOnce(Option<(usize, char)>) -> Option<(TokenKind, Option<usize>)> {
            let result = (f)(self.peek_char());
            if result.is_some() {
                self.consume_char();
            }

            result
    }

    pub(crate) fn consume_while_procedure<F>(&mut self, mut f: F) -> Option<Span>
        where F: FnMut(usize, char) -> bool {
            let mut start = None;
            let mut end = 0;
            while let Some((i, c)) = self.cursor.next_if_map(|(i, c)| if (f)(i, c) { Ok((i, c)) } else { Err((i, c)) }) {
                if start.is_none() {
                    start = Some(i);
                }

                if c == '\n' {
                    self.line += 1;
                }

                end = i + c.len_utf8();
            }

            let start = start?;
            Some(Span { start, end })
    }

    #[inline]
    pub(crate) fn consume_n_times(&mut self, n: usize) -> Option<Span> {
        if n == 0 {
            return None
        }

        let (start, _) = self.consume_char()?;
        let mut end = start;
        for _ in 1..n {
            match self.consume_char() {
                Some((i, c)) => end = i + c.len_utf8(),
                None => break
            }
        }

        Some(Span { start, end })
    }
}

// These tests were written before i decided to make it no_std.
// That's why seperation is kinda weird.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn consume_basic_chars() {
        let str = r"{}()[];,^\?_=";
        let mut stream = TokenStream::from(str);

        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CurlyBracketLeft, span: Span::new(0, 1), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CurlyBracketRight, span: Span::new(1, 2), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ParanthesisLeft, span: Span::new(2, 3), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ParanthesisRight, span: Span::new(3, 4), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::SquareBracketLeft, span: Span::new(4, 5), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::SquareBracketRight, span: Span::new(5, 6), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Semicolon, span: Span::new(6, 7), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Comma, span: Span::new(7, 8), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Caret, span: Span::new(8, 9), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Backslash, span: Span::new(9, 10), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::QuestionMark, span: Span::new(10, 11), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Underscore, span: Span::new(11, 12), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Equals, span: Span::new(12, 13), line: 0 }));
        assert_eq!(stream.next(), None)
    }

    #[test]
    pub fn consume_extra_char_test() {
        let str = ". .. ... : :: + += ++ - -= --";
        let mut stream = TokenStream::from(str);

        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Dot, span: Span::new(0, 1), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::DotDot, span: Span::new(2, 4), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::DotDotDot, span: Span::new(5, 8), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Colon, span: Span::new(9, 10), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ColonColon, span: Span::new(11, 13), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Plus, span: Span::new(14, 15), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::PlusEquals, span: Span::new(16, 18), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::PlusPlus, span: Span::new(19, 21), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Minus, span: Span::new(22, 23), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::MinusEquals, span: Span::new(24, 26), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::MinusMinus, span: Span::new(27, 29), line: 0 }));
        assert_eq!(stream.next(), None);
    }

    #[test]
    pub fn special_care_char_test() {
        let str = "< << <<= <= > >> >>= >= = == ! != & &= && | |= || ~ ~= * *= / /= % %= => $";
        let mut stream = TokenStream::from(str);

        assert_eq!(stream.next(), Some(Token { kind: TokenKind::LessThan, span: Span::new(0, 1), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::BitwiseLeft, span: Span::new(2, 4), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::BitwiseLeftCompound, span: Span::new(5, 8), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::LessThanEquals, span: Span::new(9, 11), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::GreaterThan, span: Span::new(12, 13), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::BitwiseRight, span: Span::new(14, 16), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::BitwiseRightCompound, span: Span::new(17, 20), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::GreaterThanEquals, span: Span::new(21, 23), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Equals, span: Span::new(24, 25), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::EqualsEquals, span: Span::new(26, 28), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Not, span: Span::new(29, 30), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NotEquals, span: Span::new(31, 33), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::And, span: Span::new(34, 35), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::AndEquals, span: Span::new(36, 38), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::AndAnd, span: Span::new(39, 41), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Or, span: Span::new(42, 43), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::OrEquals, span: Span::new(44, 46), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::OrOr, span: Span::new(47, 49), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Tilde, span: Span::new(50, 51), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::TildeEquals, span: Span::new(52, 54), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Star, span: Span::new(55, 56), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StarEquals, span: Span::new(57, 59), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Slash, span: Span::new(60, 61), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::SlashEquals, span: Span::new(62, 64), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Percent, span: Span::new(65, 66), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::PercentEquals, span: Span::new(67, 69), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::FatArrow, span: Span::new(70, 72), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Dollar, span: Span::new(73, 74), line: 0 }));
        assert_eq!(stream.next(), None);
    }

    #[test]
    pub fn null_bool_literal() {
        let str = "null true false";
        let mut stream = TokenStream::from(str);

        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NullKeyword, span: Span::new(0, 4), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::TrueKeyword, span: Span::new(5, 9), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::FalseKeyword, span: Span::new(10, 15), line: 0 }));
        assert_eq!(stream.next(), None);
    }

    #[test]
    pub fn identifier() {
        let str = "mrb $mrb @mrb _mrb mşrb";
        let mut stream = TokenStream::from(str);

        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Identifier, span: Span::new(0, 3), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Identifier, span: Span::new(4, 8), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Identifier, span: Span::new(9, 13), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Identifier, span: Span::new(14, 18), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Identifier, span: Span::new(19, 24), line: 0 }));
        assert_eq!(stream.next(), None);
    }

    #[test]
    pub fn keywords() {
        let str = r#"enum struct config const flags self func match
            let if else while for in sig new return feature import continue break where as pub local"#;
        let mut stream = TokenStream::from(str);

        assert_eq!(stream.next(), Some(Token { kind: TokenKind::EnumKeyword, span: Span::new(0, 4), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StructKeyword, span: Span::new(5, 11), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ConfigKeyword, span: Span::new(12, 18), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ConstKeyword, span: Span::new(19, 24), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::FlagsKeyword, span: Span::new(25, 30), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::SelfKeyword, span: Span::new(31, 35), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::FuncKeyword, span: Span::new(36, 40), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::MatchKeyword, span: Span::new(41, 46), line: 0 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::LetKeyword, span: Span::new(59, 62), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::IfKeyword, span: Span::new(63, 65), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ElseKeyword, span: Span::new(66, 70), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::WhileKeyword, span: Span::new(71, 76), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ForKeyword, span: Span::new(77, 80), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::InKeyword, span: Span::new(81, 83), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::SigKeyword, span: Span::new(84, 87), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NewKeyword, span: Span::new(88, 91), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ReturnKeyword, span: Span::new(92, 98), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::FeatureKeyword, span: Span::new(99, 106), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ImportKeyword, span: Span::new(107, 113), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::ContinueKeyword, span: Span::new(114, 122), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::BreakKeyword, span: Span::new(123, 128), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::WhereKeyword, span: Span::new(129, 134), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::AsKeyword, span: Span::new(135, 137), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::PubKeyword, span: Span::new(138, 141), line: 1 }));
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::LocalKeyword, span: Span::new(142, 147), line: 1}));
        assert_eq!(stream.next(), None);
    }

    #[test]
    pub fn num_literal() {
        let str = r#"0b0010 0b0100 0b1000 0o1234567
            0o7564312 0o7162534 0x123456789ABCDEF 0xFABCDE22 0x31245DA
            129381274 17483291 17823646 9 1 8 6.54
            5.32 2.4e9 2.432e+11 222.233e-333 51123.3932e-999 2222.4e"#;
        let mut stream = TokenStream::from(str);

        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Bin, is_floating: false }, span: Span::new(0, 6), line: 0 })); // 0b0010
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Bin, is_floating: false }, span: Span::new(7, 13), line: 0 })); // 0b0100
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Bin,  is_floating: false }, span: Span::new(14, 20), line: 0 })); // 0b1000
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Octal, is_floating: false }, span: Span::new(21, 30), line: 0 })); // 0o1234567
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Octal, is_floating: false }, span: Span::new(43, 52), line: 1 })); // 0o7564312
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Octal, is_floating: false }, span: Span::new(53, 62), line: 1 })); // 0o7162534
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Hex, is_floating: false }, span: Span::new(63, 80), line: 1 })); // 0x123456789ABCDEF
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Hex, is_floating: false }, span: Span::new(81, 91), line: 1 })); // 0xFABCDE22
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Hex, is_floating: false }, span: Span::new(92, 101), line: 1 })); // 0x31245DA
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: false }, span: Span::new(114, 123), line: 2 })); // 129381274
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: false }, span: Span::new(124, 132), line: 2 })); // 17483291
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: false }, span: Span::new(133, 141), line: 2 })); // 17823646
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: false }, span: Span::new(142, 143), line: 2 })); // 9
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: false }, span: Span::new(144, 145), line: 2 })); // 1
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: false }, span: Span::new(146, 147), line: 2 })); // 8
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: true }, span: Span::new(148, 152), line: 2 })); // 6.54
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: true }, span: Span::new(165, 169), line: 3 })); // 5.32
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: true }, span: Span::new(170, 175), line: 3 })); // 2.4e8
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: true }, span: Span::new(176, 185), line: 3 })); // 2.432e+11
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: true }, span: Span::new(186, 198), line: 3 })); // 222.233e-333
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: true }, span: Span::new(199, 214), line: 3 })); // 51123.3932e-999
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, is_floating: true }, span: Span::new(215, 222), line: 3 })); // 2222.4e
        assert_eq!(stream.next(), None);
    }

    #[test]
    pub fn char_literal() {
        let str = r#"'a' 'a '!' 'ş' '\a' '\a '\b' '\e' '\f' '\n'
            '\r' '\t' '\v' '\\' '\?' '\"' '\xF' '\x1111' '\x1000
            '\uFFFF' '\uAAAA '\U0010FFFE' '\U0010FFFE '\p' '\o' 'bbb' "#;
        let mut stream = TokenStream::from(str);

        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::None, terminated: true }, span: Span::new(0, 3), line: 0 })); // 'a'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::None, terminated: false }, span: Span::new(4, 6), line: 0 })); // 'a
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::None, terminated: true }, span: Span::new(7, 10), line: 0 })); // '!'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::None, terminated: true }, span: Span::new(11, 15), line: 0 })); // 'ş'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(16, 20), line: 0 })); // '\a'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: false }, span: Span::new(21, 24), line: 0 })); // '\a
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(25, 29), line: 0 })); // '\b'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(30, 34), line: 0 })); // '\e'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(35, 39), line: 0 })); // '\f'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(40, 44), line: 0 })); // '\n'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(57, 61), line: 1 })); // '\r'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(62, 66), line: 1 })); // '\t'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(67, 71), line: 1 })); // '\v'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(72, 76), line: 1 })); // '\\'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(77, 81), line: 1 })); // '\?'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(82, 86), line: 1 })); // '\"'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Hex, terminated: true }, span: Span::new(87, 92), line: 1 })); // '\xF'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Hex, terminated: true }, span: Span::new(93, 101), line: 1 })); // '\x1111'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Hex, terminated: false }, span: Span::new(102, 109), line: 1 })); // '\x1000
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Utf16, terminated: true }, span: Span::new(122, 130), line: 2 })); // '\uFFFF'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Utf16, terminated: false }, span: Span::new(131, 138), line: 2 })); // '\uAAAA
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Utf32, terminated: true }, span: Span::new(139, 151), line: 2 })); // '\U0010FFFE'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Utf32, terminated: false }, span: Span::new(152, 163), line: 2 })); // '\U0010FFFE
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(164, 168), line: 2 })); // '\p'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::Simple, terminated: true }, span: Span::new(169, 173), line: 2 })); // '\o'
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::None, terminated: false }, span: Span::new(174, 176), line: 2 })); // 'b
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::Identifier, span: Span::new(176, 178), line: 2 })); // bb
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::CharacterLiteral { escape_type: CharacterEscapeType::None, terminated: false }, span: Span::new(178, 180), line: 2 })); // "' "
        assert_eq!(stream.next(), None);
    }

    #[test]
    pub fn string_literal() {
        let str = r#" "" "ezscn" "escaped ezscn \t\t\n" "unterminated
            r"raw ezscn \ \ \\ \\\\ \e\e" m"ez
            scn"m mr"wtf"m MR"tf"m rm"TFF"m RM"pat"m
            m"unterminated multiline""#;
        let mut stream = TokenStream::from(str);

        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::empty(), quote_start: 1, terminated: true, ending_line: 0 }, span: Span::new(1, 3), line: 0 })); // ""
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::empty(), quote_start: 4, terminated: true, ending_line: 0 }, span: Span::new(4, 11), line: 0 })); // "ezscn"
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::empty(), quote_start: 12, terminated: true, ending_line: 0 }, span: Span::new(12, 34), line: 0 })); // "escaped ezscn \t\t\n"
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::empty(), quote_start: 35, terminated: false, ending_line: 1 }, span: Span::new(35, 48), line: 0 })); // "unterminated
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::RAWSTR, quote_start: 62, terminated: true, ending_line: 1 }, span: Span::new(61, 90), line: 1 })); // r"raw ezscn \ \ \\ \\\\ \e\e"
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::MULTILINE_STR, quote_start: 92, terminated: true, ending_line: 2 }, span: Span::new(91, 113), line: 1 })); // m"ez
                                                                                                                                                 //scn"m
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::MULTILINE_STR | StringOptions::RAWSTR, quote_start: 116, terminated: true, ending_line: 2 }, span: Span::new(114, 122), line: 2 })); // mr"wtf"m
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::MULTILINE_STR | StringOptions::RAWSTR, quote_start: 125, terminated: true, ending_line: 2 }, span: Span::new(123, 130), line: 2 })); // MR"tf"m
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::MULTILINE_STR | StringOptions::RAWSTR, quote_start: 133, terminated: true, ending_line: 2 }, span: Span::new(131, 139), line: 2 })); // rm"TFF"m
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::MULTILINE_STR | StringOptions::RAWSTR, quote_start: 142, terminated: true, ending_line: 2 }, span: Span::new(140, 148), line: 2 })); // RM"pat"m
        assert_eq!(stream.next(), Some(Token { kind: TokenKind::StringLiteral { options: StringOptions::MULTILINE_STR, quote_start: 162, terminated: false, ending_line: 3 }, span: Span::new(161, 186), line: 3 })); // m"unterminated multiline"
        assert_eq!(stream.next(), None);
    }
}
