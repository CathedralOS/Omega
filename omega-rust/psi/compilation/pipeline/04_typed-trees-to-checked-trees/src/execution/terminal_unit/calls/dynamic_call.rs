//! A selector call `Trait::member(selector, data...)` from a Unit body.
//!
//! Checking retargets the call to its instantiated slot
//! (`monomorphization::dynamic_slots`), whose roster is the selector followed
//! by the requirement's instantiated formals. The selector is planned exactly
//! like any scalar operand and then moved into the route, so the plan carries
//! the dispatch operand apart from the data arguments every implementation
//! receives. Data arguments take the same checked-body custody an ordinary
//! call's do, because every selectable implementation is a checked body; no
//! implementation's body is inspected to plan them.
//!
//! Owned structural results use the ordinary result-custody judgment against
//! the public slot signature. Returned loans still require a public origin
//! contract; erased or mutable owned scalar formals and suspending requirements
//! retain their existing proof-lane and suspension coverage boundaries.
//! A requirement contract needs no route evidence: checking discharged its
//! `requires` against these operands, and every selectable implementation
//! refines it where its conformance is checked.

use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::calls::argument_paths::checked_call_scalar_arguments;
use crate::execution::terminal_unit::calls::structural_arguments::{
    call_claim_transfers, structural_call_arguments,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::is_unit;
use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralScalarParameterPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, PrimitiveType, SymbolHandle, strips_erased_parameter,
};
use checked_trees::{
    CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitCallTarget, CheckedUnitCallTransfers,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: Option<ScalarCalleeClosure>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    caller_trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    call: &checked_trees::FlowCallFact,
    expected_call_result: Option<&super::ExpectedCallValueResult<'_>>,
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    caller_operations: &[CheckedUnitEffectOperationPlan],
    trace: &LocalConstructionTrace,
    coordinate: CheckedUnitCallCoordinate,
    call_site: &validation::CallSite<'_>,
    source_site: Option<checked_trees::NominalMachineUseSite>,
    slot: &typed_trees::dynamic_traits::DynamicSlotSignature,
) -> Option<CheckedUnitEffectOperationPlan> {
    let phase = |name: &'static str| {
        trace.phase(name);
        trace.statement(Some(coordinate.statement_index));
    };
    phase("call operation: dynamic result shape");
    let result_matches = match expected_call_result {
        None => is_unit(program, slot.return_type),
        Some(expected) => super::call_operations::call_value_result_matches(
            program,
            slot.return_type,
            expected,
            &[],
        ),
    };
    if !result_matches {
        return None;
    }
    phase("call operation: dynamic requirement suspension");
    let requirement = program.dynamic_slot_requirement(slot)?;
    if requirement.suspends {
        return None;
    }
    phase("call operation: dynamic structural arguments");
    let structural_arguments = structural_call_arguments(
        program,
        facts,
        scalar_callees,
        call,
        machine,
        state,
        caller_parameters,
        caller_trivial_affine_locals,
        super::CallTarget::dynamic_slot(slot),
        call_site,
        call.statement_index,
        caller_structural_results,
        caller_operations,
        trace,
    )?;
    phase("call operation: dynamic scalar parameters");
    let mut scalar_parameters = Vec::new();
    for (position, parameter) in slot.parameters.iter().enumerate() {
        if strips_erased_parameter(parameter)? {
            return None;
        }
        let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference)
        else {
            continue;
        };
        if parameter.is_const || parameter.is_mutable {
            return None;
        }
        scalar_parameters.push(CheckedStructuralScalarParameterPlan {
            source_position: u32::try_from(position).ok()?,
            primitive_type,
        });
    }
    // The slot's first operand is the selector; it leads the scalar lane.
    if scalar_parameters
        .first()
        .map(|parameter| (parameter.source_position, parameter.primitive_type))
        != Some((0, PrimitiveType::Selector))
    {
        return None;
    }
    phase("call operation: dynamic scalar arguments");
    let mut scalar_arguments =
        checked_call_scalar_arguments(facts, state.symbol, coordinate, &scalar_parameters, false)?;
    if scalar_arguments.is_empty() {
        return None;
    }
    let selector = scalar_arguments.remove(0);
    let structural_result = match expected_call_result {
        Some(super::ExpectedCallValueResult::Structural(result)) => Some(*result),
        _ => None,
    };
    // Structural calls use the common occurrence-bound custody judgment below,
    // including its ingress transfers. Unit/scalar calls only need ingress.
    phase("call operation: dynamic claim transfers");
    let transfers = if structural_result.is_none() {
        call_claim_transfers(
            program,
            facts,
            machine.symbol,
            state.symbol,
            call,
            caller_parameters,
            caller_structural_results,
            &structural_arguments,
        )?
    } else {
        Vec::new()
    };
    let parts = CheckedUnitCallParts {
        coordinate,
        completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
        // Structural completion retains the producer's exact authored
        // occurrence when nested callers adopt its result namespace.
        source_site: structural_result.and(source_site),
        // No single implementation's contract report describes the call;
        // the slot is the target identity.
        target: CheckedUnitCallTarget {
            machine: slot.symbol,
            state: slot.symbol,
            contract_report_fingerprint: 0,
            service_reach: call.service_reach,
        },
        route: CheckedUnitCallRoute::Dynamic {
            selector,
            interface_identity: slot.interface_identity.clone(),
            requirement: slot.requirement,
        },
        scalar_arguments,
        structural_arguments,
        result: structural_result.map_or(CheckedUnitCallResult::Unit, |binding| {
            CheckedUnitCallResult::Structural {
                binding: binding.clone(),
                discard_result_on_return: binding.multiplicity
                    == language_semantics::Multiplicity::Affine,
            }
        }),
        transfers: if structural_result.is_some() {
            CheckedUnitCallTransfers::StructuralCustody(Default::default())
        } else {
            CheckedUnitCallTransfers::Claims(transfers)
        },
    };
    if structural_result.is_some() {
        phase("call operation: dynamic structural custody");
        super::structural_result_call::with_structural_custody(
            program,
            facts,
            machine.symbol,
            state.symbol,
            trace,
            parts,
        )
    } else {
        CheckedUnitCallPlan::new(parts).map(CheckedUnitEffectOperationPlan::Call)
    }
}
