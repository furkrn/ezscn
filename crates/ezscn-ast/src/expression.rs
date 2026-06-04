use alloc::borrow::Cow;
use alloc::boxed::Box;
use ezscn_tokens::Span;
use thin_vec::ThinVec;
use ordered_float::OrderedFloat;

use crate::{Block, Identifier, Path};

pub type StringLiteral<'s> = Cow<'s, str>;

#[derive(Debug, Eq, PartialEq)]
pub struct Expression<'s> {
    pub kind: ExpressionKind<'s>,
    pub span: Span,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExpressionKind<'s> {
    Literal(LiteralExpression<'s>),
    Access(AccessExpression<'s>),
    Reference(Box<Expression<'s>>, Box<Expression<'s>>),
    Call(Box<Expression<'s>>, ThinVec<Expression<'s>>),
    New(Path<'s>, ThinVec<StructInitialization<'s>>),
    Array(ThinVec<Expression<'s>>),
    Unary(UnaryOperator, Box<Expression<'s>>),
    Assignment(Box<Expression<'s>>, AssignmentOperator, Box<Expression<'s>>),
    Conditional(Box<Expression<'s>>, ConditionalOperator, Box<Expression<'s>>),
    Logical(Box<Expression<'s>>, LogicalOperator, Box<Expression<'s>>),
    Equality(Box<Expression<'s>>, EqualityOperator, Box<Expression<'s>>),
    Comparision(Box<Expression<'s>>, ComparisionOperator, Box<Expression<'s>>),
    Binary(Box<Expression<'s>>, BinaryOperator, Box<Expression<'s>>),
    PostOp(Box<Expression<'s>>, PostOperator),
    Index(Box<Expression<'s>>, Box<Expression<'s>>),
    Tuple(ThinVec<Expression<'s>>),
    Match(Box<Expression<'s>>, ThinVec<MatchArm<'s>>),
    ShortCurcuit(Box<Expression<'s>>),
    Ternary(Box<Expression<'s>>, Box<Expression<'s>>, Box<Expression<'s>>),
}

#[derive(Debug, Eq, PartialEq)]
pub enum AccessExpression<'s> {
    Identifier(Identifier<'s>),
    SelfAccess,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LiteralExpression<'s> {
    Bool(bool),
    Null,
    String(StringLiteral<'s>),
    Integer(u128),
    Floating(OrderedFloat<f64>),
    Char(char),
}

#[derive(Debug, Eq, PartialEq)]
pub struct StructInitialization<'s> {
    pub identifier: Identifier<'s>,
    pub expression: Expression<'s>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum UnaryOperator {
    Negative,
    Not,
    Plus,
    Addition,
    Substraction,
    Complement,
}

#[derive(Debug, Eq, PartialEq)]
pub enum AssignmentOperator {
    Assign,
    BitLeft,
    BitRight,
    And,
    Or,
    Xor,
    Complement,
    Multiplication,
    Division,
    Addition,
    Substraction,
    Modulo
}

#[derive(Debug, Eq, PartialEq)]
pub enum ConditionalOperator {
    And,
    Or,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LogicalOperator {
    Or,
    And,
    Xor,
}

#[derive(Debug, Eq, PartialEq)]
pub enum EqualityOperator {
    Equals,
    NotEquals,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ComparisionOperator {
    LessThanEquals,
    LessThan,
    GreaterThan,
    GreaterThanEquals,
}

#[derive(Debug, Eq, PartialEq)]
pub enum BinaryOperator {
    BitLeft,
    BitRight,
    Multiplication,
    Division,
    Addition,
    Substraction,
    Modulo,
}

#[derive(Debug, Eq, PartialEq)]
pub enum PostOperator {
    Increment,
    Decrement,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MatchArm<'e> {
    pub expression: Option<Expression<'e>>,
    pub block: Block<'e>,
}
