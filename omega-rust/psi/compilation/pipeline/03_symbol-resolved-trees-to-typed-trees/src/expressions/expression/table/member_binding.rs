//! Bind missing members through the receiver's exact declared type or
//! constructor owner. Static and runtime-capable value binders retain their
//! declared carriers even before a concrete application exists. Their payload
//! projections need the same exact field identity as ordinary parameters;
//! specialization must not inherit an unresolved member from its template.
//! Substituted constant values are ordinary constructors and use that identity
//! before checked scalar replay.

use super::lowerer::ExpressionTableLowerer;
use crate::expressions::call_results::peel;
use crate::lowerer::{exact_field_symbol, exact_top_level_data_symbol};
use symbol_resolved_trees as resolved;
use symbols::{SymbolHandle, SymbolKind};

use resolved::data::{DataField, DataMember};
use resolved::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};
use resolved::types::TypeReference;

/// A projected place selects a method through the same exact declared type
/// used to bind its fields. Existing selections remain the caller's owner.
pub(crate) fn declared_receiver_method_target(
    program: &resolved::SymbolResolvedTrees,
    expressions: &resolved::expression::ExpressionTable,
    receiver: ExpressionHandle,
    target: &resolved::name::DiagnosticName,
) -> SymbolHandle {
    declared_type(program, expressions, receiver, 0)
        .map(|reference| {
            crate::expressions::call_results::declared_type_method_target(
                program, reference, target,
            )
        })
        .unwrap_or_default()
}

impl ExpressionTableLowerer<'_, '_, '_> {
    pub(super) fn declared_member(
        &self,
        member: &TableMemberExpression,
    ) -> (SymbolHandle, Option<resolved::name::DiagnosticName>) {
        // A nonzero selection already has an owner. Preserve it, including a
        // conflicting/stale selection, for the existing validation contract.
        if member.member_symbol.is_valid() {
            return (member.member_symbol, member.case_variant.clone());
        }
        self.program
            .and_then(|program| declared_receiver_field(program, self.source, member, 0))
            // Substituted constructors have no runtime place to bind through.
            // Recover the exact declared field, preserving a payload's case
            // selector while making a common-field read ordinary. A same-named
            // field in another case is never a candidate for this projection.
            .map(|field| {
                let payload = self.program.is_some_and(|program| {
                    let owner = program.symbols.get(field.symbol).parent;
                    program.symbols.get(owner).kind == SymbolKind::Variant
                });
                (
                    field.symbol,
                    if payload {
                        member.case_variant.clone()
                    } else {
                        None
                    },
                )
            })
            .unwrap_or_else(|| (member.member_symbol, member.case_variant.clone()))
    }
}

fn declared_receiver_field<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    expressions: &resolved::expression::ExpressionTable,
    member: &TableMemberExpression,
    depth: usize,
) -> Option<&'program DataField> {
    if depth >= 128 || !expressions.expression_is_valid(member.receiver) {
        return None;
    }
    if let ExpressionNode::StructLiteral(literal) = expressions.expression(member.receiver) {
        return declared_data_field(program, literal.type_symbol, member);
    }
    let receiver = declared_type(program, expressions, member.receiver, depth + 1)?;
    declared_field(program, receiver, member)
}

fn declared_type<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    expressions: &resolved::expression::ExpressionTable,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<&'program TypeReference> {
    if !expression.is_valid() || depth >= 128 {
        return None;
    }
    match expressions.expression(expression) {
        ExpressionNode::Name(path) => declared_symbol_type(program, path.symbol),
        ExpressionNode::Call(call) => {
            crate::expressions::call_results::declared_call_return(program, call.target_symbol)
        }
        ExpressionNode::ArrayLiteral(_) => {
            // Substitution copied the initializer, not its declaration's name.
            // The retained selection still supplies the complete element type,
            // including empty dimensions and closed generic arguments. Do not
            // infer that type from whichever element happens to be selected.
            let mut declared = None;
            for occurrence in expressions.authored_selection_occurrences(expression) {
                let selection = program.authored_declaration_selections().get(occurrence)?;
                if selection.source_span() != expressions.source_span(expression) {
                    continue;
                }
                let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target() else {
                    continue;
                };
                let Some(constant) = program.const_declaration(selected.selected_symbol()) else {
                    continue;
                };
                if !matches!(constant.declared_type, TypeReference::FixedArray(_))
                    || declared.is_some_and(|previous| previous != &constant.declared_type)
                {
                    return None;
                }
                declared = Some(&constant.declared_type);
            }
            declared
        }
        ExpressionNode::Borrow(borrow) => {
            declared_type(program, expressions, borrow.target, depth + 1)
        }
        ExpressionNode::Indexed(indexed) => {
            if !indexed.index.is_valid()
                || matches!(
                    expressions.expression(indexed.index),
                    ExpressionNode::Range(_)
                )
            {
                return None;
            }
            let collection = declared_type(program, expressions, indexed.collection, depth + 1)?;
            // As in computed receiver typing, each [] consumes exactly one
            // array/slice layer; wrapper peeling never consumes an element.
            let element = match peel(program, collection) {
                TypeReference::FixedArray(array) => array.element_type,
                TypeReference::Slice(slice) => slice.element_type,
                _ => return None,
            };
            element
                .is_valid()
                .then(|| program.child_type_reference(element))
        }
        ExpressionNode::Member(member) => {
            Some(&declared_receiver_field(program, expressions, member, depth + 1)?.type_reference)
        }
        _ => None,
    }
}

