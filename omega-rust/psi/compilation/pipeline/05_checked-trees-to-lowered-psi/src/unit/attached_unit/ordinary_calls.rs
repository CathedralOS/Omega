//! Shared ordinary call operands, requirements and crash substitution.
use super::super::{
    BTreeMap, MachineId, ObligationId, Proposition, StructuralArgument,
    StructuralParameterDeclaration, StructuralTypeDeclaration, ValueId,
    structural_crash_argument_type, substitute_structural_crash_route_roots,
};
use super::parameters::call_index_selectors;
use super::{
    CheckedTrees, CheckedUnitEffectOperationPlan, ClaimTransfer, LoweringError, OperationResult,
    PlaceId, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, ValueDeclaration,
    allocate_dense, argument_evaluation, lookup_type_id, lower_checked_crash_route_buckets,
    lower_structural_arguments, lower_structural_crash_route_buckets, place_id, primitive_locals,
    unsupported,
};
use crate::direct_calls::{DirectCall, DirectCallResult};
use crate::emission::claim_bindings::emitted_claim_transfers;
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::terminal_identities::StructuralDomainBinding;
use crate::unit::runtime_requirements::substitute_runtime_requirement_scalar_values;

pub(crate) struct Target<'a> {
    pub parameters: &'a [StructuralParameterDeclaration],
    pub scalar_parameters: &'a [ValueDeclaration],
    pub predicate_parameters: &'a [StructuralParameterDeclaration],
    /// The exact `requires` rows the callee's emitted contract carries, in
    /// contract order: closed authored clauses merged ahead of runtime
    /// requirements.
    pub requires: &'a [Proposition],
    pub runtime_requirements: &'a [Proposition],
    pub source_checked_requires: &'a [Proposition],
}

pub(crate) struct PreparedCall {
    pub arguments: Vec<ValueId>,
    /// Proof-only erased actuals in the callee's erased-formal order.
    pub erased_arguments: Vec<semantic_vocabulary::ScalarTerm>,
    /// Erased proof-only actuals in the callee's erased-proof roster order.
    pub erased_proof_arguments: Vec<semantic_vocabulary::ProofTerm>,
    pub structural_arguments: Vec<StructuralArgument>,
    /// The emitted transfer roster: checked rows plus each completed-result
    /// `self` consume's minted claim, in canonical order.
    pub claim_transfers: Vec<ClaimTransfer>,
    pub requirement_obligations: Vec<ObligationId>,
    pub crash_continuations: Vec<terminal_psi::CrashRouteBucket>,
}

