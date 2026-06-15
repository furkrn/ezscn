use alloc::boxed::Box;
use core::str::FromStr;
use ezscn_ast::expression::*;
use ezscn_ast::statement::{Statement, StatementKind};
use ezscn_error::{LiteralKind, ParseError, ParseErrorKind};
use ezscn_tokens::{BaseN, CharacterEscapeType, Token,
    TokenKind, Span, SpanImpl, Spanned, StringOptions};
use ordered_float::OrderedFloat;
use thin_vec::thin_vec;

use crate::statement::block;
use crate::Parser;
use crate::string::*;

#[inline]
pub fn expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    assignment_expression(parser)
}

#[inline]
pub fn assignment_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = ternary_expression(parser)?;
    while let Some(op) = assignment_op(parser) {
        let right = ternary_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Assignment(Box::new(left), op, Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
fn assignment_op(parser: &mut Parser<'_>) -> Option<AssignmentOperator> {
    parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::Equals, .. }) =>
                Ok(AssignmentOperator::Assign),
            Some(Token { kind: TokenKind::BitwiseLeftCompound, .. }) =>
                Ok(AssignmentOperator::BitLeft),
            Some(Token { kind: TokenKind::BitwiseRightCompound, .. }) =>
                Ok(AssignmentOperator::BitRight),
            Some(Token { kind: TokenKind::AndEquals, .. }) =>
                Ok(AssignmentOperator::And),
            Some(Token { kind: TokenKind::OrEquals, .. }) =>
                Ok(AssignmentOperator::Or),
            Some(Token { kind: TokenKind::CaretEquals, .. }) =>
                Ok(AssignmentOperator::Xor),
            Some(Token { kind: TokenKind::TildeEquals, .. }) =>
                Ok(AssignmentOperator::Complement),
            Some(Token { kind: TokenKind::StarEquals, .. }) =>
                Ok(AssignmentOperator::Multiplication),
            Some(Token { kind: TokenKind::SlashEquals, .. }) =>
                Ok(AssignmentOperator::Division),
            Some(Token { kind: TokenKind::PlusEquals, .. }) =>
                Ok(AssignmentOperator::Addition),
            Some(Token { kind: TokenKind::MinusEquals, .. }) =>
                Ok(AssignmentOperator::Substraction),
            Some(Token { kind: TokenKind::PercentEquals, .. }) =>
                Ok(AssignmentOperator::Modulo),
            token => Err(token),
        }
    })
}

#[inline]
pub fn ternary_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let matcher = conditional_or_expression(parser)?;
    if parser.next_if(|t| t.kind == TokenKind::QuestionMark).is_none() {
        return Some(matcher)
    }

    let on_match_hand = conditional_or_expression(parser)?;
    parser.advance_until_kind(TokenKind::Colon)?;
    let else_hand = conditional_or_expression(parser)?;
    let span = Span::new_spanned(matcher.span, else_hand.span);
    let kind = ExpressionKind::Ternary(Box::new(matcher), Box::new(on_match_hand), Box::new(else_hand));

    Some(Expression { kind, span })
}

