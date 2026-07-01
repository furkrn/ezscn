use core::ops::ControlFlow;

use crate::*;
use crate::expression::*;
use crate::statement::*;

macro_rules! walk_list {
    ($item_name: ident, $t: ident, $walk_fn: ident) => {
        pub fn $item_name<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, items: &'a ThinVec<$t<'t>>) -> ControlFlow<V::BreakType> {
            for item in items {
                $walk_fn(visitor, item)?;
            }

            ControlFlow::Continue(())
        }
    };

    ($($item_name: ident, $t: ident, $walk_fn: ident);*;) => {
        $(walk_list!($item_name, $t, $walk_fn);)*
    }
}

macro_rules! walk_list_spanned {
    ($item_name: ident, $(&$spanned_lf:lifetime)? $ty: ident, $walk_fn: ident) => {
        pub fn $item_name<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, items: &'a Spanned<ThinVec<$ty<'t>>>) -> ControlFlow<V::BreakType> {
            for item in &items.data {
                $walk_fn(visitor, item)?;
            }

            ControlFlow::Continue(())
        }
    };

    ($($item_name: ident, $ty: ident, $walk_fn: ident);*;) => {
        $(walk_list_spanned!($item_name, $ty, $walk_fn);)*
    }
}

macro_rules! walk_visitor {
    ($walk_fn: ident, $visit_fn: ident $(, $param: ident : $(&$ref: lifetime)? $ty: ident $(<$lf: lifetime>)?),*) => {
        pub fn $walk_fn<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, $($param: $(&$ref)? $ty$(<$lf>)?),*) -> ControlFlow<V::BreakType> {
            visitor.$visit_fn($($param),*)
        }
    };

    ($($walk_fn: ident, $visit_fn: ident $(, $param: ident : $(&$ref: lifetime)? $ty: ident $(<$lf: lifetime>)?),*);*;) => {
        $(walk_visitor!($walk_fn, $visit_fn, $($param : $(&$ref)? $ty $(<$lf>)?),*);)*
    }
}

macro_rules! walk_optional {
    ($item_name: ident, $t: ident, $walk_fn: ident) => {
        pub fn $item_name<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, opt_item: Option<&'a $t<'t>>) -> ControlFlow<V::BreakType> {
            if let Some(item) = opt_item {
                $walk_fn(visitor, item)?;
            }

            ControlFlow::Continue(())
        }
    };

    ($($item_name: ident, $t: ident, $walk_fn: ident);*;) => {
        $(walk_optional!($item_name, $t, $walk_fn);)*
    }
}

macro_rules! empty_visitor {
    ($fn_name: ident, $($param: ident : $(&$ref: lifetime)? $ty: ident $(<$lf: lifetime>)?),*) => {
        fn $fn_name(&mut self, $($param: $(&$ref)? $ty$(<$lf>)?),*) -> ControlFlow<Self::BreakType> {
            ControlFlow::Continue(())
        }
    };

    ($($fn_name: ident $(,)? $($param: ident : $(&$ref: lifetime)? $ty: ident $(<$lf: lifetime>)?),*);*;) => {
        $(empty_visitor!($fn_name, $($param : $(&$ref)? $ty $(<$lf>)?),*);)*
    }
}

macro_rules! op_exp_visitor {
    ($fn_name: ident, $ty: ident, $($exps: ident),+) => {
        fn $fn_name(&mut self, _op: &'a $ty, $($exps: &'a Expression<'t>),+, _span: Span) -> ControlFlow<Self::BreakType> {
            $(walk_expression(self, $exps)?;)+
            ControlFlow::Continue(())
        }
    };

    ($($fn_name: ident, $ty: ident, $($exps: ident),+);*;) => {
        $(op_exp_visitor!($fn_name, $ty, $($exps),+);)*
    }
}

pub trait Visitor<'a, 't>: Sized where 'a: 't {
    type BreakType;

    fn visit_block(&mut self, block: &'a Block<'t>) -> ControlFlow<Self::BreakType> {
        walk_block(self, block)
    }

