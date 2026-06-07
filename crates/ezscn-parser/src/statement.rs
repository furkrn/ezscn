use ezscn_ast::*;
use ezscn_ast::statement::*;
use ezscn_error::{ParseError, ParseErrorKind};
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
        Token { kind: TokenKind::ReturnKeyword, .. } => return_statement(parser),
        Token { kind: TokenKind::LetKeyword, .. } => let_statement(parser),
        Token { kind: TokenKind::IfKeyword, .. } => if_statement(parser),
        Token { kind: TokenKind::ForKeyword, .. } => for_statement(parser),
        Token { kind: TokenKind::WhileKeyword, .. } => while_statement(parser),
        Token { kind: TokenKind::BreakKeyword, .. } => break_statement(parser),
        Token { kind: TokenKind::ContinueKeyword, .. } => continue_statement(parser),
        _ => expression_statement(parser),
    }
}

#[inline]
pub fn expression_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let exp = parser.expression()?;
    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;

    let span = Span::new_spanned(exp.span, semicolon.span);
    let kind = StatementKind::Expression(exp);

    Some(Statement { kind, span })
}

#[inline]
pub fn return_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let return_kw = parser.next_if_kind_errored(TokenKind::ReturnKeyword)?;
    let exp = parser.expression()?;

    let span = Span::new_spanned(return_kw.span, exp.span);
    let kind = StatementKind::Return(exp);

    Some(Statement { kind, span })
}

#[inline]
pub fn let_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>> {
    let let_kw = parser.next_if_kind_errored(TokenKind::LetKeyword)?;
    let mut identifiers = thin_vec![];
    if parser.peek().is_some_and(|t| t.kind == TokenKind::ParanthesisLeft) {
        parser.advance_until_kind(TokenKind::ParanthesisLeft)?;
        loop {
            if parser.token_stream.is_next(TokenKind::ParanthesisRight) {
                break
            }

            identifiers.push(let_ident(parser)?);
            if !parser.token_stream.is_next(TokenKind::ParanthesisRight) {
                parser.advance_until_kind(TokenKind::Comma)?;
            }
        }
        parser.advance_until_kind(TokenKind::ParanthesisRight)?;
    } else {
        identifiers.push(let_ident(parser)?)
    }

    let expression = if parser.token_stream.is_next(TokenKind::Equals) {
        parser.advance_until_kind(TokenKind::Equals)?;
        Some(parser.expression()?)
    } else {
        None
    };

    let semicolon = parser.advance_until_kind(TokenKind::Semicolon)?;

    let span = Span::new_spanned(let_kw.span, semicolon.span);
    let kind = StatementKind::Let(identifiers, expression);

    Some(Statement { kind, span })
}

#[inline]
fn let_ident<'t>(parser: &mut Parser<'t>) -> Option<IdentifierOrUnderscore<'t>> {
    parser.advance_map(|token| {
        match token {
            Some(Token { kind: TokenKind::Identifier, span }) =>
                Ok(IdentifierOrUnderscore::Identifier(&parser.input[span])),
            Some(Token { kind: TokenKind::Underscore, .. }) =>
                Ok(IdentifierOrUnderscore::Underscore),
            Some(Token { kind: found, span }) =>
                Err(ParseError::new(ParseErrorKind::UnexpectedToken(TokenKind::Identifier, found), span)),
            None =>
                Err(ParseError::new(ParseErrorKind::ExpectedToken(TokenKind::Identifier), Span::default()))
        }
    })
}

#[inline]
pub fn if_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>>{
    let if_kw = parser.next_if_kind_errored(TokenKind::IfKeyword)?;
    let clause = parser.expression()?;
    let if_arm = IfArm { clause, block: block(parser)? };
    let mut else_if_arms = thin_vec![];
    let mut else_arm = None;
    while parser.next_if(|t| t.kind == TokenKind::ElseKeyword).is_some() {
        let is_else_if = parser.next_if(|t| t.kind == TokenKind::IfKeyword)
            .is_some();

        if is_else_if {
            let clause = parser.expression()?;
            let block = block(parser)?;

            else_if_arms.push(IfArm { clause, block })
        } else {
            else_arm = block(parser);
        }
    }

    let end_span = else_arm.as_ref()
        .map(|t| t.span)
        .or_else(|| else_if_arms.last().map(|t| t.block.span))
        .unwrap_or(if_arm.block.span);

    let span = Span::new_spanned(if_kw.span, end_span);
    let if_statement = IfStatement { if_arm, else_if_arms, else_arm };
    let kind = StatementKind::If(if_statement);

    Some(Statement { kind, span })
}

#[inline]
pub fn for_statement<'t>(parser: &mut Parser<'t>) -> Option<Statement<'t>>{
    let for_kw = parser.next_if_kind_errored(TokenKind::ForKeyword)?;
    let identifier = parser.advance_until_identifier()?;
    parser.advance_until_kind(TokenKind::InKeyword)?;
    let expression = parser.expression()?;
    let block = block(parser)?;

    let span = Span::new_spanned(for_kw.span, block.span);
    let for_loop_statement = ForLoopStatement { identifier, expression, block };
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