#[inline]
pub fn conditional_or_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = conditional_and_expression(parser)?;
    while parser.next_if(|t| t.kind == TokenKind::OrOr).is_some() {
        let right = conditional_and_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Conditional(Box::new(left), ConditionalOperator::Or, Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn conditional_and_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = logical_or_expression(parser)?;
    while parser.next_if(|t| t.kind == TokenKind::AndAnd).is_some() {
        let right = logical_or_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Conditional(Box::new(left), ConditionalOperator::And, Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn logical_or_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = logical_xor_expression(parser)?;
    while parser.next_if(|t| t.kind == TokenKind::Or).is_some() {
        let right = logical_xor_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Logical(Box::new(left), LogicalOperator::Or, Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)

}

#[inline]
pub fn logical_xor_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = logical_and_expression(parser)?;
    while parser.next_if(|t| t.kind == TokenKind::Caret).is_some() {
        let right = logical_and_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Logical(Box::new(left), LogicalOperator::Xor, Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn logical_and_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = equality_expression(parser)?;
    while parser.next_if(|t| t.kind == TokenKind::And).is_some() {
        let right = equality_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Logical(Box::new(left), LogicalOperator::And, Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn equality_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = comparision_expression(parser)?;
    while let Some(op) = equality_op(parser){
        let right = comparision_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Equality(Box::new(left), op, Box::new(right));
        left = Expression { kind, span }
    };

    Some(left)
}

fn equality_op(parser: &mut Parser<'_>) -> Option<EqualityOperator> {
    parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::EqualsEquals, .. }) =>
                Ok(EqualityOperator::Equals),
            Some(Token { kind: TokenKind::NotEquals, ..}) =>
                Ok(EqualityOperator::NotEquals),
            token => Err(token),
        }
    })
}

#[inline]
pub fn comparision_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let left = instanceof_expression(parser)?;
    let Some(op) = parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::GreaterThan, .. }) =>
                Ok(ComparisionOperator::GreaterThan),
            Some(Token { kind: TokenKind::GreaterThanEquals, .. }) =>
                Ok(ComparisionOperator::GreaterThanEquals),
            Some(Token { kind: TokenKind::LessThan, .. }) =>
                Ok(ComparisionOperator::LessThan),
            Some(Token { kind: TokenKind::LessThanEquals, .. }) =>
                Ok(ComparisionOperator::LessThanEquals),
            token => Err(token),
        }
    }) else {
        return Some(left)
    };

    let right = instanceof_expression(parser)?;
    let span = Span::new_spanned(left.span, right.span);
    let kind = ExpressionKind::Comparision(Box::new(left), op, Box::new(right));

    Some(Expression { kind, span })
}

#[inline]
pub fn instanceof_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let left = shift_expression(parser)?;
    if parser.next_if_kind(TokenKind::IsKeyword).is_none() {
        return Some(left)
    }

    let type_path = parser.advance_until_path()?;
    let mut end_span = type_path.span;
    let identifier = parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::Identifier, span }) => {
                end_span = span;
                Ok(&parser.input[span])
            },
            _ => Err(t)
        }
    });

    let span = Span::new_spanned(left.span, end_span);
    let kind = ExpressionKind::InstanceOf(Box::new(left), type_path, identifier);

    Some(Expression { kind, span })
}

#[inline]
pub fn shift_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = additive_expression(parser)?;
    while let Some(op) = shift_op(parser) {
        let right = additive_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Binary(Box::new(left), op, Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
fn shift_op(parser: &mut Parser<'_>) -> Option<BinaryOperator> {
    parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::BitwiseLeft, .. }) =>
                Ok(BinaryOperator::BitLeft),
            Some(Token { kind: TokenKind::BitwiseRight, .. }) =>
                Ok(BinaryOperator::BitRight),
            token => Err(token),
        }
    })
}

#[inline]
pub fn additive_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = multiplicative_expression(parser)?;
    while let Some(op) = additive_op(parser) {
        let right = multiplicative_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Binary(Box::new(left), op, Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
fn additive_op(parser: &mut Parser<'_>) -> Option<BinaryOperator> {
    parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::Plus, .. }) =>
                Ok(BinaryOperator::Addition),
            Some(Token { kind: TokenKind::Minus, .. }) =>
                Ok(BinaryOperator::Substraction),
            token => Err(token),
        }
    })
}

#[inline]
pub fn multiplicative_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = unary_expression(parser)?;
    while let Some(op) = multiplicative_op(parser) {
        let right = unary_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Binary(Box::new(left), op, Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
fn multiplicative_op(parser: &mut Parser<'_>) -> Option<BinaryOperator> {
    parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::Star, .. }) =>
                Ok(BinaryOperator::Multiplication),
            Some(Token { kind: TokenKind::Slash, .. }) =>
                Ok(BinaryOperator::Division),
            Some(Token { kind: TokenKind::Percent, .. }) =>
                Ok(BinaryOperator::Modulo),
            token => Err(token),
        }
    })
}

