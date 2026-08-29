use psi_arena::{Arena, HandleSpan};
use psi_symbol_resolved_trees::data::{TypeParameter, TypeParameterKind};
use psi_symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_symbol_resolved_trees::proposition::PropositionBody;
use psi_symbol_resolved_trees::types::TypeReference;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::expressions::assign_struct_literal_symbols;
use super::lookup::{child_symbol_by_kinds, top_level_symbol_by_kinds};
use super::targets::{
    resolve_free_machine_entry_state_symbol, resolve_proposition_binder_argument_symbol,
};

pub(super) fn assign_proposition_expression_symbols(
    program: &mut psi_symbol_resolved_trees::SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let proposition_binders = &program.tables.declarations.proposition_binders;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    let expressions = &mut program.tables.bodies.expressions;
    let propositions = &program.roots.propositions;
    for proposition in propositions {
        let PropositionBody::Transparent { proposition: body } = proposition.body else {
            continue;
        };
        let local_type_parameters = proposition_binders
            .span_or_empty(proposition.binders)
            .iter()
            .map(|binder| TypeParameter {
                symbol: binder.symbol,
                name: binder.name.clone(),
                kind: TypeParameterKind::Type,
                bounds: binder.bounds,
            })
            .collect::<Vec<_>>();
        assign_expression_symbols(
            expressions,
            child_type_references,
            symbols,
            proposition.symbol,
            &local_type_parameters,
            body,
        );
    }
}

fn assign_expression_span_symbols(
    expressions: &mut ExpressionTable,
    child_type_references: &mut Arena<TypeReference>,
    symbols: &SymbolTable,
    proposition_symbol: SymbolHandle,
    local_type_parameters: &[TypeParameter],
    span: HandleSpan<ExpressionHandle>,
) {
    let children = expressions.expression_handles(span).to_vec();
    for child in children {
        assign_expression_symbols(
            expressions,
            child_type_references,
            symbols,
            proposition_symbol,
            local_type_parameters,
            child,
        );
    }
}

fn assign_expression_symbols(
    expressions: &mut ExpressionTable,
    child_type_references: &mut Arena<TypeReference>,
    symbols: &SymbolTable,
    proposition_symbol: SymbolHandle,
    local_type_parameters: &[TypeParameter],
    expression: ExpressionHandle,
) {
    macro_rules! recurse {
        ($expression:expr) => {
            assign_expression_symbols(
                expressions,
                child_type_references,
                symbols,
                proposition_symbol,
                local_type_parameters,
                $expression,
            )
        };
    }
    match expressions.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => assign_expression_span_symbols(
            expressions,
            child_type_references,
            symbols,
            proposition_symbol,
            local_type_parameters,
            values,
        ),
        ExpressionNode::Atomic(atomic) => {
            recurse!(atomic.value);
            if atomic.result.is_valid() {
                recurse!(atomic.result);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse!(binary.left);
            recurse!(binary.right);
        }
        ExpressionNode::Cast(cast) => {
            recurse!(cast.value);
            let mut target_type = child_type_references.get(cast.target_type).clone();
            crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type(
                symbols,
                child_type_references,
                local_type_parameters,
                SymbolHandle::invalid(),
                &mut target_type,
            );
            *child_type_references.get_mut(cast.target_type) = target_type;
            for offset in 0..cast.semantic_domain_arguments.count() {
                let start = cast.semantic_domain_arguments.start();
                let handle =
                    psi_arena::Handle::from_parts(start.arena_index() + offset, start.generation());
                let mut argument = child_type_references.get(handle).clone();
                crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type(
                    symbols,
                    child_type_references,
                    local_type_parameters,
                    SymbolHandle::invalid(),
                    &mut argument,
                );
                *child_type_references.get_mut(handle) = argument;
            }
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                recurse!(call.receiver);
            }
            assign_expression_span_symbols(
                expressions,
                child_type_references,
                symbols,
                proposition_symbol,
                local_type_parameters,
                call.arguments,
            );
            let target_symbol = if call.receiver.is_valid() {
                SymbolHandle::invalid()
            } else {
                let proposition = top_level_symbol_by_kinds(
                    symbols,
                    &[SymbolKind::Proposition],
                    call.target.as_str(),
                );
                if proposition.is_valid() {
                    proposition
                } else {
                    let builtin = top_level_symbol_by_kinds(
                        symbols,
                        &[SymbolKind::BuiltinFunction],
                        call.target.as_str(),
                    );
                    if builtin.is_valid() {
                        builtin
                    } else {
                        resolve_free_machine_entry_state_symbol(symbols, call.target.as_str())
                    }
                }
            };
            if let ExpressionNode::Call(call) = expressions.expression_mut(expression) {
                call.target_symbol = target_symbol;
                for argument in &mut call.machine_arguments {
                    argument.symbol = resolve_proposition_binder_argument_symbol(
                        symbols,
                        proposition_symbol,
                        &argument.path,
                    );
                }
            }
        }
        ExpressionNode::Indexed(indexed) => {
            recurse!(indexed.collection);
            recurse!(indexed.index);
        }
        ExpressionNode::Membership(membership) => recurse!(membership.value),
        ExpressionNode::Member(member) => recurse!(member.receiver),
        ExpressionNode::Borrow(inner) => recurse!(inner.target),
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let symbol = if let [name] = members {
                child_symbol_by_kinds(
                    symbols,
                    proposition_symbol,
                    &[
                        SymbolKind::Parameter,
                        SymbolKind::TypeParameter,
                        SymbolKind::PropositionMachineParameter,
                    ],
                    name.as_str(),
                )
            } else {
                SymbolHandle::invalid()
            };
            if let ExpressionNode::Name(path) = expressions.expression_mut(expression) {
                path.head_symbol = symbol;
                path.symbol = symbol;
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                recurse!(range.start);
            }
            if range.end.is_valid() {
                recurse!(range.end);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            let fields = expressions.struct_fields(struct_literal.fields).to_vec();
            for field in fields {
                recurse!(field.value);
            }
            assign_struct_literal_symbols(symbols, expressions, expression);
        }
        ExpressionNode::Unary(unary) => recurse!(unary.operand),
        ExpressionNode::ZeroValue(type_reference) => {
            let mut target_type = child_type_references.get(type_reference).clone();
            crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type(
                symbols,
                child_type_references,
                local_type_parameters,
                SymbolHandle::invalid(),
                &mut target_type,
            );
            *child_type_references.get_mut(type_reference) = target_type;
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}
