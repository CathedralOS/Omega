//! Calls to authored boundary requirements, including legacy operators.
//!
//! The requirement's formals stay generic; the call's checked
//! specialization supplies each `Type` binder's actual and each `machine`
//! binder's admitted provider, and the replay substitutes them before the
//! argument and result evidence is compared with the caller's concrete
//! shapes. Owned arguments transfer into the boundary, and every live claim
//! gets a normal-completion receipt at its argument.

use crate::execution::terminal_unit::calls::view_subslice;

use crate::execution::terminal_unit::calls::argument_paths::{
    byte_sequence_literal_argument, checked_call_scalar_arguments,
};
use crate::execution::terminal_unit::calls::boundary_admission::{
    boundary_value_result_matches, parameter_binding_receiver_matches,
    provider_attachment_receiver_matches,
};
use crate::execution::terminal_unit::calls::parameter_argument_paths::{
    ParameterArgument, SourceParameter, boundary_parameter_path,
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
    byte_sequence_type_identity, is_unit, signature_contracts_are_exact_parameter_qualifications,
    structural_access_for_type_reference,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralScalarParameterPlan, CheckedUnitCallCoordinate,
    CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, SymbolHandle, TypedTrees, is_reference,
};

/// Authored requirement declarations share one call planner. This is a
/// borrowed view of the actual declaration, never a manufactured machine or
/// a choice of provider; declaration identity survives into the call row.
#[derive(Clone, Copy)]
pub(super) enum BoundaryRequirement<'program> {
    Trait {
        definition: &'program typed_trees::trait_definition::TraitDefinition,
        signature: &'program typed_trees::signature::StateSignature,
    },
    Operator(&'program typed_trees::operator::OperatorDefinition),
}

