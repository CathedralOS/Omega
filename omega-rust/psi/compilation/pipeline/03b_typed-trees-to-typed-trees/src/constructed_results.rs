//! Retain complete constructed value types before checking freezes them.
//!
//! A projection can know its element or member without knowing its complete
//! value type. Ownership needs the latter: an array's authored extent and a
//! constructor's applied declaration determine each child's claim path. Derive
//! compound values from completed children after final call binding, never from a
//! same-shaped declaration or the consumer's formal type. Child-before-parent
//! preparation gives nested constructors the same ordinary result-type route.
//! No ownership, membership or loan permission is established by this derivation.
//! Reference formation also needs a value type before static call inference:
//! its referent is a declared place, but its value retains authored access.
//! Reborrowing lends the existing referent, not the reference carrier itself.

use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

fn occurrences(
    program: &TypedTrees,
    selected: fn(&ExpressionNode) -> bool,
) -> Vec<(
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    ExpressionHandle,
)> {
    let mut occurrences = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut expressions = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                validation::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
            }
            let mut visited = Vec::new();
            for expression in expressions {
                // A child may be shared by several parents. Actual postorder,
                // not reversed preorder, ensures it is derived first.
                validation::expression_traversal::visit_post_order(
                    program,
                    expression,
                    &mut visited,
                    |expression, node| {
                        if selected(node) {
                            occurrences.push((machine.symbol, state.symbol, expression));
                        }
                    },
                );
            }
        }
    }
    occurrences
}

/// Static inference needs the argument value, not the place stripped of its
/// borrow. Derive each formation in its own owner after substitution; this
/// records type vocabulary only. Loan admission still checks storage, access,
/// lifetime correspondence and parent lineage independently.
pub(crate) fn retain_borrow_result_types(program: &mut TypedTrees) {
    for (machine_symbol, state_symbol, expression) in
        occurrences(program, |node| matches!(node, ExpressionNode::Borrow(_)))
    {
        let derived = (|| {
            let machine = program.machine_by_symbol(machine_symbol)?;
            let state = program.state_by_symbol(state_symbol)?;
            let borrow = program.expression_table.borrow_expression(expression)?;
            let target =
                validation::declared_place_type_raw(program, machine, Some(state), borrow.target)?;
            let carrier = program
                .type_reference_table
                .unconstrained_type_reference(target);
            let (referee, lifetime) = match program.type_reference_table.type_reference(carrier) {
                TypeReferenceNode::Reference {
                    referee, lifetime, ..
                } => (*referee, lifetime.clone()),
                _ => (target, None),
            };
            Some(TypeReferenceNode::Reference {
                referee,
                access: borrow.access,
                lifetime,
            })
        })();
        let reference = derived
            .map(|node| {
                // Preparation repeats as calls specialize. Reuse exact vocabulary
                // rather than adding another shell on every fixed-point round.
                let retained = program
                    .expression_table
                    .constructed_value_type_reference(expression);
                let existing = if retained.is_valid()
                    && program.type_reference_table.type_reference(retained) == &node
                {
                    Some(retained)
                } else {
                    program.type_reference_table.type_reference_rows().find_map(
                        |(reference, candidate)| (candidate == &node).then_some(reference),
                    )
                };
                existing.unwrap_or_else(|| program.type_reference_table.insert(node))
            })
            .unwrap_or_else(TypeReferenceHandle::invalid);
        program
            .expression_table
            .set_constructed_value_type_reference(expression, reference);
    }
}

pub(crate) fn retain_constructed_result_types(program: &mut TypedTrees) {
    retain_borrow_result_types(program);
    for (machine_symbol, state_symbol, expression) in occurrences(program, |node| {
        matches!(
            node,
            ExpressionNode::ArrayLiteral(_) | ExpressionNode::StructLiteral(_)
        )
    }) {
        let reference = if program
            .expression_table
            .struct_literal_node(expression)
            .is_some()
        {
            retain_constructor_result_type(program, machine_symbol, state_symbol, expression)
        } else {
            let derived = (|| {
                let machine = program.machine_by_symbol(machine_symbol)?;
                let state = program.state_by_symbol(state_symbol)?;
                let elements = program.expression_table.array_literal_node(expression)?;
                let elements = program.expression_table.expression_handles(*elements);
                let references = elements
                    .iter()
                    .map(|element| {
                        validation::expression_result_type_reference(
                            program, machine, state, *element,
                        )
                    })
                    .collect::<Vec<_>>();
                let element_type = validation::join_result_type_references(
                    program,
                    &references,
                    &vec![false; references.len()],
                )?;
                Some((element_type, elements.len()))
            })();
            derived.map(|(element_type, length)| {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    })
            })
        };
        let Some(reference) = reference else {
            // A private execution graph may be prepared again after call
            // rebinding. Unresolved new children cannot retain the old type.
            program
                .expression_table
                .set_constructed_value_type_reference(
                    expression,
                    typed_trees::types::TypeReferenceHandle::invalid(),
                );
            continue;
        };
        program
            .expression_table
            .set_constructed_value_type_reference(expression, reference);
    }
}