    fn visit_item(&mut self, i: &'a Item<'t>) -> ControlFlow<Self::BreakType> {
        walk_item(self, i)
    }

    fn visit_enum_item(&mut self, i: &'a EnumItem<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, i.identifier)?;
        walk_enum_members(self, &i.items)?;
        walk_return_type_option(self, i.derived_type.as_ref())?;

        ControlFlow::Continue(())
    }

    fn visit_enum_member(&mut self, m: &'a EnumMember<'t>) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, m.identifier)?;
        walk_expression_option(self, m.default_value.as_ref())?;

        ControlFlow::Continue(())
    }

    fn visit_struct_item(&mut self, s: &'a StructItem<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, s.identifier)?;
        walk_struct_member_definition(self, &s.members)?;
        walk_generics_option(self, s.generics.as_ref())?;
        walk_where_clause_option(self, s.where_clause.as_ref())?;

        ControlFlow::Continue(())
    }

    fn visit_field_struct(&mut self, s: &'a ThinVec<Field<'t>>) -> ControlFlow<Self::BreakType> {
        walk_fields(self, s)
    }

    fn visit_field(&mut self, f: &'a Field<'t>) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, f.identifier)?;
        walk_return_type(self, &f.return_type)?;

        ControlFlow::Continue(())
    }

    fn visit_tuple_struct(&mut self, rts: &'a ThinVec<ReturnType<'t>>) -> ControlFlow<Self::BreakType> {
        walk_return_types(self, rts)
    }

    fn visit_config_item(&mut self, c: &'a ConfigItem<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_config_members(self, &c.members)
    }

    fn visit_config_member(&mut self, m: &'a ConfigMember<'t>) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, m.identifier)?;
        walk_expression(self, &m.expression)?;

        ControlFlow::Continue(())
    }

    fn visit_const_item(&mut self, c: &'a ConstItem<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, c.identifier)?;
        walk_return_type(self, &c.return_type)?;
        walk_expression(self, &c.eq)?;

        ControlFlow::Continue(())
    }

    fn visit_func_item(&mut self, f: &'a FuncItem<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, f.identifier)?;
        walk_block_option(self, f.block.as_ref())?;
        walk_func_params(self, &f.params)?;
        walk_generics_option(self, f.generics.as_ref())?;
        walk_where_clause_option(self, f.where_clause.as_ref())?;

        ControlFlow::Continue(())
    }

    fn visit_func_param(&mut self, p: &'a FuncParam<'t>) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, p.identifier)?;
        walk_return_type(self, &p.return_type)?;

        ControlFlow::Continue(())
    }

    fn visit_sig_item(&mut self, s: &'a SigItem<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_return_type_option(self, s.sig_type.as_ref())?;
        walk_identifier(self, s.identifier)?;

        ControlFlow::Continue(())
    }

    fn visit_import_item(&mut self, i: &'a ImportItem<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_path(self, &i.path)?;
        walk_identifier_option(self, i.alias)?;

        ControlFlow::Continue(())
    }

    fn visit_feature_item(&mut self, f: &'a FeatureItem<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_path(self, &f.feature_ident)?;
        walk_path_option(self, f.implementation.as_ref())?;
        walk_generics_option(self, f.generics.as_ref())?;
        walk_where_clause_option(self, f.where_clause.as_ref())?;
        walk_items(self, &f.items)?;

        ControlFlow::Continue(())
    }

    fn visit_generic_param(&mut self, g: &'a GenericParam<'t>) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, g.identifier)?;
        walk_return_types_option(self, g.constraits.as_ref())?;

        ControlFlow::Continue(())
    }

    fn visit_generic_constrait(&mut self, c: &'a GenericConstrait<'t>) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, c.identifier)?;
        walk_return_types(self, &c.constraits)?;

        ControlFlow::Continue(())
    }

    fn visit_statement_item(&mut self, s: &'a Statement<'t>) -> ControlFlow<Self::BreakType> {
        walk_statement(self, s)
    }

    fn visit_visible_item(&mut self, _v: &'a VisibilityModifiers, item: &'a Item<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_item(self, item)
    }

    fn visit_attribute_collected_item(&mut self, attributes: &'a ThinVec<Path<'t>>, item: &'a Item<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_paths(self, attributes)?;
        walk_item(self, item)?;

        ControlFlow::Continue(())
    }

    fn visit_expression_statement(&mut self, expr: &'a Expression<'t>, _is_return: bool, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expression(self, expr)
    }

    fn visit_return_statement(&mut self, e: Option<&'a Expression<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expression_option(self, e)
    }

    fn visit_let_statement(&mut self, idents: &'a ThinVec<IdentifierOrUnderscore<'t>>, return_type: Option<&'a ReturnType<'t>>,
        exp: Option<&'a Expression<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
            walk_identifier_or_underscores(self, idents)?;
            walk_return_type_option(self, return_type)?;
            walk_expression_option(self, exp)?;

            ControlFlow::Continue(())
        }

    fn visit_for_statement(&mut self, s: &'a ForLoopStatement<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_identifier_or_underscores(self, &s.identifiers)?;
        walk_expression(self, &s.expression)?;
        walk_block(self, &s.block)?;

        ControlFlow::Continue(())
    }

    fn visit_while_statement(&mut self, s: &'a WhileLoopStatement<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expression(self, &s.expression)?;
        walk_block(self, &s.block)?;

        ControlFlow::Continue(())
    }

    fn visit_reference_expression(&mut self, left: &'a Expression<'t>, right: &'a Expression<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expression(self, left)?;
        walk_expression(self, right)?;

        ControlFlow::Continue(())
    }

    fn visit_call_expression(&mut self, left: &'a Expression<'t>, params: &'a ThinVec<Expression<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expression(self, left)?;
        walk_expressions(self, params)?;

        ControlFlow::Continue(())
    }

    fn visit_new_expression(&mut self, rt: &'a ReturnType<'t>, expr: &'a NewExprType<'t>, span: Span) -> ControlFlow<Self::BreakType> {
        walk_return_type(self, rt)?;
        walk_new_expr(self, expr, span)?;

        ControlFlow::Continue(())
    }

    fn visit_field_init_new_expression(&mut self, fields: &'a ThinVec<FieldInitialization<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_field_inits(self, fields)
    }

    fn visit_field_init(&mut self, f: &'a FieldInitialization<'t>) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, f.identifier)?;
        walk_expression(self, &f.expression)?;

        ControlFlow::Continue(())
    }

    fn visit_tuple_init_new_expression(&mut self, exprs: &'a ThinVec<Expression<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expressions(self, exprs)
    }

    fn visit_array_expression(&mut self, exprs: &'a ThinVec<Expression<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expressions(self, exprs)
    }

    fn visit_index_expression(&mut self, left: &'a Expression<'t>, params: &'a ThinVec<Expression<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expression(self, left)?;
        walk_expressions(self, params)?;

        ControlFlow::Continue(())
    }

    fn visit_tuple_expression(&mut self, exprs: &'a ThinVec<Expression<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expressions(self, exprs)
    }

    fn visit_match_expression(&mut self, matcher: &'a Expression<'t>, arms: &'a ThinVec<MatchArm<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expression(self, matcher)?;
        walk_match_arms(self, arms)?;

        ControlFlow::Continue(())
    }

    fn visit_match_arm(&mut self, arm: &'a MatchArm<'t>) -> ControlFlow<Self::BreakType> {
        walk_expression_option(self, arm.expression.as_ref())?;
        walk_expression_option(self, arm.if_clause.as_ref())?;
        walk_block(self, &arm.block)?;

        ControlFlow::Continue(())
    }

    fn visit_short_circuit_expression(&mut self, expr: &'a Expression<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expression(self, expr)
    }

    fn visit_if_expression(&mut self, if_arm: &'a IfArm<'t>, else_if_arms: &'a ThinVec<IfArm<'t>>,
        else_block: Option<&'a Block<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
            walk_if_arm(self, if_arm)?;
            walk_if_arms(self, else_if_arms)?;
            walk_block_option(self, else_block)?;

            ControlFlow::Continue(())
        }


    fn visit_if_arm(&mut self, arm: &'a IfArm<'t>) -> ControlFlow<Self::BreakType> {
        walk_expression(self, &arm.clause)?;
        walk_block(self, &arm.block)?;

        ControlFlow::Continue(())
    }

    fn visit_closure_expression(&mut self, params: &'a ThinVec<ClosureParam<'t>>, block: &'a Block<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_closure_params(self, params)?;
        walk_block(self, block)?;

        ControlFlow::Continue(())
    }

    fn visit_closure_param(&mut self, param: &'a ClosureParam<'t>) -> ControlFlow<Self::BreakType> {
        walk_identifier(self, param.identifier)?;
        walk_return_type_option(self, param.return_type.as_ref())?;

        ControlFlow::Continue(())
    }

    fn visit_path_expression(&mut self, left: &'a Expression<'t>, right: &'a Expression<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_expression(self, left)?;
        walk_expression(self, right)?;

        ControlFlow::Continue(())
    }

    fn visit_return_type(&mut self, rt: &'a ReturnType<'t>) -> ControlFlow<Self::BreakType> {
        walk_return_type(self, rt)
    }

    fn visit_path_return_type(&mut self, path: &'a Path<'t>) -> ControlFlow<Self::BreakType> {
        walk_path(self, path)
    }

    fn visit_generic_return_type(&mut self, left: &'a ReturnType<'t>, params: &'a ThinVec<ReturnType<'t>>) -> ControlFlow<Self::BreakType> {
        walk_return_type(self, left)?;
        walk_return_types(self, params)?;

        ControlFlow::Continue(())
    }

    fn visit_tuple_return_type(&mut self, rts: &'a ThinVec<ReturnType<'t>>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_return_types(self, rts)
    }

    fn visit_array_return_type(&mut self, left: &'a ReturnType<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_return_type(self, left)
    }

    fn visit_nullable_return_type(&mut self, left: &'a ReturnType<'t>, _span: Span) -> ControlFlow<Self::BreakType> {
        walk_return_type(self, left)
    }

    empty_visitor! {
        visit_zero_struct;
        visit_empty_statement, _span: Span;
        visit_break_statement, _span: Span;
        visit_continue_statement, _span: Span;
        visit_path, _path : &'a Path<'t>;
        visit_zero_init_new_expression, _span: Span;
        visit_access_expression, _kind: &'a AccessExpression<'t>, _span: Span;
        visit_literal_expression, _kind: &'a LiteralExpression<'t>, _span: Span;
        visit_identifier, _i: Identifier<'t>;
        visit_identifier_or_underscore, _i: &'a IdentifierOrUnderscore<'t>;
    }

    op_exp_visitor! {
        visit_unary_expression, UnaryOperator, right;
        visit_assignment_expression, AssignmentOperator, left, right;
        visit_conditional_expression, ConditionalOperator, left, right;
        visit_logical_expression, LogicalOperator, left, right;
        visit_equality_expression, EqualityOperator, left, right;
        visit_comparision_expression, ComparisionOperator, left, right;
        visit_binary_expression, BinaryOperator, left, right;
        visit_post_op_expression, PostOperator, left;
    }
}