#[inline]
pub fn unary_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let unary_matcher = parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::Not, span }) =>
                Ok((UnaryOperator::Not, span)),
            Some(Token { kind: TokenKind::Plus, span }) =>
                Ok((UnaryOperator::Plus, span)),
            Some(Token { kind: TokenKind::Minus, span }) =>
                Ok((UnaryOperator::Negative, span)),
            Some(Token { kind: TokenKind::PlusPlus, span }) =>
                Ok((UnaryOperator::Addition, span)),
            Some(Token { kind: TokenKind::MinusMinus, span }) =>
                Ok((UnaryOperator::Substraction, span)),
            Some(Token { kind: TokenKind::Tilde, span }) =>
                Ok((UnaryOperator::Complement, span)),
            token => Err(token),
        }
    });

    let Some((op, start_span)) = unary_matcher else {
        return postfix_expression(parser);
    };

    let expr = unary_expression(parser)?;
    let span = Span::new_spanned(start_span, expr.span);
    let kind = ExpressionKind::Unary(op, Box::new(expr));

    Some(Expression { kind, span })
}

#[inline]
pub fn postfix_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let primary_exp = primary_expression(parser);
    match parser.peek().map(|t| t.kind) {
        Some(TokenKind::ParanthesisLeft) =>
            call_expression(parser, primary_exp),
        Some(TokenKind::SquareBracketLeft) =>
            index_expression(parser, primary_exp),
        Some(TokenKind::Dot) =>
            reference_expression(parser, primary_exp),
        Some(TokenKind::PlusPlus | TokenKind::MinusMinus) =>
            post_op_expression(parser, primary_exp),
        Some(TokenKind::QuestionMark) =>
            short_curcuit_expression(parser, primary_exp),
        _ => primary_exp,
    }
}

#[inline]
pub fn call_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while parser.next_if_kind(TokenKind::ParanthesisLeft).is_some() {
        let args = parser.comma_seperated_map(TokenKind::ParanthesisRight, expression)?;
        let pr_token = parser.advance_until_kind(TokenKind::ParanthesisRight)?;
        let span = Span::new_spanned(left.span, pr_token.span);
        let kind = ExpressionKind::Call(Box::new(left), args);
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn index_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while parser.next_if_kind(TokenKind::SquareBracketLeft).is_some() {
        let expr = expression(parser)?;
        let sbr_token = parser.advance_until_kind(TokenKind::SquareBracketRight)?;
        let span = Span::new_spanned(left.span, sbr_token.span);
        let kind = ExpressionKind::Index(Box::new(left), Box::new(expr));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn reference_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while parser.next_if_kind(TokenKind::Dot).is_some() {
        let right = postfix_expression(parser)?;
        let span = Span::new_spanned(left.span, right.span);
        let kind = ExpressionKind::Reference(Box::new(left), Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn post_op_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while let Some((op, end_span)) = post_op_span(parser) {
        let span = Span::new_spanned(left.span, end_span);
        let kind = ExpressionKind::PostOp(Box::new(left), op);
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
fn post_op_span(parser: &mut Parser<'_>) -> Option<(PostOperator, Span)> {
    parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::PlusPlus, span }) =>
                Ok((PostOperator::Increment, span)),
            Some(Token { kind: TokenKind::MinusMinus, span }) =>
                Ok((PostOperator::Decrement, span)),
            token => Err(token),
        }
    })
}

