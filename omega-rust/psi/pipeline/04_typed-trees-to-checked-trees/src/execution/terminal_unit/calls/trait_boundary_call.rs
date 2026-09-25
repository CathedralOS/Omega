//! A call to a boundary trait requirement through a static binder.
//!
//! The requirement's formals stay generic; the call's checked
//! specialization supplies each `Type` binder's actual and each `machine`
//! binder's admitted provider, and the replay substitutes them before the
//! argument and result evidence is compared with the caller's concrete
//! shapes. Owned arguments transfer into the boundary, and every live claim
//! gets a normal-completion receipt at its argument.

use crate::execution::terminal_unit::calls::view_subslice;

use crate::execution::terminal_unit::calls::argument_paths::{
    byte_sequence_literal_argument, checked_call_scalar_arguments, projected_argument_path,
};
use crate::execution::terminal_unit::calls::boundary_admission::{
    boundary_argument_presentation_is_admitted, boundary_value_result_matches,
    fixed_byte_array_view_is_admitted, provider_attachment_receiver_matches,
};
use crate::execution::terminal_unit::calls::result_arguments;
use crate::execution::terminal_unit::calls::structural_arguments::{
    call_claim_transfers, exact_structural_argument_access,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::byte_sequence_carrier;
use crate::execution::terminal_unit::types::{
    base_type_identity_with_substitutions, substituted_formal_type,
};
use crate::execution::terminal_unit::types::{
    byte_sequence_type_identity, is_unit, parameter_root_symbol,
    signature_contracts_are_exact_parameter_qualifications, structural_access_for_type_reference,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, Multiplicity,
    PermissionEventKind, SymbolHandle, TypedTrees, is_reference,
};

/// Plan a call whose target is `signature`, the one requirement of boundary
/// trait `definition` it names. `None` means some argument, result, receiver,
/// or contract of this call has no admitted boundary presentation.
pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    call: &checked_trees::FlowCallFact,
    expected_call_result: Option<&super::ExpectedCallValueResult<'_>>,
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    trace: &LocalConstructionTrace,
    coordinate: CheckedUnitCallCoordinate,
    call_site: &crate::semantic::calls::CallSite<'_>,
    source_site: Option<checked_trees::NominalMachineUseSite>,
    definition: &typed_trees::trait_definition::TraitDefinition,
    signature: &typed_trees::signature::StateSignature,
    selected_realization: Option<(SymbolHandle, SymbolHandle)>,
) -> Option<CheckedUnitEffectOperationPlan> {
    let phase = |name: &'static str| {
        trace.phase(name);
        trace.statement(Some(coordinate.statement_index));
    };
    phase("call operation: boundary signature");
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, call_site);
    let source_parameters = program.state_signature_parameters(signature);
    // A boundary signature is a foreign ABI contract; an erased position
    // has no agreed foreign transfer yet, so the call fails closed here.
    if source_parameters
        .iter()
        .any(|parameter| parameter.relevance.is_erased())
    {
        return None;
    }
    let abi_parameters = source_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| !parameter.is_self)
        .collect::<Vec<_>>();
    let caller_source_parameters = program.state_parameters(state);
    let signature_type_parameters = program.state_signature_type_parameters(signature);
    // MP2b's admission of this call's static machine arguments survives
    // checking as a specialization fact: each `Type` binder's derived
    // actual and each `machine` binder's admitted provider entry. The
    // requirement's formals stay generic, so the replay below substitutes
    // them before comparing argument and result evidence against the
    // caller's concrete shapes.
    let specialization = source_site.and_then(|site| {
        facts
            .requirement_call_specializations
            .for_site(site, call.target_symbol)
    });
    let substitutions = specialization
        .map(|specialization| {
            specialization
                .type_bindings
                .iter()
                .map(|binding| (binding.parameter, binding.actual))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // The authored static machine arguments the ordinal of each retained
    // selection indexes: validation filtered the `<>` argument list to
    // machine-typed members before recording `static_machine_ordinal`.
    let authored_machine_arguments = match &call_site {
        crate::semantic::calls::CallSite::Statement(call) => call.machine_arguments.as_ref(),
        crate::semantic::calls::CallSite::Expression { call, .. } => {
            call.machine_arguments.as_ref()
        }
        crate::semantic::calls::CallSite::TransitionNamed { .. } => &[],
    }
    .iter()
    .filter(|argument| {
        matches!(
            program.symbols.get(argument.symbol).kind,
            symbols::SymbolKind::State | symbols::SymbolKind::MachineParameter
        )
    })
    .collect::<Vec<_>>();
    // A retained specialization discharges the signature telescope the
    // nominal-use callback admission cannot see: every `Type` binder
    // carries its derived actual and every `machine` binder's retained
    // selection is exactly the authored static argument at its ordinal.
    // Binders the specialization does not cover stay rejected.
    let specialized_telescope = specialization.is_some_and(|specialization| {
        signature_type_parameters
            .iter()
            .all(|parameter| match &parameter.kind {
                typed_trees::data::TypeParameterKind::Type => specialization
                    .type_bindings
                    .iter()
                    .any(|binding| binding.parameter == parameter.symbol),
                typed_trees::data::TypeParameterKind::Machine { .. } => {
                    specialization.machine_selections.iter().any(|selection| {
                        selection.parameter == parameter.symbol
                            && usize::try_from(selection.static_machine_ordinal)
                                .ok()
                                .and_then(|ordinal| authored_machine_arguments.get(ordinal))
                                .is_some_and(|argument| argument.symbol == selection.selected)
                    })
                }
                _ => false,
            })
    });
    // Each `machine` signature type parameter is a nominal binder discharged
    // by a `nominal_machine_use` at this exact call site: same registration
    // operation, binder ordinal, and satisfaction row. The callback's
    // destination comes from one of two places — an ABI `native callback`
    // parameter declared at the same telescope ordinal, or a target-owned
    // private slot whose `PrivateCallbackSlot` conformance supplied the
    // evaluated boundary calling plan retained on the use.
    let nominal_use_at_binder = |ordinal: usize, parameter: &typed_trees::data::TypeParameter| {
        let typed_trees::data::TypeParameterKind::Machine {
            contract:
                typed_trees::data::MachineParameterContract::Nominal {
                    trait_definition,
                    requirement,
                },
        } = parameter.kind
        else {
            return None;
        };
        facts.nominal_machine_uses.uses.iter().find(|nominal_use| {
            Some(nominal_use.site) == source_site
                && nominal_use.registration_operation == signature.symbol
                && usize::try_from(nominal_use.static_machine_ordinal).ok() == Some(ordinal)
                && nominal_use.satisfaction_trait == trait_definition
                && nominal_use.satisfaction_requirement == requirement
        })
    };
    let admitted_callback_telescope = source_site.is_some()
        && signature.native_callback_parameters.len() <= signature_type_parameters.len()
        && signature_type_parameters
            .iter()
            .enumerate()
            .all(|(ordinal, parameter)| {
                let Some(nominal_use) = nominal_use_at_binder(ordinal, parameter) else {
                    return false;
                };
                match signature.native_callback_parameters.get(ordinal) {
                    Some(callback) => parameter.name == callback.binder,
                    None => nominal_use.callback_placement.is_some(),
                }
            });
    let routed_service_parameter_receiver = caller_parameters.iter().any(|parameter| {
        parameter
            .fused_service_erasure
            .as_ref()
            .is_some_and(|receipt| {
                receipt.source_parameter == call.receiver_symbol
                    && receipt.requirement == definition.symbol
            })
    });
    let mut scalar_parameters = Vec::new();
    let mut structural_arguments = Vec::new();
    phase("call operation: boundary arguments");
    for (abi_position, ((_, parameter), argument)) in
        abi_parameters.iter().zip(arguments.iter()).enumerate()
    {
        let source_position = u32::try_from(abi_position).ok()?;
        // A retained specialization resolves a bare `Type` formal to the
        // actual its admission derived; a compound formal keeps its own
        // reference and substitutes inside the identity checks below.
        let formal =
            substituted_formal_type(program, parameter.type_reference, substitutions.as_slice());
        if let Some(primitive_type) = program.primitive_type_reference(formal) {
            scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                source_position,
                primitive_type,
            });
            continue;
        }
        let byte_sequence = byte_sequence_carrier(program, formal, substitutions.as_slice());
        if validation::is_closed_primitive_array_type(program, formal) {
            return None;
        }
        let target_identity = if byte_sequence.is_some() {
            byte_sequence_type_identity(program, formal, &[], substitutions.as_slice())?
        } else {
            base_type_identity_with_substitutions(program, formal, &[], substitutions.as_slice())?
        };
        if let Some(literal) = byte_sequence_literal_argument(program, formal, *argument) {
            structural_arguments.push(literal);
            continue;
        }
        if let Some(subslice) = view_subslice::admit(
            program,
            facts,
            machine,
            state,
            caller_parameters,
            formal,
            *argument,
            call.statement_index,
            checked_trees::CheckedSubsliceSite::CallArgument {
                call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                argument_ordinal: u32::try_from(structural_arguments.len()).ok()?,
            },
        ) && subslice.range.kind == view_subslice::ViewKind::Bytes
        {
            structural_arguments.push(subslice.argument());
            continue;
        }

        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            call.statement_index,
            *argument,
        )?;
        if let Some((result, _)) = caller_structural_results
            .iter()
            .find(|(_, root)| *root == place.root)
        {
            if result.multiplicity == Multiplicity::Unrestricted {
                return None;
            }
            structural_arguments.push(result_arguments::argument(
                program,
                facts,
                machine.symbol,
                state.symbol,
                call,
                *argument,
                &place,
                result,
                parameter,
                &target_identity,
                false,
            )?);
            continue;
        }
        let facts::PlaceRoot::Symbol(source_symbol) = place.root else {
            return None;
        };
        // An authored `self.field` argument roots at the `self` parameter's
        // own symbol, while an implicit receiver place roots at the machine
        // the parameter is attached to. Both spellings name one parameter.
        let source_parameter = caller_source_parameters.iter().find(|candidate| {
            parameter_root_symbol(machine.symbol, candidate) == source_symbol
                || candidate.symbol == source_symbol
        })?;
        if validation::is_closed_primitive_array_type(program, source_parameter.type_reference) {
            return None;
        }
        let source_position = caller_source_parameters
            .iter()
            .position(|candidate| candidate.symbol == source_parameter.symbol)?;
        let source_parameter_index = caller_parameters.iter().position(|candidate| {
            candidate.position == u32::try_from(source_position).unwrap_or(u32::MAX)
        })?;
        let caller_parameter = caller_parameters.get(source_parameter_index)?;
        let path = if place.segments.is_empty() {
            // A whole array and its field projection lend the same fixed
            // range. Absence of a projection does not require a fake view
            // type identity; access and source custody still replay below.
            if caller_parameter.type_identity != target_identity
                && !(caller_parameter.qualifications.is_empty()
                    && fixed_byte_array_view_is_admitted(
                        program,
                        source_parameter.type_reference,
                        formal,
                    ))
            {
                return None;
            }
            Vec::new()
        } else if matches!(
            place.segments.last(),
            Some(facts::PlaceSegment::FixedRange { .. })
        ) {
            if !matches!(
                caller_parameter.access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::SharedBorrow
            ) || caller_parameter.multiplicity != Multiplicity::Unrestricted
                || !caller_parameter.qualifications.is_empty()
            {
                return None;
            }
            super::argument_paths::fixed_byte_array_range_path(
                program,
                state.symbol,
                call.statement_index,
                &place,
                formal,
            )?
        } else {
            if !caller_parameter.qualifications.is_empty() {
                return None;
            }
            let (projected_type, path) =
                projected_argument_path(program, state.symbol, call.statement_index, &place)?;
            if !boundary_argument_presentation_is_admitted(
                program,
                projected_type,
                formal,
                &target_identity,
                substitutions.as_slice(),
            ) && !fixed_byte_array_view_is_admitted(program, projected_type, formal)
            {
                return None;
            }
            path
        };
        structural_arguments.push(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(source_parameter_index).ok()?,
            },
            path,
            type_identity: target_identity,
            access: exact_structural_argument_access(
                program,
                facts,
                machine.symbol,
                state.symbol,
                call,
                &place,
                structural_access_for_type_reference(program, formal)?,
            )?,
        });
    }
    phase("call operation: boundary telescope");
    if !program.trait_type_parameters(definition).is_empty() {
        return None;
    }
    if !signature_type_parameters.is_empty()
        && !admitted_callback_telescope
        && !specialized_telescope
    {
        return None;
    }
    phase("call operation: boundary parameter custody");
    if program
        .state_signature_parameters(signature)
        .iter()
        // `is_mutable` is set by a `mut` binding and by an exclusive
        // borrow alike, and the requirement's exclusive borrow is
        // already carried exactly by the argument's presented access.
        // An owned mutable scalar is an argument destination, not
        // mutable storage in this Unit caller. Other owned mutable
        // carriers remain outside this call surface.
        .any(|parameter| {
            !parameter.is_self
                && (parameter.is_const
                    || (parameter.is_mutable
                        && !is_reference(
                            program,
                            substituted_formal_type(
                                program,
                                parameter.type_reference,
                                substitutions.as_slice(),
                            ),
                        )
                        && crate::values::mutable_scalar_parameter_type(program, parameter)
                            .is_none()))
        })
    {
        return None;
    }
    phase("call operation: boundary receiver");
    if arguments.len() != abi_parameters.len() {
        return None;
    }
    if if let Some((_, qualifier)) = selected_realization {
        call.has_receiver && call.receiver_symbol != qualifier
    } else {
        !call.has_receiver
            || (call.receiver_symbol != definition.symbol
                && !provider_attachment_receiver_matches(
                    program,
                    machine,
                    call_site,
                    definition.symbol,
                )
                && !routed_service_parameter_receiver)
    } {
        return None;
    }
    phase("call operation: boundary result");
    let signature_return =
        substituted_formal_type(program, signature.return_type, substitutions.as_slice());
    if validation::is_closed_primitive_array_type(program, signature_return) {
        return None;
    }
    if match expected_call_result {
        None => !is_unit(program, signature_return),
        Some(expected) => !boundary_value_result_matches(
            program,
            signature.return_type,
            expected,
            substitutions.as_slice(),
        ),
    } {
        return None;
    }
    phase("call operation: boundary contracts");
    if !signature_contracts_are_exact_parameter_qualifications(
        program,
        definition.symbol,
        signature,
    ) {
        return None;
    }
    // Suspension parks the activation, which this synchronous call
    // shape cannot express. Blocking only occupies the worker while
    // the boundary waits (effects.md, `blocks;`): the call returns
    // through the same edge, the site already acknowledged `block`
    // during checking, and the envelope travels on the target
    // contract fingerprint below, so a blocking boundary is planned
    // exactly like a nonblocking one.
    if signature.suspends {
        return None;
    }
    let capsule = facts
        .contract_plans
        .crash_capsule(definition.symbol, signature.symbol)?;
    // Trait calls transfer each owned argument into the boundary. Their
    // permission events are transfers, not the terminal consumption used
    // by an owned receiver. Reuse the exact call-site custody replay so
    // every live claim has a normal-completion receipt at its argument.
    phase("call operation: boundary claim transfers");
    let completion_receipts = call_claim_transfers(
        facts,
        machine.symbol,
        state.symbol,
        call,
        caller_parameters,
        entry_claims,
        caller_structural_results,
        &structural_arguments,
        PermissionEventKind::Transfer,
    )?;
    Some(CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        source_site,
        target_machine: signature.symbol,
        target_state: signature.symbol,
        target_contract_report_fingerprint: capsule.target_contract_report_fingerprint(),
        service_reach: call.service_reach,
        scalar_arguments: checked_call_scalar_arguments(
            facts,
            state.symbol,
            coordinate,
            &scalar_parameters,
            true,
        )?,
        structural_arguments,
        completion_receipts,
    })
}
