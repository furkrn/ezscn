#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use ezscn_ast::*;
use ezscn_ast::expression::Expression;
use ezscn_ast::statement::IdentifierOrUnderscore;
use ezscn_error::{ParseError, ParseErrorKind};
use ezscn_lexer::TokenStream;
use ezscn_tokens::{Span, SpanImpl, Spanned, Token, TokenKind};
use thin_vec::{thin_vec, ThinVec};

pub mod expression;
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

#[derive(Default, Debug)]
pub struct ParseErrored;

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
            TokenKind::PubKeyword => self.modifier_item(),
            TokenKind::Tag => self.attribute_collection_item(),
            _ => self.statement_item()
        }
    }

    #[inline]
    pub fn modifier_item(&mut self) -> Option<Item<'t>> {
        let vis = self.modifier()?;
        let item = self.item()?;

        let span = Span::new_spanned(vis.span, item.span);
        let kind = ItemKind::Visible(vis.data, Box::new(item));

        Some(Item { kind, span })
    }

    #[inline]
    fn modifier(&mut self) -> Option<Spanned<VisibilityModifiers>> {
        let t = self.advance_until_kind(TokenKind::PubKeyword)?;

        Some(Spanned::new(VisibilityModifiers::Public, t.span))
    }

    #[inline]
    pub fn attribute_collection_item(&mut self) -> Option<Item<'t>> {
        let first_t_token = self.advance_until_kind(TokenKind::Tag)?;
        let mut current = first_t_token;
        let mut attributes = thin_vec![];
        loop {
            let peek = self.peek()?;
            if peek.kind == TokenKind::Tag {
                current = self.next()?;
            } else if peek.kind == TokenKind::Comma {
                self.next()?;
            } else if peek.line == current.line {
                attributes.push(self.advance_until_path()?)
            } else {
                break
            }
        }

        let item = self.item()?;
        let span = Span::new_spanned(first_t_token.span, item.span);
        let kind = ItemKind::AttributeCollectedItem(attributes, Box::new(item));

        Some(Item { kind, span })
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
        let struct_kw = self.advance_until_kind(TokenKind::StructKeyword)?;
        let identifier_token = self.advance_until_identifier_spanned()?;
        let generics = self.generics()
            .ok()?;

        let where_clause = self.where_clause()
            .ok()?;

        let token = self.advance_map(|token| {
            match token {
                Ok(token) if matches!(token.kind, TokenKind::Semicolon | TokenKind::ParanthesisLeft | TokenKind::CurlyBracketLeft) =>
                    Ok(token),
                Ok(token) =>
                    Err(ParseError::new(ParseErrorKind::InvalidStructToken(token.kind), token.span, token.line)),
                Err(EndLineInformation { len, .. }) => {
                    let span = Span::new_spanned(struct_kw.span, Span::empty_from_start(len));
                    Err(ParseError::new(ParseErrorKind::UnterminatedStruct, span, struct_kw.line))
                }
            }
        })?;

        let identifier = identifier_token.data;
        let (members, end_token) = match token.kind {
            TokenKind::ParanthesisLeft => {
                let tuple_list = self.comma_seperated_map(TokenKind::ParanthesisRight, Self::return_type)?;
                self.advance_until_kind(TokenKind::ParanthesisRight)?;

                (StructMemberDefinition::Tuple(tuple_list), self.advance_until_kind(TokenKind::Semicolon)?)
            },
            TokenKind::CurlyBracketLeft => {
                let tuple_list = self.comma_seperated_map(TokenKind::CurlyBracketRight, Self::struct_field)?;

                (StructMemberDefinition::Field(tuple_list), self.advance_until_kind(TokenKind::CurlyBracketRight)?)
            },
            _ => (StructMemberDefinition::Zero, token),
        };

        let span = Span::new_spanned(struct_kw.span, end_token.span);
        let kind = ItemKind::Struct(StructItem { identifier, members, generics, where_clause });

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
        let generics = self.generics()
            .ok()?;

        let implementation = if self.next_if_kind(TokenKind::ForKeyword).is_some() {
            Some(self.advance_until_path()?)
        } else {
            None
        };

        let where_clause = self.where_clause()
            .ok()?;

        self.advance_until_kind(TokenKind::CurlyBracketLeft)?;
        let mut items = thin_vec![];
        while !self.is_next(TokenKind::CurlyBracketRight) {
            items.push(self.item()?)
        }

        let cbr = self.advance_until_kind(TokenKind::CurlyBracketRight)?;
        let span = Span::new_spanned(feature_kw.span, cbr.span);
        let kind = ItemKind::Feature(FeatureItem { feature_ident, implementation, items, generics, where_clause });

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
        let generics = self.generics()
            .ok()?;

        self.advance_until_kind(TokenKind::ParanthesisLeft)?;
        let params = self.comma_seperated_map(TokenKind::ParanthesisRight, Self::func_param)?;
        self.advance_until_kind(TokenKind::ParanthesisRight)?;
        let where_clause = self.where_clause()
            .ok()?;

        let return_type = if self.next_if_kind(TokenKind::Colon).is_some() {
            Some(self.return_type()?)
        } else {
            None
        };

        let (block, end_span) = if !self.is_next(TokenKind::Semicolon) {
            let block = statement::block(self)?;
            let span = block.span;

            (Some(block), span)
        } else {
            (None, self.next()?.span)
        };

        let span = Span::new_spanned(func_kw.span, end_span);
        let kind = ItemKind::Func(FuncItem { identifier, params, block, return_type, where_clause, generics });

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
    pub fn generics(&mut self) -> Result<Option<Generics<'t>>, ParseErrored> {
        let Some(sbl) = self.next_if_kind(TokenKind::SquareBracketLeft) else {
            return Ok(None);
        };

        let generic_parameters = self.comma_seperated_map(TokenKind::SquareBracketRight, Self::generic_param)
            .ok_or(ParseErrored)?;

        let sbr = self.advance_until_kind(TokenKind::SquareBracketRight)
            .ok_or(ParseErrored)?;

        let span = Span::new_spanned(sbl.span, sbr.span);

        Ok(Some(Generics::new(generic_parameters, span)))
    }

    #[inline]
    fn generic_param(&mut self) -> Option<GenericParam<'t>> {
        let identifier_spanned = self.advance_until_identifier_spanned()?;
        let identifier = identifier_spanned.data;
        let mut last_span = identifier_spanned.span;
        let constrait = if self.next_if_kind(TokenKind::Colon).is_some() {
            let mut constrait = thin_vec![];
            loop {
                let return_type = self.return_type()?;
                last_span = return_type.span;
                constrait.push(return_type);

                if !self.next_if_kind(TokenKind::Plus).is_none() {
                    break
                }
            }

            Some(constrait)
        } else {
            None
        };

        let span = Span::new_spanned(identifier_spanned.span, last_span);

        Some(GenericParam { identifier, constrait, span })
    }

    #[inline]
    pub fn where_clause(&mut self) -> Result<Option<WhereClause<'t>>, ParseErrored> {
        let Some(where_clause) = self.next_if_kind(TokenKind::WhereKeyword) else {
            return Ok(None);
        };

        let mut generics = thin_vec![];
        let mut last_span = where_clause.span;
        loop {
            if !self.is_next(TokenKind::Identifier) {
                break
            }

            let generic_constrait = self.generic_constrait()
                .ok_or(ParseErrored)?;

            if let Some(comma) = self.next_if_kind(TokenKind::Comma) {
                last_span = comma.span;
            } else {
                last_span = generic_constrait.span;
            }

            generics.push(generic_constrait);
        }

        let span = Span::new_spanned(where_clause.span, last_span);

        Ok(Some(WhereClause::new(generics, span)))
    }

    fn generic_constrait(&mut self) -> Option<GenericConstrait<'t>> {
        let identifier_spanned = self.advance_until_identifier_spanned()?;
        let identifier = identifier_spanned.data;
        let mut constraits = thin_vec![];
        let last_span = loop {
            let return_type = self.return_type()?;
            let rt_span = return_type.span;
            constraits.push(return_type);

            if !self.next_if_kind(TokenKind::Plus).is_none() {
                break rt_span
            }
        };

        let span = Span::new_spanned(identifier_spanned.span, last_span);

        Some(GenericConstrait { identifier, constraits, span })
    }

    #[inline]
    pub fn return_type(&mut self) -> Option<ReturnType<'t>> {
        let mut return_type = if let Some(pl) = self.next_if_kind(TokenKind::ParanthesisLeft) {
            let types = self.comma_seperated_map(TokenKind::ParanthesisRight, Self::return_type)?;
            let pr = self.advance_until_kind(TokenKind::ParanthesisRight)?;
            let span = Span::new_spanned(pl.span, pr.span);
            let kind = ReturnTypeKind::Tuple(types);

            ReturnType { kind, span }
        } else {
            let type_name = self.advance_until_path()?;
            let span = type_name.span;
            let kind = ReturnTypeKind::Type(type_name);

            ReturnType { kind, span }
        };

        loop {
            return_type = match self.next_if(|t| matches!(t.kind, TokenKind::SquareBracketLeft | TokenKind::QuestionMark)) {
                Some(Token { kind: TokenKind::SquareBracketLeft, .. }) => {
                    let start_span = return_type.span;
                    let kind = if self.is_next(TokenKind::SquareBracketRight) {
                        ReturnTypeKind::Array(Box::new(return_type))
                    } else {
                        let generic_parameters = self.comma_seperated_map(TokenKind::SquareBracketRight, Self::return_type)?;
                        ReturnTypeKind::Generic(Box::new(return_type), generic_parameters)
                    };

                    let sbr = self.advance_until_kind(TokenKind::SquareBracketRight)?;
                    let span = Span::new_spanned(start_span, sbr.span);

                    ReturnType { kind, span }
                },
                Some(Token { kind: TokenKind::QuestionMark, span, .. }) => {
                    let span = Span::new_spanned(return_type.span, span);
                    let kind = ReturnTypeKind::Nullable(Box::new(return_type));

                    ReturnType { kind, span }
                }
                _ => break
            };
        }

        Some(return_type)
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

    #[inline]
    pub(crate) fn reached_eof(&mut self) -> bool {
        self.token_stream.reached_eof()
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

#[cfg(test)]
mod tests {
    use core::assert_matches;
    use ezscn_ast::{expression::{ExpressionKind, LiteralExpression}};
    use super::*;
    // TODO: Test cases
}
