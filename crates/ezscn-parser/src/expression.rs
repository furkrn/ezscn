use alloc::boxed::Box;
use core::str::FromStr;
use ezscn_ast::expression::*;
use ezscn_error::{LiteralKind, ParseError, ParseErrorKind};
use ezscn_tokens::{BaseN, CharacterEscapeType, Token,
    TokenKind, Span, SpanImpl, Spanned, StringOptions};
use ordered_float::OrderedFloat;
use thin_vec::thin_vec;

use crate::items::return_type;
use crate::statement::{statement, block};
use crate::{EndLineInformation, Parser};
use crate::string::*;

#[inline]
pub fn expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    assignment_expression(parser)
}

#[inline]
pub fn assignment_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = conditional_or_expression(parser)?;
    while let Some(op) = assignment_op(parser) {
        let right = conditional_or_expression(parser)?;
        let span = Span::merge(left.span, right.span);
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
pub fn conditional_or_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = conditional_and_expression(parser)?;
    while parser.next_if(|t| t.kind == TokenKind::OrOr).is_some() {
        let right = conditional_and_expression(parser)?;
        let span = Span::merge(left.span, right.span);
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
        let span = Span::merge(left.span, right.span);
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
        let span = Span::merge(left.span, right.span);
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
        let span = Span::merge(left.span, right.span);
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
        let span = Span::merge(left.span, right.span);
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
        let span = Span::merge(left.span, right.span);
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
    let left = shift_expression(parser)?;
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

    let right = shift_expression(parser)?;
    let span = Span::merge(left.span, right.span);
    let kind = ExpressionKind::Comparision(Box::new(left), op, Box::new(right));

    Some(Expression { kind, span })
}

#[inline]
pub fn shift_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = additive_expression(parser)?;
    while let Some(op) = shift_op(parser) {
        let right = additive_expression(parser)?;
        let span = Span::merge(left.span, right.span);
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
        let span = Span::merge(left.span, right.span);
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
        let span = Span::merge(left.span, right.span);
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
            Some(Token { kind: TokenKind::Not, span, .. }) =>
                Ok((UnaryOperator::Not, span)),
            Some(Token { kind: TokenKind::Plus, span, .. }) =>
                Ok((UnaryOperator::Plus, span)),
            Some(Token { kind: TokenKind::Minus, span, .. }) =>
                Ok((UnaryOperator::Negative, span)),
            Some(Token { kind: TokenKind::PlusPlus, span, .. }) =>
                Ok((UnaryOperator::Increment, span)),
            Some(Token { kind: TokenKind::MinusMinus, span, .. }) =>
                Ok((UnaryOperator::Decrement, span)),
            Some(Token { kind: TokenKind::Tilde, span, .. }) =>
                Ok((UnaryOperator::Complement, span)),
            token => Err(token),
        }
    });

    let Some((op, start_span)) = unary_matcher else {
        return postfix_expression(parser);
    };

    let expr = unary_expression(parser)?;
    let span = Span::merge(start_span, expr.span);
    let kind = ExpressionKind::Unary(op, Box::new(expr));

    Some(Expression { kind, span })
}

#[inline]
pub fn postfix_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let mut left = primary_expression(parser);
    loop {
        left = match parser.peek().map(|t| t.kind) {
            Some(TokenKind::ParanthesisLeft) =>
                call_expression(parser, left),
            Some(TokenKind::SquareBracketLeft) =>
                index_expression(parser, left),
            Some(TokenKind::Dot) =>
                reference_expression(parser, left),
            Some(TokenKind::PlusPlus | TokenKind::MinusMinus) =>
                post_op_expression(parser, left),
            Some(TokenKind::QuestionMark) =>
                short_circuit_expression(parser, left),
            Some(TokenKind::ColonColon) =>
                path_expression(parser, left),
            _ => break left
        };
    }
}

