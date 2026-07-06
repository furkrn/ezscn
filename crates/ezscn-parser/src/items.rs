use alloc::boxed::Box;
use ezscn_ast::*;
use ezscn_error::{ParseError, ParseErrorKind};
use ezscn_tokens::{Token, TokenKind, Span, SpanImpl, Spanned};
use thin_vec::thin_vec;

use crate::{EndLineInformation, Parser};
use crate::statement::{block, statement};

#[derive(Default, Debug)]
pub struct ParseErrored;

#[inline]
pub fn item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    match parser.peek().map(|t| t.kind)? {
        TokenKind::EnumKeyword => enum_item(parser),
        TokenKind::StructKeyword => struct_item(parser),
        TokenKind::LocalKeyword => field_item(parser),
        TokenKind::ConfigKeyword => config_item(parser),
        TokenKind::FuncKeyword => func_item(parser),
        TokenKind::SigKeyword => sig_item(parser),
        TokenKind::ImportKeyword => import_item(parser),
        TokenKind::FeatureKeyword => feature_item(parser),
        TokenKind::PubKeyword => modifier_item(parser),
        TokenKind::Tag => attribute_collection_item(parser),
        _ => statement_item(parser)
    }
}

#[inline]
pub fn modifier_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let vis = modifier(parser)?;
    let item = item(parser)?;

    let span = Span::merge(vis.span, item.span);
    let kind = ItemKind::Visible(vis.data, Box::new(item));

    Some(Item { kind, span })
}

#[inline]
fn modifier<'t>(parser: &mut Parser<'t>) -> Option<Spanned<VisibilityModifiers>> {
    let t = parser.advance_until_kind(TokenKind::PubKeyword)?;

    Some(Spanned::new(VisibilityModifiers::Public, t.span))
}

#[inline]
pub fn attribute_collection_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let first_t_token = parser.advance_until_kind(TokenKind::Tag)?;
    let mut current = first_t_token;
    let mut attributes = thin_vec![];
    loop {
        let peek = parser.peek()?;
        if peek.kind == TokenKind::Tag {
            current = parser.next()?;
        } else if peek.kind == TokenKind::Comma {
            parser.next()?;
        } else if peek.line == current.line {
            attributes.push(parser.advance_until_path()?)
        } else {
            break
        }
    }

    let item = item(parser)?;
    let span = Span::merge(first_t_token.span, item.span);
    let kind = ItemKind::AttributeCollectedItem(attributes, Box::new(item));

    Some(Item { kind, span })
}

#[inline]
pub fn enum_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let enum_kw = parser.advance_until_kind(TokenKind::EnumKeyword)?;
    let flags = parser.next_if_kind(TokenKind::FlagsKeyword).is_some();
    let identifier = parser.advance_until_identifier()?;
    let derived_type = if parser.next_if_kind(TokenKind::Colon).is_some() {
        Some(return_type(parser)?)
    } else {
        None
    };

    parser.advance_until_kind(TokenKind::CurlyBracketLeft)?;
    let items = parser.comma_seperated_map(TokenKind::CurlyBracketRight, enum_member)?;
    let cbr = parser.advance_until_kind(TokenKind::CurlyBracketRight)?;
    let span = Span::merge(enum_kw.span, cbr.span);
    let kind = ItemKind::Enum(EnumItem { identifier, items, flags, derived_type });

    Some(Item { kind, span })
}

#[inline]
fn enum_member<'t>(parser: &mut Parser<'t>) -> Option<EnumMember<'t>> {
    let identifier = parser.advance_until_identifier()?;
    let default_value = if parser.next_if_kind(TokenKind::Equals).is_some() {
        Some(parser.expression()?)
    } else {
        None
    };

    Some(EnumMember { identifier, default_value })
}