impl PreparedCall {
    /// Emit the prepared lanes as one direct call to `callee` through the
    /// single call emitter, for `result` to choose its Terminal operation.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        self,
        operations: &mut OperationBuffer,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        source_target: symbols::SymbolHandle,
        record: crate::direct_calls::CallRecord<'_>,
        result: OperationResult,
        callee: MachineId,
        result_kind: DirectCallResult,
    ) -> Result<semantic_vocabulary::OperationId, LoweringError> {
        crate::direct_calls::emit_direct_call(
            operations,
            coordinate,
            source_site,
            source_target,
            record,
            result,
            DirectCall::new(
                callee,
                self.arguments,
                self.erased_arguments,
                self.erased_proof_arguments,
                self.structural_arguments,
                self.claim_transfers,
                self.requirement_obligations,
                self.crash_continuations,
                result_kind,
            ),
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    target: Target<'_>,
    evaluated_scalar_arguments: Option<&[ValueDeclaration]>,
    evaluated_selectors: Option<&[ValueDeclaration]>,
    caller_scalar_values: &[ValueDeclaration],
    caller_erased_scalar_parameters: &[ValueDeclaration],
    caller_erased_proof_parameters: &[checked_trees::CheckedErasedProofParameterPlan],
    parameters: &[StructuralParameterDeclaration],
    local_places: &[StructuralPlaceDeclaration],
    structural_result_places: &[(Option<StructuralPlaceDeclaration>, bool)],
    primitive_local_places: &[primitive_locals::PrimitiveLocal],
    structural_locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    scalar_bindings: &crate::expression_preparation::bindings::ScalarBindings,
    structural_types: &[StructuralTypeDeclaration],
    call_byte_places: &[PlaceId],
    calls: &mut CallEmissionContext<'_>,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<PreparedCall, LoweringError> {
    // All ordinary result kinds share operand custody and callee contracts.
    // Result formation and publication remain with the operation frame.
    let Some((call, erased_scalar_arguments, erased_proof_arguments)) = operation
        .call()
        .filter(|call| call.is_module())
        .and_then(|call| {
            Some((
                call,
                call.erased_scalar_arguments()?,
                call.erased_proof_arguments()?,
            ))
        })
    else {
        return unsupported("ordinary call preparation has no call operation");
    };
    let target_machine = &call.target.machine;
    let scalar_arguments = &call.scalar_arguments;
    let structural_arguments = &call.structural_arguments;
    let claim_transfers = call.claim_transfers();
    if structural_arguments.len() != target.parameters.len() {
        return unsupported("ordinary call has an unsupported structural operand arity");
    }
    let erased_proof_arguments = erased_proof_arguments
        .iter()
        .map(|argument| {
            crate::scalar_lowering::contracts::checked_proof_term(
                identities,
                checked,
                argument,
                caller_erased_proof_parameters,
            )
            .and_then(|term| {
                crate::scalar_lowering::contracts::lowered_proof_term(
                    checked,
                    &term,
                    caller_scalar_values,
                    caller_erased_scalar_parameters,
                )
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let erased_arguments = erased_scalar_arguments
        .iter()
        .map(|argument| {
            let checked_trees::CheckedCallScalarArgument::Pure(expression) = argument else {
                return unsupported("erased Unit call actual must be a pure checked expression");
            };
            crate::proofs::crash_routes::checked_scalar_term(
                identities,
                expression,
                caller_scalar_values,
                caller_erased_scalar_parameters,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if scalar_arguments.len() != target.scalar_parameters.len() {
        return unsupported("Unit call scalar argument count disagrees with its target");
    }
    let terminal_scalar_values = argument_evaluation::validated_values(
        evaluated_scalar_arguments,
        &target
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.value_type())
            .collect::<Vec<_>>(),
    )?;
    let terminal_scalar_arguments = terminal_scalar_values
        .iter()
        .map(|value| value.id)
        .collect();
    // The argument schedule captures address operands at their authored point;
    // the call consumes those values without rereading their sources.
    let mut selector_values =
        call_index_selectors(structural_arguments, evaluated_selectors)?.into_iter();

    let claim_transfers = emitted_claim_transfers(
        identities,
        claim_transfers,
        calls.claims.bindings(),
        target.parameters,
        structural_types,
    )?;
    let terminal_arguments = lower_structural_arguments(
        identities,
        checked,
        structural_arguments,
        structural_types,
        parameters,
        local_places,
        structural_result_places,
        &operations.structural_values.clone(),
        call_byte_places,
        primitive_local_places,
        structural_locals,
        Some(scalar_bindings),
        Some(caller_scalar_values),
        &mut selector_values,
        Some(calls),
        Some((next_value, operations)),
    )?;

    let crash_continuations = prepare_crash_continuations(
        identities,
        checked,
        SourceCallCoordinate {
            state,
            statement_index: call.coordinate.statement_index as usize,
            call_ordinal: call.coordinate.call_ordinal as usize,
        },
        &operations.loan_captures,
        Target { ..target },
        &crate::unit::effective_crash_routes(checked, *target_machine)?,
        &terminal_scalar_values,
        &terminal_arguments,
        structural_types,
        |argument, index| {
            if call_byte_places.contains(&argument.place) {
                Ok(target.parameters[index].structural_type)
            } else {
                structural_crash_argument_type(
                    argument,
                    parameters,
                    local_places,
                    structural_result_places,
                    primitive_local_places,
                    &operations.operations,
                )
            }
        },
    )?;
    let requirement_obligations = prepare_requirement_obligations(&target, calls, operations)?;
    Ok(PreparedCall {
        arguments: terminal_scalar_arguments,
        erased_arguments,
        erased_proof_arguments,
        structural_arguments: terminal_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    })
}

/// The lanes of a computed module call, prepared once for every route that
/// completes one: an expression scalar binding
/// (`emission/operation_emission/calls.rs`) and a branch-state effect
/// (`scalar_lowering/effects.rs`) both evaluate their scalar actuals first
/// and then owe the callee the same requirement obligations, erased proof
/// terms, structural arguments, crash continuations and claim transfers
/// before `PreparedCall::emit`. A target with a lowered signature takes the
/// signature's contract; a standalone scalar catalog has no signature and
/// keeps its exact requirement count with checked crash routes over the
/// evaluated actuals, which is what a callee without structural formals
/// receives from the signature route as well.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_computed_module_call(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    coordinate: SourceCallCoordinate,
    target_machine: symbols::SymbolHandle,
    arguments: &[ValueDeclaration],
    erased_arguments: Vec<semantic_vocabulary::ScalarTerm>,
    erased_proof_arguments: &[crate::scalar_lowering::contracts::LoweredProofTerm],
    structural_arguments: &[crate::emission::operation_emission::calls::PreparedStructuralArgument],
    claim_transfers: &[crate::emission::claim_bindings::PreparedClaimTransfer],
    crash_routes: &[checked_trees::CrashRouteBucket],
    source_values: &[ValueDeclaration],
    caller_erased_formals: &[ValueDeclaration],
    calls: &mut CallEmissionContext<'_>,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<PreparedCall, LoweringError> {
    let signature = calls
        .signatures
        .iter()
        .find(|signature| signature.source == target_machine);
    let requirement_obligations = if let Some(signature) = signature {
        prepare_requirement_obligations(&signature.call_target(), calls, operations)?
    } else {
        let requirement_count = calls
            .requirement_counts
            .iter()
            .find_map(|(source, count)| (*source == target_machine).then_some(*count))
            .ok_or(LoweringError::Unsupported(
                "computed call target has no prepared contract",
            ))?;
        (0..requirement_count)
            .map(|_| calls.allocate_requirement())
            .collect::<Result<Vec<_>, _>>()?
    };
    let erased_proof_arguments = erased_proof_arguments
        .iter()
        .map(|term| {
            crate::scalar_lowering::contracts::lowered_proof_term(
                checked,
                term,
                source_values,
                caller_erased_formals,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let emitted_structural_arguments = structural_arguments
        .iter()
        .map(|argument| argument.emit(source_values, calls, next_value, operations))
        .collect::<Result<Vec<_>, _>>()?;
    let crash_continuations = match signature {
        Some(signature) => prepare_crash_continuations(
            identities,
            checked,
            coordinate,
            &operations.loan_captures,
            signature.call_target(),
            crash_routes,
            arguments,
            &emitted_structural_arguments,
            calls.structural_types,
            |argument, index| {
                if argument.place == emitted_structural_arguments[index].place {
                    structural_arguments[index].root_type.or_else(|| {
                        argument
                            .path
                            .is_empty()
                            .then_some(signature.parameters[index].structural_type)
                    })
                } else {
                    calls
                        .signatures
                        .iter()
                        .flat_map(|signature| &signature.parameters)
                        .find(|parameter| parameter.place == argument.place)
                        .map(|parameter| parameter.structural_type)
                        .or_else(|| {
                            operations.operations.iter().find_map(|operation| {
                                operation
                                    .result
                                    .structural()
                                    .filter(|result| result.place == argument.place)
                                    .map(|result| result.structural_type)
                            })
                        })
                }
                .ok_or(LoweringError::Unsupported(
                    "computed call crash argument has no physical root carrier",
                ))
            },
        )?,
        None if structural_arguments.is_empty() => {
            lower_checked_crash_route_buckets(identities, crash_routes, arguments)?
        }
        None => return unsupported("computed call with structural operands has no signature"),
    };
    let claim_transfers = crate::emission::claim_bindings::emit_prepared_claim_transfers(
        claim_transfers,
        calls.claims.bindings(),
    )?;
    Ok(PreparedCall {
        arguments: arguments.iter().map(|argument| argument.id).collect(),
        erased_arguments,
        erased_proof_arguments,
        structural_arguments: emitted_structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    })
}

/// Retain the signature's ordered requirement identities and the source
/// checker's exact closed-clause discharge for every ordinary call adapter.
pub(crate) fn prepare_requirement_obligations(
    target: &Target<'_>,
    calls: &mut CallEmissionContext<'_>,
    operations: &mut OperationBuffer,
) -> Result<Vec<ObligationId>, LoweringError> {
    // Retain one identity per published requirement while certificate production
    // remains. The source checker has already discharged authored
    // clauses at this exact call, including projected aggregate observations.
    // Re-searching those decisions after lowering loses source fact transport
    // and duplicates checking. Generated runtime requirements retain their
    // separate decision route until the checker cutover completes.
    target
        .requires
        .iter()
        .map(|requirement| {
            let obligation = calls.allocate_requirement()?;
            if target.source_checked_requires.contains(requirement) {
                operations.source_checked_obligations.push(obligation);
            }
            Ok(obligation)
        })
        .collect()
}

/// The callee's allocated formal namespace and completed actual roots are
/// shared by frame calls and computed calls. Caller adapters only supply the
/// physical argument prefix; none may lower structural guards as scalar terms.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_crash_continuations(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    coordinate: SourceCallCoordinate,
    captures: &[lowered_psi::LoweredSourceLoanCapture],
    target: Target<'_>,
    routes: &[checked_trees::CrashRouteBucket],
    terminal_scalar_values: &[ValueDeclaration],
    terminal_arguments: &[StructuralArgument],
    structural_types: &[StructuralTypeDeclaration],
    mut root_type: impl FnMut(&StructuralArgument, usize) -> Result<StructuralTypeId, LoweringError>,
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    if target.parameters.len() != terminal_arguments.len() {
        return unsupported("ordinary call has an unsupported structural operand arity");
    }
    if target.scalar_parameters.len() != terminal_scalar_values.len() {
        return unsupported("Unit call scalar requirement arity is inconsistent");
    }
    let scalar_substitutions = target
        .scalar_parameters
        .iter()
        .zip(terminal_scalar_values)
        .map(|(formal, actual)| {
            if formal.scalar_type != actual.scalar_type {
                return unsupported("Unit call scalar requirement parameter type is inconsistent");
            }
            Ok((formal.id, *actual))
        })
        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
    let mut crash_continuations = {
        let target_routes = routes.to_vec();
        if target.parameters.is_empty() {
            lower_checked_crash_route_buckets(identities, &target_routes, terminal_scalar_values)?
        } else {
            // Only structural crash substitution reads the callee's runtime
            // requirements, instantiated at this call. A callee without
            // structural formals may keep a requirement over an erased
            // formal, which has no runtime actual to substitute. Preserve
            // callee requirement slots after substitution, even if reordered
            // or equal arguments change canonical term order.
            let target_runtime_requirements = target
                .runtime_requirements
                .iter()
                .map(|requirement| {
                    let mut requirement = requirement.clone();
                    substitute_runtime_requirement_scalar_values(
                        &mut requirement,
                        &scalar_substitutions,
                    )?;
                    Ok(requirement)
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            lower_structural_crash_route_buckets(
                identities,
                &target_routes,
                terminal_scalar_values,
                target.predicate_parameters,
                structural_types,
                &target_runtime_requirements,
            )?
        }
    };
    if !crash_continuations.is_empty() {
        // Only observed formal roots need a canonical crash prefix. An
        // unrelated actual may legitimately contain a runtime selector or a
        // reference projection that the crash predicate never reads.
        substitute_structural_crash_route_roots(&mut crash_continuations, &mut |root| {
            let Some((index, parameter)) = target
                .parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.place == root)
            else {
                return Ok(None);
            };
            let argument = crate::proofs::crash_routes::captured_crash_argument(
                checked,
                coordinate,
                target
                    .predicate_parameters
                    .iter()
                    .find(|predicate_parameter| predicate_parameter.place == parameter.place)
                    .ok_or(LoweringError::Unsupported(
                        "crash observation lost its authored formal position",
                    ))?
                    .position,
                &terminal_arguments[index],
                captures,
                structural_types,
                &mut |argument| root_type(argument, index),
            )?;
            let prefix = crate::proofs::crash_routes::structural_crash_route_prefix_from_type(
                &argument,
                root_type(&argument, index)?,
                structural_types,
            )?;
            Ok(Some((argument.place, prefix)))
        })?;
    }
    Ok(crash_continuations)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_structural(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    prepared: PreparedCall,
    callee: MachineId,
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    domain_ids: &[StructuralDomainBinding],
    claim_bindings: &mut crate::emission::claim_bindings::ClaimBindings<'_>,
    // Graph evaluation attaches the source local and registers this value as
    // one operation; ordinary and nested operands register it here instead.
    register_binding: bool,
    frontier_before_call: &lowered_psi::LoweredSourceFrontierBindings,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let Some((call, result)) = operation
        .call()
        .filter(|call| call.is_module())
        .and_then(|call| Some((call, call.structural_result()?)))
    else {
        return unsupported("structural call emission requires an owned result call");
    };
    let coordinate = &call.coordinate;
    let target_state = &call.target.state;
    let place = place_id(allocate_dense(next_place)?);
    let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
    let returned = super::call_results::prepare(
        identities,
        checked,
        call,
        place,
        structural_type,
        structural_types,
    )?
    .emit(
        identities,
        checked,
        state,
        *coordinate,
        domain_ids,
        structural_types,
        claim_bindings,
    )?;
    // Scalar values and structural custody are separate operand namespaces,
    // not different ownership rules: a mixed call retains the same checked
    // claim and content correspondence as the scalar-free encoding.
    let id = prepared.emit(
        operations,
        SourceCallCoordinate {
            state,
            statement_index: coordinate.statement_index as usize,
            call_ordinal: coordinate.call_ordinal as usize,
        },
        call.source_site().flatten(),
        *target_state,
        crate::direct_calls::CallRecord::Frontier(frontier_before_call),
        OperationResult::Structural(returned.clone()),
        callee,
        DirectCallResult::Structural {
            multiplicity: returned.multiplicity,
        },
    )?;
    if register_binding
        && operations
            .structural_values
            .iter()
            .any(|(ordinal, _)| *ordinal == result.binding_ordinal)
    {
        return unsupported("structural call result was published twice");
    }
    if register_binding {
        operations.bind_structural_value(result.binding_ordinal, returned.into());
    }
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}