/// Plan a requirement call with its original argument/result custody.
/// `None` means some operand, receiver, or contract has no admitted boundary
/// presentation; selection never weakens that signature.
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
    requirement: BoundaryRequirement<'_>,
    selected_realization: Option<(SymbolHandle, SymbolHandle)>,
) -> Option<CheckedUnitEffectOperationPlan> {
    let phase = |name: &'static str| {
        trace.phase(name);
        trace.statement(Some(coordinate.statement_index));
    };
    phase("call operation: boundary signature");
    let (
        owner,
        target,
        source_parameters,
        signature_type_parameters,
        return_type,
        callback_parameters,
    ) = match requirement {
        BoundaryRequirement::Trait {
            definition,
            signature,
        } => (
            definition.symbol,
            signature.symbol,
            program.state_signature_parameters(signature),
            program.state_signature_type_parameters(signature),
            signature.return_type,
            signature.native_callback_parameters.as_slice(),
        ),
        BoundaryRequirement::Operator(operator) => (
            operator.symbol,
            operator.symbol,
            program.operator_parameters(operator),
            program.operator_type_parameters(operator),
            operator.return_type,
            &[][..],
        ),
    };
    let operator_arguments = match (requirement, call_site) {
        (
            BoundaryRequirement::Operator(_),
            crate::semantic::calls::CallSite::Expression { call, .. },
        ) => Some(crate::facts::operator_crashes::named_call_operands(
            program,
            call,
            source_parameters,
        )?),
        _ => None,
    };
    let arguments = operator_arguments.as_deref().unwrap_or_else(|| {
        crate::semantic::calls::call_site_argument_expressions(program, call_site)
    });
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
        .filter(|(_, parameter)| {
            !parameter.is_self || matches!(requirement, BoundaryRequirement::Operator(_))
        })
        .collect::<Vec<_>>();
    let caller_source_parameters = program.state_parameters(state);
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
                && nominal_use.registration_operation == target
                && usize::try_from(nominal_use.static_machine_ordinal).ok() == Some(ordinal)
                && nominal_use.satisfaction_trait == trait_definition
                && nominal_use.satisfaction_requirement == requirement
        })
    };
    let admitted_callback_telescope = source_site.is_some()
        && callback_parameters.len() <= signature_type_parameters.len()
        && signature_type_parameters
            .iter()
            .enumerate()
            .all(|(ordinal, parameter)| {
                let Some(nominal_use) = nominal_use_at_binder(ordinal, parameter) else {
                    return false;
                };
                match callback_parameters.get(ordinal) {
                    Some(callback) => parameter.name == callback.binder,
                    None => nominal_use.callback_placement.is_some(),
                }
            });
    let routed_service_parameter_receiver = caller_parameters.iter().any(|parameter| {
        parameter
            .fused_service_erasure
            .as_ref()
            .is_some_and(|receipt| {
                receipt.source_parameter == call.receiver_symbol && receipt.requirement == owner
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
            // A boundary call stages no whole-field view to narrow.
            && subslice.range.collection.is_empty()
        {
            structural_arguments.push(subslice.argument());
            continue;
        }

        // A copied field has already been materialized by the shared operand
        // sequencer. Pass that independent owned value, not the borrowed
        // receiver path which supplied its contents. Ordinary calls make the
        // same distinction between evaluated operands and source storage.
        let place = caller_structural_results
            .iter()
            .find(|(_, root)| *root == facts::PlaceRoot::Expression(*argument))
            .map(|_| crate::flow::CanonicalPlace {
                root: facts::PlaceRoot::Expression(*argument),
                segments: Vec::new(),
            })
            .or_else(|| {
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    call.statement_index,
                    *argument,
                )
            })?;
        if let Some((result, _)) = caller_structural_results
            .iter()
            .find(|(_, root)| *root == place.root)
        {
            // Result operands own type/presentation and loan admission. A
            // borrowed formal need not itself be the local's owned array type.
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
        let source = SourceParameter::resolve(
            machine.symbol,
            caller_source_parameters,
            caller_parameters,
            source_symbol,
        )?;
        let custody = source.custody(structural_access_for_type_reference(program, formal));
        let argument = ParameterArgument {
            program,
            machine: machine.symbol,
            state: state.symbol,
            statement_index: call.statement_index,
            place: &place,
            target: formal,
            target_identity: &target_identity,
        };
        let path = boundary_parameter_path(argument, custody, substitutions.as_slice())?;
        // A whole array and its field projection lend the same fixed range;
        // access and source custody still replay below.
        // Whole bounded storage presents the same live byte view as an inline
        // field. Its qualifications do not turn a shared observation into a
        // fixed-capacity array loan; access and predicate custody replay below.
        if path.is_empty()
            && !argument.whole_parameter_is_admitted(
                source,
                custody.unqualified,
                substitutions.as_slice(),
            )
        {
            return None;
        }
        structural_arguments.push(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(source.index).ok()?,
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
                custody.target_access?,
            )?,
        });
    }
    phase("call operation: boundary telescope");
    if let BoundaryRequirement::Trait { definition, .. } = requirement
        && !program.trait_type_parameters(definition).is_empty()
    {
        return None;
    }
    if !signature_type_parameters.is_empty()
        && !admitted_callback_telescope
        && !specialized_telescope
    {
        return None;
    }
    phase("call operation: boundary parameter custody");
    if source_parameters
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
    } else if matches!(requirement, BoundaryRequirement::Trait { .. }) {
        !call.has_receiver
            || (call.receiver_symbol != owner
                && !provider_attachment_receiver_matches(program, machine, call_site, owner)
                && !parameter_binding_receiver_matches(
                    program,
                    state,
                    coordinate.statement_index as usize,
                    caller_parameters,
                    call_site,
                    owner,
                )
                && !routed_service_parameter_receiver)
    } else {
        false
    } {
        return None;
    }
    phase("call operation: boundary result");
    let signature_return = substituted_formal_type(program, return_type, substitutions.as_slice());
    if match expected_call_result {
        None => !is_unit(program, signature_return),
        Some(expected) => {
            !boundary_value_result_matches(program, return_type, expected, substitutions.as_slice())
        }
    } {
        return None;
    }
    phase("call operation: boundary contracts");
    if let BoundaryRequirement::Trait {
        definition,
        signature,
    } = requirement
        && (!signature_contracts_are_exact_parameter_qualifications(
            program,
            definition.symbol,
            signature,
        ) || signature.suspends)
    {
        return None;
    }
    // Suspension parks the activation, which this synchronous call
    // shape cannot express. Blocking only occupies the worker while
    // the boundary waits (effects.md, `blocks;`): the call returns
    // through the same edge, the site already acknowledged `block`
    // during checking, and the envelope travels on the target
    // contract fingerprint below, so a blocking boundary is planned
    // exactly like a nonblocking one.
    let capsule = facts.contract_plans.crash_capsule(owner, target)?;
    // The common caller-custody join accounts for each owned argument.
    // Here those rows are normal-completion receipts because the selected
    // destination is a boundary, rather than a checked body's entry roster.
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
    )?;
    checked_trees::CheckedUnitCallPlan::new(checked_trees::CheckedUnitCallParts {
        coordinate,
        source_site,
        target: checked_trees::CheckedUnitCallTarget {
            machine: target,
            state: target,
            contract_report_fingerprint: capsule.target_contract_report_fingerprint(),
            service_reach: call.service_reach,
        },
        route: checked_trees::CheckedUnitCallRoute::Boundary,
        scalar_arguments: checked_call_scalar_arguments(
            facts,
            state.symbol,
            coordinate,
            &scalar_parameters,
            true,
        )?,
        structural_arguments,
        result: checked_trees::CheckedUnitCallResult::Unit,
        transfers: checked_trees::CheckedUnitCallTransfers::CompletionReceipts(completion_receipts),
    })
    .map(CheckedUnitEffectOperationPlan::Call)
}
