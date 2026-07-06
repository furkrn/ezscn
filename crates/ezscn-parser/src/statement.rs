use ezscn_ast::*;
use ezscn_ast::statement::*;
use ezscn_tokens::{Span, SpanImpl, Spanned, Token, TokenKind};
use thin_vec::thin_vec;

use crate::Parser;
use crate::items::return_type;

#[inline]
pub fn block<'t>(parser: &mut Parser<'t>) -> Option<Block<'t>> {
    let cbl_span = parser.advance_until_kind(TokenKind::CurlyBracketLeft)?.span;
    let mut statements = thin_vec![];
    while !parser.is_next(TokenKind::CurlyBracketRight) {
        statements.push(statement(parser)?)
    }

    let cbr_span = parser.advance_until_kind(TokenKind::CurlyBracketRight)?.span;
    let span = Span::merge(cbl_span, cbr_span);

    Some(Spanned::new(statements, span))
}

#[inline]
pub fn statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    match parser.peek()? {
        Token { kind: TokenKind::Semicolon, .. } => empty_statement(parser),
        Token { kind: TokenKind::ReturnKeyword, .. } => return_statement(parser),
        Token { kind: TokenKind::LetKeyword, .. } => let_statement(parser),
        Token { kind: TokenKind::ForKeyword, .. } => for_statement(parser),
        Token { kind: TokenKind::WhileKeyword, .. } => while_statement(parser),
        Token { kind: TokenKind::BreakKeyword, .. } => break_statement(parser),
        Token { kind: TokenKind::ContinueKeyword, .. } => continue_statement(parser),
        _ => expression_statement(parser),
    }
}

#[inline]
pub fn empty_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let semicolon = parser.next_if_kind(TokenKind::Semicolon)?;

    let span = semicolon.span;
    let kind = StatementKind::Empty;

    Some(Statement { kind, span })
}

#[inline]
pub fn expression_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let exp = parser.expression()?;
    let semicolon = parser.next_if_kind(TokenKind::Semicolon);

    let span = Span::merge(exp.span, semicolon.map(|s| s.span).unwrap_or(exp.span));
    let kind = StatementKind::Expression(exp, semicolon.is_none());

    Some(Statement { kind, span })
}

#[inline]
pub fn return_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let return_kw = parser.next_if_kind_errored(TokenKind::ReturnKeyword)?;
    let exp = if !parser.is_next(TokenKind::Semicolon) {
        Some(parser.expression()?)
    } else {
        None
    };

    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;
    let span = Span::merge(return_kw.span, semicolon.span);
    let kind = StatementKind::Return(exp);

    Some(Statement { kind, span })
}

#[inline]
pub fn let_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let let_kw = parser.next_if_kind_errored(TokenKind::LetKeyword)?;
    let identifiers = if parser.next_if_kind(TokenKind::ParanthesisLeft).is_some() {
        let identifiers = parser.comma_seperated_map(TokenKind::ParanthesisRight, Parser::advance_until_identifier_or_underscore)?;
        parser.advance_until_kind(TokenKind::ParanthesisRight)?;

        identifiers
    } else {
        thin_vec![parser.advance_until_identifier_or_underscore()?]
    };

    let return_type = if parser.next_if_kind(TokenKind::Colon).is_some() {
        Some(return_type(parser)?)
    } else {
        None
    };

    let expression = if parser.token_stream.is_next(TokenKind::Equals) {
        parser.advance_until_kind(TokenKind::Equals)?;
        Some(parser.expression()?)
    } else {
        None
    };

    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;

    let span = Span::merge(let_kw.span, semicolon.span);
    let kind = StatementKind::Let(identifiers, return_type, expression);

    Some(Statement { kind, span })
}

#[inline]
pub fn for_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>>{
    let for_kw = parser.next_if_kind_errored(TokenKind::ForKeyword)?;
    let identifiers = if parser.next_if_kind(TokenKind::ParanthesisLeft).is_some() {
        let identifiers = parser.comma_seperated_map(TokenKind::ParanthesisRight, Parser::advance_until_identifier_or_underscore)?;
        parser.advance_until_kind(TokenKind::ParanthesisRight)?;

        identifiers
    } else {
        thin_vec![parser.advance_until_identifier_or_underscore()?]
    };

    parser.advance_until_kind(TokenKind::InKeyword)?;
    let expression = parser.expression()?;
    let block = block(parser)?;

    let span = Span::merge(for_kw.span, block.span);
    let for_loop_statement = ForLoopStatement { identifiers, expression, block };
    let kind = StatementKind::ForLoop(for_loop_statement);

    Some(Statement { kind, span })
}