walk_optional! {
    walk_return_type_option, ReturnType, walk_return_type;
    walk_block_option, Block, walk_block;
    walk_expression_option, Expression, walk_expression;
    walk_path_option, Path, walk_path;
    walk_generics_option, Generics, walk_generics;
    walk_where_clause_option, WhereClause, walk_where_clause;
    walk_return_types_option, ReturnTypes, walk_return_types;
}

walk_list! {
    walk_return_types, ReturnType, walk_return_type;
    walk_expressions, Expression, walk_expression;
    walk_identifier_or_underscores, IdentifierOrUnderscore, walk_identifier_or_underscore;
    walk_closure_params, ClosureParam, walk_closure_param;
    walk_paths, Path, walk_path;
    walk_items, Item, walk_item;
    walk_func_params, FuncParam, walk_func_param;
    walk_fields, Field, walk_field;
    walk_enum_members, EnumMember, walk_enum_member;
    walk_if_arms, IfArm, walk_if_arm;
    walk_match_arms, MatchArm, walk_match_arm;
    walk_config_members, ConfigMember, walk_config_member;
    walk_field_inits, FieldInitialization, walk_field_init;
}

walk_list_spanned! {
    walk_block, Statement, walk_statement;
    walk_generics, GenericParam, walk_generic_param;
    walk_where_clause, GenericConstrait, walk_generic_constrait;
}