#[inline]
pub fn call_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while parser.next_if_kind(TokenKind::ParanthesisLeft).is_some() {
        let args = parser.comma_seperated_map(TokenKind::ParanthesisRight, expression)?;
        let pr_token = parser.advance_until_kind(TokenKind::ParanthesisRight)?;
        let span = Span::merge(left.span, pr_token.span);
        let kind = ExpressionKind::Call(Box::new(left), args);
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn index_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while parser.next_if_kind(TokenKind::SquareBracketLeft).is_some() {
        let exprs = parser.comma_seperated_map(TokenKind::SquareBracketRight, expression)?;
        let sbr_token = parser.advance_until_kind(TokenKind::SquareBracketRight)?;
        let span = Span::merge(left.span, sbr_token.span);
        let kind = ExpressionKind::Index(Box::new(left), exprs);
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn reference_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while parser.next_if_kind(TokenKind::Dot).is_some() {
        let right = postfix_expression(parser)?;
        let span = Span::merge(left.span, right.span);
        let kind = ExpressionKind::Reference(Box::new(left), Box::new(right));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn post_op_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while let Some((op, end_span)) = post_op_span(parser) {
        let span = Span::merge(left.span, end_span);
        let kind = ExpressionKind::PostOp(Box::new(left), op);
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
fn post_op_span(parser: &mut Parser<'_>) -> Option<(PostOperator, Span)> {
    parser.next_if_map(|t| {
        match t {
            Some(Token { kind: TokenKind::PlusPlus, span, .. }) =>
                Ok((PostOperator::Increment, span)),
            Some(Token { kind: TokenKind::MinusMinus, span, .. }) =>
                Ok((PostOperator::Decrement, span)),
            token => Err(token),
        }
    })
}

#[inline]
pub fn short_circuit_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while let Some(mark) = parser.next_if_kind(TokenKind::QuestionMark) {
        let span = Span::merge(left.span, mark.span);
        let kind = ExpressionKind::ShortCircuit(Box::new(left));
        left = Expression { kind, span }
    }

    Some(left)
}

#[inline]
pub fn path_expression<'t>(parser: &mut Parser<'t>, left: Option<Expression<'t>>) -> Option<Expression<'t>> {
    let mut left = left.or_else(|| primary_expression(parser))?;
    while parser.next_if_kind(TokenKind::ColonColon).is_some() {
        let expr = expression(parser)?;
        let span = Span::merge(left.span, expr.span);
        let kind = ExpressionKind::Path(Box::new(left), Box::new(expr));
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
        Some(Token { kind: TokenKind::IfKeyword, .. }) =>
            if_expression(parser),
        Some(Token { kind: TokenKind::Dollar, .. }) =>
            closure_expression(parser),
        _ => {
            let token = parser.next()?;
            let error = ParseError::new(ParseErrorKind::UnexpectedToken(token.kind), token.span, token.line);
            parser.error(error);
            primary_expression(parser)
        }
    }
}

#[inline]
pub fn access_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (access_expression, span) = parser.advance_map(|t| {
        match t {
            Ok(Token { kind: TokenKind::Identifier, span, .. }) =>
                Ok((AccessExpression::Identifier(&parser.input[span]), span)),
            Ok(Token { kind: TokenKind::SelfKeyword, span, .. }) =>
                Ok((AccessExpression::SelfAccess, span)),
            Ok(Token { kind: found, span, line }) => {
                let kind = ParseErrorKind::InvalidToken(TokenKind::Identifier, found);
                Err(ParseError::new(kind, span, line))
            },
            Err(EndLineInformation { line, len }) => {
                Err(ParseError::new(ParseErrorKind::ExpectedToken(TokenKind::Identifier), Span::empty_from_start(len), line))
            },
        }
    })?;

    let kind = ExpressionKind::Access(access_expression);

    Some(Expression { kind, span })
}

#[inline]
pub fn new_init_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let new_kw = parser.next_if_kind_errored(TokenKind::NewKeyword)?;
    let return_type = return_type(parser)?;
    let (inits, end_span) = if parser.next_if_kind(TokenKind::CurlyBracketLeft).is_some() {
        let inits = parser.comma_seperated_map(TokenKind::CurlyBracketRight, new_field_member)?;
        let cbr = parser.advance_until_kind(TokenKind::CurlyBracketRight)?;

        (NewExprType::Field(inits), cbr.span)
    } else if parser.next_if_kind(TokenKind::ParanthesisLeft).is_some() {
        let inits = parser.comma_seperated_map(TokenKind::ParanthesisRight, expression)?;
        let pr = parser.advance_until_kind(TokenKind::ParanthesisRight)?;

        (NewExprType::Tuple(inits), pr.span)
    } else {
        (NewExprType::Zero, return_type.span)
    };

    let span = Span::merge(new_kw.span, end_span);
    let kind = ExpressionKind::New(return_type, inits);

    Some(Expression { kind, span })
}

#[inline]
fn new_field_member<'t>(parser: &mut Parser<'t>) -> Option<FieldInitialization<'t>> {
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

    Some(FieldInitialization { identifier, expression })
}

#[inline]
pub fn tuple_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let pl = parser.advance_until_kind(TokenKind::ParanthesisLeft)?;
    let exprs = parser.comma_seperated_map(TokenKind::ParanthesisRight, expression)?;
    let pr = parser.advance_until_kind(TokenKind::ParanthesisRight)?;
    let span = Span::merge(pl.span, pr.span);
    let kind = ExpressionKind::Tuple(exprs);

    Some(Expression { kind, span })
}

#[inline]
pub fn array_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let sbl_span = parser.advance_until_kind(TokenKind::SquareBracketLeft)?.span;
    let exprs = parser.comma_seperated_map(TokenKind::SquareBracketRight, expression)?;
    let sbr_span = parser.advance_until_kind(TokenKind::SquareBracketRight)?.span;
    let span = Span::merge(sbl_span, sbr_span);
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
            let error = ParseError::new(ParseErrorKind::LiteralsExpected(Some(token.kind)), token.span, token.line);
            parser.error(error);
            literal_expression(parser)
        }
    }
}

