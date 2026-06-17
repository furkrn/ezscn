use core::iter::{Peekable, Iterator};
use core::str::Chars;
use ezscn_error::{ParseError, ParseErrorKind};
use ezscn_tokens::{Span, SpanImpl};
use thin_vec::ThinVec;

#[derive(Debug)]
pub struct UnescapedStringBuilder<'t> {
    iter: Peekable<Chars<'t>>,
    input: &'t str,
    current: usize,
    line: usize,
    errors: &'t mut ThinVec<ParseError>,
}

impl<'t> Iterator for UnescapedStringBuilder<'t> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.consume_char() {
            Some('\\') => self.escape_seq(),
            opt => opt,
        }
    }
}

impl<'t> UnescapedStringBuilder<'t> {
    #[inline]
    pub fn new(input: &'t str, line: usize, errors: &'t mut ThinVec<ParseError>) -> Self {
        let iter = input.chars()
            .peekable();

        UnescapedStringBuilder { iter, input, current: 0, line, errors }
    }

    #[inline]
    fn consume_char(&mut self) -> Option<char> {
        let current = &mut self.current;
        let line = &mut self.line;
        self.iter.next()
            .inspect(|c| { 
                *current += c.len_utf8();
                if *c == '\n' {
                    *line += 1;
                }
            })
    }

    fn next_if(&mut self, f: impl FnOnce(&char) -> bool) -> Option<char> {
        let current = &mut self.current;
        let line = &mut self.line;
        self.iter.next_if(f)
            .inspect(|c| { 
                *current += c.len_utf8();
                if *c == '\n' {
                    *line += 1;
                }
            })
    }

    #[inline]
    fn escape_seq(&mut self) -> Option<char> {
        let Some(current_char) = self.consume_char() else {
            let span = Span::empty_from_start(self.current - 1);
            self.errors.push(ParseError::new(ParseErrorKind::EmptyEscapeSequence, span, self.line));
            return None
        };

        match current_char {
            'u' => self.utf16_seq(),
            'U' => self.utf32_seq(),
            'x' => self.hex_seq(),
            c => char_escape_sequence_map(c)
        }
    }

    fn consume_n_times_while(&mut self, n: usize, f: impl Fn(char) -> bool) -> bool {
        let mut i = 0;
        loop {
            if i >= n {
                break
            }

            if !self.consume_char().is_some_and(&f) {
                return false
            }

            i += 1;
        }

        true
    }

    #[inline]
    fn utf16_seq(&mut self) -> Option<char> {
        let prev = self.current;
        self.consume_n_times_while(4, |c| c.is_ascii_hexdigit()); // TODO: Error
        let u = u32::from_str_radix(&self.input[prev..self.current], 16)
            .ok()?;

        char::from_u32(u)
    }

    #[inline]
    fn utf32_seq(&mut self) -> Option<char> {
        let prev = self.current;
        self.consume_n_times_while(8, |c| c.is_ascii_hexdigit()); // TODO: Error
        let u = u32::from_str_radix(&self.input[prev..self.current], 16)
            .ok()?;

        char::from_u32(u)
    }

    #[inline]
    fn hex_seq(&mut self) -> Option<char> {
        let prev = self.current;
        let mut i = 0;
        loop {
            if i >= 4 {
                break
            }

            if self.next_if(|c| c.is_ascii_hexdigit()).is_none() {
                break
            }

            i += 1;
        }

        let u = u32::from_str_radix(&self.input[prev..self.current], 16)
            .ok()?;

        char::from_u32(u)
    }
}

#[inline]
pub fn char_escape_sequence_map_str(str: &str) -> Option<char> {
    char_escape_sequence_map(str.chars().nth(1)?)
}

#[inline]
pub fn char_escape_sequence_map(char: char) -> Option<char> {
    match char {
        'a' => Some(0x07 as char),
        'b' => Some(0x08 as char),
        'e' => Some(0x1B as char),
        'f' => Some(0x0C as char),
        'n' => Some(0x0A as char),
        'r' => Some(0x0D as char),
        't' => Some(0x09 as char),
        'v' => Some(0x0B as char),
        '\\' => Some(0x5C as char),
        '\'' => Some(0x27 as char),
        '"' => Some(0x22 as char),
        '?' => Some(0x3F as char),
        _ => None
    }
}

#[inline]
pub fn hex_escape_sequence(raw: &str) -> Option<char> {
    let u = u32::from_str_radix(&raw[2..], 16)
        .ok()?;

    char::from_u32(u)
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use thin_vec::thin_vec;
    use super::*;

    #[test]
    fn can_produce_simple_string() {
        let raw_str = r#"ahhfhdkjskfgşgreşgşreşgerş
            nfgjkewhgjkerhgjkrehkjgerger"#;

        let mut errors = thin_vec![];
        let sb = UnescapedStringBuilder::new(raw_str, 0, &mut errors);

        let cow_str: Cow<_> = sb.collect();

        assert!(errors.is_empty());
        assert_eq!(&cow_str, raw_str);
    }

    #[test]
    fn can_produce_correct_string_simple_escaped() {
        let raw_str = r"mrb \r\r\n\n\n";
        let mut errors = thin_vec![];
        let sb = UnescapedStringBuilder::new(raw_str, 0, &mut errors);

        let cow_str: Cow<_> = sb.collect();

        assert!(errors.is_empty());
        assert_eq!(&cow_str, "mrb \r\r\n\n\n");
    }

    #[test]
    fn can_produce_correct_string_hex_escaped() {
        let raw_str = r#"mrb \r\n \t\t\t \x0D \u0027 \U00000022"#;
        let mut errors = thin_vec![];
        let sb = UnescapedStringBuilder::new(raw_str, 0, &mut errors);

        let cow_str: Cow<_> = sb.collect();

        assert!(errors.is_empty());
        assert_eq!(&cow_str, "mrb \r\n \t\t\t \r \' \"");
    }
}
