//! Value identities for immutable invocation-entry element observations.
//!
//! A runtime selector is executable observation custody, not static geometry.
//! Reuse the structural term owner's typed indexed read and semantic identity;
//! never encode a callee expression handle as a caller place or mint a new
//! field/index fact key. Both the collection and every selector must derive
//! from this exact state's immutable owned/shared formals. Local aggregates
//! need the completed-prefix owner because an immutable local binding does not
//! freeze its contents. Stored references likewise begin another epoch.
//!
//! This identity supplies no bound, reference origin, access, or proposition.
//! Formation independently proves every index against its actual collection.

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// An authenticated scalar observation in one exact entry namespace. The key
/// stays private to its existing owner; consumers may compare identities only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImmutableEntryReadIdentity {
    machine: SymbolHandle,
    state: SymbolHandle,
    value: String,
}

pub fn immutable_entry_indexed_read_identity(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ImmutableEntryReadIdentity> {
    program.expression_table.indexed_node(expression)?;
    let symbol = super::binary::expression_scope(program, expression, 0)?;
    let (machine, state) = super::binary::scoped_machine_state(program, symbol)?;
    let reference = immutable_formal_expression(program, machine, state, expression, 0)?;
    if !program
        .primitive_type_reference(reference)?
        .is_fixed_integer()
        || program.arithmetic_domain_for_type_reference(reference)
            != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    let term = super::structural_term(
        program,
        Some((machine, state, state.return_type)),
        Some((machine.symbol, program.state_parameters(state))),
        expression,
        None,
    )?;
    Some(ImmutableEntryReadIdentity {
        machine: machine.symbol,
        state: state.symbol,
        value: term.observation_identity_key()?,
    })
}

/// Substitute an inert callee read through its complete declared actual
/// telescope. Both the collection and selector become caller observations;
/// no callee-local indexing handle becomes a caller place segment.
pub fn instantiate_immutable_entry_indexed_read_identity(
    program: &dyn crate::CheckingInput,
    predicate: SymbolHandle,
    expression: ExpressionHandle,
    arguments: &[ExpressionHandle],
    caller_machine: &Machine,
    caller_state: &State,
) -> Option<ImmutableEntryReadIdentity> {
    let machine = program.machine(predicate)?;
    let entry = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(entry);
    if parameters.len() != arguments.len() {
        return None;
    }
    program.expression_table.indexed_node(expression)?;
    let reference = immutable_formal_expression(program, machine, entry, expression, 0)?;
    if !program
        .primitive_type_reference(reference)?
        .is_fixed_integer()
        || program.arithmetic_domain_for_type_reference(reference)
            != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    let mut actuals = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        immutable_formal_expression(program, caller_machine, caller_state, *argument, 0)?;
        let reference = crate::expression_result_type_reference(
            program,
            caller_machine,
            caller_state,
            *argument,
        )?;
        if !crate::declared_carriers_match(program, parameter.type_reference, reference) {
            return None;
        }
        actuals.push(super::structural_term(
            program,
            Some((caller_machine, caller_state, caller_state.return_type)),
            Some((
                caller_machine.symbol,
                program.state_parameters(caller_state),
            )),
            *argument,
            None,
        )?);
    }
    let bindings = |expression| {
        let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
            return None;
        };
        let position = parameters
            .iter()
            .position(|parameter| parameter.symbol == name.symbol)?;
        actuals.get(position).cloned()
    };
    let term = super::structural_term(
        program,
        Some((machine, entry, entry.return_type)),
        Some((machine.symbol, parameters)),
        expression,
        Some(&bindings),
    )?;
    Some(ImmutableEntryReadIdentity {
        machine: caller_machine.symbol,
        state: caller_state.symbol,
        value: term.observation_identity_key()?,
    })
}

fn immutable_formal_expression(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<TypeReferenceHandle> {
    if depth >= 32 {
        return None;
    }
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Name(name) => {
            if !name.symbol.is_valid()
                || name.head_symbol != name.symbol
                || program
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
            {
                return None;
            }
            let parameter = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == name.symbol && !parameter.is_mutable)?;
            match crate::reference_shell(program, parameter.type_reference) {
                Some(shell) if shell.access == language_core::ReferenceAccess::Shared => {
                    shell.referee
                }
                Some(_) => return None,
                None => parameter.type_reference,
            }
        }
        ExpressionNode::Member(member) => {
            if member.case_variant.is_some() {
                return None;
            }
            let receiver =
                immutable_formal_expression(program, machine, state, member.receiver, depth + 1)?;
            let symbol = crate::value_custody::places::effective_member_symbol(
                program,
                member.receiver,
                member,
            );
            crate::owned_projection_type_reference(
                program,
                receiver,
                &[facts::PlaceSegment::Field { symbol }],
            )?
        }
        ExpressionNode::Indexed(indexed) => {
            if !crate::value_custody::places::place_has_builtin_coordinates(
                program,
                machine,
                Some(state),
                expression,
            ) || program.expression_table.indexed_on_range(indexed)
            {
                return None;
            }
            let collection = immutable_formal_expression(
                program,
                machine,
                state,
                indexed.collection,
                depth + 1,
            )?;
            let selector =
                immutable_formal_expression(program, machine, state, indexed.index, depth + 1)?;
            if !program
                .primitive_type_reference(selector)?
                .is_fixed_integer()
                || program.arithmetic_domain_for_type_reference(selector)
                    != numerics::arithmetic::ArithmeticDomain::Exact
            {
                return None;
            }
            let collection =
                crate::value_custody::places::unwrapped_type_reference(program, collection)?;
            let TypeReferenceNode::FixedArray { element_type, .. } =
                program.type_reference_table.type_reference(collection)
            else {
                return None;
            };
            *element_type
        }
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_core::ReferenceAccess::Shared =>
        {
            immutable_formal_expression(program, machine, state, borrow.target, depth + 1)?
        }
        ExpressionNode::Integer(_) => {
            crate::expression_result_type_reference(program, machine, state, expression)?
        }
        _ => return None,
    };
    // Only a formal's outer shared presentation is readable here. A reference
    // stored inside an owned path is a descriptor, never a referent snapshot.
    crate::reference_shell(program, reference)
        .is_none()
        .then_some(reference)
}