#[inline]
pub fn string_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (options, quote_start, span, line) = parser.next_if_map_errored(|t| {
        match t {
            Ok(Token { kind: TokenKind::StringLiteral { options, quote_start, terminated, ending_line }, span, line }) =>
                if terminated {
                    Ok((options, quote_start, span, line))
                } else {
                    Err(ParseError::new(ParseErrorKind::UnterminatedString(ending_line), span, line))
                },
            Ok(Token { kind, span, line }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::String, Some(kind)), span, line)),
            Err(EndLineInformation { line, len }) => {
                let span = Span::empty_from_start(len);
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::String, None), span, line))
            }
        }
    })?;

    let end = if options.contains(StringOptions::MULTILINE_STR) {
        span.end() - 2
    } else {
        span.end() - 1
    };

    let raw_str = &parser.input[quote_start + 1..end];
    let string_literal = if options.contains(StringOptions::RAWSTR) {
        StringLiteral::Borrowed(raw_str)
    } else {
        UnescapedStringBuilder::new(raw_str, line, &mut parser.errors)
            .collect()
    };

    let kind = ExpressionKind::Literal(LiteralExpression::String(string_literal));
    Some(Expression { kind, span })
}

#[inline]
pub fn char_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (escape_type, span, line) = parser.next_if_map_errored(|t| {
        match t {
            Ok(Token { kind: TokenKind::CharacterLiteral { escape_type, terminated }, span, line }) =>
                if terminated {
                    Ok((escape_type, span, line))
                } else {
                    Err(ParseError::new(ParseErrorKind::UnterminatedChar, span, line))
                }
            Ok(Token { kind, span, line }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Char, Some(kind)), span, line)),
            Err(EndLineInformation { line, len }) => {
                let span = Span::empty_from_start(len);
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Char, None), span, line))
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
        parser.error(ParseError::new(ParseErrorKind::UnknownEscapeSequence, span, line));
        None
    }
}

#[inline]
pub fn integer_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (base, span, line) = parser.next_if_map_errored(|t| {
        match t {
            Ok(Token { kind: TokenKind::NumberLiteral { base, is_floating }, span, line }) => {
                if is_floating {
                    Err(ParseError::new(ParseErrorKind::ExpectedIntegerFoundFloating, span, line))
                } else {
                    Ok((base, span, line))
                }
            },
            Ok(Token { kind, span, line }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Integer, Some(kind)), span, line)),
            Err(EndLineInformation { line, len }) => {
                let span = Span::empty_from_start(len);
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Integer, None), span, line))
            }
        }
    })?;

    let str_span = if base != BaseN::Decimal {
        span.shift_start_right(2)
    } else {
        span
    };

    let literal = u128::from_str_radix(&parser.input[str_span], base as u32)
        .inspect_err(|k| parser.error(ParseError::new(ParseErrorKind::IntError(*k.kind()), span, line)))
        .map(LiteralExpression::Integer)
        .ok()?;

    let kind = ExpressionKind::Literal(literal);

    Some(Expression { kind, span })
}

#[inline]
pub fn float_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (span, line) = parser.next_if_map_errored(|t| {
        match t {
            Ok(Token { kind: TokenKind::NumberLiteral { base: BaseN::Decimal, .. }, span, line }) =>
                Ok((span, line)),
            Ok(Token { kind, span, line }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Float, Some(kind)), span, line)),
            Err(EndLineInformation { line, len }) => {
                let span = Span::empty_from_start(len);
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Float, None), span, line))
            }
        }
    })?;

    let literal = f64::from_str(&parser.input[span])
        .inspect_err(|_| parser.error(ParseError::new(ParseErrorKind::FloatError, span, line)))
        .map(|f| LiteralExpression::Floating(OrderedFloat(f)))
        .ok()?;

    let kind = ExpressionKind::Literal(literal);

    Some(Expression { kind, span })
}

#[inline]
pub fn boolean_literal<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let (bool_result, span) = parser.next_if_map_errored(|t| {
        match t {
            Ok(Token { kind: TokenKind::TrueKeyword, span, .. }) =>
                Ok((true, span)),
            Ok(Token { kind: TokenKind::FalseKeyword, span, .. }) =>
                Ok((false, span)),
            Ok(Token { kind, span, line }) =>
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Boolean, Some(kind)), span, line)),
            Err(EndLineInformation { line, len }) => {
                let span = Span::empty_from_start(len);
                Err(ParseError::new(ParseErrorKind::LiteralExpected(LiteralKind::Boolean, None), span, line))
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
    let span = Span::merge(match_kw.span, cbr.span);
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

    let if_clause = if parser.next_if_kind(TokenKind::IfKeyword).is_some() {
        Some(parser.expression()?)
    } else {
        None
    };

    parser.advance_until_kind(TokenKind::FatArrow)?;
    let block = if parser.is_next(TokenKind::CurlyBracketLeft) {
        block(parser)?
    } else {
        let statement = statement(parser)?;
        let span = statement.span;

        Spanned::new(thin_vec![statement], span)
    };

    Some(MatchArm { expression, if_clause, block })
}

#[inline]
pub fn if_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
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

    let span = Span::merge(if_kw.span, end_span);
    let kind = ExpressionKind::If(Box::new(if_arm), else_if_arms, else_arm);

    Some(Expression { kind, span })
}