/// A constructor's selected declaration supplies its telescope; completed
/// child types supply the arguments. Incomplete or conflicting tuples stay
/// unresolved. This establishes a type, never a resource claim or field proof.
fn retain_constructor_result_type(
    program: &mut TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let literal = program.expression_table.struct_literal_node(expression)?;
    let owner = program.unique_data_definition_by_symbol(literal.type_symbol)?;
    let parameters = program.data_type_parameters(owner);
    if parameters.is_empty() && owner.lifetime_parameters.is_empty() {
        return program
            .type_reference_table
            .find_named_type_reference(owner.symbol);
    }
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = program.state_by_symbol(state_symbol)?;
    let type_parameters = parameters
        .iter()
        .filter(|parameter| parameter.kind == typed_trees::data::TypeParameterKind::Type)
        .map(|parameter| (parameter.symbol, parameter.name.to_string()))
        .collect::<Vec<_>>();
    let const_parameters = parameters
        .iter()
        .filter_map(|parameter| match parameter.kind {
            typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                Some((parameter.symbol, parameter.name.to_string(), type_reference))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut type_proposals = Vec::new();
    let mut const_proposals = Vec::new();
    let mut lifetime_bindings = Vec::new();
    let mut lifetime_conflict = false;
    for field in program.expression_table.struct_fields(literal.fields) {
        // Inference reads an open declaration, not concrete construction
        // obligations. Select by the authored member handle inside this owner.
        let required = program
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(declared)
                    if declared.symbol == field.field_symbol =>
                {
                    Some(declared.type_reference)
                }
                typed_trees::data::DataMember::Variant(case)
                    if Some(case.symbol) == literal.case_symbol =>
                {
                    program
                        .data_payload_fields(case)
                        .iter()
                        .find(|declared| declared.symbol == field.field_symbol)
                        .map(|declared| declared.type_reference)
                }
                _ => None,
            })?;
        let Some(actual) =
            validation::expression_result_type_reference(program, machine, state, field.value)
        else {
            continue;
        };
        crate::monomorphization::infer_static_bindings(
            program,
            required,
            actual,
            &type_parameters,
            &const_parameters,
            None,
            0,
            &mut type_proposals,
            &mut const_proposals,
        );
        crate::conformance::conformance_application_lifetimes::collect_lifetime_bindings(
            program,
            required,
            actual,
            &owner.lifetime_parameters,
            &mut lifetime_bindings,
            &mut lifetime_conflict,
        );
    }
    if lifetime_conflict {
        return None;
    }
    let lifetime_arguments = owner
        .lifetime_parameters
        .iter()
        .map(|parameter| {
            lifetime_bindings
                .iter()
                .find(|(name, _)| name == parameter.as_str())
                .map(|(_, argument)| argument.as_str().into())
        })
        .collect::<Option<Vec<_>>>()?;
    let mut arguments = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let proposals = match parameter.kind {
            typed_trees::data::TypeParameterKind::Type => {
                let ordinal = type_parameters
                    .iter()
                    .position(|(symbol, _)| *symbol == parameter.symbol)?;
                type_proposals
                    .iter()
                    .filter(|(_, candidate, _)| *candidate == ordinal)
                    .map(|(_, _, reference)| *reference)
                    .collect::<Vec<_>>()
            }
            typed_trees::data::TypeParameterKind::Const { .. }
            | typed_trees::data::TypeParameterKind::Value { .. } => {
                let ordinal = const_parameters
                    .iter()
                    .position(|(symbol, _, _)| *symbol == parameter.symbol)?;
                const_proposals
                    .iter()
                    .filter(|(_, candidate, _)| *candidate == ordinal)
                    .map(|(_, _, reference)| *reference)
                    .collect::<Vec<_>>()
            }
            _ => return None,
        };
        let first = *proposals.first()?;
        let identity =
            typed_trees::type_identity::applied_type_identity(program, first, machine_symbol, &[])?;
        if proposals.iter().any(|reference| {
            typed_trees::type_identity::applied_type_identity(
                program,
                *reference,
                machine_symbol,
                &[],
            )
            .is_none_or(|candidate| candidate != identity)
        }) {
            return None;
        }
        arguments.push(first);
    }
    let base_symbol = owner.symbol;
    let base_name = owner.name.clone();
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles(arguments);
    Some(
        program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            }),
    )
}