fn declared_field<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    receiver: &TypeReference,
    member: &TableMemberExpression,
) -> Option<&'program DataField> {
    let definition =
        super::lowerer::resolved_data_definition_for_type(program, peel(program, receiver))?;
    declared_data_field(program, definition.symbol, member)
}

fn declared_data_field<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    symbol: SymbolHandle,
    member: &TableMemberExpression,
) -> Option<&'program DataField> {
    let mut definitions = program
        .data_definitions
        .iter()
        .filter(|definition| definition.symbol == symbol);
    let definition = definitions.next()?;
    if definitions.next().is_some() || !exact_top_level_data_symbol(program, definition) {
        return None;
    }
    if let Some(selected) = &member.case_variant {
        let mut variants = program
            .data_members(definition.members)
            .iter()
            .filter_map(|node| {
                let DataMember::Variant(variant) = node else {
                    return None;
                };
                (variant.name.as_str() == selected.as_str()).then_some(variant)
            });
        let variant = variants.next()?;
        if variants.next().is_some()
            || !variant.symbol.is_valid()
            || program.symbols.get(variant.symbol).parent != definition.symbol
        {
            return None;
        }
        let mut payload = program
            .data_payload_fields(variant.payload)
            .iter()
            .filter(|field| {
                field.name.as_str() == member.member.as_str()
                    && exact_field_symbol(program, variant.symbol, field)
            });
        if let Some(field) = payload.next() {
            if payload.next().is_some()
                || (member.member_symbol.is_valid() && member.member_symbol != field.symbol)
            {
                return None;
            }
            return Some(field);
        }
    }
    let mut fields = program
        .data_members(definition.members)
        .iter()
        .filter_map(|node| {
            let DataMember::Field(field) = node else {
                return None;
            };
            (field.name.as_str() == member.member.as_str()
                && exact_field_symbol(program, definition.symbol, field))
            .then_some(field)
        });
    let field = fields.next()?;
    if fields.next().is_some()
        || (member.member_symbol.is_valid() && member.member_symbol != field.symbol)
    {
        return None;
    }
    Some(field)
}

pub(super) fn declared_symbol_type(
    program: &resolved::SymbolResolvedTrees,
    symbol: SymbolHandle,
) -> Option<&TypeReference> {
    if !symbol.is_valid() {
        return None;
    }
    let declaration = program.symbols.get(symbol);
    match declaration.kind {
        SymbolKind::TypeParameter => program
            .tables
            .declarations
            .data_type_parameters
            .iter()
            .find_map(|(_, parameter)| {
                if parameter.symbol != symbol {
                    return None;
                }
                // Staging controls when the value is supplied, not the type
                // of its fields. Resolve through the authored carrier without
                // guessing a value or treating a type binder as a value.
                match &parameter.kind {
                    resolved::data::TypeParameterKind::Const { type_reference }
                    | resolved::data::TypeParameterKind::Value { type_reference } => {
                        Some(type_reference)
                    }
                    _ => None,
                }
            }),
        SymbolKind::Parameter
            if program.symbols.get(declaration.parent).kind == SymbolKind::Domain =>
        {
            program.domain_definitions.iter().find_map(|domain| {
                (domain.symbol == declaration.parent && domain.subject_symbol == symbol)
                    .then_some(&domain.target_type)
            })
        }
        SymbolKind::Parameter
            if program.symbols.get(declaration.parent).kind == SymbolKind::Measure =>
        {
            let measure = program
                .measures
                .iter()
                .find(|measure| measure.symbol == declaration.parent)?;
            let parameter = measure.parameter.as_ref()?;
            (parameter.symbol == symbol).then_some(&parameter.type_reference)
        }
        // Value parameters also belong to abstract signatures and propositions,
        // not only executable states. Their retained symbol is the declaration
        // identity; requiring a machine state here loses otherwise resolved
        // member receivers in nested contracts.
        SymbolKind::Parameter => program
            .tables
            .declarations
            .state_parameters
            .iter()
            .find_map(|(_, parameter)| {
                (parameter.symbol == symbol).then_some(&parameter.type_reference)
            }),
        SymbolKind::Local => {
            let state = program
                .tables
                .declarations
                .machine_states
                .iter()
                .find_map(|(_, state)| (state.symbol == declaration.parent).then_some(state))?;
            program
                .state_statements(state.statements)
                .iter()
                .find_map(|statement| {
                    let resolved::statement::Statement::LocalData(local) = statement else {
                        return None;
                    };
                    (local.symbol == symbol).then_some(&local.type_reference)
                })
        }
        SymbolKind::Field => {
            for definition in &program.data_definitions {
                if let Some(reference) = program.data_members(definition.members).iter().find_map(
                    |member| match member {
                        DataMember::Field(field)
                            if field.symbol == symbol
                                && exact_field_symbol(program, definition.symbol, field) =>
                        {
                            Some(&field.type_reference)
                        }
                        _ => None,
                    },
                ) {
                    return Some(reference);
                }
            }
            None
        }
        _ => None,
    }
}