#[inline]
pub fn short_curcuit_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while let Some(mark) = parser.next_if_kind(TokenKind::QuestionMark) {
        let span = Span::new_spanned(left.span, mark.span);
        let kind = ExpressionKind::ShortCurcuit(Box::new(left));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn primary_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    match parser.peek() {
        Some(Token { kind: TokenKind::NewKeyword, .. }) =>
            new_init_expression(parser),
        Some(Token { kind: TokenKind::Identifier | TokenKind::SelfKeyword, .. }) =>
            access_expression(parser),
        Some(Token { kind: TokenKind::ParanthesisLeft, .. }) =>
            tuple_expression(parser),
        Some(Token { kind: TokenKind::SquareBracketLeft, .. }) =>
            array_expression(parser),
        Some(Token { kind: TokenKind::StringLiteral { .. }, .. }) =>
            string_literal(parser),
        Some(Token { kind: TokenKind::CharacterLiteral { .. }, .. }) =>
            char_literal(parser),
        Some(Token { kind: TokenKind::NumberLiteral { is_floating, .. }, .. }) if *is_floating =>
            float_literal(parser),
        Some(Token { kind: TokenKind::NumberLiteral { .. }, .. }) =>
            integer_literal(parser),
        Some(Token { kind: TokenKind::FalseKeyword | TokenKind::TrueKeyword, .. }) =>
            boolean_literal(parser),
        Some(Token { kind: TokenKind::NullKeyword, .. }) =>
            null_literal(parser),
        Some(Token { kind: TokenKind::MatchKeyword, .. }) =>
            match_expression(parser),
        _ => {
            let token = parser.next()?;
            let error = ParseError::new(ParseErrorKind::UnexpectedToken(token.kind), token.span);
            parser.error(error);
            primary_expression(parser)
        }
    }
}

#[inline]
pub fn access_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (access_expression, span) = parser.advance_map(|t| {
        match t {
            Some(Token { kind: TokenKind::Identifier, span }) =>
                Ok((AccessExpression::Identifier(&parser.input[span]), span)),
            Some(Token { kind: TokenKind::SelfKeyword, span }) =>
                Ok((AccessExpression::SelfAccess, span)),
            Some(Token { kind: found, span }) => {
                let kind = ParseErrorKind::InvalidToken(TokenKind::Identifier, found);
                Err(ParseError::new(kind, span))
            },
            None =>
                Err(ParseError::new(ParseErrorKind::ExpectedToken(TokenKind::Identifier), Span::empty_from_start(parser.input.len()))),
        }
    })?;

    let kind = ExpressionKind::Access(access_expression);

    Some(Expression { kind, span })
}

#[inline]
pub fn new_init_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let new_kw = parser.next_if_kind_errored(TokenKind::NewKeyword)?;
    let identifier = parser.advance_until_path()?;
    let (inits, end_span) = if parser.next_if_kind(TokenKind::CurlyBracketLeft).is_some() {
        let inits = parser.comma_seperated_map(TokenKind::CurlyBracketRight, new_init_member)?;
        let cbr = parser.advance_until_kind(TokenKind::CurlyBracketRight)?;

        (inits, cbr.span)
    } else {
        (thin_vec![], identifier.span)
    };

    let span = Span::new_spanned(new_kw.span, end_span);
    let kind = ExpressionKind::New(identifier, inits);

    Some(Expression { kind, span })
}

#[inline]
fn new_init_member<'t>(parser: &mut Parser<'t>) -> Option<StructInitialization<'t>> {
    let identifier_token = parser.advance_until_kind(TokenKind::Identifier)?;
    let identifier = &parser.input[identifier_token.span];
    let expression = if parser.is_next(TokenKind::Equals) {
        parser.advance_until_kind(TokenKind::Equals)?;
        expression(parser)?
    } else {
        let kind = ExpressionKind::Access(AccessExpression::Identifier(identifier));
        let span = identifier_token.span;
        Expression { kind, span }
    };

    Some(StructInitialization { identifier, expression })
}

#[inline]
pub fn tuple_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let pl = parser.advance_until_kind(TokenKind::ParanthesisLeft)?;
    let exprs = parser.comma_seperated_map(TokenKind::ParanthesisRight, expression)?;
    let pr = parser.advance_until_kind(TokenKind::ParanthesisRight)?;
    let span = Span::new_spanned(pl.span, pr.span);
    let kind = ExpressionKind::Tuple(exprs);

    Some(Expression { kind, span })
}

