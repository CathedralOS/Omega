use arena::{Arena, HandleSpan};
use symbol_resolved_trees::data::{TypeParameter, TypeParameterKind};
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use symbol_resolved_trees::proposition::PropositionBody;
use symbol_resolved_trees::types::TypeReference;
use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolTable};

use super::expressions::assign_struct_literal_symbols;
use super::lookup::child_symbol_by_kinds;
use super::scoped_paths::{
    resolve_state_scoped_table_path, resolve_state_scoped_table_path_member_symbols,
};
use super::targets::{assign_static_argument_symbols, resolve_free_call_target_symbol};

pub(super) fn assign_proposition_expression_symbols(
    program: &mut symbol_resolved_trees::SymbolResolvedTrees,
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
    owner_symbol: SymbolHandle,
    local_type_parameters: &[TypeParameter],
    span: HandleSpan<ExpressionHandle>,
) {
    let children = expressions.expression_handles(span).to_vec();
    for child in children {
        assign_expression_symbols(
            expressions,
            child_type_references,
            symbols,
            owner_symbol,
            local_type_parameters,
            child,
        );
    }
}

/// Resolve the authored names inside one proof-surface expression.
/// `owner_symbol` is the declaration whose binder/parameter children are in
/// scope; callable declarations use the shared source target resolver.
/// Exact lexical binders precede authored declarations, which precede the
/// outermost compiler builtins. Ambiguous declarations stay unresolved.
/// Shared by proposition formulas and mathematical `let`/`boundary let`
/// terms (PROOF-CONTRACT-MIGRATION).
pub(super) fn assign_expression_symbols(
    expressions: &mut ExpressionTable,
    child_type_references: &mut Arena<TypeReference>,
    symbols: &SymbolTable,
    owner_symbol: SymbolHandle,
    local_type_parameters: &[TypeParameter],
    expression: ExpressionHandle,
) {
    macro_rules! recurse {
        ($expression:expr) => {
            assign_expression_symbols(
                expressions,
                child_type_references,
                symbols,
                owner_symbol,
                local_type_parameters,
                $expression,
            )
        };
    }
    match expressions.expression(expression).clone() {
        ExpressionNode::Match(dispatch) => {
            recurse!(dispatch.subject);
            for arm in expressions.match_arms(dispatch.arms).to_vec() {
                for pattern in expressions
                    .match_pattern_dependencies(arm.pattern)
                    .collect::<Vec<_>>()
                {
                    recurse!(pattern);
                }
                recurse!(arm.value);
            }
        }
        ExpressionNode::SavedSubject(_) => {}
        ExpressionNode::ArrayLiteral(values) => assign_expression_span_symbols(
            expressions,
            child_type_references,
            symbols,
            owner_symbol,
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
            crate::symbols::type_references::reassign_type_reference_symbols(
                symbols,
                child_type_references,
                local_type_parameters,
                cast.target_type,
            );
            for handle in cast.semantic_domain_arguments.handles() {
                crate::symbols::type_references::reassign_type_reference_symbols(
                    symbols,
                    child_type_references,
                    local_type_parameters,
                    handle,
                );
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
                owner_symbol,
                local_type_parameters,
                call.arguments,
            );
            let target_symbol = if call.receiver.is_valid() {
                // Qualified `receiver::target` calls are bound by
                // `normalize_static_module_calls` once every declaration
                // exists — including the fixed `core::*` vocabulary, which
                // carries no `Module` symbol the ordinary namespace arm
                // could find.
                SymbolHandle::invalid()
            } else {
                // Mathematical function parameters retain their lexical
                // binder identity. Their Pi formation, rather than spelling,
                // decides whether application is valid.
                let parameter = child_symbol_by_kinds(
                    symbols,
                    owner_symbol,
                    &[
                        SymbolKind::Parameter,
                        SymbolKind::PropositionParameter,
                        SymbolKind::MachineParameter,
                    ],
                    call.target.as_str(),
                );
                if parameter.is_valid() {
                    parameter
                } else {
                    resolve_free_call_target_symbol(symbols, &call.target)
                }
            };
            if let ExpressionNode::Call(call) = expressions.expression_mut(expression) {
                call.target_symbol = target_symbol;
                for argument in &mut call.machine_arguments {
                    assign_static_argument_symbols(symbols, owner_symbol, &[], argument, true);
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
            let (head_symbol, symbol) = if let [name] = members {
                let symbol = child_symbol_by_kinds(
                    symbols,
                    owner_symbol,
                    &[
                        SymbolKind::Parameter,
                        SymbolKind::TypeParameter,
                        SymbolKind::PropositionMachineParameter,
                    ],
                    name.as_str(),
                );
                (symbol, symbol)
            } else if let Some(symbol) =
                symbols::FixedCoreDeclaration::from_name(&members.joined_path())
                    .and_then(|declaration| symbols.fixed_core_declaration_symbol(declaration))
            {
                // The fixed core namespace has no runtime declaration head.
                (SymbolHandle::invalid(), symbol)
            } else {
                // Proof terms share ordinary source-visible namespace/member
                // resolution. Retain exact installed member handles rather
                // than discarding qualified values and recovering by spelling
                // in a later carrier or proof consumer.
                resolve_state_scoped_table_path(
                    symbols,
                    owner_symbol,
                    SymbolHandle::invalid(),
                    expressions,
                    &path,
                    None,
                )
            };
            // A bare declaration binder was selected in this exact lexical
            // telescope above. Its complete path is that one symbol; asking
            // the executable-state lookup again would drop abstract binders
            // because entry parameters cannot leak into named states.
            let member_symbols = if members.len() == 1 && head_symbol.is_valid() {
                vec![head_symbol]
            } else if head_symbol.is_valid() {
                resolve_state_scoped_table_path_member_symbols(
                    symbols,
                    owner_symbol,
                    SymbolHandle::invalid(),
                    expressions,
                    &path,
                )
            } else {
                Vec::new()
            };
            for (offset, member_symbol) in (0u32..).zip(member_symbols) {
                expressions.set_name_path_member_symbol_at_offset(
                    path.member_symbols,
                    offset,
                    member_symbol,
                );
            }
            if let ExpressionNode::Name(path) = expressions.expression_mut(expression) {
                path.head_symbol = head_symbol;
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
            crate::symbols::type_references::reassign_type_reference_symbols(
                symbols,
                child_type_references,
                local_type_parameters,
                type_reference,
            );
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}