#[inline]
pub fn struct_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let struct_kw = parser.advance_until_kind(TokenKind::StructKeyword)?;
    let identifier = parser.advance_until_identifier()?;
    let generics = generics(parser)
        .ok()?;

    let where_clause = where_clause(parser)
        .ok()?;

    let mut tuple_members = thin_vec![];
    if parser.next_if_kind(TokenKind::ParanthesisLeft).is_some() {
        tuple_members = parser.comma_seperated_map(TokenKind::ParanthesisRight, return_type)?;
        parser.advance_until_kind(TokenKind::ParanthesisRight)?;
    }

    let mut items = thin_vec![];
    let token = parser.advance_map(|token| {
        match token {
            Ok(token) if matches!(token.kind, TokenKind::Semicolon | TokenKind::CurlyBracketLeft) =>
                Ok(token),
            Ok(token) =>
                Err(ParseError::new(ParseErrorKind::InvalidStructToken(token.kind), token.span, token.line)),
            Err(EndLineInformation { len, .. }) => {
                let span = Span::merge(struct_kw.span, Span::empty_from_start(len));
                Err(ParseError::new(ParseErrorKind::UnterminatedStruct, span, struct_kw.line))
            }
        }
    })?;

    let end_token = if token.kind == TokenKind::CurlyBracketLeft {
        while !parser.is_next(TokenKind::CurlyBracketRight) {
            items.push(item(parser)?)
        }

        parser.advance_until_kind(TokenKind::CurlyBracketRight)?
    } else {
        token
    };

    let span = Span::merge(struct_kw.span, end_token.span);
    let kind = ItemKind::Struct(StructItem { identifier, tuple_members, generics, where_clause, items });

    Some(Item { kind, span })
}

#[inline]
fn field_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let local_kw = parser.advance_until_kind(TokenKind::LocalKeyword)?;
    let identifier = parser.advance_until_identifier()?;
    parser.advance_until_kind(TokenKind::Colon)?;
    let return_type = return_type(parser)?;
    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;

    let field = FieldItem { identifier, return_type };
    let span = Span::merge(local_kw.span, semicolon.span);

    Some(Item { kind: ItemKind::Field(field), span })
}

#[inline]
pub fn feature_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let feature_kw = parser.advance_until_kind(TokenKind::FeatureKeyword)?;
    let feature_ident = parser.advance_until_path()?;
    let generics = generics(parser)
        .ok()?;

    let implementation = if parser.next_if_kind(TokenKind::ForKeyword).is_some() {
        Some(parser.advance_until_path()?)
    } else {
        None
    };

    let where_clause = where_clause(parser)
        .ok()?;

    parser.advance_until_kind(TokenKind::CurlyBracketLeft)?;
    let mut items = thin_vec![];
    while !parser.is_next(TokenKind::CurlyBracketRight) {
        items.push(item(parser)?)
    }

    let cbr = parser.advance_until_kind(TokenKind::CurlyBracketRight)?;
    let span = Span::merge(feature_kw.span, cbr.span);
    let kind = ItemKind::Feature(FeatureItem { feature_ident, implementation, items, generics, where_clause });

    Some(Item { kind, span })
}


#[inline]
pub fn import_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let import_kw = parser.advance_until_kind(TokenKind::ImportKeyword)?;
    let path = parser.advance_until_path()?;
    let alias = if parser.next_if_kind(TokenKind::AsKeyword).is_some() {
        Some(parser.advance_until_identifier()?)
    } else {
        None
    };
    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;
    let span = Span::merge(import_kw.span, semicolon.span);
    let kind = ItemKind::Import(ImportItem { path, alias });

    Some(Item { kind, span })
}

#[inline]
pub fn config_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let config_kw = parser.advance_until_kind(TokenKind::ConfigKeyword)?;
    parser.advance_until_kind(TokenKind::CurlyBracketLeft)?;
    let members = parser.comma_seperated_map(TokenKind::CurlyBracketRight, config_member)?;
    let cbr = parser.advance_until_kind(TokenKind::CurlyBracketRight)?;
    let span = Span::merge(config_kw.span, cbr.span);
    let kind = ItemKind::Config(ConfigItem { members });

    Some(Item { kind, span })
}