#[inline]
pub fn array_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let sbl_span = parser.advance_until_kind(TokenKind::SquareBracketLeft)?.span;
    let exprs = parser.comma_seperated_map(TokenKind::SquareBracketRight, expression)?;
    let sbr_span = parser.advance_until_kind(TokenKind::SquareBracketRight)?.span;
    let span = Span::new_spanned(sbl_span, sbr_span);
    let kind = ExpressionKind::Array(exprs);

    Some(Expression { kind, span })
}

#[inline]
pub fn literal_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    match parser.peek() {
        Some(Token { kind: TokenKind::StringLiteral { .. }, .. }) =>
            string_literal(parser),
        Some(Token { kind: TokenKind::CharacterLiteral { .. }, .. }) =>
            char_literal(parser),
        Some(Token { kind: TokenKind::NumberLiteral { is_floating, .. }, .. }) if *is_floating =>
            float_literal(parser),
        Some(Token { kind: TokenKind::NumberLiteral { .. }, .. }) =>
            integer_literal(parser),
        Some(Token { kind: TokenKind::FalseKeyword | TokenKind::TrueKeyword, .. }) =>
            boolean_literal(parser),
        Some(Token { kind: TokenKind::NullKeyword, .. }) =>
            null_literal(parser),
        _ => {
            let token = parser.next()?;
            let error = ParseError::new(ParseErrorKind::LiteralsExpected(Some(token.kind)), token.span);
            parser.error(error);
            literal_expression(parser)
        }
    }
}

#[inline]
pub fn string_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (options, quote_start, span) = parser.next_if_map_errored(|t| {
        match t {
            Some(Token { kind: TokenKind::StringLiteral { options, quote_start, terminated }, span }) =>
                if terminated {
                    Ok((options, quote_start, span))
                } else {
                    Err(ParseError::new(ParseErrorKind::UnterminatedString, span))
                },
            Some(Token { kind, span }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::String, Some(kind)), span)),
            None => {
                let span = Span::empty_from_start(parser.input.len());
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::String, None), span))
            }
        }
    })?;

    let end = if options.contains(StringOptions::MULTILINE_STR) {
        span.end() - 2
    } else {
        span.end() - 1
    };

    let raw_str = &parser.input[quote_start..end];
    let string_literal = if options.contains(StringOptions::RAWSTR) {
        StringLiteral::Borrowed(raw_str)
    } else {
        UnescapedStringBuilder::new(raw_str, &mut parser.errors)
            .collect()
    };

    let kind = ExpressionKind::Literal(LiteralExpression::String(string_literal));
    Some(Expression { kind, span })
}

#[inline]
pub fn char_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (escape_type, span) = parser.next_if_map_errored(|t| {
        match t {
            Some(Token { kind: TokenKind::CharacterLiteral { escape_type, terminated }, span }) =>
                if terminated {
                    Ok((escape_type, span))
                } else {
                    Err(ParseError::new(ParseErrorKind::UnterminatedChar, span))
                }
            Some(Token { kind, span }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Char, Some(kind)), span)),
            None => {
                let span = Span::empty_from_start(parser.input.len());
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Char, None), span))
            }
        }
    })?;

    let char_range = span.shift_end_left(1)
        .shift_start_right(1);

    let raw_char_str = &parser.input[char_range];
    let literal_matcher = match escape_type {
        CharacterEscapeType::None => raw_char_str.chars().next(),
        CharacterEscapeType::Simple => char_escape_sequence_map_str(raw_char_str),
        _ => hex_escape_sequence(raw_char_str),
    };

    if let Some(char) = literal_matcher {
        let literal = LiteralExpression::Char(char);
        let kind = ExpressionKind::Literal(literal);
        Some(Expression { kind, span })
    } else {
        parser.error(ParseError::new(ParseErrorKind::UnknownEscapeSequence, span));
        None
    }
}

