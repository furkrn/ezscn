#![no_std]

extern crate alloc;
use alloc::boxed::Box;
use ezscn_tokens::{Span, Spanned};
use thin_vec::ThinVec;

use crate::statement::Statement;
use crate::expression::Expression;

pub mod expression;
pub mod statement;

pub type Ast<'i> = ThinVec<Item<'i>>;
pub type Identifier<'s> = &'s str;
pub type Block<'s> = Spanned<ThinVec<Statement<'s>>>;

#[derive(Debug, Eq, PartialEq)]
pub struct Item<'i> {
    pub kind: ItemKind<'i>,
    pub span: Span,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ItemKind<'i> {
    Enum(EnumItem<'i>),
    Struct(StructItem<'i>),
    Config(ConfigItem<'i>),
    Const(ConstItem<'i>),
    Func(FuncItem<'i>),
    Sig(SigItem<'i>),
    Import(Path<'i>),
    Feature(FeatureItem<'i>),
    Statement(Statement<'i>),
    Visible(VisibilityModifiers, Box<Item<'i>>),
    AttributeCollectedItem(ThinVec<Path<'i>>, Box<Item<'i>>),
}

#[derive(Debug, Eq, PartialEq)]
pub struct EnumItem<'i> {
    pub identifier: Identifier<'i>,
    pub items: ThinVec<EnumMember<'i>>,
    pub flags: bool,
    pub derived_type: Option<ReturnType<'i>>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct EnumMember<'i> {
    pub identifier: Identifier<'i>,
    pub default_value: Option<Expression<'i>>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct StructItem<'i> {
    pub identifier: Identifier<'i>,
    pub members: StructMemberDefinition<'i>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum StructMemberDefinition<'i> {
    Field(ThinVec<Field<'i>>),
    Tuple(ThinVec<ReturnType<'i>>),
    Zero,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Field<'i> {
    pub identifier: Identifier<'i>,
    pub return_type: ReturnType<'i>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReturnType<'t> {
    pub kind: ReturnTypeKind<'t>,
    pub span: Span,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ReturnTypeKind<'i> {
    Type(Path<'i>),
    Tuple(ThinVec<ReturnType<'i>>),
    Array(Box<ReturnType<'i>>),
    Nullable(Box<ReturnType<'i>>),
}

#[derive(Debug, Eq, PartialEq)]
pub struct ConfigItem<'i> {
    pub members: ThinVec<ConfigMember<'i>>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ConfigMember<'i> {
    pub identifier: Identifier<'i>,
    pub expression: Expression<'i>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ConstItem<'i> {
    pub identifier: Identifier<'i>,
    pub return_type: ReturnType<'i>,
    pub eq: Expression<'i>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct FuncItem<'i> {
    pub identifier: Identifier<'i>,
    pub params: ThinVec<FuncParam<'i>>,
    pub block: Option<Block<'i>>,
    pub return_type: Option<ReturnType<'i>>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct FuncParam<'i> {
    pub identifier: Identifier<'i>,
    pub return_type: ReturnType<'i>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct SigItem<'i> {
    pub sig_type: Option<ReturnType<'i>>,
    pub identifier: Identifier<'i>
}

#[derive(Debug, Eq, PartialEq)]
pub struct FeatureItem<'i> {
    pub feature_ident: Path<'i>,
    pub implementation: Option<Path<'i>>,
    pub items: ThinVec<Item<'i>>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Path<'i> {
    pub identifiers: ThinVec<Identifier<'i>>,
    pub span: Span,
}

#[derive(Debug, Eq, PartialEq)]
pub enum VisibilityModifiers {
    Public,
}