fn config_member<'t>(parser: &mut Parser<'t>) -> Option<ConfigMember<'t>> {
    let identifier = parser.advance_until_identifier()?;
    parser.advance_until_kind(TokenKind::Equals)?;
    let expression = parser.expression()?;

    Some(ConfigMember { identifier, expression })
}

#[inline]
pub fn func_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let func_kw = parser.advance_until_kind(TokenKind::FuncKeyword)?;
    let identifier = parser.advance_until_identifier()?;
    let generics = generics(parser)
        .ok()?;

    parser.advance_until_kind(TokenKind::ParanthesisLeft)?;
    let params = parser.comma_seperated_map(TokenKind::ParanthesisRight, func_param)?;
    parser.advance_until_kind(TokenKind::ParanthesisRight)?;
    let return_type = if parser.next_if_kind(TokenKind::Colon).is_some() {
        Some(return_type(parser)?)
    } else {
        None
    };

    let where_clause = where_clause(parser)
        .ok()?;

    let (block, end_span) = if !parser.is_next(TokenKind::Semicolon) {
        let block = block(parser)?;
        let span = block.span;

        (Some(block), span)
    } else {
        (None, parser.next()?.span)
    };

    let span = Span::merge(func_kw.span, end_span);
    let kind = ItemKind::Func(FuncItem { identifier, params, block, return_type, where_clause, generics });

    Some(Item { kind, span })
}

#[inline]
fn func_param<'t>(parser: &mut Parser<'t>) -> Option<FuncParam<'t>> {
    let fn_param = if parser.next_if_kind(TokenKind::SelfKeyword).is_some() {
        FuncParam::SelfP
    } else {
        let identifier = parser.advance_until_identifier()?;
        parser.advance_until_kind(TokenKind::Colon)?;
        let return_type = return_type(parser)?;

        FuncParam::Typed(identifier, return_type)
    };

    Some(fn_param)
}

#[inline]
pub fn sig_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let sig_kw = parser.advance_until_kind(TokenKind::SigKeyword)?;
    let sig_type = if parser.next_if(|t| t.kind == TokenKind::SquareBracketLeft).is_some() {
        let return_type = return_type(parser)?;
        parser.advance_until_kind(TokenKind::SquareBracketRight)?;

        Some(return_type)
    } else {
        None
    };

    let identifier = parser.advance_until_identifier()?;
    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;

    let span = Span::merge(sig_kw.span, semicolon.span);
    let kind = ItemKind::Sig(SigItem { sig_type, identifier });

    Some(Item { kind, span })
}

#[inline]
pub fn statement_item<'t>(parser: &mut Parser<'t>) -> Option<Item<'t>> {
    let statement = statement(parser)?;
    let span = statement.span;
    let kind = ItemKind::Statement(statement);

    Some(Item { kind, span })
}

#[inline]
pub fn generics<'t>(parser: &mut Parser<'t>) -> Result<Option<Generics<'t>>, ParseErrored> {
    let Some(sbl) = parser.next_if_kind(TokenKind::SquareBracketLeft) else {
        return Ok(None);
    };

    let generic_parameters = parser.comma_seperated_map(TokenKind::SquareBracketRight, generic_param)
        .ok_or(ParseErrored)?;

    let sbr = parser.advance_until_kind(TokenKind::SquareBracketRight)
        .ok_or(ParseErrored)?;

    let span = Span::merge(sbl.span, sbr.span);

    Ok(Some(Generics::new(generic_parameters, span)))
}

