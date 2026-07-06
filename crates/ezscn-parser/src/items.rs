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

    let mut tuple_members = thin_vec![];
    if parser.next_if_kind(TokenKind::ParanthesisLeft).is_some() {
        tuple_members = parser.comma_seperated_map(TokenKind::ParanthesisRight, return_type)?;
        parser.advance_until_kind(TokenKind::ParanthesisRight)?;
    }

    let where_clause = where_clause(parser)
        .ok()?;

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

#[cfg(test)]
mod tests {
    use core::assert_matches;
    use ezscn_ast::{expression::{Expression, ExpressionKind, LiteralExpression}};
    use super::*;

    #[test]
    pub fn enum_item() {
        fn matches_e1_members(m: &[EnumMember<'_>]) -> bool {
            let mut iter = m.iter();

            assert_matches!(iter.next(), Some(EnumMember { identifier: "A", default_value: None }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "B", default_value: None }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "C", default_value: None }));
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_e2_members(m: &[EnumMember<'_>]) -> bool {
            let mut iter = m.iter();

            assert_matches!(iter.next(), Some(EnumMember { identifier: "X", default_value: Some(Expression { kind: ExpressionKind::Literal(..), .. }) }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "Y", default_value: Some(Expression { kind: ExpressionKind::Binary(..), .. }) }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "Z", default_value: Some(Expression { kind: ExpressionKind::Binary(..), .. }) }));
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_e3_members(m: &[EnumMember<'_>]) -> bool {
            let mut iter = m.iter();

            assert_matches!(iter.next(), Some(EnumMember { identifier: "M", default_value: None }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "N", default_value: None }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "P", default_value: Some(Expression { kind: ExpressionKind::Literal(..), .. }) }));
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_e4_members(m: &[EnumMember<'_>]) -> bool {
            let mut iter = m.iter();

            assert_matches!(iter.next(), Some(EnumMember { identifier: "K", default_value: None }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "L", default_value: None }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "M", default_value: None }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "N", default_value: None }));
            assert_matches!(iter.next(), Some(EnumMember { identifier: "P", default_value: None }));
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from(r#"
            enum E1 { A, B, C }
            enum flags E2 { X = 1, Y = 1 << 1, Z = 1 << 2 }
            enum E3: a { M, N, P = 9 }
            enum flags E4: bb { K, L, M, N, P }
            "#);

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Enum(EnumItem { identifier: "E1", items, flags: false, derived_type: None }), .. }) if matches_e1_members(&items));
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Enum(EnumItem { identifier: "E2", items, flags: true, derived_type: None }), .. }) if matches_e2_members(&items));
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Enum(EnumItem { identifier: "E3", items, flags: false, derived_type: Some(_) }), .. }) if matches_e3_members(&items));
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Enum(EnumItem { identifier: "E4", items, flags: true, derived_type: Some(_) }), .. }) if matches_e4_members(&items));
        assert!(parser.errors.is_empty());
        assert_matches!(item(&mut parser), None);
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn struct_item() {
        fn matches_tuple(p: &[ReturnType<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Path(Path { identifiers, .. }, ..), .. }) if *identifiers == ["a", "b", "c"]);
            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Array(_), .. }));
            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Nullable(_), .. }));
            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Tuple(t), .. }) if t.len() == 3);
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_zero_generic_struct_generics(p: &[GenericParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(GenericParam { identifier: "T", constraits: Some(c), .. }) if c.len() == 1);
            assert_matches!(iter.next(), Some(GenericParam { identifier: "A", constraits: None, .. }));
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_zero_generic_struct_where(p: &[GenericConstrait<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(GenericConstrait { identifier: "A", constraits, .. }) if constraits.len() == 1);
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_generic_tuple_generics(p: &[GenericParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(GenericParam { identifier: "T", constraits: None, .. }));
            assert_matches!(iter.next(), Some(GenericParam { identifier: "A", constraits: None, .. }));
            assert_matches!(iter.next(), Some(GenericParam { identifier: "B", constraits: Some(g), .. }) if g.len() == 1);
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_generic_tuple_where(p: &[GenericConstrait<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(GenericConstrait { identifier: "A", constraits, .. }) if constraits.len() == 2);
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_generic_field_generics(p: &[GenericParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(GenericParam { identifier: "T", constraits: None, .. }));
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_generic_field_where(p: &[GenericConstrait<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(GenericConstrait { identifier: "T", constraits, .. }) if constraits.len() == 2);
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from(r#"
            struct Zero;
            struct Tuple(a::b::c, a::b::c[], a::b::c[]?, (a, b, c));
            struct Tuple(a::b::c, a::b::c[], a::b::c[]?, (a, b, c)) {
                local x: y::z;
            }
            struct Field {
                local x: a::b::c;
                local y: a::b::c[];
                local z: a::b::c[]?;
                local t: (a, b, c);
            }

            struct ZeroGeneric[T: X, A] where A:T;
            struct ZeroGeneric[T: X, A];
            struct GenericTuple[T, A, B: x::y::z](T, A, B) where A: b::c::d + T;
            struct GenericTupleNW[T, A, B: x::y::z](T, A, B) { }
            struct GenericField[T] where T: x::y::z + a::b::c {
                local x: T;
            }

            struct GenericFieldNW[T] {
                local xyhjyltr: T;
            }
            "#);

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "Zero", generics: None, where_clause: None, tuple_members, items, .. }), ..})
            if tuple_members.is_empty() && items.is_empty());

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "Tuple", generics: None, where_clause: None, tuple_members, items, .. }), .. })
            if matches_tuple(&tuple_members) && items.is_empty());

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "Tuple", generics: None, where_clause: None, tuple_members, items, .. }), .. })
            if matches_tuple(&tuple_members) && items.len() == 1);

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "Field", generics: None, where_clause: None, tuple_members, items, .. }), .. })
            if tuple_members.is_empty() && items.len() == 4);

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "ZeroGeneric", generics: Some(g), where_clause: Some(w), tuple_members, items, .. }), ..})
            if tuple_members.is_empty() && items.is_empty() && matches_zero_generic_struct_generics(&g.data) && matches_zero_generic_struct_where(&w.data));

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "ZeroGeneric", generics: Some(g), where_clause: None, tuple_members, items, .. }), ..})
            if tuple_members.is_empty() && items.is_empty() && matches_zero_generic_struct_generics(&g.data));
        
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "GenericTuple", generics: Some(g), where_clause: Some(w), tuple_members, items, .. }), ..})
            if tuple_members.len() == 3 && items.is_empty() && matches_generic_tuple_generics(&g.data) && matches_generic_tuple_where(&w.data));

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "GenericTupleNW", generics: Some(g), where_clause: None, tuple_members, items, .. }), ..})
            if tuple_members.len() == 3 && items.is_empty() && matches_generic_tuple_generics(&g.data));
        
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "GenericField", generics: Some(g), where_clause: Some(w), tuple_members, items, .. }), ..})
            if tuple_members.is_empty() && items.len() == 1 && matches_generic_field_generics(&g.data) && matches_generic_field_where(&w.data));

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Struct(StructItem { identifier: "GenericFieldNW", generics: Some(g), where_clause: None, tuple_members, items, .. }), ..})
            if tuple_members.is_empty() && items.len() == 1 && matches_generic_field_generics(&g.data));

        assert_matches!(item(&mut parser), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn field_item() {
        let mut parser = Parser::from(r#"
            local x: ();
            local y: thing;
            "#);

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Field(FieldItem { identifier: "x", return_type: ReturnType { kind: ReturnTypeKind::Tuple(..), .. }}), .. }));
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Field(FieldItem { identifier: "y", return_type: ReturnType { kind: ReturnTypeKind::Path(..), .. }}), .. }));
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn feature_item() {
        let mut parser = Parser::from(r#"
            feature c::b::u {
                func test(): u8;
            }

            feature c::b::u for x::y::z {
                func test(): u8 { return 0; }
            }"#);

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Feature(FeatureItem { implementation: None, .. }), .. }));
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Feature(FeatureItem { implementation: Some(_), .. }), .. }));
        assert_matches!(item(&mut parser), None);
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn config_item() {
        fn matches_members(m: &[ConfigMember<'_>]) -> bool {
            let mut iter = m.iter();

            assert_matches!(iter.next(), Some(ConfigMember { identifier: "Ver", expression: Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. }}));
            assert_matches!(iter.next(), Some(ConfigMember { identifier: "Sio", expression: Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. }}));
            assert_matches!(iter.next(), Some(ConfigMember { identifier: "N", expression: Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(3)), .. }}));
            true
        }

        let mut parser = Parser::from(r#"
            config {
                Ver = 1,
                Sio = 2,
                N = 3,
            }"#);

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Config(ConfigItem { members }), .. }) if matches_members(&members));
        assert_matches!(item(&mut parser), None);
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn func_item() {
        fn matches_abstract_fn_params(p: &[FuncParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(FuncParam::SelfP));
            assert_matches!(iter.next(), Some(FuncParam::Typed("a", ReturnType { kind: ReturnTypeKind::Path(Path { identifiers, .. }), .. })) if *identifiers == ["b"]);
            assert_matches!(iter.next(), Some(FuncParam::Typed("c", ReturnType { kind: ReturnTypeKind::Array(_), .. })));
            assert_matches!(iter.next(), Some(FuncParam::Typed("e", ReturnType { kind: ReturnTypeKind::Nullable(_), .. })));
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_normal_fn_params(p: &[FuncParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(FuncParam::Typed("a", ReturnType { kind: ReturnTypeKind::Path(Path { identifiers, .. }), .. })) if *identifiers == ["b"]);
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_abstract_generic_fn_generics(p: &[GenericParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(GenericParam { identifier: "T", constraits: Some(g), .. }) if g.len() == 2);
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_abstract_generic_fn_params(p: &[FuncParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(FuncParam::SelfP));
            assert_matches!(iter.next(), Some(FuncParam::Typed("a", ..)));
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_abstract_generic_where_fn_generics(p: &[GenericParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(GenericParam { identifier: "X", constraits: None, .. }));
            assert_matches!(iter.next(), Some(GenericParam { identifier: "Y", constraits: None, .. }));
            assert_matches!(iter.next(), Some(GenericParam { identifier: "Z", constraits: Some(c), .. }) if c.len() == 1);
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_abstract_generic_where_fn_where(p: &[GenericConstrait<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(GenericConstrait { identifier: "X", constraits, .. }) if constraits.len() == 1);
            assert_matches!(iter.next(), Some(GenericConstrait { identifier: "Y", constraits, .. }) if constraits.len() == 2);
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_abstract_generic_where_fn_params(p: &[FuncParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(FuncParam::SelfP));
            assert_matches!(iter.next(), Some(FuncParam::Typed("x", ..)));
            assert_matches!(iter.next(), Some(FuncParam::Typed("y", ..)));
            assert_matches!(iter.next(), Some(FuncParam::Typed("z", ..)));
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from(r#"
            func abstract(self, a: b, c: d[], e: f?): h::h::h;
            func normal(a: b): hhh { a; b; c; }
            func empty() {}
            func empty_abstract();
            func abstract_generics[T: a::b::c + x::y::z](self, a: T): h::h::h;
            func abstract_generics_where[X, Y, Z: x::y::z](self, x: X, y: Y, z: Z): h::h::h where X: a::b::c, Y: a::b::c + d::e::f;
            func abstract_generics_where_nr[X, Y, Z: x::y::z](self, x: X, y: Y, z: Z) where X: a::b::c, Y: a::b::c + d::e::f;
            "#);

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Func(FuncItem { identifier: "abstract", params, block: None, return_type: Some(..), generics: None, where_clause: None }), .. })
            if matches_abstract_fn_params(&params));

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Func(FuncItem { identifier: "normal", params, block: Some(..), return_type: Some(..), generics: None, where_clause: None }), .. })
            if matches_normal_fn_params(&params));

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Func(FuncItem { identifier: "empty", params, block: Some(..), return_type: None, generics: None, where_clause: None }), .. })
            if params.is_empty());

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Func(FuncItem { identifier: "empty_abstract", params, block: None, return_type: None, generics: None, where_clause: None }), .. })
            if params.is_empty());

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Func(FuncItem { identifier: "abstract_generics", generics: Some(g), where_clause: None, params, block: None, return_type: Some(..) }), .. })
            if matches_abstract_generic_fn_generics(&g.data) && matches_abstract_generic_fn_params(&params));

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Func(FuncItem { identifier: "abstract_generics_where", generics: Some(g), where_clause: Some(w), params, block: None, return_type: Some(..) }), .. })
            if matches_abstract_generic_where_fn_generics(&g.data) && matches_abstract_generic_where_fn_params(&params) && matches_abstract_generic_where_fn_where(&w.data));

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Func(FuncItem { identifier: "abstract_generics_where_nr", generics: Some(g), where_clause: Some(w), params, block: None, return_type: None }), .. })
            if matches_abstract_generic_where_fn_generics(&g.data) && matches_abstract_generic_where_fn_params(&params) && matches_abstract_generic_where_fn_where(&w.data));

        assert_matches!(item(&mut parser), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn sig_item() {
        let mut parser = Parser::from("sig empty; sig[x::y::z] deftype; sig[a::b::c[]] arrtype; sig[m::n::p?] nulltype; sig[(a, b, c)] tuptype;");

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Sig(SigItem { sig_type: None, identifier: "empty" }), .. }));
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Sig(SigItem { sig_type: Some(ReturnType { kind: ReturnTypeKind::Path(..), .. }), identifier: "deftype" }), .. }));
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Sig(SigItem { sig_type: Some(ReturnType { kind: ReturnTypeKind::Array(..), .. }), identifier: "arrtype" }), .. }));
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Sig(SigItem { sig_type: Some(ReturnType { kind: ReturnTypeKind::Nullable(..), .. }), identifier: "nulltype" }), .. }));
        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Sig(SigItem { sig_type: Some(ReturnType { kind: ReturnTypeKind::Tuple(..), .. }), identifier: "tuptype" }), .. }));
        assert!(parser.errors.is_empty());
        assert_matches!(item(&mut parser), None);
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn visibility_item() {
        let mut parser = Parser::from("pub sig empty;");

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::Visible(.., item), .. }) if matches!(&*item, Item { kind: ItemKind::Sig(..), .. }));
        assert_matches!(item(&mut parser), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn attribute_collected_item() {
        fn matches_attribute_paths(p: &[Path<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(Path { identifiers, .. }) if identifiers == &["attrib1"]);
            assert_matches!(iter.next(), Some(Path { identifiers, .. }) if identifiers == &["x", "attrib2"]);
            assert_matches!(iter.next(), Some(Path { identifiers, .. }) if identifiers == &["y", "z", "attrib3"]);
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from(r#"
            #attrib1,x::attrib2
            #y::z::attrib3,
            sig empty;
            "#);

        assert_matches!(item(&mut parser), Some(Item { kind: ItemKind::AttributeCollectedItem(attributes, sig), .. })
            if matches_attribute_paths(&attributes) && matches!(sig.kind, ItemKind::Sig(..)));
        assert_matches!(item(&mut parser), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn return_types() {
        fn matches_inner_type(r: &ReturnType<'_>, i: &[&'_ str]) -> bool {
            matches!(r, ReturnType { kind: ReturnTypeKind::Path(Path { identifiers, .. }, ..), .. } if identifiers == i)
        }

        fn matches_tuple(r: &[ReturnType<'_>]) -> bool {
            let mut iter = r.iter();

            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Path(..), .. }));
            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Nullable(_), .. }));
            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Nullable(t), .. }) if matches!(&**t, ReturnType { kind: ReturnTypeKind::Array(_), .. }));
            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Tuple(t), .. }) if t.len() == 3);
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_generic_parameters(r: &[ReturnType<'_>]) -> bool {
            let mut iter = r.iter();
            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Path(Path { identifiers, .. }), .. }) if identifiers == &["b"]);
            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Generic(l, generics), .. })
                if matches!(l.kind, ReturnTypeKind::Path(..)) && matches!(generics.first().map(|t| &t.kind), Some(ReturnTypeKind::Path(..))));
            assert_matches!(iter.next(), Some(ReturnType { kind: ReturnTypeKind::Generic(l, generics), .. })
                if matches!(l.kind, ReturnTypeKind::Path(..)) && generics.len() == 2);
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from("xaylon::yaylon::zaylon a::b::c[] m::n::p[]? k::l::m?[] (x::y::z, a::b::c?, m::n::p[]?, (a, b, c)) a[b, c[d], e[f, g]]");

        assert_matches!(return_type(&mut parser), Some(ReturnType { kind: ReturnTypeKind::Path(Path { identifiers, .. }, ..), .. }) if identifiers == ["xaylon", "yaylon", "zaylon"]);
        assert_matches!(return_type(&mut parser), Some(ReturnType { kind: ReturnTypeKind::Array(ty), .. }) if matches_inner_type(&ty, &["a", "b", "c"]));
        assert_matches!(return_type(&mut parser), Some(ReturnType { kind: ReturnTypeKind::Nullable(ty), .. }) if matches!(&*ty, ReturnType { kind: ReturnTypeKind::Array(..), .. }));
        assert_matches!(return_type(&mut parser), Some(ReturnType { kind: ReturnTypeKind::Array(ty), .. }) if matches!(&*ty, ReturnType { kind: ReturnTypeKind::Nullable(_), .. }));
        assert_matches!(return_type(&mut parser), Some(ReturnType { kind: ReturnTypeKind::Tuple(t), .. }) if matches_tuple(&t));
        assert_matches!(return_type(&mut parser), Some(ReturnType { kind: ReturnTypeKind::Generic(ty, generics), .. })
            if matches!(&*ty, ReturnType { kind: ReturnTypeKind::Path(..), .. }) && matches_generic_parameters(&generics));
        assert_matches!(return_type(&mut parser), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }
}