walk_visitor! {
    walk_func_param, visit_func_param, param: &'a FuncParam<'t>;
    walk_generic_param, visit_generic_param, generic_param: &'a GenericParam<'t>;
    walk_generic_constrait, visit_generic_constrait, constrait: &'a GenericConstrait<'t>;
    walk_path, visit_path, constrait: &'a Path<'t>;
    walk_identifier_or_underscore, visit_identifier_or_underscore, i: &'a IdentifierOrUnderscore<'t>;
    walk_closure_param, visit_closure_param, param: &'a ClosureParam<'t>;
    walk_identifier, visit_identifier, identifier: &'t str;
    walk_field, visit_field, field: &'a Field<'t>;
    walk_enum_member, visit_enum_member, member: &'a EnumMember<'t>;
    walk_if_arm, visit_if_arm, if_arm: &'a IfArm<'t>;
    walk_match_arm, visit_match_arm, match_arm: &'a MatchArm<'t>;
    walk_config_member, visit_config_member, config_member: &'a ConfigMember<'t>;
    walk_field_init, visit_field_init, field: &'a FieldInitialization<'t>;
}

pub fn walk_identifier_option<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, identifier_option: Option<&'t str>) -> ControlFlow<V::BreakType> {
    if let Some(identifier) = identifier_option {
        walk_identifier(visitor, identifier)?;
    }

    ControlFlow::Continue(())
}