#[inline]
fn generic_param<'t>(parser: &mut Parser<'t>) -> Option<GenericParam<'t>> {
    let identifier_spanned = parser.advance_until_identifier_spanned()?;
    let identifier = identifier_spanned.data;
    let mut last_span = identifier_spanned.span;
    let constraits = if parser.next_if_kind(TokenKind::Colon).is_some() {
        let mut vec = thin_vec![];
        loop {
            let return_type = return_type(parser)?;
            last_span = return_type.span;
            vec.push(return_type);

            if !parser.is_next(TokenKind::Plus) {
                break
            }

            parser.advance_until_kind(TokenKind::Plus)?;
        }

        Some(vec)
    } else {
        None
    };

    let span = Span::merge(identifier_spanned.span, last_span);

    Some(GenericParam { identifier, constraits, span })
}

#[inline]
pub fn where_clause<'t>(parser: &mut Parser<'t>) -> Result<Option<WhereClause<'t>>, ParseErrored> {
    let Some(where_clause) = parser.next_if_kind(TokenKind::WhereKeyword) else {
        return Ok(None);
    };

    let mut generics = thin_vec![];
    let mut last_span = where_clause.span;
    loop {
        if !parser.is_next(TokenKind::Identifier) {
            break
        }

        let generic_constrait = generic_constrait(parser)
            .ok_or(ParseErrored)?;

        if let Some(comma) = parser.next_if_kind(TokenKind::Comma) {
            last_span = comma.span;
        } else {
            last_span = generic_constrait.span;
        }

        generics.push(generic_constrait);
    }

    let span = Span::merge(where_clause.span, last_span);

    Ok(Some(WhereClause::new(generics, span)))
}

fn generic_constrait<'t>(parser: &mut Parser<'t>) -> Option<GenericConstrait<'t>> {
    let identifier_spanned = parser.advance_until_identifier_spanned()?;
    let identifier = identifier_spanned.data;
    parser.advance_until_kind(TokenKind::Colon)?;
    let mut constraits = thin_vec![];
    let last_span = loop {
        let return_type = return_type(parser)?;
        let rt_span = return_type.span;
        constraits.push(return_type);

        if !parser.is_next(TokenKind::Plus) {
            break rt_span
        }

        parser.next_if_kind(TokenKind::Plus)?;
    };

    let span = Span::merge(identifier_spanned.span, last_span);

    Some(GenericConstrait { identifier, constraits, span })
}

#[inline]
pub fn return_type<'t>(parser: &mut Parser<'t>) -> Option<ReturnType<'t>> {
    let mut return_type = if let Some(pl) = parser.next_if_kind(TokenKind::ParanthesisLeft) {
        let types = parser.comma_seperated_map(TokenKind::ParanthesisRight, return_type)?;
        let pr = parser.advance_until_kind(TokenKind::ParanthesisRight)?;
        let span = Span::merge(pl.span, pr.span);
        let kind = ReturnTypeKind::Tuple(types);

        ReturnType { kind, span }
    } else {
        let type_name = parser.advance_until_path()?;
        let span = type_name.span;
        let kind = ReturnTypeKind::Path(type_name);

        ReturnType { kind, span }
    };

    loop {
        return_type = match parser.next_if(|t| matches!(t.kind, TokenKind::SquareBracketLeft | TokenKind::QuestionMark)) {
            Some(Token { kind: TokenKind::SquareBracketLeft, .. }) => {
                let start_span = return_type.span;
                let kind = if parser.is_next(TokenKind::SquareBracketRight) {
                    ReturnTypeKind::Array(Box::new(return_type))
                } else {
                    let generic_parameters = parser.comma_seperated_map(TokenKind::SquareBracketRight, self::return_type)?;
                    ReturnTypeKind::Generic(Box::new(return_type), generic_parameters)
                };

                let sbr = parser.advance_until_kind(TokenKind::SquareBracketRight)?;
                let span = Span::merge(start_span, sbr.span);

                ReturnType { kind, span }
            },
            Some(Token { kind: TokenKind::QuestionMark, span, .. }) => {
                let span = Span::merge(return_type.span, span);
                let kind = ReturnTypeKind::Nullable(Box::new(return_type));

                ReturnType { kind, span }
            }
            _ => break
        };
    }

    Some(return_type)
}
