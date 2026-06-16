use ezscn_tokens::Span;
use thin_vec::ThinVec;

use crate::{Identifier, Block};
use crate::expression::Expression;

#[derive(Debug, Eq, PartialEq)]
pub struct Statement<'e> {
    pub kind: StatementKind<'e>,
    pub span: Span,
}

#[derive(Debug, Eq, PartialEq)]
pub enum StatementKind<'e> {
    Expression(Expression<'e>),
    Return(Option<Expression<'e>>),
    Let(ThinVec<IdentifierOrUnderscore<'e>>, Option<Expression<'e>>),
    ForLoop(ForLoopStatement<'e>),
    WhileLoop(WhileLoopStatement<'e>),
    Break,
    Continue,
}

#[derive(Debug, Eq, PartialEq)]
pub enum IdentifierOrUnderscore<'i> {
    Identifier(Identifier<'i>),
    Underscore,
}

#[derive(Debug, Eq, PartialEq)]
pub enum WaitStatementKind<'e> {
    Expression(Expression<'e>),
    Until(Identifier<'e>),
}

#[derive(Debug, Eq, PartialEq)]
pub struct ForLoopStatement<'e> {
    pub identifier: IdentifierOrUnderscore<'e>,
    pub expression: Expression<'e>,
    pub block: Block<'e>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct WhileLoopStatement<'e> {
    pub expression: Expression<'e>,
    pub block: Block<'e>,
}