#[inline]
pub fn while_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>>{
    let while_kw = parser.next_if_kind_errored(TokenKind::WhileKeyword)?;
    let expression = parser.expression()?;
    let block = block(parser)?;

    let span = Span::merge(while_kw.span, block.span);
    let while_loop = WhileLoopStatement { expression, block };
    let kind = StatementKind::WhileLoop(while_loop);

    Some(Statement { kind, span })
}

#[inline]
pub fn break_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let break_kw = parser.next_if_kind_errored(TokenKind::BreakKeyword)?;
    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;

    let span = Span::merge(break_kw.span, semicolon.span);
    let kind = StatementKind::Break;

    Some(Statement { kind, span })
}

#[inline]
pub fn continue_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let continue_kw = parser.next_if_kind_errored(TokenKind::ContinueKeyword)?;
    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;

    let span = Span::merge(continue_kw.span, semicolon.span);
    let kind = StatementKind::Continue;

    Some(Statement { kind, span })
}

#[cfg(test)]
mod tests {
    use core::assert_matches;
    use crate::items::statement_item;
    use super::*;

    #[test]
    pub fn expression_statement() {
        let mut parser = Parser::from("5;6;e;hello();ereturn");

        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Expression(_, false), .. }), .. }));
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Expression(_, false), .. }), .. }));
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Expression(_, false), .. }), .. }));
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Expression(_, false), .. }), .. }));
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Expression(_, true), .. }), .. }));
        assert!(parser.errors.is_empty());
    }

    #[test]
    pub fn return_statement() {
        let mut parser = Parser::from("return 5; return 6; return 7; return;return");

        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Return(Some(_)), .. }), .. }));
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Return(Some(_)), .. }), .. }));
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Return(Some(_)), .. }), .. }));
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Return(None), .. }), .. }));
        assert!(parser.errors.is_empty());
        assert_matches!(statement_item(&mut parser), None);
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn let_statement() {
        let mut parser = Parser::from("let c = 7; let (c, d) = 8; let i; let (_, _r, g) = -1; let s: (); let c: () = 0;");
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Let(idents, None, Some(..)), .. }), .. })
            if idents == [IdentifierOrUnderscore::Identifier("c")]);
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Let(idents, None, Some(..)), .. }), .. })
            if idents == [IdentifierOrUnderscore::Identifier("c"), IdentifierOrUnderscore::Identifier("d")]);
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Let(idents, None, None), .. }), .. })
            if idents == [IdentifierOrUnderscore::Identifier("i")]);
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Let(idents, None, Some(..)), .. }), .. })
            if idents == [IdentifierOrUnderscore::Underscore, IdentifierOrUnderscore::Identifier("_r"), IdentifierOrUnderscore::Identifier("g")]);
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Let(idents, Some(_), None), .. }), .. })
            if idents == [IdentifierOrUnderscore::Identifier("s")]);
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Let(idents, Some(_), Some(_)), .. }), .. })
            if idents == [IdentifierOrUnderscore::Identifier("c")]);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn for_statement() {
        let mut parser = Parser::from(r#"for _ in x { a; } for z in y { b; } for m in n { c; } for (a, b, c) in l { d; }"#);

        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::ForLoop(ForLoopStatement { identifiers, .. }), .. }), .. })
            if identifiers == [IdentifierOrUnderscore::Underscore]);
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::ForLoop(ForLoopStatement { identifiers, .. }), .. }), .. })
            if identifiers == [IdentifierOrUnderscore::Identifier("z")]);
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::ForLoop(ForLoopStatement { identifiers, .. }), .. }), .. })
            if identifiers == [IdentifierOrUnderscore::Identifier("m")]);
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::ForLoop(ForLoopStatement { identifiers, .. }), .. }), .. })
            if identifiers == [IdentifierOrUnderscore::Identifier("a"), IdentifierOrUnderscore::Identifier("b"), IdentifierOrUnderscore::Identifier("c")]);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn while_statement() {
        let mut parser = Parser::from(r#"while x { a; } while y { b; } while z { c; }"#);

        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::WhileLoop(..), .. }), .. }));
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::WhileLoop(..), .. }), .. }));
        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::WhileLoop(..), .. }), .. }));
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn break_statement() {
        let mut parser = Parser::from("break; break");

        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Break, .. }), .. }));
        assert!(parser.errors.is_empty());
        assert_matches!(statement_item(&mut parser), None);
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn continue_statement() {
        let mut parser = Parser::from("continue; continue");

        assert_matches!(statement_item(&mut parser), Some(Item { kind: ItemKind::Statement(Statement { kind: StatementKind::Continue, .. }), .. }));
        assert!(parser.errors.is_empty());
        assert_matches!(statement_item(&mut parser), None);
        assert!(parser.reached_eof());
    }
}