#[inline]
pub fn closure_expression<'t>(parser: &mut Parser<'t>) -> Option<Expression<'t>> {
    let dollar_token = parser.advance_until_kind(TokenKind::Dollar)?;
    parser.advance_until_kind(TokenKind::ParanthesisLeft)?;
    let param_list = parser.comma_seperated_map(TokenKind::ParanthesisRight, closure_param)?;
    parser.advance_until_kind(TokenKind::ParanthesisRight)?;
    parser.advance_until_kind(TokenKind::FatArrow)?;
    let block = block(parser)?;

    let span = Span::merge(dollar_token.span, block.span);
    let kind = ExpressionKind::Closure(param_list, block);

    Some(Expression { kind, span })
}

#[inline]
fn closure_param<'t>(parser: &mut Parser<'t>) -> Option<ClosureParam<'t>> {
    let identifier_token = parser.advance_until_identifier_spanned()?;
    let identifier = identifier_token.data;
    let mut span = identifier_token.span;
    let return_type = if parser.next_if_kind(TokenKind::Colon).is_some() {
        let return_type = return_type(parser)?;
        span = Span::merge(span, return_type.span);

        Some(return_type)
    } else {
        None
    };

    Some(ClosureParam { identifier, return_type, span })
}

#[cfg(test)]
mod tests {
    use core::assert_matches;
    use super::*;

    macro_rules! assert_matches_capture {
        ($left: expr, $(|)? $($pattern:pat_param)|+ $(if $guard: expr)? => ($($cap:ident),+)) => {
            match $left {
                $($pattern)|+ $(if $guard)? => ($($cap),+),
                left => {
                    assert_matches!(left, $($pattern)|+ $(if $guard)?);
                    panic!()
                }
            }
        };
    }