pub fn walk_item<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, item: &'a Item<'t>) -> ControlFlow<V::BreakType> {
    let span = item.span;
    match item.kind {
        ItemKind::Enum(ref item) => visitor.visit_enum_item(item, span),
        ItemKind::Struct(ref item) => visitor.visit_struct_item(item, span),
        ItemKind::Config(ref item) => visitor.visit_config_item(item, span),
        ItemKind::Const(ref item) => visitor.visit_const_item(item, span),
        ItemKind::Func(ref item) => visitor.visit_func_item(item, span),
        ItemKind::Sig(ref item) => visitor.visit_sig_item(item, span),
        ItemKind::Import(ref item) => visitor.visit_import_item(item, span),
        ItemKind::Feature(ref item) => visitor.visit_feature_item(item, span),
        ItemKind::Statement(ref item) => visitor.visit_statement_item(item),
        ItemKind::Visible(ref vis, ref item) => visitor.visit_visible_item(vis, item, span),
        ItemKind::AttributeCollectedItem(ref attribs, ref item) => visitor.visit_attribute_collected_item(attribs, item, span),
    }
}

pub fn walk_statement<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, stmt: &'a Statement<'t>) -> ControlFlow<V::BreakType> {
    match stmt.kind {
        StatementKind::Empty => visitor.visit_empty_statement(stmt.span),
        StatementKind::Expression(ref expr, is_return) => visitor.visit_expression_statement(expr, is_return, stmt.span),
        StatementKind::Return(ref expr) => visitor.visit_return_statement(expr.as_ref(), stmt.span),
        StatementKind::Let(ref idents, ref return_types, ref expr) => visitor.visit_let_statement(idents, return_types.as_ref(), expr.as_ref(), stmt.span),
        StatementKind::ForLoop(ref for_stmt) => visitor.visit_for_statement(for_stmt, stmt.span),
        StatementKind::WhileLoop(ref while_stmt) => visitor.visit_while_statement(while_stmt, stmt.span),
        StatementKind::Break => visitor.visit_break_statement(stmt.span),
        StatementKind::Continue => visitor.visit_continue_statement(stmt.span),
    }
}