#[inline]
pub fn integer_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (base, span) = parser.next_if_map_errored(|t| {
        match t {
            Some(Token { kind: TokenKind::NumberLiteral { base, is_floating }, span }) => {
                if is_floating {
                    Err(ParseError::new(ParseErrorKind::ExpectedIntegerFoundFloating, span))
                } else {
                    Ok((base, span))
                }
            },
            Some(Token { kind, span }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Integer, Some(kind)), span)),
            None => {
                let span = Span::empty_from_start(parser.input.len());
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Integer, None), span))
            }
        }
    })?;

    let str_span = if base != BaseN::Decimal {
        span.shift_start_right(2)
    } else {
        span
    };

    let literal = u128::from_str_radix(&parser.input[str_span], base as u32)
        .inspect_err(|k| parser.error(ParseError::new(ParseErrorKind::IntError(*k.kind()), span)))
        .map(LiteralExpression::Integer)
        .ok()?;

    let kind = ExpressionKind::Literal(literal);

    Some(Expression { kind, span })
}

#[inline]
pub fn float_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let span = parser.next_if_map_errored(|t| {
        match t {
            Some(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, .. }, span }) =>
                Ok(span),
            Some(Token { kind, span }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Float, Some(kind)), span)),
            None => {
                let span = Span::empty_from_start(parser.input.len());
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Float, None), span))
            }
        }
    })?;

    let literal = f64::from_str(&parser.input[span])
        .inspect_err(|_| parser.error(ParseError::new(ParseErrorKind::FloatError, span)))
        .map(|f| LiteralExpression::Floating(OrderedFloat(f)))
        .ok()?;

    let kind = ExpressionKind::Literal(literal);

    Some(Expression { kind, span })
}

#[inline]
pub fn boolean_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (bool_result, span) = parser.next_if_map_errored(|t| {
        match t {
            Some(Token { kind: TokenKind::TrueKeyword, span }) =>
                Ok((true, span)),
            Some(Token { kind: TokenKind::FalseKeyword, span }) =>
                Ok((false, span)),
            Some(Token { kind, span }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Boolean, Some(kind)), span)),
            None => {
                let span = Span::empty_from_start(parser.input.len());
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Boolean, None), span))
            }
        }
    })?;

    let literal_kind = LiteralExpression::Bool(bool_result);
    let kind = ExpressionKind::Literal(literal_kind);

    Some(Expression { kind, span })
}

#[inline]
pub fn null_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let null_token = parser.next_if_kind_errored(TokenKind::NullKeyword)?;
    let kind = ExpressionKind::Literal(LiteralExpression::Null);
    let span = null_token.span;

    Some(Expression { kind, span })
}

#[inline]
pub fn match_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let match_kw = parser.next_if_kind_errored(TokenKind::MatchKeyword)?;
    let matcher = parser.expression()?;
    parser.advance_until_kind(TokenKind::CurlyBracketLeft)?;
    let match_arms = parser.comma_seperated_map(TokenKind::CurlyBracketRight, match_arm)?;
    let cbr = parser.advance_until_kind(TokenKind::CurlyBracketRight)?;
    let span = Span::new_spanned(match_kw.span, cbr.span);
    let kind = ExpressionKind::Match(Box::new(matcher), match_arms);

    Some(Expression { kind, span })
}

#[inline]
fn match_arm<'t>(parser: &mut Parser<'t>) -> Option<MatchArm<'t>> {
    let expression = if parser.next_if(|t| t.kind == TokenKind::Underscore).is_some() {
        None
    } else {
        parser.expression()
    };

    parser.advance_until_kind(TokenKind::FatArrow)?;
    let block = if parser.is_next(TokenKind::ParanthesisLeft) {
        block(parser)?
    } else {
        let expr_block = assignment_expression(parser)?;
        let span = expr_block.span;
        let kind = StatementKind::Expression(expr_block);
        Spanned::new(thin_vec![Statement { kind, span }], span)

    };
    Some(MatchArm { expression, block })
}

#[cfg(test)]
mod tests {
    use super::*;
}
