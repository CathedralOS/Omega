//! Calls to authored boundary requirements, including legacy operators.
//!
//! The requirement's formals stay generic; the call's checked
//! specialization supplies each `Type` binder's actual and each `machine`
//! binder's admitted provider, and the replay substitutes them before the
//! argument and result evidence is compared with the caller's concrete
//! shapes. Owned arguments transfer into the boundary, and every live claim
//! gets a normal-completion receipt at its argument. Source checking owns
//! authored preconditions and normal guarantees; this planner retains the exact
//! checked requirement commitment rather than replaying a contract shape gate.

use crate::execution::terminal_unit::calls::view_subslice;

use crate::execution::terminal_unit::calls::argument_paths::{
    byte_sequence_literal_argument, checked_call_scalar_arguments,
};
use crate::execution::terminal_unit::calls::boundary_admission::{
    BoundaryTraitReceiver, boundary_trait_receiver_source,
};
use crate::execution::terminal_unit::calls::call_operations::call_value_result_matches;
use crate::execution::terminal_unit::calls::parameter_argument_paths::{
    ParameterArgument, SourceParameter, boundary_parameter_path,
};
use crate::execution::terminal_unit::calls::result_arguments;
use crate::execution::terminal_unit::calls::structural_arguments::{
    call_claim_transfers, exact_structural_argument_access,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{base_type_identity, substituted_formal_type};
use crate::execution::terminal_unit::types::{is_unit, structural_access_for_type_reference};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralScalarParameterPlan, CheckedUnitCallCoordinate,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, SymbolHandle, is_reference,
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    call: &checked_trees::FlowCallFact,
    expected_call_result: Option<&super::ExpectedCallValueResult<'_>>,
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    caller_operations: &[CheckedUnitEffectOperationPlan],
    trace: &LocalConstructionTrace,
    coordinate: CheckedUnitCallCoordinate,
    call_site: &validation::CallSite<'_>,
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
        (BoundaryRequirement::Operator(_), validation::CallSite::Expression { call, .. }) => Some(
            crate::facts::operator_crashes::named_call_operands(program, call, source_parameters)?,
        ),
        _ => None,
    };
    let arguments = operator_arguments
        .as_deref()
        .unwrap_or_else(|| validation::call_site_argument_expressions(program, call_site));
    // A boundary signature is a foreign ABI contract; an erased position
    // has no agreed foreign transfer yet, so the call fails closed here.
    if source_parameters
        .iter()
        .any(|parameter| parameter.relevance.is_erased())
    {
        return None;
    }
    let abi_parameters = source_parameters.iter().enumerate().collect::<Vec<_>>();
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
        validation::CallSite::Statement(call) => call.machine_arguments.as_ref(),
        validation::CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
        validation::CallSite::TransitionNamed { .. } => &[],
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
    // private slot whose placement Omega must supply. Neither requires an
    // evaluated native ABI in the source callback's checked facts.
    let nominal_use_at_binder = |ordinal: usize, parameter: &typed_trees::data::TypeParameter| {
        let typed_trees::data::TypeParameterKind::Machine {
            contract:
                typed_trees::data::MachineParameterContract::Nominal {
                    trait_definition,
                    requirement,
                    ..
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
                    None => nominal_use.callback_entry.is_some(),
                }
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
        phase("call operation: boundary argument identity");
        let target_identity = base_type_identity(program, formal, &[], substitutions.as_slice())?;
        // A retained argument owns its evaluated value, including a literal
        // descriptor. Boundary calls consume that result just as checked-body
        // calls do; reconstructing its source would orphan the producer.
        let produced_place = caller_structural_results
            .iter()
            .find(|(_, root)| *root == facts::PlaceRoot::Expression(*argument))
            .map(|_| validation::CanonicalPlace {
                root: facts::PlaceRoot::Expression(*argument),
                segments: Vec::new(),
            });
        if produced_place.is_none() {
            if let Some(literal) = byte_sequence_literal_argument(program, formal, *argument) {
                structural_arguments.push(literal);
                continue;
            }
            if validation::subslice_expression(program, *argument).is_some() {
                // The ordinary operand scheduler establishes a descriptor for a
                // projected array before narrowing it. Boundary and checked-body
                // calls consume the same range value, with its exact source path,
                // endpoint occurrences and access; a range cannot fall back to a
                // fixed projection when its own admission fails.
                let subslice = view_subslice::admit(
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
                )?;
                if let Some(argument) =
                    subslice.call_argument(program, state, call.statement_index, true)
                {
                    structural_arguments.push(argument);
                    continue;
                }
            }
        }

        // A copied field has already been materialized by the shared operand
        // sequencer. Pass that independent owned value, not the borrowed
        // receiver path which supplied its contents. Ordinary calls make the
        // same distinction between evaluated operands and source storage.
        phase("call operation: boundary argument place");
        let place = produced_place.or_else(|| {
            validation::canonical_place_from_expression_in_state(
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
            phase("call operation: boundary result argument");
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
                target_identity,
                caller_operations,
                trace,
            )?);
            continue;
        }
        phase("call operation: boundary argument parameter");
        let source_symbol = place.root.as_symbol()?;
        // Boundary calls lend the same primitive local home as checked-body
        // calls. Its existing admission retains the authored type and exact
        // borrow occurrence; a scalar local is not a structural parameter.
        if super::super::structural_scalar_store::primitive_local_before(
            program,
            state,
            call.statement_index,
            source_symbol,
        )
        .is_some()
        {
            structural_arguments.push(super::computation_arguments::primitive_local_argument(
                program,
                &facts.borrow,
                machine.symbol,
                state,
                call,
                &place,
                &place,
                formal,
            )?);
            continue;
        }
        // A reference operand lends the same established endpoint to a
        // boundary requirement as to a checked body. Scalar formals live in
        // the scalar lane, so the structural-only parameter presentation below
        // cannot supply their home. The shared join retains the exact loan,
        // declared referent and dense source coordinate without transferring
        // the caller's custody.
        phase("call operation: boundary borrowed storage");
        let mut receiving_parameter = (*parameter).clone();
        receiving_parameter.type_reference = formal;
        if let Some(argument) = super::computation_arguments::borrowed_storage_argument(
            program,
            &facts.borrow,
            machine.symbol,
            state,
            call,
            &place,
            &receiving_parameter,
            None,
        ) {
            structural_arguments.push(argument);
            continue;
        }
        phase("call operation: boundary argument parameter");
        let source =
            SourceParameter::resolve(caller_source_parameters, caller_parameters, source_symbol)?;
        let custody = source.custody(structural_access_for_type_reference(program, formal));
        let argument = ParameterArgument {
            program,
            machine: machine.symbol,
            state: state.symbol,
            statement_index: call.statement_index,
            place: &place,
            target: formal,
            target_identity,
            selectors: None,
        };
        phase("call operation: boundary argument projection");
        let path = boundary_parameter_path(argument, custody, substitutions.as_slice())?;
        // A whole array and its field projection lend the same fixed range;
        // access and source custody still replay below.
        // Whole bounded storage presents the same live byte view as an inline
        // field. Its qualifications do not turn a shared observation into a
        // fixed-capacity array loan; access and predicate custody replay below.
        phase("call operation: boundary argument presentation");
        if path.is_empty()
            && !argument.whole_parameter_is_admitted(
                source,
                custody.unqualified,
                substitutions.as_slice(),
            )
        {
            return None;
        }
        phase("call operation: boundary argument access");
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
        && !program
            .closed_requirement_application(target)
            .is_some_and(|application| application.declaring_trait == definition.symbol)
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
            parameter.is_const
                || (parameter.is_mutable
                    && !is_reference(
                        program,
                        substituted_formal_type(
                            program,
                            parameter.type_reference,
                            substitutions.as_slice(),
                        ),
                    )
                    && crate::values::mutable_scalar_parameter_type(program, parameter).is_none())
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
        let receiver = boundary_trait_receiver_source(program, state, call, call_site, owner);
        if receiver.is_none()
            && let Some(reason) =
                super::boundary_authority_refusal(program, machine.symbol, state, call)
        {
            phase(reason);
        }
        !call.has_receiver
            || receiver.is_none_or(|receiver| match receiver {
                BoundaryTraitReceiver::Parameters(sources) => sources.iter().any(|source| {
                    !caller_parameters.iter().any(|parameter| {
                        program
                            .state_parameters(state)
                            .get(parameter.position as usize)
                            .is_some_and(|parameter| parameter.symbol == *source)
                    })
                }),
                BoundaryTraitReceiver::Namespace => false,
            })
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
            !call_value_result_matches(program, return_type, expected, substitutions.as_slice())
        }
    } {
        return None;
    }
    // Source checking discharged the selected requirement's authored contracts.
    // Its checked commitment remains on the declaration and call; execution
    // planning must not re-admit the same contract through a structural-only
    // whitelist. Parameter/result qualifications and actual call custody retain
    // their independent representation joins above and below.
    // Suspension retains this ordinary call's incomplete operation, not a
    // different completion edge. Lowering joins the checked crossing and its
    // exact live frontier to the emitted call; unsupported storage and claims
    // still reject there. Blocking likewise preserves the completion edge.
    // Neither acknowledgement grants a provider or runtime preservation.
    let capsule = facts.contract_plans.crash_capsule(owner, target)?;
    // Completion belongs to the exact source requirement, never its provider.
    // Ordinary calls retain normal receipts; external completion abandons
    // in-domain obligations and demands independently admitted runtime authority.
    phase("call operation: boundary claim transfers");
    let completion = validation::boundary_requirement_completion(program, target);
    let transfers = match completion {
        semantic_vocabulary::BoundaryCompletion::External(_) => {
            checked_trees::CheckedUnitCallTransfers::Abandonment
        }
        semantic_vocabulary::BoundaryCompletion::NormalReturn => {
            checked_trees::CheckedUnitCallTransfers::CompletionReceipts(call_claim_transfers(
                program,
                facts,
                machine.symbol,
                state.symbol,
                call,
                caller_parameters,
                caller_structural_results,
                &structural_arguments,
            )?)
        }
    };
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
        completion,
        scalar_arguments: checked_call_scalar_arguments(
            facts,
            state.symbol,
            coordinate,
            &scalar_parameters,
            true,
        )?,
        structural_arguments,
        result: checked_trees::CheckedUnitCallResult::Unit,
        transfers,
    })
    .map(CheckedUnitEffectOperationPlan::Call)
}