    #[test]
    pub fn primary_access_expression() {
        let mut parser = Parser::from("self ident _ident $ident @ident");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Access(AccessExpression::SelfAccess), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident")), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("_ident")), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("$ident")), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("@ident")), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[allow(clippy::zero_prefixed_literal, reason = "readability")]
    #[test]
    pub fn primary_tuple_expression() {
        fn matches_expr(values: &[Expression<'_>]) -> bool {
            let mut iter = values.iter();

            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(094)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(23)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(3214)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(25)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(654745)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Array(t), .. }) if t.is_empty());
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from("(094,23,3214,25,654745,[],)");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Tuple(e), .. }) if matches_expr(&e));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn empty_tuple_expression() {
        let mut parser = Parser::from("()");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Tuple(e), .. }) if e.is_empty());
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[allow(clippy::zero_prefixed_literal, reason = "readability")]
    #[test]
    pub fn primary_array_expression() {
        fn matches_expr(values: &[Expression<'_>]) -> bool {
            let mut iter = values.iter();

            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(094)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(23)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(3214)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(25)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(654745)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Array(t), .. }) if t.is_empty());
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from("[094,23,3214,25,654745,[]]");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Array(e), .. }) if matches_expr(&e));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn empty_array_expression() {
        let mut parser = Parser::from("[]");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Array(e), .. }) if e.is_empty());
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_string_literal_expression() {
        fn matches_string_sequence(p: Option<Expression<'_>>, str: &str) {
            assert_matches!(p, Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::String(s)), ..}) if s == str)
        }

        let mut parser = Parser::from(r#"
            "" "ezscn" "escaped ezscn \t\t\n"
                r"raw ezscn \ \ \\ \\\\ \e\e" m"ez
                scn"m mr"wtf"m MR"tf"m rm"TFF"m RM"pat"m
                "#);
        
        matches_string_sequence(parser.expression(), "");
        matches_string_sequence(parser.expression(), "ezscn");
        matches_string_sequence(parser.expression(), "escaped ezscn \t\t\n");
        matches_string_sequence(parser.expression(), r#"raw ezscn \ \ \\ \\\\ \e\e"#);
        matches_string_sequence(parser.expression(), r#"ez
                scn"#);
        matches_string_sequence(parser.expression(), "wtf");
        matches_string_sequence(parser.expression(), "tf");
        matches_string_sequence(parser.expression(), "TFF");
        matches_string_sequence(parser.expression(), "pat");
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_char_literal_expression() {
        fn matches_char_sequence(e: Option<Expression<'_>>, char: char) {
            assert_matches!(e, Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Char(c)), .. }) if c == char)
        }

        let mut parser = Parser::from(r#"'a' '!' 'ş' '\a' '\b' '\e' '\f' '\n'
            '\r' '\t' '\v' '\\' '\?' '\"' '\xF' '\x1111'
            '\uFFFF' '\U0010FFFE'"#);

        matches_char_sequence(parser.expression(), 'a');
        matches_char_sequence(parser.expression(), '!');
        matches_char_sequence(parser.expression(), 'ş');
        matches_char_sequence(parser.expression(), 0x07 as char);
        matches_char_sequence(parser.expression(), 0x08 as char);
        matches_char_sequence(parser.expression(), 0x1B as char);
        matches_char_sequence(parser.expression(), 0x0C as char);
        matches_char_sequence(parser.expression(), '\n');
        matches_char_sequence(parser.expression(), '\r');
        matches_char_sequence(parser.expression(), '\t');
        matches_char_sequence(parser.expression(), 0x0B as char);
        matches_char_sequence(parser.expression(), '\\');
        matches_char_sequence(parser.expression(), 0x3F as char);
        matches_char_sequence(parser.expression(), '\"');
        matches_char_sequence(parser.expression(), '\u{F}');
        matches_char_sequence(parser.expression(), '\u{1111}');
        matches_char_sequence(parser.expression(), '\u{FFFF}');
        matches_char_sequence(parser.expression(), '\u{10FFFE}');
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_integer_literal_expression() {
        let mut parser = Parser::from("0 2 43278493287 4277894234 7532412832198 54743758934981 48481881818818");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(43278493287)), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(4277894234)), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(7532412832198)), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(54743758934981)), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(48481881818818)), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_float_literal_expression() {
        let mut parser = Parser::from("0.33 0.312321 0.3214123567 0.4234235E+1 0.43325723985734895e-1 0.423742374e-1321321 0.1e-1");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Floating(OrderedFloat(0.33))), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Floating(OrderedFloat(0.312321))), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Floating(OrderedFloat(0.3214123567))), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Floating(OrderedFloat(0.4234235E+1))), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Floating(OrderedFloat(0.43325723985734895e-1))), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Floating(OrderedFloat(0.423742374e-1321321))), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Floating(OrderedFloat(0.1e-1))), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_boolean_literal_expression() {
        let mut parser = Parser::from("true false false true");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Bool(true)), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Bool(false)), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Bool(false)), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Bool(true)), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_null_literal_expression() {
        let mut parser = Parser::from("null null null");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Null), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Null), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Null), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_match_expression() {
        fn matches_arms(a: &[MatchArm<'_>]) -> bool {
            let mut iter = a.iter();

            assert_matches!(iter.next(), Some(MatchArm {
                expression: Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("z")), .. }),
                if_clause: None,
                block })
                    if block.data.len() == 1);

            assert_matches!(iter.next(), Some(MatchArm {
                expression: Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("x")), .. }),
                if_clause: None,
                block })
                    if block.data.len() == 1);

            assert_matches!(iter.next(), Some(MatchArm {
                expression: Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("y")), .. }),
                if_clause: None,
                block })
                    if block.data.len() == 1);

            assert_matches!(iter.next(), Some(MatchArm {
                expression: Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("m")), .. }),
                if_clause: None,
                block })
                    if block.data.len() == 1);

            assert_matches!(iter.next(), Some(MatchArm {
                expression: Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("n")), .. }),
                if_clause: Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("l")), .. }),
                block })
                    if block.data.len() == 1);

            assert_matches!(iter.next(), Some(MatchArm {
                expression: None,
                if_clause: None,
                block })
                    if block.data.len() == 1);
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from(r#"match x {
                z => 1,
                x => 2,
                y => 3,
                m => {
                    4;
                },
                n if l => 0,
                _ => 0
            }
            "#);

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Match(matcher, arms), .. })
            if matches_arms(&arms) && matches!(matcher.kind, ExpressionKind::Access(..)));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_if_expression() {
        let mut parser = Parser::from(r#"
            if x {
                a;
            } else if y {
                b;
            } else if z {
                c;
            } else {
                d;
            }

            if i { e; }

            if l {
                f;
            } else {
                g;
            }

            if k {
                h;
            } else if m {
                k;
            }


            "#);

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::If(_, else_if, Some(_)), .. }) if else_if.len() == 2);
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::If(_, else_if, None), .. }) if else_if.is_empty());
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::If(_, else_if, Some(_)), .. }) if else_if.is_empty());
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::If(_, else_if, None), .. }) if else_if.len() == 1);
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_new_expression() {
        let mut parser = Parser::from(r#"
            new EmptyIdent
            new EmptyIdentCBL {}
            new IdentCBLWithInits {
                S = 0,
                I = 2,
                H = 1,
                T = 3,
            }
            new IdentCBLWithOptionalInits {
            a,b,c,d = 5,
            }"#);

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::New(.., NewExprType::Zero), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::New(.., NewExprType::Field(props)), .. }) if props.is_empty());
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::New(.., NewExprType::Field(props)), .. }) if props.len() == 4);
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::New(.., NewExprType::Field(props)), .. }) if props.len() == 4);
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn primary_closure_expression() {
        fn matches_second_closure_params(p: &[ClosureParam<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(ClosureParam { identifier: "a", return_type: None, .. }));
            assert_matches!(iter.next(), Some(ClosureParam { identifier: "b", return_type: Some(_), .. }));
            assert_matches!(iter.next(), Some(ClosureParam { identifier: "c", return_type: None, .. }));
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from("$() => {} $(a, b: X, c) => {}");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Closure(params, ..), .. }) if params.is_empty());
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Closure(params, ..), .. }) if matches_second_closure_params(&params));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }

    #[test]
    pub fn assignment_expression() {
        let mut parser = Parser::from("a = b = c d <<= 0 e >>= 1 f &= 2 g |= 3 h ^= 4 i ~= 5 j *= 6 k /= 7 l += 8 m -= 9 n %= 10");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(left, AssignmentOperator::Assign, _), .. })
            if matches!(&*left, Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::Assign, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::BitLeft, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::BitRight, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::And, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::Or, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::Xor, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::Complement, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::Multiplication, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::Division, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::Addition, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::Substraction, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Assignment(_, AssignmentOperator::Modulo, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn conditional_or_expression() {
        let mut parser = Parser::from("a || b || c");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Conditional(l, ConditionalOperator::Or, _), .. })
            if matches!(&*l, Expression { kind: ExpressionKind::Conditional(_, ConditionalOperator::Or, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn conditional_and_expression() {
        let mut parser = Parser::from("a && b && c");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Conditional(l, ConditionalOperator::And, _), .. })
            if matches!(&*l, Expression { kind: ExpressionKind::Conditional(_, ConditionalOperator::And, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn logical_or_expression() {
        let mut parser = Parser::from("a | b | c");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Logical(l, LogicalOperator::Or, _), .. })
            if matches!(&*l, Expression { kind: ExpressionKind::Logical(_, LogicalOperator::Or, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn logical_xor_expression() {
        let mut parser = Parser::from("a ^ b ^ c");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Logical(l, LogicalOperator::Xor, _), .. })
            if matches!(&*l, Expression { kind: ExpressionKind::Logical(_, LogicalOperator::Xor, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn logical_and_expression() {
        let mut parser = Parser::from("a & b & c");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Logical(l, LogicalOperator::And, _), .. })
            if matches!(&*l, Expression { kind: ExpressionKind::Logical(_, LogicalOperator::And, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn equality_expression() {
        let mut parser = Parser::from("a != b c == d");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Equality(_, EqualityOperator::NotEquals, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Equality(_, EqualityOperator::Equals, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn comparision_expression() {
        let mut parser = Parser::from("a < b c > d e >= f g <= h");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Comparision(_, ComparisionOperator::LessThan, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Comparision(_, ComparisionOperator::GreaterThan, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Comparision(_, ComparisionOperator::GreaterThanEquals, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Comparision(_, ComparisionOperator::LessThanEquals, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn shift_expression() {
        let mut parser = Parser::from("0 >> 1 2 << 2 5 << 2 >> 1");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Binary(_, BinaryOperator::BitRight, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Binary(_, BinaryOperator::BitLeft, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Binary(l, BinaryOperator::BitRight, _), .. })
            if matches!(&*l, Expression { kind: ExpressionKind::Binary(_, BinaryOperator::BitLeft, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    #[allow(unused_variables, reason = "rustc doesn't care about captures...")]
    pub fn additive_expression() {
        let mut parser = Parser::from("0 + 1 + 2 + 3 - 4");

        let (left, right) = assert_matches_capture!(parser.expression(), Some(Expression { kind: ExpressionKind::Binary(left, BinaryOperator::Substraction, right), .. }) => (left, right));
        assert_matches!(*right, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(4)), .. });
        let (left, right) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Binary(left, BinaryOperator::Addition, right), .. } => (left, right));
        assert_matches!(*right, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(3)), .. });
        let (left, right) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Binary(left, BinaryOperator::Addition, right), .. } => (left, right));
        assert_matches!(*right, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. });
        let (left, right) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Binary(left, BinaryOperator::Addition, right), .. } => (left, right));
        assert_matches!(*right, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. });
        assert_matches!(*left, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), ..});
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    #[allow(unused_variables, reason = "rustc doesn't care about captures...")]
    pub fn multiplicative_expression() {
        let mut parser = Parser::from("0 * 1 * 2 * 3 / 3 % 6");


        let (left, right) = assert_matches_capture!(parser.expression(), Some(Expression { kind: ExpressionKind::Binary(left, BinaryOperator::Modulo, right), .. }) => (left, right));
        assert_matches!(*right, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(6)), .. });
        let (left, right) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Binary(left, BinaryOperator::Division, right), .. } => (left, right));
        assert_matches!(*right, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(3)), .. });
        let (left, right) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Binary(left, BinaryOperator::Multiplication, right), .. } => (left, right));
        assert_matches!(*right, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(3)), .. });
        let (left, right) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Binary(left, BinaryOperator::Multiplication, right), .. } => (left, right));
        assert_matches!(*right, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. });
        let (left, right) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Binary(left, BinaryOperator::Multiplication, right), .. } => (left, right));
        assert_matches!(*right, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. });
        assert_matches!(*left, Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), ..});
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn unary_expression() {
        let mut parser = Parser::from("!a ~b");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Unary(UnaryOperator::Not, _), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Unary(UnaryOperator::Complement, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn unary_minus() {
        let mut parser = Parser::from("-a");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Unary(UnaryOperator::Negative, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn unary_positive() {
        let mut parser = Parser::from("+a");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Unary(UnaryOperator::Plus, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn pre_increment() {
        let mut parser = Parser::from("++x");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Unary(UnaryOperator::Increment, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn pre_decrement() {
        let mut parser = Parser::from("--x");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Unary(UnaryOperator::Decrement, _), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    #[allow(unused_variables, reason = "rustc doesn't care about captures...")]
    pub fn postfix_combined() {
        let mut parser = Parser::from("s.a[0]?.b()[a]++--");

        let (left, right) = assert_matches_capture!(parser.expression(), Some(Expression { kind: ExpressionKind::Reference(left, right), .. }) => (left, right));
        assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("s")), .. });

        let (left, right) = assert_matches_capture!(*right, Expression { kind: ExpressionKind::Reference(left, right), .. } => (left, right));
        let index = assert_matches_capture!(*left, Expression { kind: ExpressionKind::ShortCircuit(exp), .. } => (exp));

        let (acs, params) = assert_matches_capture!(*index, Expression { kind: ExpressionKind::Index(acs, params), .. } => (acs, params));
        assert_matches!(*acs, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("a")), .. });
        assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
        assert!(params.len() == 1);

        let exp = assert_matches_capture!(*right, Expression { kind: ExpressionKind::PostOp(exp, PostOperator::Decrement), .. } => (exp));
        let exp = assert_matches_capture!(*exp, Expression { kind: ExpressionKind::PostOp(exp, PostOperator::Increment), .. } => (exp));

        let (left, params) = assert_matches_capture!(*exp, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
        assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("a")), .. }));
        assert!(params.len() == 1);

        let (left, params) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Call(left, params), .. } => (left, params));
        assert!(params.is_empty());
        assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("b")), .. });

        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    #[allow(unused_variables, reason = "rustc doesn't care about captures...")]
    pub fn postfix_call_expression() {
        fn matches_first_expr(e: Option<Expression<'_>>) {
            let left = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Call(left, params), .. }) if params.is_empty() => (left));
            let left = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Call(left, params), .. } if params.is_empty() => (left));
            let (left, params) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Call(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. }));
            assert!(params.len() == 1);

            let (left, params) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Call(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. }));
            assert!(params.len() == 1);

            let (left, params) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Call(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
            assert!(params.len() == 1);
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("s")), .. });
        }

        fn matches_second_expr(e: Option<Expression<'_>>) {
            let left = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Call(left, params), .. }) if params.is_empty() => (left));
            let (left, params) = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Call(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. }));
            assert!(params.len() == 1);

            let left = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Call(left, params), .. } if matches_second_expr_args(&params) => (left));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("s")), .. });
        }

        fn matches_second_expr_args(p: &[Expression<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. }));
            assert_matches!(iter.next(), None);
            true
        }

        fn matches_third_expr(e: Option<Expression<'_>>) {
            let left = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Call(left, params), .. }) if params.is_empty() => (left));
            let left = assert_matches_capture!(*left, Expression { kind: ExpressionKind::Call(left, params), .. } if matches_third_expr_args(&params) => (left));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("s")), .. });
        }

        fn matches_third_expr_args(p: &[Expression<'_>]) -> bool {
            let mut iter = p.iter();

            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. }));
            assert_matches!(iter.next(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. }));
            assert_matches!(iter.next(), None);
            true
        }

        let mut parser = Parser::from("s(0)(1)(2)()() s(0, 1)(2)() s(0, 1, 2)() s()");

        matches_first_expr(parser.expression());
        matches_second_expr(parser.expression());
        matches_third_expr(parser.expression());
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Call(left, params), .. })
            if params.is_empty() && matches!(left.kind, ExpressionKind::Access(AccessExpression::Identifier("s"))));

        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    #[allow(unused_variables, reason = "rustc doesn't care about captures...")]
    pub fn postfix_index_expression() {
        fn matches_first_expr(e: Option<Expression<'_>>) {
            let (left_index, params) = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Index(left, params), .. }) => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(5)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(4)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(3)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. }));
            assert!(params.len() == 1);

            let (left, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
            assert!(params.len() == 1);

            assert_matches!(*left, Expression { kind: ExpressionKind::Access(..), .. });
        }

        fn matches_second_expr(e: Option<Expression<'_>>) {
            let (left_index, params) = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Index(left, params), .. }) => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(4)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(3)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. }));
            assert!(params.len() == 1);

            let (left, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
            assert!(params.len() == 1);

            assert_matches!(*left, Expression { kind: ExpressionKind::Access(..), .. });
        }

        fn matches_third_expr(e: Option<Expression<'_>>) {
            let (left_index, params) = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Index(left, params), .. }) => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(3)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. }));
            assert!(params.len() == 1);

            let (left, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
            assert!(params.len() == 1);

            assert_matches!(*left, Expression { kind: ExpressionKind::Access(..), .. });
        }

        fn matches_forth_expr(e: Option<Expression<'_>>) {
            let (left_index, params) = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Index(left, params), .. }) => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(2)), .. }));
            assert!(params.len() == 1);

            let (left_index, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. }));
            assert!(params.len() == 1);

            let (left, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
            assert!(params.len() == 1);

            assert_matches!(*left, Expression { kind: ExpressionKind::Access(..), .. });
        }

        fn matches_fifth_expr(e: Option<Expression<'_>>) {
            let (left_index, params) = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Index(left, params), .. }) => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(1)), .. }));
            assert!(params.len() == 1);

            let (left, params) = assert_matches_capture!(*left_index, Expression { kind: ExpressionKind::Index(left, params), .. } => (left, params));
            assert_matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(LiteralExpression::Integer(0)), .. }));
            assert!(params.len() == 1);

            assert_matches!(*left, Expression { kind: ExpressionKind::Access(..), .. });
        }

        let mut parser = Parser::from("s[0][1][2][3][4][5] s[0][1][2][3][4] s[0][1][2][3] s[0][1][2] s[0][1] s[0]");

        matches_first_expr(parser.expression());
        matches_second_expr(parser.expression());
        matches_third_expr(parser.expression());
        matches_forth_expr(parser.expression());
        matches_fifth_expr(parser.expression());
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Index(left, params), .. })
            if matches!(left.kind, ExpressionKind::Access(..)) && params.len() == 1 &&
                matches!(params.first(), Some(Expression { kind: ExpressionKind::Literal(..), .. })));

        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    #[allow(unused_variables, reason = "rustc doesn't care about captures...")]
    pub fn postfix_reference_expression() {
        fn matches_second_expr(e: Option<Expression<'_>>) {
            let (left, right) = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Reference(left, right), .. }) => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident")), .. });
            let (left, right) = assert_matches_capture!(*right, Expression { kind: ExpressionKind::Reference(left, right), .. } => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident2")), .. });
            assert_matches!(*right, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident3")), .. });
        }

        fn matches_third_expr(e: Option<Expression<'_>>) {
            let (left, right) = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Reference(left, right), .. }) => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident1")), .. });
            let (left, right) = assert_matches_capture!(*right, Expression { kind: ExpressionKind::Reference(left, right), .. } => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident2")), .. });
            let (left, right) = assert_matches_capture!(*right, Expression { kind: ExpressionKind::Reference(left, right), .. } => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident3")), .. });
            let (left, right) = assert_matches_capture!(*right, Expression { kind: ExpressionKind::Reference(left, right), .. } => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident4")), .. });
            assert_matches!(*right, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident5")), .. });
        }

        fn matches_forth_expr(e: Option<Expression<'_>>) {
            let (left, right) = assert_matches_capture!(e, Some(Expression { kind: ExpressionKind::Reference(left, right), .. }) => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::SelfAccess), .. });
            let (left, right) = assert_matches_capture!(*right, Expression { kind: ExpressionKind::Reference(left, right), .. } => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::SelfAccess), .. });
            let (left, right) = assert_matches_capture!(*right, Expression { kind: ExpressionKind::Reference(left, right), .. } => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident")), .. });
            let (left, right) = assert_matches_capture!(*right, Expression { kind: ExpressionKind::Reference(left, right), .. } => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::SelfAccess), .. });
            let (left, right) = assert_matches_capture!(*right, Expression { kind: ExpressionKind::Reference(left, right), .. } => (left, right));
            assert_matches!(*left, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("ident")), .. });
            assert_matches!(*right, Expression { kind: ExpressionKind::Access(AccessExpression::Identifier("a")), .. });
        }

        let mut parser = Parser::from("self.ident ident.ident2.ident3 ident1.ident2.ident3.ident4.ident5 self.self.ident.self.ident.a self.0");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Reference(l, r), .. })
            if matches!(l.kind, ExpressionKind::Access(AccessExpression::SelfAccess)) && matches!(r.kind, ExpressionKind::Access(AccessExpression::Identifier("ident"))));

        matches_second_expr(parser.expression());
        matches_third_expr(parser.expression());
        matches_forth_expr(parser.expression());

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::Reference(l, r), .. })
            if matches!(l.kind, ExpressionKind::Access(AccessExpression::SelfAccess)) && matches!(r.kind, ExpressionKind::Literal(LiteralExpression::Integer(0))));

        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn postfix_post_op_expression() {
        let mut parser = Parser::from("a++ a--");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::PostOp(_, PostOperator::Increment), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::PostOp(_, PostOperator::Decrement), .. }));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof())
    }

    #[test]
    pub fn postfix_short_circuit_expression() {
        let mut parser = Parser::from("a? b?? c???");

        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::ShortCircuit(_), .. }));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::ShortCircuit(e), .. }) if matches!(e.kind, ExpressionKind::ShortCircuit(_)));
        assert_matches!(parser.expression(), Some(Expression { kind: ExpressionKind::ShortCircuit(e), .. })
            if matches!(&e.kind, ExpressionKind::ShortCircuit(x)
                if matches!(x.kind, ExpressionKind::ShortCircuit(_))));
        assert_matches!(parser.expression(), None);
        assert!(parser.errors.is_empty());
        assert!(parser.reached_eof());
    }
}
