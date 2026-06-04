#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use ezscn_ast::*;
use ezscn_ast::expression::Expression;
use ezscn_ast::statement::Statement;
use ezscn_error::{ParseError, ParseErrorKind};
use ezscn_lexer::TokenStream;
use ezscn_tokens::{Span, Spanned, Token, TokenKind};
use thin_vec::{thin_vec, ThinVec};

pub mod expression;
pub mod statement;
pub(crate) mod string;

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
    pub fn item(&mut self) -> Option<Item<'t>> {
        match self.peek().map(|t| t.kind)? {
            TokenKind::EnumKeyword => self.enum_item(),
            TokenKind::StructKeyword => self.struct_item(),
            TokenKind::ConfigKeyword => self.config_item(),
            TokenKind::FuncKeyword => self.func_item(),
            TokenKind::SigKeyword => self.sig_item(),
            TokenKind::ImportKeyword => self.import_item(),
            TokenKind::FeatureKeyword => self.feature_item(),
            _ => self.statement_item()
        }
    }

    #[inline]
    pub fn enum_item(&mut self) -> Option<Item<'t>> {
        let enum_kw = self.advance_until_kind(TokenKind::EnumKeyword)?;
        let flags = self.next_if_kind(TokenKind::FlagsKeyword).is_some();
        let identifier = self.advance_until_identifier()?;
        let derived_type = if self.next_if_kind(TokenKind::Colon).is_some() {
            Some(self.return_type()?)
        } else {
            None
        };

        self.advance_until_kind(TokenKind::CurlyBracketLeft)?;
        let items = self.comma_seperated_map(TokenKind::CurlyBracketRight, Self::enum_member)?;
        let cbr = self.advance_until_kind(TokenKind::CurlyBracketRight)?;
        let span = Span::new_spanned(enum_kw.span, cbr.span);
        let kind = ItemKind::Enum(EnumItem { identifier, items, flags, derived_type });

        Some(Item { kind, span })
    }

    #[inline]
    fn enum_member(&mut self) -> Option<EnumMember<'t>> {
        let identifier = self.advance_until_identifier()?;
        let default_value = if self.next_if_kind(TokenKind::Equals).is_some() {
            Some(self.expression()?)
        } else {
            None
        };

        Some(EnumMember { identifier, default_value })
    }

    #[inline]
    pub fn struct_item(&mut self) -> Option<Item<'t>> {
        enum MemberType {
            Field,
            Tuple,
            Zero,
        }

        let struct_kw = self.advance_until_kind(TokenKind::StructKeyword)?;
        let identifier_token = self.advance_until_identifier_spanned()?;
        let member_type = self.advance_map(|token| {
            match token {
                Some(Token { kind: TokenKind::ParanthesisLeft, .. }) =>
                    Ok(MemberType::Tuple),
                Some(Token { kind: TokenKind::CurlyBracketLeft, .. }) =>
                    Ok(MemberType::Field),
                Some(Token { kind: TokenKind::Semicolon, .. }) =>
                    Ok(MemberType::Zero),
                Some(Token { kind, span }) =>
                    Err(ParseError::new(ParseErrorKind::InvalidStructToken(kind), span)),
                None => {
                    let span = Span::new_spanned(struct_kw.span, identifier_token.span);
                    Err(ParseError::new(ParseErrorKind::UnterminatedStruct, span))
                }
            }
        })?;

        let identifier = identifier_token.data;
        let (members, end_token) = match member_type {
            MemberType::Tuple => {
                let tuple_list = self.comma_seperated_map(TokenKind::ParanthesisRight, Self::return_type)?;
                let pr = self.advance_until_kind(TokenKind::ParanthesisRight)?;

                (StructMemberDefinition::Tuple(tuple_list), pr)
            },
            MemberType::Field => {
                let tuple_list = self.comma_seperated_map(TokenKind::CurlyBracketRight, Self::struct_field)?;
                let cbr = self.advance_until_kind(TokenKind::CurlyBracketRight)?;

                (StructMemberDefinition::Field(tuple_list), cbr)
            },
            MemberType::Zero => (StructMemberDefinition::Zero, self.advance_until_kind(TokenKind::Semicolon)?),
        };

        let span = Span::new_spanned(struct_kw.span, end_token.span);
        let kind = ItemKind::Struct(StructItem { identifier, members });

        Some(Item { kind, span })
    }

    #[inline]
    fn struct_field(&mut self) -> Option<Field<'t>> {
        let identifier = self.advance_until_identifier()?;
        self.advance_until_kind(TokenKind::Colon)?;
        let return_type = self.return_type()?;

        Some(Field { identifier, return_type })
    }

    #[inline]
    pub fn feature_item(&mut self) -> Option<Item<'t>> {
        let feature_kw = self.advance_until_kind(TokenKind::FeatureKeyword)?;
        let feature_ident = self.advance_until_path()?;
        let implementation = if self.next_if_kind(TokenKind::ForKeyword).is_some() {
            Some(self.advance_until_path()?)
        } else {
            None
        };

        self.advance_until_kind(TokenKind::CurlyBracketLeft)?;
        let mut items = thin_vec![];
        while !self.is_next(TokenKind::CurlyBracketRight) {
            items.push(self.item()?)
        }

        let cbr = self.advance_until_kind(TokenKind::CurlyBracketRight)?;
        let span = Span::new_spanned(feature_kw.span, cbr.span);
        let kind = ItemKind::Feature(FeatureItem { feature_ident, implementation, items });

        Some(Item { kind, span })
    }


    #[inline]
    pub fn import_item(&mut self) -> Option<Item<'t>> {
        let import_kw = self.advance_until_kind(TokenKind::ImportKeyword)?;
        let path = self.advance_until_path()?;
        let semicolon = self.advance_until_kind(TokenKind::Semicolon)?;
        let span = Span::new_spanned(import_kw.span, semicolon.span);
        let kind = ItemKind::Import(path);

        Some(Item { kind, span })
    }

    #[inline]
    pub fn config_item(&mut self) -> Option<Item<'t>> {
        let config_kw = self.advance_until_kind(TokenKind::ConfigKeyword)?;
        self.advance_until_kind(TokenKind::CurlyBracketLeft)?;
        let members = self.comma_seperated_map(TokenKind::CurlyBracketRight, Self::config_member)?;
        let cbr = self.advance_until_kind(TokenKind::CurlyBracketRight)?;
        let span = Span::new_spanned(config_kw.span, cbr.span);
        let kind = ItemKind::Config(ConfigItem { members });

        Some(Item { kind, span })
    }

    fn config_member(&mut self) -> Option<ConfigMember<'t>> {
        let identifier = self.advance_until_identifier()?;
        self.advance_until_kind(TokenKind::Equals)?;
        let expression = self.expression()?;

        Some(ConfigMember { identifier, expression })
    }

    #[inline]
    pub fn func_item(&mut self) -> Option<Item<'t>> {
        let func_kw = self.advance_until_kind(TokenKind::FuncKeyword)?;
        let identifier = self.advance_until_identifier()?;
        self.advance_until_kind(TokenKind::ParanthesisLeft)?;
        let params = self.comma_seperated_map(TokenKind::ParanthesisRight, Self::func_param)?;
        let return_type = if self.next_if_kind(TokenKind::Colon).is_some() {
            Some(self.return_type()?)
        } else {
            None
        };

        let block = statement::block(self)?;
        let span = Span::new_spanned(func_kw.span, block.span);
        let kind = ItemKind::Func(FuncItem { identifier, params, block, return_type });

        Some(Item { kind, span })
    }

    #[inline]
    fn func_param(&mut self) -> Option<FuncParam<'t>> {
        let identifier = self.advance_until_identifier()?;
        self.advance_until_kind(TokenKind::Colon)?;
        let return_type = self.return_type()?;

        Some(FuncParam { identifier, return_type })
    }

    #[inline]
    pub fn sig_item(&mut self) -> Option<Item<'t>> {
        let sig_kw = self.advance_until_kind(TokenKind::SigKeyword)?;
        let sig_type = if self.next_if(|t| t.kind == TokenKind::LessThan).is_some() {
            let return_type = self.return_type()?;
            self.advance_until_kind(TokenKind::GreaterThan)?;

            Some(return_type)
        } else {
            None
        };

        let identifier = self.advance_until_identifier()?;
        let semicolon = self.advance_until_kind(TokenKind::Semicolon)?;

        let span = Span::new_spanned(sig_kw.span, semicolon.span);
        let kind = ItemKind::Sig(SigItem { sig_type, identifier });

        Some(Item { kind, span })
    }

    #[inline]
    pub fn statement_item(&mut self) -> Option<Item<'t>> {
        let statement = statement::statement(self)?;
        let span = statement.span;
        let kind = ItemKind::Statement(statement);

        Some(Item { kind, span })
    }

    #[inline]
    pub fn expression(&mut self) -> Option<Expression<'t>> {
        expression::expression(self)
    }

    #[inline]
    pub fn return_type(&mut self) -> Option<ReturnTypes<'t>> {
        let primary_return_type = if self.next_if_kind(TokenKind::ParanthesisLeft).is_some() {
            let types = self.comma_seperated_map(TokenKind::ParanthesisRight, Self::return_type)?;
            self.advance_until_kind(TokenKind::ParanthesisRight)?;

            ReturnTypes::Tuple(types)
        } else {
            ReturnTypes::Type(self.advance_until_path()?)
        };

        let post_return_type = if self.next_if_kind(TokenKind::SquareBracketLeft).is_some() {
            self.advance_until_kind(TokenKind::SquareBracketRight)?;

            ReturnTypes::Array(Box::new(primary_return_type))
        } else {
            primary_return_type
        };

        if self.next_if_kind(TokenKind::QuestionMark).is_some() {
            Some(ReturnTypes::Nullable(Box::new(post_return_type)))
        } else {
            Some(post_return_type)
        }
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

        let span = Span::new_spanned(first_identifier.span, last_span);
        Some(Path { identifiers, span })
    }

    #[inline]
    pub fn advance_until_identifier(&mut self) -> Option<&'t str> {
        let token = self.advance_until_kind(TokenKind::Identifier)?;
        Some(&self.input[token.span])
    }

    #[inline]
    pub fn advance_until_identifier_spanned(&mut self) -> Option<Spanned<&'t str>> {
        let token = self.advance_until_kind(TokenKind::Identifier)?;
        let span = token.span;

        Some(Spanned::new(&self.input[token.span], span))
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
    pub(crate) fn is_next(&mut self, kind: TokenKind) -> bool {
        self.token_stream.is_next(kind)
    }

    #[inline]
    pub(crate) fn reached_eof(&mut self) -> bool {
        self.token_stream.reached_eof()
    }

    pub fn next_if_map<T>(&mut self, f: impl FnOnce(Option<Token>) -> Result<T, Option<Token>>) -> Option<T> {
        self.token_stream.next_if_map(f)
    }

    pub fn next_if(&mut self, f: impl FnOnce(&Token) -> bool) -> Option<Token> {
        self.token_stream.next_if(f)
    }

    #[inline]
    pub fn next_if_kind(&mut self, kind: TokenKind) -> Option<Token> {
        self.next_if(|t| t.kind == kind)
    }

    #[inline]
    pub(crate) fn advance_until_kind(&mut self, expected: TokenKind) -> Option<Token> {
        self.advance_map(|t| {
            match t {
                Some(token) if token.kind == expected => Ok(token),
                Some(token) => {
                    let kind = ParseErrorKind::UnexpectedToken(expected, token.kind);
                    let span = token.span;
                    Err(ParseError { kind, span })
                },
                None => {
                    let kind = ParseErrorKind::ExpectedToken(expected);
                    let span = Span::empty_from_start(self.input.len());
                    Err(ParseError { kind, span })
                }
            }
        })
    }

    pub(crate) fn advance_map<T>(&mut self, f: impl Fn(Option<Token>) -> Result<T, ParseError>) -> Option<T> {
        let errors = &mut self.errors;
        while !self.token_stream.reached_eof() {
            let t_option = self.token_stream.next_if_map(|token| {
                match (f)(token) {
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

#[cfg(test)]
mod tests {
    use super::*;
    // TODO: Test cases
}