pub fn walk_expression<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, expr: &'a Expression<'t>) -> ControlFlow<V::BreakType> {
    match expr.kind {
        ExpressionKind::Literal(ref lit) => visitor.visit_literal_expression(lit, expr.span),
        ExpressionKind::Access(ref acs) => visitor.visit_access_expression(acs, expr.span),
        ExpressionKind::Reference(ref left, ref right) => visitor.visit_reference_expression(left, right, expr.span),
        ExpressionKind::Call(ref left, ref params) => visitor.visit_call_expression(left, params, expr.span),
        ExpressionKind::New(ref return_type, ref new_expr) => visitor.visit_new_expression(return_type, new_expr, expr.span),
        ExpressionKind::Array(ref exprs) => visitor.visit_array_expression(exprs, expr.span),
        ExpressionKind::Unary(ref op, ref right) => visitor.visit_unary_expression(op, right, expr.span),
        ExpressionKind::Assignment(ref left, ref op, ref right) => visitor.visit_assignment_expression(op, left, right, expr.span),
        ExpressionKind::Conditional(ref left, ref op, ref right) => visitor.visit_conditional_expression(op, left, right, expr.span),
        ExpressionKind::Logical(ref left, ref op, ref right) => visitor.visit_logical_expression(op, left, right, expr.span),
        ExpressionKind::Equality(ref left, ref op, ref right) => visitor.visit_equality_expression(op, left, right, expr.span),
        ExpressionKind::Comparision(ref left, ref op, ref right) => visitor.visit_comparision_expression(op, left, right, expr.span),
        ExpressionKind::Binary(ref left, ref op, ref right) => visitor.visit_binary_expression(op, left, right, expr.span),
        ExpressionKind::PostOp(ref left, ref op) => visitor.visit_post_op_expression(op, left, expr.span),
        ExpressionKind::Index(ref left, ref exprs) => visitor.visit_index_expression(left, exprs, expr.span),
        ExpressionKind::Tuple(ref exprs) => visitor.visit_tuple_expression(exprs, expr.span),
        ExpressionKind::Match(ref matcher, ref arms) => visitor.visit_match_expression(matcher, arms, expr.span),
        ExpressionKind::ShortCircuit(ref expr) => visitor.visit_short_circuit_expression(expr, expr.span),
        ExpressionKind::If(ref if_arm, ref else_if_arms, ref else_block) => visitor.visit_if_expression(if_arm, else_if_arms, else_block.as_ref(), expr.span),
        ExpressionKind::Closure(ref params, ref block) => visitor.visit_closure_expression(params, block, expr.span),
        ExpressionKind::Path(ref left, ref right) => visitor.visit_path_expression(left, right, expr.span),
    }
}

pub fn walk_return_type<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, rt: &'a ReturnType<'t>) -> ControlFlow<V::BreakType> {
    match rt.kind {
        ReturnTypeKind::Array(ref ty) => visitor.visit_array_return_type(ty, rt.span),
        ReturnTypeKind::Nullable(ref ty) => visitor.visit_nullable_return_type(ty, rt.span),
        ReturnTypeKind::Tuple(ref tys) => visitor.visit_tuple_return_type(tys, rt.span),
        ReturnTypeKind::Generic(ref left, ref params) => visitor.visit_generic_return_type(left, params),
        ReturnTypeKind::Path(ref path) => visitor.visit_path_return_type(path),
    }
}

pub fn walk_struct_member_definition<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, def: &'a StructMemberDefinition<'t>) -> ControlFlow<V::BreakType> {
    match def {
        StructMemberDefinition::Field(f) => visitor.visit_field_struct(f),
        StructMemberDefinition::Tuple(t) => visitor.visit_tuple_struct(t),
        StructMemberDefinition::Zero => visitor.visit_zero_struct(),
    }
}

pub fn walk_new_expr<'a, 't, V: Visitor<'a, 't>>(visitor: &mut V, expr: &'a NewExprType<'t>, span: Span) -> ControlFlow<V::BreakType> {
    match expr {
        NewExprType::Field(f) => visitor.visit_field_init_new_expression(f, span),
        NewExprType::Tuple(t) => visitor.visit_tuple_init_new_expression(t, span),
        NewExprType::Zero => visitor.visit_zero_init_new_expression(span),
    }
}
