use ezscn_ast::*;
use ezscn_ast::statement::*;
use ezscn_tokens::{Span, SpanImpl, Spanned, Token, TokenKind};
use thin_vec::thin_vec;

use crate::Parser;

#[inline]
pub fn block<'t>(parser: &mut Parser<'t>) -> Option<Block<'t>> {
    let cbl_span = parser.advance_until_kind(TokenKind::CurlyBracketLeft)?.span;
    let mut statements = thin_vec![];
    while !parser.is_next(TokenKind::CurlyBracketRight) {
        statements.push(statement(parser)?)
    }

    let cbr_span = parser.advance_until_kind(TokenKind::CurlyBracketRight)?.span;
    let span = Span::new_spanned(cbl_span, cbr_span);

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

    let span = Span::new_spanned(exp.span, semicolon.map(|s| s.span).unwrap_or(exp.span));
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
    let span = Span::new_spanned(return_kw.span, semicolon.span);
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
        Some(parser.return_type()?)
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

    let span = Span::new_spanned(let_kw.span, semicolon.span);
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

    let span = Span::new_spanned(for_kw.span, block.span);
    let for_loop_statement = ForLoopStatement { identifiers, expression, block };
    let kind = StatementKind::ForLoop(for_loop_statement);

    Some(Statement { kind, span })
}

#[inline]
pub fn while_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>>{
    let while_kw = parser.next_if_kind_errored(TokenKind::WhileKeyword)?;
    let expression = parser.expression()?;
    let block = block(parser)?;

    let span = Span::new_spanned(while_kw.span, block.span);
    let while_loop = WhileLoopStatement { expression, block };
    let kind = StatementKind::WhileLoop(while_loop);

    Some(Statement { kind, span })
}

#[inline]
pub fn break_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let break_kw = parser.next_if_kind_errored(TokenKind::BreakKeyword)?;
    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;

    let span = Span::new_spanned(break_kw.span, semicolon.span);
    let kind = StatementKind::Break;

    Some(Statement { kind, span })
}

#[inline]
pub fn continue_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let continue_kw = parser.next_if_kind_errored(TokenKind::ContinueKeyword)?;
    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;

    let span = Span::new_spanned(continue_kw.span, semicolon.span);
    let kind = StatementKind::Continue;

    Some(Statement { kind, span })
}