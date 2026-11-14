/*
Receiving structural-foundation validator (`validate_structural_foundation`) run during canonical
`decode_module` admission to verify declaration closure, structural types, places, paths, block
operations, and reference-origin families before re-encoding comparison.
*/

pub(crate) mod call_foundations {

    use super::super::wire::strictly_increasing;
    use super::{
        StructuralArgumentPresentation, callee_exact_payloadless_return, has_service,
        has_structural_type, validate_claim_actions, validate_structural_arguments,
        validate_structural_path, validate_structural_qualifications,
    };
    use crate::codec_error::{CodecError, malformed};
    use semantic_vocabulary::StructuralPlaceKind;
    use std::collections::BTreeSet;
    use terminal_psi::{
        Block, BoundaryDeclarationRole, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
        ClaimTransfer, ModuleInvocationInputs, Operation, OperationKind, OperationResult,
        RegisterCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralPlaceDeclaration, StructuralResultClaimBinding, TerminalDynamicClaimPath,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
        call_structural_pair_pattern, find_boundary_machine,
        structural_type_has_borrowed_storage_by,
    };

    fn validate_call_tail(
        module: &TerminalModule,
        machine: &TerminalMachine,
        structural_arguments: &[StructuralArgument],
        structural_parameters: &[StructuralParameterDeclaration],
        claim_transfers: &[ClaimTransfer],
    ) -> Result<(), CodecError> {
        validate_structural_arguments(
            module,
            machine,
            structural_arguments,
            structural_parameters,
            StructuralArgumentPresentation::Ordinary,
        )?;
        validate_claim_actions(
            machine,
            structural_arguments,
            claim_transfers
                .iter()
                .map(|transfer| (transfer.claim, transfer.argument_index)),
        )?;
        for transfer in claim_transfers {
            let parameter = structural_parameters
                .get(transfer.argument_index as usize)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "claim transfer has no formal parameter",
                ))?;
            validate_structural_path(module, parameter.structural_type, &transfer.path)?;
        }
        Ok(())
    }

    pub(crate) fn validate_module_invocation_entry(
        module: &TerminalModule,
        machine: &TerminalMachine,
        invocation: ModuleInvocationInputs<'_>,
    ) -> Result<(), CodecError> {
        let callee =
            module
                .machine(invocation.callee)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "module invocation references an unknown callee",
                ))?;
        if invocation.arguments.len() != callee.parameters.len()
            || invocation.erased_arguments.len() != callee.contract.erased_scalar_formals.len()
            || invocation.erased_proof_arguments.len() != callee.contract.erased_proof_formals.len()
            || invocation.structural_arguments.len() != callee.structural_parameters.len()
            || invocation.requirement_obligations.len() != callee.contract.requires.len()
        {
            return malformed("module invocation has the wrong entry or obligation arity");
        }
        for (actual, formal) in invocation.arguments.iter().zip(&callee.parameters) {
            let mut declarations = machine
                .parameters
                .iter()
                .copied()
                .chain(machine.block_parameters().copied())
                .chain(
                    machine
                        .operations()
                        .filter_map(|operation| operation.result.scalar()),
                )
                .filter(|declaration| declaration.id == *actual);
            if declarations
                .next()
                .is_none_or(|declaration| declaration.scalar_type != formal.scalar_type)
                || declarations.next().is_some()
            {
                return malformed("module invocation argument has no exact scalar declaration");
            }
        }
        for (actual, formal) in invocation
            .erased_arguments
            .iter()
            .zip(&callee.contract.erased_scalar_formals)
        {
            if actual.scalar_type() != formal.scalar_type {
                return malformed("module invocation erased argument has the wrong carrier");
            }
        }
        if invocation
            .requirement_obligations
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != invocation.requirement_obligations.len()
        {
            return malformed("module invocation repeats an entry obligation identity");
        }
        validate_call_tail(
            module,
            machine,
            invocation.structural_arguments,
            &callee.structural_parameters,
            invocation.claim_transfers,
        )
    }

    pub(crate) fn validate_no_normal_invocation(
        module: &TerminalModule,
        block: &Block,
    ) -> Result<(), CodecError> {
        let Terminator::InvokeNoNormal { callee, .. } = block.terminator else {
            return Ok(());
        };
        match terminal_semantics::control_graph::normal_completion_possible(module, callee) {
            Ok(false) => Ok(()),
            Ok(true) => malformed("no-normal invocation callee has a normal return"),
            Err(_) => malformed("no-normal invocation has no exact closed callee control graph"),
        }
    }

    pub(crate) fn validate_call_unit(
        module: &TerminalModule,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::CallUnit {
            callee,
            structural_arguments,
            ..
        } = &operation.kind
        else {
            unreachable!("dispatched validate_call_unit")
        };
        if operation.result != OperationResult::Unit {
            return malformed("unit call declares a scalar result");
        }
        let Some(callee) = module.machine(*callee) else {
            return malformed("unit call references an unknown callee");
        };
        if callee.result != TerminalMachineResult::Unit
            || structural_arguments.len() != callee.structural_parameters.len()
        {
            return malformed("unit call has the wrong callee result or structural arity");
        }
        Ok(())
    }

    pub(crate) fn validate_call_machine(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::CallMachine {
            callee,
            contract,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            ..
        } = &operation.kind
        else {
            unreachable!("dispatched validate_call_machine")
        };
        let contract = module.callables.contract(*contract).ok_or(
            CodecError::MalformedStructuralFoundation(
                "machine call names an absent callable contract",
            ),
        )?;
        let declaration = |value| {
            let mut declarations = machine
                .parameters
                .iter()
                .copied()
                .chain(machine.block_parameters().copied())
                .chain(
                    machine
                        .operations()
                        .filter_map(|operation| operation.result.scalar()),
                )
                .filter(|declaration| declaration.id == value);
            let declaration = declarations.next();
            if declarations.next().is_some() {
                None
            } else {
                declaration
            }
        };
        if declaration(*callee).map(|value| value.scalar_type)
            != Some(semantic_vocabulary::ScalarType::Callable(contract.id))
        {
            return malformed("machine call operand has no exact callable carrier declaration");
        }
        if !contract.result.matches_operation_result(&operation.result)
            || arguments.len() != contract.scalar_parameters.len()
            || erased_arguments.len() != contract.proof.erased_scalar_formals.len()
            || erased_proof_arguments.len() != contract.proof.erased_proof_formals.len()
            || structural_arguments.len() != contract.structural_parameters.len()
            || requirement_obligations.len() != contract.proof.requires.len()
        {
            return malformed("machine call disagrees with its retained contract shape");
        }
        for (actual, formal) in arguments.iter().zip(&contract.scalar_parameters) {
            if declaration(*actual).map(|value| value.scalar_type) != Some(formal.scalar_type) {
                return malformed("machine call argument has no exact scalar carrier declaration");
            }
        }
        for (actual, formal) in erased_arguments
            .iter()
            .zip(&contract.proof.erased_scalar_formals)
        {
            if actual.scalar_type() != formal.scalar_type {
                return malformed("machine call erased argument changes its carrier");
            }
        }
        if requirement_obligations
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != requirement_obligations.len()
        {
            return malformed("machine call repeats an entry obligation identity");
        }
        for transfer in claim_transfers {
            if !contract.entry_claim_paths.iter().any(|path| {
                path.argument_index == transfer.argument_index && path.path == transfer.path
            }) {
                return malformed("machine call claim transfer changes the retained owned path");
            }
        }
        if let Some(result) = operation.result.structural() {
            validate_structural_call_result_declaration(module, machine, operation, result)?;
            validate_result_claim_shape(module, result, std::iter::empty())?;
        }
        validate_call_tail(
            module,
            machine,
            structural_arguments,
            &contract.structural_parameters,
            claim_transfers,
        )
    }

    pub(crate) fn validate_call_dynamic(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::CallDynamic {
            interface,
            slot,
            arguments,
            family_actuals,
            structural_arguments,
            claim_transfers,
            ..
        } = &operation.kind
        else {
            unreachable!("dispatched validate_call_dynamic")
        };
        let Some(interface_slots) = module
            .dynamic_dispatch
            .interface(*interface)
            .map(|interface| interface.slots.as_slice())
        else {
            return malformed("dynamic call names an unknown interface slot");
        };
        let Some(shape) = usize::try_from(*slot)
            .ok()
            .and_then(|index| interface_slots.get(index))
        else {
            return malformed("dynamic call names an unknown interface slot");
        };
        if !family_actuals.is_empty() {
            let mut roster = Vec::new();
            for candidate in interface_slots
                .iter()
                .skip(usize::try_from(*slot).unwrap_or(usize::MAX))
            {
                if candidate.family_tuple.is_empty()
                    || candidate.requirement_identity != shape.requirement_identity
                {
                    break;
                }
                roster.push(&candidate.family_tuple);
            }
            if roster.is_empty()
                || roster
                    .iter()
                    .any(|tuple| tuple.len() != family_actuals.len())
                || {
                    let mut ordered = roster.clone();
                    ordered.sort();
                    ordered.windows(2).any(|pair| pair[0] >= pair[1])
                }
            {
                return malformed("dynamic family call disagrees with its slot roster");
            }
        } else if !shape.family_tuple.is_empty() {
            return malformed("dynamic family call carries no family actuals");
        }
        if !shape.matches_operation_result(&operation.result)
            || arguments.len() != shape.scalar_parameters.len()
            || structural_arguments.len() != shape.structural_parameters.len()
        {
            return malformed("dynamic call disagrees with its slot's call shape");
        }
        if !strictly_increasing(shape.result_claim_paths.iter()) {
            return malformed("dynamic result claim paths are not canonical");
        }
        if let Some(result) = operation.result.structural() {
            validate_structural_call_result_declaration(module, machine, operation, result)?;
            validate_result_claim_shape(module, result, std::iter::empty())?;
        }
        if !strictly_increasing(shape.entry_claim_paths.iter()) {
            return malformed("dynamic slot claim paths are not canonical");
        }
        for (position, expected) in shape.entry_claim_paths.iter().enumerate() {
            let Some(formal) = shape
                .structural_parameters
                .get(expected.argument_index as usize)
            else {
                return malformed("dynamic slot claim has no structural formal");
            };
            if formal.access != StructuralAccess::Owned
                || formal.multiplicity == StructuralMultiplicity::Unrestricted
                || (expected.path.is_empty()
                    && formal.multiplicity != StructuralMultiplicity::Linear)
                || expected.path.iter().any(|segment| {
                    !matches!(
                        segment,
                        StructuralPathSegment::Field(_)
                            | StructuralPathSegment::Case(_)
                            | StructuralPathSegment::FixedIndex(_)
                    )
                })
                || shape.entry_claim_paths[..position].iter().any(|prior| {
                    prior.argument_index == expected.argument_index
                        && expected.path.starts_with(&prior.path)
                })
            {
                return malformed("dynamic slot claim is not an independent owned path");
            }
            validate_structural_path(module, formal.structural_type, &expected.path)?;
        }
        if shape
            .structural_parameters
            .iter()
            .enumerate()
            .any(|(argument_index, formal)| {
                formal.access == StructuralAccess::Owned
                    && formal.multiplicity == StructuralMultiplicity::Linear
                    && !shape
                        .entry_claim_paths
                        .iter()
                        .any(|claim| claim.argument_index as usize == argument_index)
            })
        {
            return malformed("dynamic slot linear formal has no entry claim path");
        }
        for transfer in claim_transfers {
            if !shape.entry_claim_paths.iter().any(|expected| {
                expected.argument_index == transfer.argument_index && expected.path == transfer.path
            }) {
                return malformed("dynamic call claim path disagrees with its slot");
            }
        }
        for row in module
            .dynamic_dispatch
            .tables
            .iter()
            .filter(|table| table.interface == *interface)
            .filter_map(|table| table.rows.get(*slot as usize).copied().flatten())
        {
            let Some(callee) = module.machine(row) else {
                return malformed("dynamic table row references an unknown machine");
            };
            if !shape.result.matches_machine_result(&callee.result) {
                return malformed("dynamic row result disagrees with its slot");
            }
            if let Some(result) = callee.result.structural() {
                if !terminal_semantics::reference_result_refines_regions(
                    module.structural_types.iter(),
                    &callee.structural_parameters,
                    result,
                    &shape.result_reference_regions,
                ) {
                    return malformed(
                        "dynamic row returned origins exceed its public input regions",
                    );
                }
                if !terminal_semantics::result_claim_paths_match_returns(
                    &shape.result_claim_paths,
                    callee
                        .blocks
                        .iter()
                        .filter_map(|block| match &block.terminator {
                            Terminator::ReturnStructural {
                                returned_claims, ..
                            } => Some(returned_claims.as_slice()),
                            _ => None,
                        }),
                ) {
                    return malformed("dynamic row return claims disagree with its slot");
                }
            }
            let mut entry_claim_paths = callee
                .entry_claims
                .iter()
                .map(|claim| {
                    Some(TerminalDynamicClaimPath {
                        argument_index: u32::try_from(
                            callee
                                .structural_parameters
                                .iter()
                                .position(|parameter| parameter.place == claim.input)?,
                        )
                        .ok()?,
                        path: claim.path.clone(),
                    })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "dynamic row entry claim has no parameter",
                ))?;
            entry_claim_paths.sort();
            if entry_claim_paths != shape.entry_claim_paths {
                return malformed("dynamic row claim frontier disagrees with its slot");
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                StructuralArgumentPresentation::Ordinary,
            )?;
        }
        validate_claim_actions(
            machine,
            structural_arguments,
            claim_transfers
                .iter()
                .map(|transfer| (transfer.claim, transfer.argument_index)),
        )?;
        Ok(())
    }

    pub(crate) fn validate_call_structural_scalar(
        module: &TerminalModule,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::CallStructuralScalar {
            callee,
            arguments,
            structural_arguments,
            ..
        } = &operation.kind
        else {
            unreachable!("dispatched validate_call_structural_scalar")
        };
        let Some(callee) = module.machine(*callee) else {
            return malformed("structural scalar call references an unknown callee");
        };
        if arguments.len() != callee.parameters.len()
            || operation.result.scalar().map(|result| result.scalar_type)
                != callee.result.scalar().map(|result| result.scalar_type)
            || operation.result == OperationResult::Unit
            || structural_arguments.len() != callee.structural_parameters.len()
        {
            return malformed(
                "structural scalar call has the wrong callee signature or structural arity",
            );
        }
        Ok(())
    }

    pub(crate) fn validate_call_structural(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let (call_structural_pair_pattern!(
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        )) = &operation.kind
        else {
            unreachable!("dispatched validate_call_structural")
        };
        let arguments = match &operation.kind {
            OperationKind::CallStructuralWithScalarArguments { arguments, .. } => {
                arguments.as_slice()
            }
            _ => &[],
        };
        let selected_evidence = match &operation.kind {
            call_structural_pair_pattern!(selected_evidence, ..) => selected_evidence.as_slice(),
            _ => &[],
        };
        let Some(callee) = module.machine(*callee) else {
            return malformed("structural call references an unknown callee");
        };
        let Some(expected_result) = callee.result.structural() else {
            return malformed("structural call references a non-structural-result callee");
        };
        let Some(actual_result) = operation.result.structural() else {
            return malformed("structural call has no structural operation result");
        };
        let exact_payloadless = callee.structural_parameters.iter().all(|parameter| {
            parameter.access == StructuralAccess::Owned
                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && !structural_type_has_borrowed_storage_by(
                    |id| module.structural_type(id),
                    parameter.structural_type,
                )
        }) && callee.entry_claims.is_empty()
            && callee.content_entry_claims.is_empty()
            && callee.contract.requires.is_empty()
            && callee.contract.ensures.is_empty()
            && callee.contract.crash_routes.is_empty()
            && module
                .evidence_contract_lanes
                .iter()
                .all(|lane| lane.owner != terminal_psi::EvidenceCallableOwner::Machine(callee.id))
            && claim_transfers.is_empty()
            && requirement_obligations.is_empty()
            && crash_continuations.is_empty()
            && actual_result.multiplicity == StructuralMultiplicity::Unrestricted
            && expected_result.multiplicity == StructuralMultiplicity::Unrestricted
            && actual_result.qualifications.is_empty()
            && expected_result.qualifications.is_empty()
            && actual_result.claims.is_empty()
            && callee_exact_payloadless_return(callee);
        if arguments.len() != callee.parameters.len()
            || structural_arguments.len() != callee.structural_parameters.len()
            || (!selected_evidence.is_empty() && !exact_payloadless)
            || actual_result.structural_type != expected_result.structural_type
            || actual_result.multiplicity != expected_result.multiplicity
            || actual_result.qualifications != expected_result.qualifications
            || !expected_result
                .projected_qualifications
                .iter()
                .all(|row| actual_result.projected_qualifications.contains(row))
        {
            return malformed("structural call has the wrong callee signature or structural arity");
        }
        if selected_evidence.iter().any(|binding| {
            !callee.contract.outcome_specific_ensures.iter().any(|row| {
                row.guard == binding.guard
                    && row.position == binding.position
                    && row.obligation == binding.callee_obligation
                    && row.proposition
                        == semantic_vocabulary::Proposition::Atom(binding.callee_proposition)
                    && row.evidence.as_ref().is_some_and(|evidence| {
                        evidence.term == binding.callee_term
                            && evidence.output_field == binding.output_field
                            && evidence.argument_subjects == binding.argument_substitutions
                    })
            })
        }) {
            return malformed(
                "guarded call argument substitution disagrees with its callee declaration",
            );
        }
        if actual_result
            .projected_qualifications
            .iter()
            .any(|row| !expected_result.projected_qualifications.contains(row))
        {
            let preserved = terminal_semantics::ReferenceResultMemberships::new(module)
                .for_operation(machine, operation)
                .map_err(CodecError::MalformedStructuralFoundation)?;
            let unpreserved = actual_result
                .projected_qualifications
                .iter()
                .filter(|row| {
                    !expected_result.projected_qualifications.contains(row)
                        && !preserved.contains(row)
                })
                .collect::<Vec<_>>();
            if !unpreserved.is_empty() {
                return Err(CodecError::MalformedStructuralOperation {
                    machine: format!("{:?}", machine.id),
                    operation: operation.id,
                    reason: format!(
                        "structural call adds an unpreserved caller membership: callee {:?} {:?}, extra {unpreserved:?}, declared {:?}, preserved {preserved:?}",
                        callee.id,
                        callee.declaration.as_ref().and_then(|declaration| module
                            .identities
                            .text(declaration.machine.identity())),
                        expected_result.projected_qualifications,
                    ),
                });
            }
        }
        validate_structural_call_result_declaration(module, machine, operation, actual_result)?;
        if !exact_payloadless && requirement_obligations.len() != callee.contract.requires.len() {
            return malformed("structural call obligations disagree with its callee contract");
        }
        validate_result_claim_shape(
            module,
            actual_result,
            callee
                .blocks
                .iter()
                .filter_map(|block| match &block.terminator {
                    Terminator::ReturnStructural {
                        returned_claims, ..
                    } => Some(returned_claims.as_slice()),
                    _ => None,
                }),
        )?;
        if exact_payloadless {
            return Ok(());
        }
        Ok(())
    }

    fn validate_structural_call_result_declaration(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
        actual_result: &StructuralOperationResult,
    ) -> Result<(), CodecError> {
        let Some(StructuralPlaceDeclaration {
            kind:
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                },
            ..
        }) = machine.structural_place(actual_result.place)
        else {
            return malformed("structural call result has no operation-result place declaration");
        };
        if *producer != operation.id || *structural_type != actual_result.structural_type {
            return malformed("structural call result place disagrees with its producer");
        }
        if !has_structural_type(module, actual_result.structural_type) {
            return malformed("structural call result has an unknown structural type");
        }
        validate_structural_qualifications(
            module,
            actual_result.structural_type,
            &actual_result.qualifications,
            &actual_result.projected_qualifications,
        )?;
        Ok(())
    }

    fn validate_result_claim_shape<'returns>(
        module: &TerminalModule,
        actual_result: &StructuralOperationResult,
        returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
    ) -> Result<(), CodecError> {
        for binding in &actual_result.claims {
            validate_structural_path(module, actual_result.structural_type, &binding.path)?;
        }
        terminal_semantics::validate_result_claim_catalog(
            module.structural_types.iter(),
            actual_result.structural_type,
            &actual_result.claims,
            returns,
        )
        .map_err(|error| {
            CodecError::MalformedStructuralFoundation(match error {
                terminal_semantics::ResultClaimCatalogError::NonStaticOwnedPath => {
                    "structural call result claim is not static owned storage"
                }
                terminal_semantics::ResultClaimCatalogError::OverlappingPaths => {
                    "structural call result has overlapping claim paths"
                }
                terminal_semantics::ResultClaimCatalogError::ReturnPathMismatch => {
                    "structural callee return disagrees with the caller's result claims"
                }
            })
        })?;
        let caller_result_claims = actual_result
            .claims
            .iter()
            .map(|binding| binding.claim)
            .collect::<BTreeSet<_>>();
        let whole_root_claim =
            matches!(actual_result.claims.as_slice(), [binding] if binding.path.is_empty());
        if caller_result_claims.len() != actual_result.claims.len()
            || match actual_result.multiplicity {
                StructuralMultiplicity::Linear => !whole_root_claim,
                StructuralMultiplicity::Affine => actual_result
                    .claims
                    .iter()
                    .any(|binding| binding.path.is_empty()),
                StructuralMultiplicity::Unrestricted => !actual_result.claims.is_empty(),
            }
        {
            return malformed("structural call result claims disagree with its multiplicity");
        }
        Ok(())
    }

    pub(crate) fn validate_external_completion(
        module: &TerminalModule,
        machine: &TerminalMachine,
        block: &Block,
    ) -> Result<(), CodecError> {
        let Terminator::ExternalCompletion {
            boundary,
            kind,
            arguments,
            structural_arguments,
            requirement_obligations,
            ..
        } = &block.terminator
        else {
            return Ok(());
        };
        let Some(declaration) = find_boundary_machine(&module.boundary_machines, *boundary) else {
            return malformed("external completion references an unknown boundary");
        };
        if declaration.role != BoundaryDeclarationRole::Callable
            || declaration.completion != terminal_psi::BoundaryCompletion::External(*kind)
            || declaration.result != BoundaryMachineResult::Unit
            || declaration.scalar_parameters.len() != 1
            || !matches!(declaration.scalar_parameters.first(),
                Some(semantic_vocabulary::ScalarType::Integer(integer))
                    if integer.sign() == semantic_vocabulary::IntegerSign::Signed
                        && integer.bits() == 32 && !integer.is_address())
            || !declaration.structural_parameters.is_empty()
        {
            return malformed(
                "external completion disagrees with the canonical process-exit signature",
            );
        }
        if arguments.len() != declaration.scalar_parameters.len()
            || structural_arguments.len() != declaration.structural_parameters.len()
            || requirement_obligations.len() != declaration.scalar_requires.len()
        {
            return malformed("external completion has the wrong argument or obligation arity");
        }
        super::scalar_boundary_contracts::validate_arguments(
            module,
            machine,
            declaration,
            arguments,
        )?;
        for (argument, expected) in arguments.iter().zip(&declaration.scalar_parameters) {
            let mut declarations = machine
                .parameters
                .iter()
                .copied()
                .chain(block.parameters.iter().copied())
                .chain(
                    block
                        .operations
                        .iter()
                        .filter_map(|operation| operation.result.scalar()),
                )
                .filter(|candidate| candidate.id == *argument);
            if declarations
                .next()
                .is_none_or(|candidate| candidate.scalar_type != *expected)
                || declarations.next().is_some()
            {
                return malformed(
                    "external completion status has no exact runtime scalar declaration",
                );
            }
        }
        validate_structural_arguments(
            module,
            machine,
            structural_arguments,
            &declaration.structural_parameters,
            StructuralArgumentPresentation::Boundary,
        )
    }

    pub(crate) fn validate_boundary_call(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            requirement_obligations,
        } = &operation.kind
        else {
            unreachable!("dispatched validate_boundary_call")
        };
        let Some(boundary) = find_boundary_machine(&module.boundary_machines, *boundary) else {
            return malformed("boundary call references an unknown boundary");
        };
        if boundary.completion != semantic_vocabulary::BoundaryCompletion::NormalReturn {
            return malformed("external completion requires a closed terminal invocation");
        }
        if boundary.role != BoundaryDeclarationRole::Callable {
            return malformed("boundary call references contract evidence rather than a callable");
        }
        let actual_result = match &operation.result {
            OperationResult::Unit => Some(BoundaryMachineResult::Unit),
            OperationResult::Scalar(result) => {
                Some(BoundaryMachineResult::Scalar(result.value_type()))
            }
            OperationResult::Structural(result) => {
                validate_result_claim_shape(module, result, std::iter::empty())?;
                Some(BoundaryMachineResult::Structural(
                    BoundaryStructuralResultDeclaration {
                        structural_type: result.structural_type,
                        multiplicity: result.multiplicity,
                        qualifications: result.qualifications.clone(),
                        projected_qualifications: result.projected_qualifications.clone(),
                    },
                ))
            }
        };
        if actual_result.as_ref() != Some(&boundary.result) {
            return malformed("boundary call result disagrees with its declaration");
        }
        if let OperationResult::Structural(result) = &operation.result {
            let paths = result
                .claims
                .iter()
                .map(|row| row.path.clone())
                .collect::<Vec<_>>();
            for candidate in module
                .provider_candidates
                .iter()
                .filter(|candidate| candidate.boundary == boundary.id)
            {
                let Some(provider) = module.machine(candidate.candidate) else {
                    return malformed("boundary result provider has no machine");
                };
                if !terminal_semantics::result_claim_paths_match_returns(
                    &paths,
                    provider
                        .blocks
                        .iter()
                        .filter_map(|block| match &block.terminator {
                            Terminator::ReturnStructural {
                                returned_claims, ..
                            } => Some(returned_claims.as_slice()),
                            _ => None,
                        }),
                ) {
                    return malformed(
                        "boundary provider return claims disagree with its result paths",
                    );
                }
            }
        }
        if arguments.len() != boundary.scalar_parameters.len() {
            return malformed("boundary call has the wrong scalar arity");
        }
        super::scalar_boundary_contracts::validate_arguments(module, machine, boundary, arguments)?;
        if structural_arguments.len() != boundary.structural_parameters.len() {
            return malformed("boundary call has the wrong structural arity");
        }
        if requirement_obligations.len() != boundary.scalar_requires.len() {
            return malformed("boundary call has the wrong requirement obligation arity");
        }
        validate_structural_arguments(
            module,
            machine,
            structural_arguments,
            &boundary.structural_parameters,
            StructuralArgumentPresentation::Boundary,
        )?;
        validate_claim_actions(
            machine,
            structural_arguments,
            completion_receipts
                .iter()
                .map(|settlement| (settlement.claim, settlement.argument_index)),
        )?;
        for receipt in completion_receipts {
            let parameter = boundary
                .structural_parameters
                .get(receipt.argument_index as usize)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "completion receipt has no structural formal",
                ))?;
            validate_structural_path(module, parameter.structural_type, &receipt.path)?;
        }
        Ok(())
    }

    pub(crate) fn validate_port_write(
        module: &TerminalModule,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::PortWrite { service, .. } = &operation.kind else {
            unreachable!("dispatched validate_port_write")
        };
        if operation.result != OperationResult::Unit {
            return malformed("port write declares a scalar result");
        }
        if !has_service(module, *service) {
            return malformed("port write references an unknown service");
        }
        Ok(())
    }

    pub(crate) fn validate_register_access(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::RegisterAccess { service, event } = &operation.kind else {
            return malformed("register validator received another operation");
        };
        if !event.operation().is_admitted()
            || service.is_some() != event.operation().requires_service_authority()
            || service.is_some_and(|service| !has_service(module, service))
        {
            return malformed("register access disagrees with its catalog service authority");
        }
        let matches_carrier = |scalar, carrier: RegisterCarrier| {
            matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer)
                if !integer.is_address() && integer.bits() == carrier.bits()
                    && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned)
        };
        let signature = event.operation();
        let result_matches = match (signature.result_carrier(), operation.result.scalar()) {
            (None, None) => operation.result == OperationResult::Unit,
            (Some(carrier), Some(result)) => matches_carrier(result.scalar_type, carrier),
            _ => false,
        };
        if !result_matches
            || event
                .inputs()
                .zip(signature.input_carriers())
                .any(|(value, carrier)| {
                    super::declared_scalar_type(machine, *value)
                        .is_none_or(|scalar| !matches_carrier(scalar, *carrier))
                })
        {
            return malformed("register access disagrees with its exact scalar signature");
        }
        Ok(())
    }

    pub(crate) fn validate_outcome_subjects(
        module: &TerminalModule,
        machine: &TerminalMachine,
    ) -> Result<(), CodecError> {
        for row in &machine.contract.outcome_specific_ensures {
            let Some(evidence) = &row.evidence else {
                continue;
            };
            let semantic_vocabulary::Proposition::Atom(proposition) = row.proposition else {
                return malformed("guarded evidence subjects require a nominal proposition");
            };
            let Some(application) = module
                .proposition_applications
                .iter()
                .find(|application| application.id == proposition)
            else {
                return malformed("guarded evidence subjects have no proposition application");
            };
            let mut previous = None;
            for subject in &evidence.argument_subjects {
                if previous.is_some_and(|position| position >= subject.argument_position)
                    || usize::try_from(subject.argument_position)
                        .ok()
                        .is_none_or(|position| position >= application.arguments.len())
                {
                    return malformed(
                        "guarded evidence subjects repeat or exceed their proposition argument positions",
                    );
                }
                previous = Some(subject.argument_position);
                let valid = match &subject.structural_path {
                    None => usize::try_from(subject.parameter_position)
                        .ok()
                        .is_some_and(|position| position < machine.parameters.len()),
                    Some(path) => usize::try_from(subject.parameter_position)
                        .ok()
                        .and_then(|position| machine.structural_parameters.get(position))
                        .is_some_and(|parameter| {
                            !path.is_empty()
                                && path
                                    .iter()
                                    .all(|step| matches!(step, StructuralPathSegment::Field(_)))
                                && terminal_semantics::primitive_read_projection_type(
                                    module.structural_types.iter(),
                                    parameter.structural_type,
                                    path,
                                )
                                .is_some()
                        }),
                };
                if !valid {
                    return malformed(
                        "guarded evidence subject has no exact formal parameter projection",
                    );
                }
            }
        }
        Ok(())
    }
}
pub(crate) mod declaration_foundations {

    use super::{
        has_service, has_structural_type, require_known_services, validate_service_parent_graph,
        validate_structural_parameters, validate_structural_qualifications,
    };
    use std::collections::BTreeMap;

    use super::super::wire::strictly_increasing;
    use crate::codec_error::{CodecError, malformed};
    use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, StructuralPlaceKind};
    use terminal_psi::TerminalModule;
    use terminal_psi::{BoundaryMachineDeclaration, ProviderCandidateConformance, TerminalMachine};
    use terminal_semantics::{domain_predicate_is_closed, qualification_path_carrier};

    pub(crate) mod calling_signatures {

        use crate::codec_error::{CodecError, malformed};
        use terminal_psi::{
            BoundaryMachineResult, BoundaryParameterKind, CallingSignatureTelescope,
            StructuralAccess, StructuralTypeShape, TerminalModule,
        };

        pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
            super::opaque_representations::validate(module)?;
            validate_census(module)?;
            super::super::super::wire::require_increasing(
                module.calling_signatures.iter().map(|row| row.boundary),
                "calling signatures by boundary declaration",
            )?;
            for binding in &module.calling_signatures {
                let boundary = module.boundary_machine(binding.boundary).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "calling signature lost its exact boundary",
                    ),
                )?;
                if boundary.declaration.is_none() {
                    return malformed("calling signature lacks its checked declaration");
                }
                let application = match &binding.application {
                    Ok(application) => *application,
                    Err(reason) if !reason.is_empty() => continue,
                    Err(_) => return malformed("calling signature has an empty deferred refusal"),
                };
                let declaration = module.calling_abi_signature(application).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "calling signature lost its declaration ABI application",
                    ),
                )?;
                let Ok(storage) = &declaration.storage else {
                    continue;
                };
                if storage.parameters.len() != boundary.parameter_order.len() {
                    return malformed(
                        "calling telescope differs from its exact boundary parameter roster",
                    );
                }
                let mut scalars = boundary.scalar_parameters.iter();
                let mut structural = boundary.structural_parameters.iter();
                for (kind, formal) in boundary.parameter_order.iter().zip(&storage.parameters) {
                    match kind {
                        BoundaryParameterKind::Scalar => {
                            let scalar =
                                scalars
                                    .next()
                                    .ok_or(CodecError::MalformedStructuralFoundation(
                                        "calling signature lost a scalar formal",
                                    ))?;
                            if formal.access != StructuralAccess::Owned
                                || !matches!(module.structural_type(formal.structural_type).map(|row| &row.shape), Some(StructuralTypeShape::PrimitiveScalar(actual)) if actual == scalar)
                            {
                                return malformed("calling signature substitutes a scalar carrier");
                            }
                        }
                        BoundaryParameterKind::Structural => {
                            let parameter = structural.next().ok_or(
                                CodecError::MalformedStructuralFoundation(
                                    "calling signature lost a structural formal",
                                ),
                            )?;
                            if formal.structural_type != parameter.structural_type
                                || formal.access != parameter.access
                            {
                                return malformed(
                                    "calling signature substitutes a structural carrier or access",
                                );
                            }
                        }
                    }
                }
                match (&boundary.result, storage.result) {
                    (BoundaryMachineResult::Unit, None) => {}
                    (BoundaryMachineResult::Scalar(result), Some(identity)) if matches!(module.structural_type(identity).map(|row| &row.shape), Some(StructuralTypeShape::PrimitiveScalar(actual)) if *actual == result.scalar_type) =>
                        {}
                    (BoundaryMachineResult::Structural(result), Some(identity))
                        if identity == result.structural_type => {}
                    _ => {
                        return malformed("calling signature substitutes its exact result carrier");
                    }
                }
            }
            Ok(())
        }

        fn validate_census(module: &TerminalModule) -> Result<(), CodecError> {
            super::super::super::wire::require_increasing(
                module.calling_abi_signatures.iter().map(|row| row.id),
                "declaration ABI applications",
            )?;
            for row in &module.calling_abi_signatures {
                if row
                    .physical_owner
                    .as_ref()
                    .is_err_and(|reason| reason.is_empty())
                {
                    return malformed("declaration ABI has an empty physical-owner refusal");
                }
                let requirement = module
                    .identities
                    .text(row.requirement_identity)
                    .filter(|text| !text.is_empty())
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "declaration ABI lost its exact requirement identity",
                    ))?;
                let application = module.identities.text(row.declaring_application).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "declaration ABI lost its declaring application",
                    ),
                )?;
                let mut operands = Vec::with_capacity(row.operands.len());
                for operand in &row.operands {
                    let identity = module
                        .identities
                        .text(operand.identity)
                        .filter(|text| !text.is_empty())
                        .ok_or(CodecError::MalformedStructuralFoundation(
                            "declaration ABI lost a static operand",
                        ))?;
                    operands.push((operand.ordinal, operand.kind, identity));
                }
                if row
                    .operands
                    .windows(2)
                    .any(|pair| pair[0].ordinal >= pair[1].ordinal)
                    || semantic_vocabulary::CallingSignatureId::for_application(
                        requirement,
                        application,
                        operands,
                    ) != row.id
                {
                    return malformed(
                        "declaration ABI application key differs from its complete static operands",
                    );
                }
                for (ordinal, formal) in row.formals.iter().enumerate() {
                    if usize::try_from(formal.ordinal).ok() != Some(ordinal)
                        || formal.callback.is_some()
                            && formal.kind != semantic_vocabulary::CallingOperandKind::Machine
                    {
                        return malformed(
                            "declaration ABI generic formal roster is not declaration ordered",
                        );
                    }
                }
                let storage = match &row.storage {
                    Ok(storage) => storage,
                    Err(reason) if !reason.is_empty() => continue,
                    Err(_) => return malformed("declaration ABI has an empty deferred refusal"),
                };
                if application.is_empty()
                    || storage.parameters.len() != storage.telescope.parameters.len()
                    || storage.parameters.iter().any(|parameter| {
                        module.structural_type(parameter.structural_type).is_none()
                            && !(parameter.access != StructuralAccess::Owned
                                && parameter.callback_backing.is_none()
                                && module
                                    .foreign_reference_descriptor(parameter.structural_type)
                                    .is_some())
                    })
                    || storage
                        .result
                        .is_some_and(|identity| module.structural_type(identity).is_none())
                {
                    return malformed("declaration ABI storage lost an exact type or formal");
                }
                for parameter in &storage.parameters {
                    let backing = parameter.structural_type;
                    match parameter.callback_backing {
                        Some(actual)
                            if actual == backing && module.structural_type(actual).is_some() => {}
                        None if parameter.access != StructuralAccess::Owned
                            && module.structural_type(backing).is_none()
                            && module
                                .structural_referents
                                .iter()
                                .any(|row| row.id == backing && row.abstract_opaque) => {}
                        _ => {
                            return malformed(
                                "declaration ABI substitutes or omits its complete callback backing",
                            );
                        }
                    }
                }
                let required = row
                    .formals
                    .iter()
                    .filter(|formal| formal.callback.is_none())
                    .collect::<Vec<_>>();
                if required.len() != row.operands.len()
                    || required.iter().zip(&row.operands).any(|(formal, actual)| {
                        formal.ordinal != actual.ordinal || formal.kind != actual.kind
                    })
                {
                    return malformed(
                        "declaration ABI omits or substitutes a non-callback static operand",
                    );
                }
                for (machine_ordinal, formal) in row
                    .formals
                    .iter()
                    .filter(|formal| {
                        formal.kind == semantic_vocabulary::CallingOperandKind::Machine
                    })
                    .enumerate()
                {
                    let Some(callback) = formal.callback else {
                        continue;
                    };
                    if storage
                        .telescope
                        .binders
                        .iter()
                        .filter(|binder| binder.binder == callback)
                        .count()
                        != 1
                    {
                        return malformed(
                            "declaration ABI generic callback formal lost its universal binder",
                        );
                    }
                    if storage.telescope.binders.iter().any(|binder| {
                        binder.binder == callback
                            && usize::try_from(binder.static_machine_ordinal).ok()
                                != Some(machine_ordinal)
                    }) {
                        return malformed(
                            "declaration ABI callback binder has the wrong declared machine ordinal",
                        );
                    }
                }
                for binder in &storage.telescope.binders {
                    if row
                        .formals
                        .iter()
                        .filter(|formal| formal.callback == Some(binder.binder))
                        .count()
                        != 1
                    {
                        return malformed(
                            "declaration ABI callback binder lost its declared generic formal",
                        );
                    }
                }
                validate_telescope(module, &storage.telescope)?;
            }
            for row in &module.service_schemas {
                module
                    .service_schema(row.declaration, row.application)
                    .map_err(|_| {
                        CodecError::MalformedStructuralFoundation(
                            "invalid canonical service schema",
                        )
                    })?;
            }
            for row in &module.program_entry_contracts {
                if row.target.is_empty() {
                    return malformed("entry contract has an empty target");
                }
                module.program_entry_contract(&row.target).map_err(|_| {
                    CodecError::MalformedStructuralFoundation(
                        "invalid canonical program entry contract",
                    )
                })?;
            }
            for row in &module.provider_selections {
                if row.target.is_empty() {
                    return malformed("selected provider has an empty target");
                }
                module
                    .provider_selection(&row.target, row.schema_declaration, row.schema_application)
                    .map_err(|_| {
                        CodecError::MalformedStructuralFoundation(
                            "invalid canonical provider selection",
                        )
                    })?;
            }
            for request in &module.calling_policy_requests {
                match &request.declaration {
                    Ok(identity)
                        if module
                            .identities
                            .text(*identity)
                            .is_some_and(|text| !text.is_empty()) => {}
                    Err(reason) if !reason.is_empty() => {}
                    _ => {
                        return malformed(
                            "calling policy request lost its exact declaring owner or refusal",
                        );
                    }
                }
                let instances = match &request.instances {
                    Ok(instances) => instances,
                    Err(reasons)
                        if !reasons.is_empty()
                            && reasons.iter().all(|reason| !reason.is_empty()) =>
                    {
                        continue;
                    }
                    Err(_) => {
                        return malformed("calling policy request has an empty deferred refusal");
                    }
                };
                for instance in instances {
                    match &instance.policy_declaration {
                        Ok(declaration)
                            if module
                                .scalar_applications
                                .proof_scope
                                .declaration(*declaration)
                                .is_some() =>
                        {
                            if let Ok(callable) = instance.policy
                                && module
                                    .scalar_applications
                                    .proof_scope
                                    .callable_declaration(callable)
                                    != Some(*declaration)
                            {
                                return malformed(
                                    "calling policy executable differs from its checked declaration",
                                );
                            }
                        }
                        Err(reason) if !reason.is_empty() => {}
                        _ => {
                            return malformed(
                                "calling policy instance lost its checked declaration or refusal",
                            );
                        }
                    }
                    if module
                        .identities
                        .text(instance.declaring_application)
                        .is_none_or(str::is_empty)
                    {
                        return malformed("calling policy instance lost its exact application");
                    }
                    match instance.policy {
                        Ok(terminal_psi::TargetMachineCallable::Machine(identity))
                            if module.machine(identity).is_some() => {}
                        Ok(terminal_psi::TargetMachineCallable::Boundary(identity))
                            if module.boundary_machine(identity).is_some() => {}
                        Err(ref reason) if !reason.is_empty() => {}
                        _ => {
                            return malformed(
                                "calling policy instance lost its exact callable or refusal",
                            );
                        }
                    }
                    match &instance.signatures {
                        Ok(signatures)
                            if signatures.iter().all(|identity| {
                                module.calling_abi_signature(*identity).is_some()
                            }) => {}
                        Err(reason) if !reason.is_empty() => {}
                        _ => {
                            return malformed(
                                "calling policy instance lost its ordered declaration ABI applications",
                            );
                        }
                    }
                }
            }
            Ok(())
        }

        fn validate_telescope(
            module: &TerminalModule,
            telescope: &CallingSignatureTelescope,
        ) -> Result<(), CodecError> {
            let mut positions =
                Vec::with_capacity(telescope.parameters.len() + telescope.direct_callbacks.len());
            for formal in &telescope.parameters {
                positions.push((formal.native_ordinal, formal.identity));
            }
            if telescope
                .parameters
                .windows(2)
                .any(|pair| pair[0].native_ordinal >= pair[1].native_ordinal)
                || telescope
                    .binders
                    .windows(2)
                    .any(|pair| pair[0].static_machine_ordinal >= pair[1].static_machine_ordinal)
            {
                return malformed("calling telescope lost its authored formal or binder order");
            }
            for (ordinal, binder) in telescope.binders.iter().enumerate() {
                if telescope.binders[..ordinal]
                    .iter()
                    .any(|prior| prior.binder == binder.binder)
                {
                    return malformed("calling telescope repeats a nominal callback binder");
                }
                let identity = module
                    .identities
                    .text(binder.requirement_identity)
                    .filter(|text| !text.is_empty())
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "callback binder lost its requirement identity",
                    ))?;
                if semantic_vocabulary::callback_requirement_id(identity) != binder.requirement {
                    return malformed("callback binder substitutes another requirement identity");
                }
            }
            for callback in &telescope.direct_callbacks {
                if !telescope.binders.iter().any(|binder| {
                    binder.binder == callback.binder && binder.requirement == callback.requirement
                }) {
                    return malformed("direct callback lost its exact nominal binder requirement");
                }
                positions.push((callback.native_ordinal, callback.identity));
            }
            positions.sort_by_key(|(position, _)| *position);
            for (ordinal, (position, identity)) in positions.iter().enumerate() {
                if usize::try_from(*position).ok() != Some(ordinal)
                    || positions[..ordinal]
                        .iter()
                        .any(|(_, prior)| prior == identity)
                {
                    return malformed(
                        "calling telescope repeats or omits a native formal identity or position",
                    );
                }
            }
            Ok(())
        }
    }
    pub(crate) mod opaque_representations {

        use crate::codec_error::{CodecError, malformed};
        use terminal_psi::{
            StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralRecordStorage,
            StructuralTypeShape, TerminalModule,
        };

        pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
            super::super::super::wire::require_increasing(
                module.opaque_representations.iter().map(|row| row.opaque),
                "opaque representation storage applications",
            )?;
            for relation in &module.opaque_representations {
                let opaque = module.structural_type(relation.opaque).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "opaque representation lost its opaque storage",
                    ),
                )?;
                let carrier = module.structural_type(relation.carrier).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "opaque representation lost its carrier storage",
                    ),
                )?;
                if opaque.id == carrier.id
                    || opaque.identity != relation.opaque_storage_identity
                    || carrier.identity != relation.carrier_storage_identity
                    || [opaque.identity, carrier.identity]
                        .iter()
                        .any(|identity| module.identities.text(*identity).is_none_or(str::is_empty))
                {
                    return malformed(
                        "opaque representation substituted its exact storage identity",
                    );
                }
                relation
                    .application
                    .validate_identity_encoding()
                    .map_err(CodecError::NativeLayout)?;
                if !same_shape(module, &opaque.shape, &carrier.shape)
                    || opaque.scalar_restriction != carrier.scalar_restriction
                {
                    return malformed(
                        "opaque representation changed its selected carrier storage contract",
                    );
                }
                let opaque_native = module
                    .native_layouts
                    .records()
                    .find(|row| row.record == opaque.id);
                let carrier_native = module
                    .native_layouts
                    .records()
                    .find(|row| row.record == carrier.id);
                match (opaque_native, carrier_native) {
                    (None, None) => {}
                    (Some(left), Some(right)) => {
                        let left_fields = module
                            .native_layouts
                            .fields(left)
                            .map_err(CodecError::NativeLayout)?;
                        let right_fields = module
                            .native_layouts
                            .fields(right)
                            .map_err(CodecError::NativeLayout)?;
                        if left.data_identity != right.data_identity
                            || left.policy_identity != right.policy_identity
                            || left.semantic_identity != right.semantic_identity
                            || left_fields.len() != right_fields.len()
                            || left_fields.iter().zip(right_fields).any(|(left, right)| {
                                left.declaration_identity != right.declaration_identity
                            })
                            || module
                                .native_layouts
                                .realizations(left)
                                .map_err(CodecError::NativeLayout)?
                                != module
                                    .native_layouts
                                    .realizations(right)
                                    .map_err(CodecError::NativeLayout)?
                        {
                            return malformed(
                                "opaque representation changed its selected native layout provenance",
                            );
                        }
                    }
                    _ => {
                        return malformed(
                            "opaque representation lost its selected native layout provenance",
                        );
                    }
                }
            }
            Ok(())
        }

        fn same_fields(
            left: &[StructuralFieldDeclaration],
            right: &[StructuralFieldDeclaration],
        ) -> bool {
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    left.identity == right.identity
                        && left.relevance == right.relevance
                        && left.field_type == right.field_type
                        && left.storage == right.storage
                        && left.inline_carrier == right.inline_carrier
                })
        }

        fn same_cases(
            left: &[StructuralCaseDeclaration],
            right: &[StructuralCaseDeclaration],
        ) -> bool {
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    left.identity == right.identity && same_fields(&left.fields, &right.fields)
                })
        }

        fn same_shape(
            module: &TerminalModule,
            left: &StructuralTypeShape,
            right: &StructuralTypeShape,
        ) -> bool {
            match (left, right) {
                (
                    StructuralTypeShape::Record {
                        fields: left,
                        storage: left_storage,
                    },
                    StructuralTypeShape::Record {
                        fields: right,
                        storage: right_storage,
                    },
                ) => {
                    same_fields(left, right)
                        && same_record_storage(module, left, *left_storage, right, *right_storage)
                }
                (
                    StructuralTypeShape::Sum { cases: left },
                    StructuralTypeShape::Sum { cases: right },
                ) => same_cases(left, right),
                (
                    StructuralTypeShape::Mixed {
                        fields: left,
                        cases: left_cases,
                    },
                    StructuralTypeShape::Mixed {
                        fields: right,
                        cases: right_cases,
                    },
                ) => same_fields(left, right) && same_cases(left_cases, right_cases),
                _ => left == right,
            }
        }

        fn same_record_storage(
            module: &TerminalModule,
            left_fields: &[StructuralFieldDeclaration],
            left: StructuralRecordStorage,
            right_fields: &[StructuralFieldDeclaration],
            right: StructuralRecordStorage,
        ) -> bool {
            let (
                StructuralRecordStorage::TargetLayout(left),
                StructuralRecordStorage::TargetLayout(right),
            ) = (left, right)
            else {
                return left == right;
            };
            let Some(mut normalized) = module
                .target_layout_applications
                .iter()
                .find(|row| row.id == left)
                .cloned()
            else {
                return false;
            };
            let Some(expected) = module
                .target_layout_applications
                .iter()
                .find(|row| row.id == right)
            else {
                return false;
            };
            normalized.id = expected.id;
            normalized.record = expected.record;
            let rebase = |identity| {
                let ordinal = left_fields.iter().position(|field| field.id == identity)?;
                right_fields.get(ordinal).map(|field| field.id)
            };
            for realization in &mut normalized.realizations {
                for field in &mut realization.schema_argument.fields {
                    let Some(identity) = rebase(field.record_field) else {
                        return false;
                    };
                    field.record_field = identity;
                }
                for field in &mut realization.fields {
                    let Some(identity) = rebase(field.field) else {
                        return false;
                    };
                    field.field = identity;
                }
            }
            normalized == *expected
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use semantic_vocabulary::{
                AuthoredFieldStorage, AuthoredRecordStorage, StructuralFieldId, StructuralTypeId,
            };
            use terminal_psi::{
                BindingRelevance, LayoutApplicationId, StructuralFieldType,
                TargetLayoutApplication, TargetLayoutFieldStorage, TargetLayoutInput,
                TargetLayoutRealization, TargetLayoutSchemaArgument, TargetLayoutSchemaField,
                TargetLayoutSchemaFieldKind, TerminalIdentityIndex,
            };

            #[test]
            fn opaque_target_storage_compares_full_placements_after_coordinate_rebase() {
                let fields = |first| {
                    [first, first + 1].map(|identity| StructuralFieldDeclaration {
                        id: StructuralFieldId::new(identity).unwrap(),
                        identity: TerminalIdentityIndex(identity as u32),
                        relevance: BindingRelevance::Relevant,
                        storage: None,
                        inline_carrier: None,
                        field_type: StructuralFieldType::Scalar(
                            semantic_vocabulary::ScalarType::Boolean,
                        ),
                    })
                };
                let left = fields(1);
                let right = fields(3);
                let input = TargetLayoutInput {
                    target: "linux_x86_64".into(),
                    pointer_size: 8,
                    pointer_alignment: 8,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                };
                let application =
                    |raw, fields: &[StructuralFieldDeclaration]| TargetLayoutApplication {
                        id: LayoutApplicationId(raw),
                        record: StructuralTypeId::new(u64::from(raw)).unwrap(),
                        schema: StructuralTypeId::new(99).unwrap(),
                        policy_identity: TerminalIdentityIndex(1),
                        plan_identity: TerminalIdentityIndex(2),
                        target_inputs: vec![input.clone()],
                        realizations: vec![TargetLayoutRealization {
                            target: input.target.clone(),
                            pointer_size: 8,
                            pointer_alignment: 8,
                            case_tag_size: 4,
                            case_tag_alignment: 4,
                            little_endian: true,
                            storage: AuthoredRecordStorage {
                                byte_size: 16,
                                byte_alignment: 8,
                            },
                            schema_argument: TargetLayoutSchemaArgument {
                                abi_version: 3,
                                retired_field_identities: vec![],
                                fields: fields
                                    .iter()
                                    .enumerate()
                                    .map(|(ordinal, field)| TargetLayoutSchemaField {
                                        record_field: field.id,
                                        schema_field: StructuralFieldId::new(ordinal as u64 + 5)
                                            .unwrap(),
                                        key: ordinal as u64 + 1,
                                        identity: None,
                                        layout_source:
                                            terminal_psi::TargetLayoutFieldLayoutSource::Scalar,
                                        byte_size: 8,
                                        byte_alignment: 8,
                                        kind: TargetLayoutSchemaFieldKind::Scalar,
                                    })
                                    .collect(),
                            },
                            fields: fields
                                .iter()
                                .enumerate()
                                .map(|(ordinal, field)| TargetLayoutFieldStorage {
                                    field: field.id,
                                    storage: AuthoredFieldStorage::Direct {
                                        byte_offset: ordinal as u64 * 8,
                                    },
                                })
                                .collect(),
                        }],
                    };
                let mut module = TerminalModule::declarations();
                module.target_layout_applications =
                    vec![application(1, &left), application(2, &right)];
                let matches = |module: &TerminalModule| {
                    same_record_storage(
                        module,
                        &left,
                        StructuralRecordStorage::TargetLayout(LayoutApplicationId(1)),
                        &right,
                        StructuralRecordStorage::TargetLayout(LayoutApplicationId(2)),
                    )
                };
                assert!(matches(&module));
                let original = module.clone();
                let placements = &mut module.target_layout_applications[1].realizations[0].fields;
                placements[0].storage = AuthoredFieldStorage::Direct { byte_offset: 8 };
                placements[1].storage = AuthoredFieldStorage::Direct { byte_offset: 0 };
                assert!(
                    !matches(&module),
                    "equal size/alignment cannot justify swapped fields"
                );
                let mut module = original.clone();
                module.target_layout_applications[0].realizations[0].fields[0].field =
                    StructuralFieldId::new(99).unwrap();
                assert!(
                    !matches(&module),
                    "unknown local coordinates cannot be rebased"
                );
                let mut module = original.clone();
                module.target_layout_applications[1].realizations[0].case_tag_size = 8;
                assert!(
                    !matches(&module),
                    "sum-tag geometry belongs to the exact representation"
                );
                let mut module = original.clone();
                module.target_layout_applications[1].target_inputs[0].case_tag_alignment = 8;
                assert!(
                    !matches(&module),
                    "issuer tag alignment cannot change behind equal storage"
                );
                let mut module = original;
                module.target_layout_applications[1].target_inputs[0].little_endian = false;
                assert!(
                    !matches(&module),
                    "all independent target facts remain exact"
                );
            }
        }
    }
    pub(crate) use calling_signatures::validate as validate_calling_signatures;

    pub(crate) fn validate_domains(module: &TerminalModule) -> Result<(), CodecError> {
        for domain in &module.structural_domains {
            if !domain.required_memberships_are_well_formed(&module.structural_domains) {
                return malformed(
                    "structural domain has ambiguous, cyclic or wrong-carrier prerequisites",
                );
            }
            if domain.complete_predicate_theory
                && domain
                    .required_domain_closure(&module.structural_domains)
                    .is_none_or(|nodes| {
                        nodes
                            .into_iter()
                            .any(|node| !node.complete_predicate_theory || node.predicate.is_none())
                    })
            {
                return malformed("complete structural domain lost part of its required theory");
            }
            if !strictly_increasing(domain.establishment_routes.iter()) {
                return malformed("structural domain issuer routes must be canonical and unique");
            }
            for route in &domain.establishment_routes {
                let declaration = route.declaration().identity();
                let callable = route.identity();
                if [declaration, callable]
                    .iter()
                    .any(|identity| module.identities.text(*identity).is_none_or(str::is_empty))
                {
                    return malformed(
                        "structural domain issuer declaration and callable identities must be present",
                    );
                }
            }
            if domain.predicate.as_ref().is_some_and(|predicate| {
                !domain_predicate_is_closed(
                    module.structural_types.iter(),
                    domain.carrier,
                    predicate,
                )
            }) {
                return malformed(
                    "structural domain predicate escapes its declared subject or carrier",
                );
            }
            if !has_structural_type(module, domain.carrier) {
                return malformed("structural domain references an unknown carrier type");
            }
        }
        Ok(())
    }

    pub(crate) fn validate_services(module: &TerminalModule) -> Result<(), CodecError> {
        for service in &module.services {
            if service
                .parents
                .iter()
                .any(|parent| *parent == service.id || !has_service(module, *parent))
            {
                return malformed("service references itself or an unknown parent");
            }
        }
        validate_service_parent_graph(module)?;
        require_known_services(module, &module.root_service_reach.concrete)?;
        for dependency in &module.root_service_reach.installation_dependencies {
            if dependency.requirement_identity.is_absent() {
                return malformed("installation reach requirement identity must be nonempty");
            }
            require_known_services(module, &dependency.upper_bound)?;
        }
        Ok(())
    }

    pub(crate) fn validate_boundary_machine(
        module: &TerminalModule,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<(), CodecError> {
        terminal_psi::validate_numeric_intrinsic_signature(boundary, &module.structural_types)
            .map_err(CodecError::MalformedStructuralFoundation)?;
        terminal_semantics::closed_conformances::validate_boundary_callable(module, boundary)
            .map_err(CodecError::MalformedStructuralFoundation)?;
        if module
            .identities
            .text(boundary.owner.identity())
            .is_none_or(str::is_empty)
        {
            return malformed("boundary schema owner identity must be present");
        }
        if boundary.kind != terminal_psi::BoundaryRequirementKind::Machine
            && boundary.attachment.is_some()
        {
            return malformed("boundary requirement kind has an invalid nominal attachment");
        }
        if boundary
            .attachment
            .is_some_and(|attachment| !has_structural_type(module, attachment))
        {
            return malformed("boundary machine has an unknown attachment type");
        }
        validate_structural_parameters(module, &boundary.structural_parameters)?;
        terminal_semantics::validate_boundary_reference_regions(
            module.structural_types.iter(),
            boundary,
        )
        .map_err(CodecError::MalformedStructuralFoundation)?;
        super::reference_outputs::validate(module, boundary)?;
        super::scalar_observations::boundary(module, boundary)?;
        if let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result {
            if !has_structural_type(module, result.structural_type) {
                return malformed("boundary result references an unknown structural type");
            }
            validate_structural_qualifications(
                module,
                result.structural_type,
                &result.qualifications,
                &result.projected_qualifications,
            )?;
        }
        if !boundary.has_valid_parameter_order() {
            return malformed("boundary parameter order does not cover its lanes exactly");
        }
        for requirement in &boundary.requires {
            let Some(parameter) = boundary
                .structural_parameters
                .get(requirement.argument_index as usize)
            else {
                return malformed("boundary requirement has an unknown argument index");
            };
            let Some(domain) = module.structural_domain(requirement.domain) else {
                return malformed("boundary requirement references an unknown domain");
            };
            if qualification_path_carrier(
                module.structural_types.iter(),
                parameter.structural_type,
                &requirement.path,
            ) != Some(domain.carrier)
            {
                return malformed("boundary requirement domain has the wrong path carrier");
            }
        }
        for guarantee in &boundary.normal_qualifications {
            let Some(parameter) = boundary
                .structural_parameters
                .get(guarantee.argument_index as usize)
            else {
                return malformed("boundary normal guarantee has unknown parameter");
            };
            let Some(domain) = module.structural_domain(guarantee.domain) else {
                return malformed("boundary normal guarantee has unknown domain");
            };
            if parameter.access != terminal_psi::StructuralAccess::MutableBorrow
                || qualification_path_carrier(
                    module.structural_types.iter(),
                    parameter.structural_type,
                    &guarantee.path,
                ) != Some(domain.carrier)
            {
                return malformed("boundary normal guarantee has the wrong mutable path carrier");
            }
        }
        require_known_services(module, &boundary.published_service_ceiling)?;
        require_known_services(module, &boundary.fixed_service_reach)?;
        require_known_services(module, &boundary.published_synchronous_entries)?;
        validate_boundary_scalar_rows(boundary)?;
        if let Some(owner) = boundary.owning_service {
            if boundary.kind != terminal_psi::BoundaryRequirementKind::Trait {
                return malformed("non-trait boundary names an owning service");
            }
            require_known_services(module, &[owner])?;
        }
        Ok(())
    }

    pub(crate) fn validate_provider_candidate(
        module: &TerminalModule,
        candidate: &ProviderCandidateConformance,
    ) -> Result<(), CodecError> {
        if candidate.receiving_application.is_absent() {
            if !candidate.declaration_application.is_absent()
                || !candidate.receiving_region_arguments.is_empty()
            {
                return malformed("provider regions have no receiving application");
            }
        } else if module
            .identities
            .text(candidate.declaration_application)
            .is_none_or(str::is_empty)
            || module
                .identities
                .text(candidate.receiving_application)
                .is_none_or(str::is_empty)
            || candidate
                .receiving_region_arguments
                .iter()
                .any(|region| module.identities.text(*region).is_none_or(str::is_empty))
        {
            return malformed("provider receiving application references an empty identity");
        }
        if candidate.requirement_identity.is_absent()
            || module
                .identities
                .text(candidate.source_requirement_identity)
                .is_none_or(str::is_empty)
            || module
                .identities
                .text(candidate.source_candidate_identity)
                .is_none_or(str::is_empty)
            || candidate.provider_identity.is_absent()
            || candidate.candidate_identity.is_absent()
            || module
                .identities
                .text(candidate.provider_declaration.identity())
                .is_none_or(str::is_empty)
        {
            return malformed("provider candidate identities must be nonempty");
        }
        let Some(boundary) = module.boundary_machine(candidate.boundary) else {
            return malformed("provider candidate references an unknown terminal ID");
        };
        if !candidate.receiving_application.is_absent()
            && boundary.kind != terminal_psi::BoundaryRequirementKind::Trait
        {
            return malformed("provider receiving trait application names a non-trait boundary");
        }
        let Some(machine) = module.machine(candidate.candidate) else {
            return malformed("provider candidate references an unknown terminal ID");
        };
        if boundary.role != terminal_psi::BoundaryDeclarationRole::Callable {
            return malformed(
                "provider candidate references contract evidence rather than a callable",
            );
        }
        validate_provider_scalar_contract(boundary, machine, &module.structural_domains)?;
        super::scalar_boundary_contracts::validate_provider(module, boundary, &machine.parameters)?;
        for parameter in &candidate.signature.parameters {
            if !has_structural_type(module, parameter.structural_type) {
                return malformed("provider signature references an unknown structural type");
            }
            if parameter
                .qualifications
                .iter()
                .any(|domain| !module.structural_domain(*domain).is_some())
            {
                return malformed("provider signature references an unknown structural domain");
            }
        }
        require_known_services(module, &candidate.refinement.realized_service_ceiling)?;
        Ok(())
    }

    fn validate_boundary_scalar_rows(
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<(), CodecError> {
        let Some(mut formals) = boundary.scalar_contract_parameters() else {
            return malformed("boundary scalar contract formals overflow");
        };
        let places = boundary
            .structural_parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.place,
                    StructuralPlaceKind::Parameter {
                        position: parameter.position,
                    },
                )
            })
            .collect::<Vec<_>>();
        let requires = PropositionContext::from_value_types_and_places(
            formals.iter().map(|formal| (formal.id, formal.scalar_type)),
            places.iter().copied(),
        )
        .map_err(|_| {
            CodecError::MalformedStructuralFoundation("boundary scalar requires telescope")
        })?;
        for proposition in &boundary.scalar_requires {
            requires.validate(proposition).map_err(|_| {
                CodecError::MalformedStructuralFoundation(
                    "boundary scalar requires names an undeclared formal",
                )
            })?;
        }
        if boundary.scalar_ensures.is_empty() {
            return Ok(());
        }
        if let Some(result) = boundary.scalar_contract_result() {
            formals.push(result);
        }
        let ensures = PropositionContext::from_value_types_and_places(
            formals.iter().map(|formal| (formal.id, formal.scalar_type)),
            places.iter().copied(),
        )
        .map_err(|_| {
            CodecError::MalformedStructuralFoundation("boundary scalar ensures telescope")
        })?;
        for proposition in &boundary.scalar_ensures {
            ensures.validate(proposition).map_err(|_| {
                CodecError::MalformedStructuralFoundation(
                    "boundary scalar ensures names an undeclared formal",
                )
            })?;
        }
        Ok(())
    }

    fn validate_provider_scalar_contract(
        boundary: &BoundaryMachineDeclaration,
        machine: &TerminalMachine,
        domains: &[terminal_psi::StructuralDomainDeclaration],
    ) -> Result<(), CodecError> {
        if boundary.scalar_parameters.len() != machine.parameters.len()
            || boundary
                .scalar_parameters
                .iter()
                .zip(&machine.parameters)
                .any(|(required, actual)| *required != actual.scalar_type)
        {
            return malformed("provider candidate scalar signature disagrees with its boundary");
        }
        if !boundary.result.accepts_provider_result(
            &machine.result,
            boundary.identity,
            boundary
                .declaration
                .as_ref()
                .map(|callable| callable.source.semantic_declaration),
            domains,
        ) {
            return malformed("provider candidate result disagrees with its boundary");
        }
        if boundary.structural_parameters.len() != machine.structural_parameters.len()
            || boundary
                .structural_parameters
                .iter()
                .zip(&machine.structural_parameters)
                .any(|(required, actual)| {
                    required.position != actual.position
                        || required.structural_type != actual.structural_type
                })
        {
            return malformed(
                "provider candidate structural signature disagrees with its boundary",
            );
        }
        if !machine.contract.erased_scalar_formals.is_empty()
            || !machine.contract.erased_proof_formals.is_empty()
        {
            return malformed("provider candidate carries erased formals its boundary cannot bind");
        }
        let Some(formals) = boundary.scalar_contract_parameters() else {
            return malformed("boundary scalar contract formals overflow");
        };
        let mut substitutions = formals
            .iter()
            .zip(&machine.parameters)
            .map(|(formal, actual)| (formal.id, ScalarTerm::value(actual.id, actual.scalar_type)))
            .collect::<BTreeMap<_, _>>();
        if let Some(formal) = boundary.scalar_contract_result()
            && let Some(actual) = machine.result.scalar()
        {
            substitutions.insert(formal.id, ScalarTerm::value(actual.id, actual.scalar_type));
        }
        let places = boundary
            .structural_parameters
            .iter()
            .zip(&machine.structural_parameters)
            .map(|(formal, actual)| (formal.place, (actual.place, Vec::new())))
            .collect::<BTreeMap<_, _>>();
        let instantiate = |row: &Proposition| {
            let values = terminal_semantics::substitution::substitute_proposition_values(
                row,
                &substitutions,
            );
            terminal_semantics::substitution::substitute_proposition_structural_places(
                &values, &places,
            )
        };
        let declared = boundary
            .scalar_requires
            .iter()
            .map(instantiate)
            .collect::<Vec<_>>();
        let declared_conjuncts = declared.iter().flat_map(conjuncts).collect::<Vec<_>>();
        if machine
            .contract
            .source_checked_requires
            .iter()
            .flat_map(conjuncts)
            .any(|part| !declared_conjuncts.contains(&part))
        {
            return malformed(
                "provider candidate requires beyond its boundary's declared obligations",
            );
        }
        let roster = machine
            .contract
            .ensures
            .iter()
            .flat_map(|clause| conjuncts(&clause.proposition))
            .collect::<Vec<_>>();
        if boundary.scalar_ensures.iter().any(|row| {
            let substituted = instantiate(row);
            !conjuncts(&substituted)
                .iter()
                .all(|part| roster.contains(part))
        }) {
            return malformed(
                "provider candidate does not ensure its boundary's declared guarantees",
            );
        }
        Ok(())
    }

    fn conjuncts(proposition: &Proposition) -> Vec<&Proposition> {
        match proposition {
            Proposition::Conjunction(parts) => parts
                .iter()
                .filter(|part| !matches!(part, Proposition::Truth))
                .collect(),
            Proposition::Truth => Vec::new(),
            proposition => vec![proposition],
        }
    }
}
pub(crate) mod installed_interrupts {
    use super::*;
    use terminal_psi::{CheckedBoundaryExecution, OperationKind};

    pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
        let mut calls = std::collections::BTreeMap::new();
        for operation in module.operations() {
            let OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
                continue;
            };
            if module
                .boundary_machine(boundary)
                .and_then(|row| row.declaration.as_ref())
                .is_some_and(|row| {
                    matches!(
                        row.execution,
                        CheckedBoundaryExecution::StaticInterruptInstall
                    )
                })
            {
                calls.insert(operation.id, boundary);
            }
        }
        let mut previous = None;
        let mut callback_ids = module
            .callback_applications
            .iter()
            .map(|row| row.application_index)
            .collect::<std::collections::BTreeSet<_>>();
        for registration in &module.installed_interrupt_registrations {
            if previous.is_some_and(|previous| previous >= registration.operation)
                || calls.remove(&registration.operation) != Some(registration.installer)
            {
                return malformed("interrupt installation has no unique exact core operation");
            }
            previous = Some(registration.operation);
            let installer = module.boundary_machine(registration.installer).ok_or(
                CodecError::MalformedStructuralFoundation(
                    "interrupt installation lost its boundary",
                ),
            )?;
            validate_control_service(
                module,
                registration.service,
                &installer.published_service_ceiling,
            )?;
            let Some(registrar) = module
                .boundary_machine(registration.registrar_boundary)
                .and_then(|boundary| boundary.declaration.as_ref())
            else {
                return malformed("interrupt registrar lost its exact boundary declaration");
            };
            if module.identities.text(registrar.source.callable)
                != Some(registration.registrar_requirement.as_str())
            {
                return malformed("interrupt registrar boundary names another checked requirement");
            }
            if registration.registrar_application.is_empty()
                || registration.registrar_declaring_application.is_empty()
                || registration
                    .registrar_arguments
                    .iter()
                    .chain(&registration.registrar_lifetimes)
                    .any(String::is_empty)
                || registration.registrar_requirement.is_empty()
                || registration.slot_encoding.is_empty()
                || registration.slot_commitment == [0; 32]
                || language_semantics::conformance_identity::source_identity_commitments(
                    &registration.slot_encoding,
                )
                .1 != registration.slot_commitment
            {
                return malformed(
                    "interrupt installation lost its complete registrar or slot application",
                );
            }
            for identity in [
                &registration.registrar,
                &registration.registrar_owner,
                &registration.slot_declaration,
            ] {
                if module
                    .identities
                    .text(identity.identity())
                    .is_none_or(str::is_empty)
                {
                    return malformed(
                        "interrupt installation lost a selected declaration identity",
                    );
                }
            }
            if registration.callbacks.is_empty() {
                return malformed("interrupt installation has no checked callback selection");
            }
            let mut previous_callback = None;
            for callback in &registration.callbacks {
                if callback.registrar != registration.operation
                    || !callback.entry_admitted
                    || !callback_ids.insert(callback.application_index)
                    || previous_callback
                        .is_some_and(|previous| previous >= callback.application_index)
                {
                    return malformed(
                        "interrupt callback does not belong to its exact installation",
                    );
                }
                previous_callback = Some(callback.application_index);
            }
        }
        if !calls.is_empty() {
            return malformed(
                "interrupt installation call has no checked registrar and slot record",
            );
        }
        Ok(())
    }

    fn validate_control_service(
        module: &TerminalModule,
        selected: semantic_vocabulary::ServiceId,
        published_reach: &[semantic_vocabulary::ServiceId],
    ) -> Result<(), CodecError> {
        let required = language_core::static_interrupts::StaticInterruptOperation::Install
            .required_service_identity();
        let identity = module
            .services
            .iter()
            .find(|service| service.id == selected)
            .and_then(|service| module.identities.text(service.identity));
        if required.is_none() || identity != required || !published_reach.contains(&selected) {
            return malformed("interrupt installation lost its exact core control service");
        }
        Ok(())
    }
}
pub(crate) mod machine_foundations {

    use super::{
        has_structural_type, is_owned_cleanup_operation_result, require_known_services,
        structural_place_type, validate_operation_foundation,
        validate_provider_attachment_foundation, validate_structural_parameters,
        validate_structural_path,
    };
    use crate::codec_error::{CodecError, malformed};
    use crate::sections::semantic_module::structural_result_wire;
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::Block;
    use terminal_psi::{
        ByteSequenceCarrier, EntryClaim, StructuralAccess, StructuralMultiplicity,
        StructuralPathSegment, StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine,
        TerminalModule, Terminator, call_structural_argument_variants,
    };
    use terminal_semantics::{
        canonical_static_projection, runtime_structural_path_tip,
        structural_claim_paths_may_overlap,
    };

    pub(crate) fn validate_machine(
        module: &TerminalModule,
        machine: &TerminalMachine,
        transfers: &mut super::storage_foundations::StructuralTransferInputs<'_>,
    ) -> Result<(), CodecError> {
        if let Err(reason) = machine.validate_parameter_order() {
            return malformed(reason);
        }
        if machine
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.specialization.as_ref())
            .is_some_and(|specialization| {
                module
                    .identities
                    .text(specialization.template_callable)
                    .is_none_or(str::is_empty)
                    || module
                        .identities
                        .text(specialization.template_semantic_declaration)
                        .is_none_or(str::is_empty)
            })
        {
            return malformed("machine specialization lost its exact template callable");
        }
        if machine
            .attachment
            .is_some_and(|attachment| !has_structural_type(module, attachment))
        {
            return malformed("machine has an unknown attachment type");
        }
        validate_structural_parameters(module, &machine.structural_parameters)?;
        super::call_foundations::validate_outcome_subjects(module, machine)?;
        super::scalar_observations::machine(module, machine)?;
        structural_result_wire::validate_reference_sources(module, machine)?;
        for block in &machine.blocks {
            validate_block_parameters(module, machine, block)?;
        }
        validate_block_parameter_places(machine)?;
        validate_provider_attachment_foundation(module, machine)?;
        require_known_services(module, &machine.published_service_ceiling)?;
        require_known_services(module, &machine.published_synchronous_entries)?;
        validate_parameter_places(machine)?;
        validate_entry_claims(module, machine)?;
        validate_loan_completions(module, machine)?;
        for block in &machine.blocks {
            for operation in &block.operations {
                validate_operation_foundation(module, machine, operation, transfers).map_err(
                    |error| {
                        super::operation_foundation_error(
                            module, machine, block.id, operation, error,
                        )
                    },
                )?;
            }
        }
        for invocation in machine.module_invocations() {
            super::call_foundations::validate_module_invocation_entry(
                module,
                machine,
                invocation.inputs,
            )?;
        }
        validate_operation_result_places(machine)?;
        for block in &machine.blocks {
            super::call_foundations::validate_external_completion(module, machine, block)?;
            super::call_foundations::validate_no_normal_invocation(module, block)?;
            validate_partial_affine_discards(module, machine, block)?;
        }
        Ok(())
    }

    fn validate_block_parameters(
        module: &TerminalModule,
        machine: &TerminalMachine,
        block: &Block,
    ) -> Result<(), CodecError> {
        validate_structural_parameters(module, &block.structural_parameters)?;
        if block.id == machine.entry && !block.structural_parameters.is_empty() {
            return malformed("entry block cannot declare structural parameters");
        }
        for (position, parameter) in block.structural_parameters.iter().enumerate() {
            if parameter.position as usize != position {
                return malformed("structural block parameters require dense positions");
            }
            if !machine.structural_places.iter().any(|place| {
                place.id == parameter.place
                    && place.kind
                        == StructuralPlaceKind::BlockParameter {
                            block: block.id,
                            position: parameter.position,
                        }
            }) {
                return malformed(
                    "structural block parameter place disagrees with its declaration",
                );
            }
        }
        Ok(())
    }

    fn validate_block_parameter_places(machine: &TerminalMachine) -> Result<(), CodecError> {
        for place in &machine.structural_places {
            if let StructuralPlaceKind::BlockParameter { block, position } = place.kind {
                let parameter = machine
                    .block(&block)
                    .and_then(|candidate| candidate.structural_parameters.get(position as usize));
                if parameter.is_none_or(|parameter| parameter.place != place.id) {
                    return malformed("structural block place has no matching parameter");
                }
            }
        }
        Ok(())
    }

    fn validate_parameter_places(machine: &TerminalMachine) -> Result<(), CodecError> {
        for parameter in &machine.structural_parameters {
            let Some(place) = machine.structural_place(parameter.place) else {
                return malformed("structural parameter has no declared structural place");
            };
            if place.kind
                != (StructuralPlaceKind::Parameter {
                    position: parameter.position,
                })
            {
                return malformed("structural parameter place kind disagrees with its signature");
            }
        }
        Ok(())
    }

    fn validate_entry_claims(
        module: &TerminalModule,
        machine: &TerminalMachine,
    ) -> Result<(), CodecError> {
        for claim in &machine.entry_claims {
            let Some(parameter) = machine.signature_structural_parameter(claim.input) else {
                return malformed("entry claim is not bound to a structural parameter");
            };
            if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
                return malformed("entry claim cannot bind an unrestricted parameter");
            }
            validate_structural_path(module, parameter.structural_type, &claim.path)?;
        }
        let mut block_claim_inputs: Vec<&EntryClaim> = Vec::new();
        for claim in &machine.block_entry_claims {
            let Some(parameter) = machine
                .block_structural_parameters()
                .find(|parameter| parameter.place == claim.input)
            else {
                return malformed("block entry claim is not bound to a block parameter");
            };
            if parameter.access != StructuralAccess::Owned
                || parameter.multiplicity == StructuralMultiplicity::Unrestricted
            {
                return malformed("block entry claim requires an owned affine or linear parameter");
            }
            validate_structural_path(module, parameter.structural_type, &claim.path)?;
            canonical_static_projection(
                module.structural_types.iter(),
                parameter.structural_type,
                &claim.path,
            )
            .ok_or(CodecError::MalformedStructuralFoundation(
                "block entry claim path is not static owned storage",
            ))?;
            for previous in &block_claim_inputs {
                if previous.input != claim.input {
                    continue;
                }
                let overlap = structural_claim_paths_may_overlap(
                    module.structural_types.iter(),
                    parameter.structural_type,
                    &previous.path,
                    &claim.path,
                )
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "block entry claim path is not static owned storage",
                ))?;
                if overlap {
                    return malformed("block entry claims overlap a parameter path");
                }
            }
            block_claim_inputs.push(claim);
        }
        Ok(())
    }

    fn validate_loan_completions(
        module: &TerminalModule,
        machine: &TerminalMachine,
    ) -> Result<(), CodecError> {
        for completion in &machine.loan_completions {
            let operation = machine.operation(completion.operation).ok_or(
                CodecError::MalformedStructuralFoundation("loan completion has no call operation"),
            )?;
            if !matches!(
                &operation.kind,
                call_structural_argument_variants!(_arguments)
            ) {
                return malformed("loan completion is not a structural call");
            }
            let root = structural_place_type(machine, completion.place).ok_or(
                CodecError::MalformedStructuralFoundation("loan completion has no owned home"),
            )?;
            if machine
                .structural_parameter(completion.place)
                .is_some_and(|parameter| parameter.access != StructuralAccess::Owned)
            {
                return malformed("loan completion cannot own borrowed ingress");
            }
            validate_owned_completion_path(module, root, &completion.path)?;
            for claim in &completion.claims {
                if !claim.path.starts_with(&completion.path) {
                    return malformed("loan completion claim lies outside its returned region");
                }
                validate_owned_completion_path(module, root, &claim.path)?;
            }
        }
        Ok(())
    }

    fn validate_owned_completion_path(
        module: &TerminalModule,
        root: semantic_vocabulary::StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Result<(), CodecError> {
        if path.iter().any(|segment| {
            !matches!(
                segment,
                StructuralPathSegment::Case(_)
                    | StructuralPathSegment::Field(_)
                    | StructuralPathSegment::FixedIndex(_)
            )
        }) {
            return malformed("loan completion path must retain a static owned subtree");
        }
        for (position, segment) in path.iter().enumerate() {
            if !matches!(segment, StructuralPathSegment::FixedIndex(_)) {
                continue;
            }
            let parent = runtime_structural_path_tip(
                module.structural_types.iter(),
                root,
                &path[..position],
            );
            if parent.is_none_or(|declaration| {
                !matches!(
                    declaration.shape,
                    StructuralTypeShape::FixedArray { .. }
                        | StructuralTypeShape::ByteSequence(
                            ByteSequenceCarrier::BoundedOwned { .. }
                        )
                )
            }) {
                return malformed("loan completion index must select inline owned storage");
            }
        }
        validate_structural_path(module, root, path)?;
        Ok(())
    }

    fn validate_operation_result_places(machine: &TerminalMachine) -> Result<(), CodecError> {
        for place in &machine.structural_places {
            let StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } = place.kind
            else {
                continue;
            };
            let mut producers = machine
                .operations()
                .filter(|operation| operation.id == producer);
            let Some(operation) = producers.next() else {
                return malformed("structural operation-result place has no producer");
            };
            if producers.next().is_some() {
                return malformed("structural operation-result place has duplicate producers");
            }
            let Some(result) = operation.result.structural() else {
                return malformed(
                    "structural operation-result place producer has no structural result",
                );
            };
            if result.place != place.id || result.structural_type != structural_type {
                return malformed("structural operation-result place disagrees with its producer");
            }
        }
        Ok(())
    }

    fn validate_partial_affine_discards(
        module: &TerminalModule,
        machine: &TerminalMachine,
        block: &Block,
    ) -> Result<(), CodecError> {
        let rosters = match &block.terminator {
            Terminator::Jump {
                cleanup_actions, ..
            } => vec![cleanup_actions],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![&when_true.cleanup_actions, &when_false.cleanup_actions],
            Terminator::StructuralCase { cases, .. } => {
                cases.iter().map(|edge| &edge.cleanup_actions).collect()
            }
            _ => return Ok(()),
        };
        for actions in rosters {
            for discard in actions.iter().filter_map(|action| match action {
                TerminalAffineCleanupAction::DiscardResidual(discard) => Some(discard),
                _ => None,
            }) {
                let parameter = machine.signature_structural_parameter(discard.place);
                let block_parameter = || {
                    let declaration = machine.structural_place(discard.place)?;
                    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind
                    else {
                        return None;
                    };
                    machine
                        .block(&block)?
                        .structural_parameters
                        .get(position as usize)
                        .filter(|parameter| {
                            parameter.place == discard.place && parameter.position == position
                        })
                };
                let supported_root = parameter.map_or_else(
                    || {
                        is_owned_cleanup_operation_result(machine, discard.place)
                            || block_parameter().is_some_and(|parameter| {
                                matches!(
                                    parameter.multiplicity,
                                    StructuralMultiplicity::Affine | StructuralMultiplicity::Linear
                                ) && parameter.access == StructuralAccess::Owned
                            })
                    },
                    |parameter| {
                        matches!(
                            parameter.multiplicity,
                            StructuralMultiplicity::Affine | StructuralMultiplicity::Linear
                        ) && parameter.access == StructuralAccess::Owned
                    },
                );
                if !supported_root {
                    return malformed("affine region cleanup does not name an owned custody root");
                }
                if actions.iter().any(|action| matches!(action, TerminalAffineCleanupAction::DiscardRoot(place) if *place == discard.place)) {
                    return malformed("affine region cleanup overlaps a whole-root discard");
                }
                let Some(root_type) = structural_place_type(machine, discard.place) else {
                    return malformed("partial affine cleanup has no exact structural root type");
                };
                if validate_structural_path(module, root_type, &discard.path)?
                    != discard.structural_type
                {
                    return malformed("partial affine cleanup leaf type does not match its path");
                }
            }
        }
        Ok(())
    }
}
pub(crate) mod native_layouts {

    use crate::codec_error::CodecError;
    use terminal_psi::{StructuralRecordStorage, StructuralTypeShape, TerminalModule};

    pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
        let fail = |reason: &str| CodecError::NativeLayout(reason.to_owned());
        let expected = module
            .structural_types
            .iter()
            .filter_map(|row| {
                matches!(row.shape, StructuralTypeShape::Record { .. }).then_some(row.id)
            })
            .collect::<Vec<_>>();
        let observed = module
            .native_layouts
            .records()
            .map(|row| row.record)
            .collect::<Vec<_>>();
        if observed != expected || observed.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(fail(
                "native layout catalog differs from the complete ordered record inventory",
            ));
        }
        for native in module.native_layouts.records() {
            let record = module
                .structural_type(native.record)
                .ok_or_else(|| fail("native layout has no semantic record"))?;
            let StructuralTypeShape::Record { fields, storage } = &record.shape else {
                return Err(fail("native layout names a non-record"));
            };
            for identity in [
                native.data_identity,
                native.policy_identity,
                native.semantic_identity,
            ] {
                if module.identities.text(identity).is_none() {
                    return Err(fail("native layout has an out-of-range nominal identity"));
                }
            }
            let nominal_fields = module
                .native_layouts
                .fields(native)
                .map_err(CodecError::NativeLayout)?;
            if nominal_fields.len() != fields.len()
                || nominal_fields.iter().zip(fields).any(|(nominal, field)| {
                    nominal.field != field.id
                        || module
                            .identities
                            .text(nominal.declaration_identity)
                            .is_none()
                })
            {
                return Err(fail(
                    "native layout nominal field inventory differs from its record",
                ));
            }
            let realizations = module
                .native_layouts
                .realizations(native)
                .map_err(CodecError::NativeLayout)?;
            if native.policy_identity.is_absent() {
                if !realizations.is_empty()
                    || matches!(storage, StructuralRecordStorage::TargetLayout(_))
                {
                    return Err(fail(
                        "natural native layout carries authored policy storage",
                    ));
                }
                continue;
            }
            let semantic = match storage {
                StructuralRecordStorage::TargetLayout(id) => Some(
                    module
                        .target_layout_applications
                        .iter()
                        .find(|application| {
                            application.id == *id && application.record == native.record
                        })
                        .ok_or_else(|| fail("native layout lost its exact semantic application"))?,
                ),
                StructuralRecordStorage::Authored(_) => None,
                _ => {
                    return Err(fail(
                        "planned native layout has no authored semantic storage",
                    ));
                }
            };
            let expected_targets = semantic.map(|application| {
                application
                    .realizations
                    .iter()
                    .map(|row| row.target.as_str())
                    .collect::<Vec<_>>()
            });
            let actual_targets = realizations
                .iter()
                .filter_map(|row| row.target.as_ref().map(|target| target.target.as_str()))
                .collect::<Vec<_>>();
            if match &expected_targets {
                Some(targets) => {
                    *targets != actual_targets || actual_targets.len() != realizations.len()
                }
                None => realizations.len() != 1 || realizations[0].target.is_some(),
            } {
                return Err(fail(
                    "native layout alternatives differ from their complete semantic roster",
                ));
            }
            let policy = module
                .identities
                .text(native.policy_identity)
                .ok_or_else(|| fail("native layout lost its policy identity"))?;
            let semantic_identity = module
                .identities
                .text(native.semantic_identity)
                .ok_or_else(|| fail("native layout lost its semantic argument identity"))?;
            for realization in realizations {
                let report = &realization.report;
                let physical = match &realization.target {
                    Some(target) => {
                        let selected = semantic
                            .and_then(|application| {
                                application
                                    .realizations
                                    .iter()
                                    .find(|row| row.target == target.target)
                            })
                            .ok_or_else(|| {
                                fail("native target member lost its semantic storage")
                            })?;
                        if selected.pointer_size != target.pointer_size
                            || selected.pointer_alignment != target.pointer_alignment
                            || selected.case_tag_size != target.case_tag_size
                            || selected.case_tag_alignment != target.case_tag_alignment
                            || selected.little_endian != target.little_endian
                        {
                            return Err(fail("native layout changed its exact target facts"));
                        }
                        selected.storage
                    }
                    None => {
                        let StructuralRecordStorage::Authored(storage) = storage else {
                            return Err(fail(
                                "invariant native layout has pending semantic storage",
                            ));
                        };
                        *storage
                    }
                };
                if report.layout.size != Some(physical.byte_size)
                    || report.layout.align != physical.byte_alignment
                {
                    return Err(fail(
                        "native layout report differs from semantic extent or alignment",
                    ));
                }
                let slots = &report.private_callback_demands;
                if slots
                    .windows(2)
                    .any(|pair| pair[0].slot_identity >= pair[1].slot_identity)
                {
                    return Err(fail("native layout slots are not uniquely ordered"));
                }
                for slot in slots {
                    slot.slot_application
                        .validate_identity_encoding()
                        .map_err(CodecError::NativeLayout)?;
                    if semantic_identity.is_empty()
                        || slot.slot_identity
                            != format!(
                                "{}#{:016x}",
                                slot.slot_application.declaration_identity,
                                slot.slot_application.application_report_fingerprint
                            )
                        || slot.layout_subject_identity != policy
                        || slot.slot_application.layout_identity != policy
                        || slot.slot_application.semantic_identity != semantic_identity
                        || slot.callback_requirement_identity
                            != slot.slot_application.requirement_identity
                    {
                        return Err(fail(
                            "native layout slot changed its selected nominal application or roles",
                        ));
                    }
                    if slot.placements.is_empty()
                        || slot.placements.windows(2).any(|pair| {
                            layout_plans::layout_placement_sort_key(&pair[0])
                                >= layout_plans::layout_placement_sort_key(&pair[1])
                        })
                    {
                        return Err(fail(
                            "native layout slot placements are empty, duplicated or unordered",
                        ));
                    }
                    for placement in &slot.placements {
                        match *placement {
                            layout_plans::LayoutPlacementReport::At { offset } => {
                                if slot.placements.len() != 1 || offset > physical.byte_size {
                                    return Err(fail(
                                        "native layout whole-slot placement is repeated or outside storage",
                                    ));
                                }
                            }
                            layout_plans::LayoutPlacementReport::Bits {
                                container,
                                container_width,
                                destination_lsb,
                                source_lsb,
                                width,
                            } => {
                                let narrow = |value| {
                                    u16::try_from(value).map_err(|_| fail("private callback fragment width exceeds its representation"))
                                };
                                layout_plans::validate_fragment(
                                    usize::try_from(physical.byte_size).map_err(|_| {
                                        fail("private callback storage exceeds host range")
                                    })?,
                                    "private callback",
                                    container,
                                    narrow(container_width)?,
                                    narrow(destination_lsb)?,
                                    narrow(source_lsb)?,
                                    narrow(width)?,
                                )
                                .map_err(|error| fail(&error.to_string()))?;
                            }
                            layout_plans::LayoutPlacementReport::IntegerAt { .. } => {
                                return Err(fail(
                                    "private callback narrowed storage has no retained address-fit contract",
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
pub(crate) mod proof_scope {

    use super::super::wire::require_increasing;
    use crate::codec_error::{CodecError, malformed};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        ProofApplicationScope, ProofDeclarationId, ProofScopeDeclaration, TerminalIdentityIndex,
        TerminalIdentityTable,
    };

    pub(crate) fn validate(
        scope: &ProofApplicationScope,
        identities: &TerminalIdentityTable,
    ) -> Result<(), CodecError> {
        let declaration = |identity: ProofDeclarationId| {
            if scope.declaration(identity).is_some() {
                Ok(())
            } else {
                malformed("proof scope references an absent declaration")
            }
        };
        let text = |identity: TerminalIdentityIndex| {
            if identities
                .text(identity)
                .is_some_and(|text| !text.is_empty())
            {
                Ok(())
            } else {
                malformed("proof scope declaration lost its portable identity")
            }
        };
        for (_, row) in scope.declarations.iter() {
            match row {
                ProofScopeDeclaration::Missing => {
                    return malformed("proof scope declaration is absent");
                }
                ProofScopeDeclaration::Unavailable(reason) => {
                    if reason.is_empty() {
                        return malformed("unavailable proof declaration has no reason");
                    }
                }
                ProofScopeDeclaration::Nominal(identity) => text(*identity)?,
                ProofScopeDeclaration::Operator(requirement) => {
                    text(requirement.declaration().identity())?;
                    if requirement.overload().is_empty() {
                        return malformed("proof scope operator lost its exact overload");
                    }
                }
                ProofScopeDeclaration::Callable(row) => {
                    for identity in [
                        row.machine.identity(),
                        row.state.identity(),
                        row.semantic_declaration,
                        row.callable,
                        row.result_type,
                    ] {
                        text(identity)?;
                    }
                    for identity in &row.parameter_types {
                        text(*identity)?;
                    }
                    if let Some(specialization) = &row.specialization {
                        for identity in [
                            specialization.template.identity(),
                            specialization.template_callable,
                            specialization.template_semantic_declaration,
                        ] {
                            text(identity)?;
                        }
                        if let Some(capture) = &specialization.capture {
                            text(capture.template_identity)?;
                        }
                    }
                }
            }
        }
        require_increasing(
            scope
                .callable_headers
                .iter()
                .map(|(_, header)| header.declaration),
            "proof callable headers by declaration",
        )?;
        for (_, header) in scope.callable_headers.iter() {
            declaration(header.declaration)?;
            if let Err(reason) = &header.compiler_evaluation
                && reason.is_empty()
            {
                return malformed("unavailable compiler-evaluation facts have no reason");
            }
            if let Ok(facts) = &header.compiler_evaluation {
                if facts.name.is_empty() {
                    return malformed("compiler-evaluation display name is absent");
                }
                if let semantic_vocabulary::compiler_evaluation::DeclarationCustody::Package(
                    package,
                ) = facts.custody
                    && header.package != Some(package)
                {
                    return malformed(
                        "compiler-evaluation custody differs from its exact callable header",
                    );
                }
                if let Err(reason) = &facts.selections
                    && reason.is_empty()
                {
                    return malformed("uncovered compiler-evaluation selections have no reason");
                }
            }
            match &header.callable {
                Ok(callable) => {
                    text(*callable)?;
                    if let Some(ProofScopeDeclaration::Callable(description)) =
                        scope.declaration(header.declaration)
                        && description.callable != *callable
                    {
                        return malformed("callable header differs from its full declaration");
                    }
                }
                Err(reason) if reason.is_empty() => {
                    return malformed("unavailable callable header has no reason");
                }
                Err(_) => {}
            }
            if header.source_target.as_ref().is_some_and(String::is_empty) {
                return malformed("callable header has an empty source target");
            }
            match &header.synchronous_invocations {
                Ok(targets) => {
                    for target in targets {
                        text(target.declaration)?;
                        if let Some(application) = target.application {
                            text(application)?;
                        }
                    }
                }
                Err(reason) if reason.is_empty() => {
                    return malformed("unavailable synchronous invocation facts have no reason");
                }
                Err(_) => {}
            }
        }
        let mut bound_declarations = BTreeSet::new();
        let mut recursive_bodies = BTreeSet::new();
        for (_, owner) in scope.pending_evaluation_conditions.iter() {
            if owner.0 != 0 {
                declaration(*owner)?;
            }
        }
        for (_, component) in scope.recursive_components.iter() {
            let members = scope.recursive_members.span(component.members).ok_or(
                CodecError::MalformedStructuralFoundation(
                    "checked recursive component lost its members",
                ),
            )?;
            if members.is_empty() {
                return malformed("checked recursive component has no members");
            }
            for member in members {
                declaration(*member)?;
                if !recursive_bodies.insert(*member) {
                    return malformed("checked recursive body belongs to competing components");
                }
            }
            let calls = scope.recursive_calls.span(component.calls).ok_or(
                CodecError::MalformedStructuralFoundation(
                    "checked recursive component lost its calls",
                ),
            )?;
            for call in calls {
                if !members.contains(&call.caller) || !members.contains(&call.callee) {
                    return malformed("checked recursive call leaves its component");
                }
                let machine = scope
                    .calls
                    .machines()
                    .iter()
                    .find_map(|(_, row)| (row.machine == call.caller).then_some(row))
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "checked recursive caller has no call summary",
                    ))?;
                let edges = scope.calls.calls.span(machine.calls).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "checked recursive caller lost its calls",
                    ),
                )?;
                let edge = edges.get(call.call_ordinal as usize).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "checked recursive call ordinal is absent",
                    ),
                )?;
                let callee = scope.plan.machines.iter().find_map(|(_, row)| {
                    (row.machine == edge.callee
                        || scope
                            .plan
                            .symbols
                            .span_or_empty(row.states)
                            .contains(&edge.callee))
                    .then_some(row.machine)
                });
                if callee != Some(call.callee) {
                    return malformed("checked recursive call differs from its exact body edge");
                }
            }
        }
        let mut callables = BTreeSet::new();
        for (_, row) in scope.callables.iter() {
            let terminal_psi::ProofCallableAssociation::Callable {
                declaration: owner,
                callable,
            } = row
            else {
                return malformed("proof scope callable association is absent");
            };
            declaration(*owner)?;
            if !bound_declarations.insert(*owner) || !callables.insert(*callable) {
                return malformed("proof scope has an ambiguous emitted callable association");
            }
        }
        let plan = &scope.plan;
        for (_, identity) in plan.symbols.iter() {
            declaration(*identity)?;
        }
        let mut machines = BTreeSet::new();
        let mut state_owners: BTreeMap<_, _> = plan
            .machines
            .iter()
            .map(|(_, row)| (row.machine, row.machine))
            .collect();
        for (_, row) in plan.machines.iter() {
            declaration(row.machine)?;
            if !machines.insert(row.machine) {
                return malformed("proof scope repeats a machine");
            }
            let mut local_states = BTreeSet::new();
            for state in
                plan.symbols
                    .span(row.states)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "proof scope lost its state span",
                    ))?
            {
                if !local_states.insert(*state)
                    || state_owners
                        .insert(*state, row.machine)
                        .is_some_and(|owner| owner != row.machine)
                {
                    return malformed("proof scope state has ambiguous ownership");
                }
            }
            let binders =
                plan.symbols
                    .span(row.binders)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "proof scope lost its binder span",
                    ))?;
            if binders.iter().collect::<BTreeSet<_>>().len() != binders.len() {
                return malformed("proof scope repeats a static binder");
            }
            for target in [row.authored_target.as_deref(), row.source_target.as_deref()]
                .into_iter()
                .flatten()
            {
                if target.is_empty() {
                    return malformed("proof scope has an empty target");
                }
            }
            let mut targets = BTreeSet::new();
            for member in plan.target_members.span_or_empty(row.family_members) {
                declaration(member.machine)?;
                let Some(body) = plan
                    .machines
                    .iter()
                    .map(|(_, candidate)| candidate)
                    .find(|candidate| candidate.machine == member.machine)
                else {
                    return malformed("proof scope family member has no machine");
                };
                if body.authored_target.as_deref() != Some(member.target.as_str())
                    || body.family_interface
                    || plan.symbols.span_or_empty(body.binders).len()
                        != plan.symbols.span_or_empty(row.binders).len()
                {
                    return malformed(
                        "proof scope family member has a different target or binder roster",
                    );
                }
                if member.target.is_empty() || !targets.insert(&member.target) {
                    return malformed("proof scope repeats or omits a family target");
                }
            }
        }
        for (_, row) in plan.declaration_dependencies.iter() {
            declaration(row.declaration)?;
        }
        for (_, (requirement, realization)) in plan.provider_candidates.iter() {
            declaration(*requirement)?;
            declaration(*realization)?;
        }
        let graph = &scope.calls;
        let mut summaries = BTreeSet::new();
        for (_, row) in graph.machines().iter() {
            if !machines.contains(&row.machine) || !summaries.insert(row.machine) {
                return malformed("proof scope has an unknown or repeated call summary");
            }
        }
        for (_, call) in graph.calls.iter() {
            for identity in [call.binder, call.callee, call.state, call.operator] {
                if identity.0 != 0 {
                    declaration(identity)?;
                }
            }
        }
        for arguments in graph
            .calls
            .iter()
            .map(|(_, call)| call.arguments)
            .chain(graph.bindings.iter().map(|(_, binding)| binding.arguments))
        {
            let mut parameters = BTreeSet::new();
            for binding in
                graph
                    .bindings
                    .span(arguments)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "proof scope lost its binding span",
                    ))?
            {
                if !parameters.insert(binding.parameter) {
                    return malformed("proof scope static application repeats a parameter");
                }
            }
        }
        for (_, binding) in graph.bindings.iter() {
            declaration(binding.parameter)?;
            declaration(binding.selected)?;
        }
        for (_, cleanup) in graph.cleanup.iter() {
            declaration(*cleanup)?;
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use semantic_vocabulary::MachineId;

        use super::*;
        use arena::HandleSpan;
        use semantic_vocabulary::proof_applications::{
            ProofApplicationCall, ProofApplicationMachine, ProofApplicationTargetMember,
            ProofStaticBinding,
        };
        use terminal_psi::TerminalIdentityTableBuilder;

        fn scope() -> (ProofApplicationScope, TerminalIdentityTable) {
            let mut identities = TerminalIdentityTableBuilder::default();
            let identity = identities.intern("same nominal name");
            let mut scope = ProofApplicationScope::default();
            for _ in 0..4 {
                scope
                    .declarations
                    .insert(ProofScopeDeclaration::Nominal(identity));
            }
            let members = scope
                .plan
                .target_members
                .insert_many([ProofApplicationTargetMember {
                    target: "macos_arm64".into(),
                    machine: ProofDeclarationId(2),
                }]);
            scope.plan.machines.insert(ProofApplicationMachine {
                machine: ProofDeclarationId(1),
                family_interface: true,
                family_members: members,
                product_selected: true,
                ..Default::default()
            });
            scope.plan.machines.insert(ProofApplicationMachine {
                machine: ProofDeclarationId(2),
                authored_target: Some("macos_arm64".into()),
                product_selected: true,
                ..Default::default()
            });
            (scope, identities.build())
        }

        #[test]
        fn proof_scope_rejects_missing_wrong_target_and_mismatched_family_members() {
            let (scope, identities) = scope();
            validate(&scope, &identities).unwrap();
            let mut missing = scope.clone();
            missing
                .plan
                .target_members
                .for_each_mut(|_, member| member.machine = ProofDeclarationId(3));
            assert!(matches!(
                validate(&missing, &identities),
                Err(CodecError::MalformedStructuralFoundation(
                    "proof scope family member has no machine"
                ))
            ));
            let mut wrong = scope.clone();
            wrong.plan.machines.for_each_mut(|_, row| {
                if row.machine == ProofDeclarationId(2) {
                    row.authored_target = Some("windows_x86_64".into());
                }
            });
            assert!(validate(&wrong, &identities).is_err());
            let mut binders = scope;
            let binder = binders.plan.symbols.insert_many([ProofDeclarationId(3)]);
            binders.plan.machines.for_each_mut(|_, row| {
                if row.machine == ProofDeclarationId(2) {
                    row.binders = binder;
                }
            });
            assert!(validate(&binders, &identities).is_err());
        }

        #[test]
        fn proof_scope_rejects_ambiguous_state_owners_and_static_bindings() {
            let (mut scope, identities) = scope();
            let state = scope.plan.symbols.insert_many([ProofDeclarationId(3)]);
            scope
                .plan
                .machines
                .for_each_mut(|_, row| row.states = state);
            assert!(matches!(
                validate(&scope, &identities),
                Err(CodecError::MalformedStructuralFoundation(
                    "proof scope state has ambiguous ownership"
                ))
            ));
            scope
                .plan
                .machines
                .for_each_mut(|_, row| row.states = HandleSpan::default());
            let arguments = scope.calls.bindings.insert_many([
                ProofStaticBinding {
                    parameter: ProofDeclarationId(3),
                    selected: ProofDeclarationId(1),
                    ..Default::default()
                },
                ProofStaticBinding {
                    parameter: ProofDeclarationId(3),
                    selected: ProofDeclarationId(2),
                    ..Default::default()
                },
            ]);
            scope.calls.calls.insert(ProofApplicationCall {
                arguments,
                ..Default::default()
            });
            assert!(matches!(
                validate(&scope, &identities),
                Err(CodecError::MalformedStructuralFoundation(
                    "proof scope static application repeats a parameter"
                ))
            ));
        }

        #[test]
        fn proof_scope_rejects_ambiguous_callable_associations() {
            let (scope, identities) = scope();
            let body = |number| {
                terminal_psi::TargetMachineCallable::Machine(MachineId::new(number).unwrap())
            };
            for (second_owner, second_callable) in [(1, body(2)), (2, body(1))] {
                let mut duplicate = scope.clone();
                for (owner, callable) in [(1, body(1)), (second_owner, second_callable)] {
                    duplicate
                        .callables
                        .insert(terminal_psi::ProofCallableAssociation::Callable {
                            declaration: ProofDeclarationId(owner),
                            callable,
                        });
                }
                assert!(matches!(
                    validate(&duplicate, &identities),
                    Err(CodecError::MalformedStructuralFoundation(
                        "proof scope has an ambiguous emitted callable association"
                    ))
                ));
            }
        }
    }
}
pub(crate) mod reference_outputs {

    use super::{CodecError, TerminalModule, malformed};
    use terminal_psi::{
        BoundaryLifetimeDependency, BoundaryMachineDeclaration, BoundaryReferenceDestination,
        StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
    };
    use terminal_semantics::descriptor_results::{
        descriptor_value_access, provider_outputs_cover_writable_slots,
    };

    pub(crate) fn validate(
        module: &TerminalModule,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<(), CodecError> {
        if terminal_semantics::closed_conformances::is_checked_body_family_contract(
            module, boundary,
        ) {
            return if boundary.reference_outputs.is_empty() {
                Ok(())
            } else {
                malformed("checked-body family contract carries opaque reference outputs")
            };
        }
        if !provider_outputs_cover_writable_slots(&module.structural_types, boundary) {
            return malformed("boundary reference parameter contract has incomplete leaf coverage");
        }
        let parameter = |position: u32| {
            boundary
                .structural_parameters
                .get(position as usize)
                .filter(|parameter| parameter.position == position)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "boundary reference contract names an absent parameter",
                ))
        };
        let mut destinations = std::collections::BTreeSet::new();
        for output in &boundary.reference_outputs {
            if !destinations.insert((output.destination, output.path.clone())) {
                return malformed("boundary reference contract repeats an output destination");
            }
            let (root, multiplicity) = match output.destination {
                BoundaryReferenceDestination::Parameter(position) => {
                    let parameter = parameter(position)?;
                    if !matches!(
                        parameter.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                    ) {
                        return malformed(
                            "boundary reference output has no writable parameter storage",
                        );
                    }
                    (parameter.structural_type, parameter.multiplicity)
                }
            };
            validate_leaf(
                module,
                root,
                multiplicity,
                &output.path,
                output.access,
                false,
            )?;
            if output.dependencies.is_empty() {
                return malformed("boundary reference output has no lifetime anchor");
            }
            let mut dependencies = std::collections::BTreeSet::new();
            for dependency in &output.dependencies {
                if !dependencies.insert(dependency) {
                    return malformed("boundary reference output repeats a lifetime anchor");
                }
                match dependency {
                    BoundaryLifetimeDependency::Referent {
                        argument_index,
                        access,
                    } => {
                        let parameter = parameter(*argument_index)?;
                        if *access == StructuralAccess::Owned || parameter.access != *access {
                            return malformed(
                                "boundary reference lifetime anchor differs from formal access",
                            );
                        }
                    }
                    BoundaryLifetimeDependency::Stored {
                        argument_index,
                        path,
                        access,
                    } => {
                        let parameter = parameter(*argument_index)?;
                        if parameter.access == StructuralAccess::WriteOnlyBorrow {
                            return malformed(
                                "boundary reference lifetime anchor reads write-only storage",
                            );
                        }
                        validate_leaf(
                            module,
                            parameter.structural_type,
                            parameter.multiplicity,
                            path,
                            *access,
                            parameter.access == StructuralAccess::SharedBorrow,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_leaf(
        module: &TerminalModule,
        root: semantic_vocabulary::StructuralTypeId,
        multiplicity: StructuralMultiplicity,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        shared_storage: bool,
    ) -> Result<(), CodecError> {
        if path.iter().any(|segment| {
            !matches!(
                segment,
                StructuralPathSegment::Field(_)
                    | StructuralPathSegment::Case(_)
                    | StructuralPathSegment::FixedIndex(_)
            )
        }) {
            return malformed("boundary reference contract has a nonstorage projection");
        }
        let selected = terminal_semantics::runtime_structural_path_tip(
            module.structural_types.iter(),
            root,
            path,
        )
        .ok_or(CodecError::MalformedStructuralFoundation(
            "boundary reference contract has an invalid storage path",
        ))?;
        let declared = match selected.shape {
            StructuralTypeShape::Reference { access, .. } => Some(access),
            _ => descriptor_value_access(module.structural_types.iter(), selected.id, multiplicity),
        };
        let declared = if shared_storage {
            match declared {
                Some(StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow) => {
                    Some(StructuralAccess::SharedBorrow)
                }
                _ => None,
            }
        } else {
            declared
        };
        if access == StructuralAccess::Owned || declared != Some(access) {
            return malformed(
                "boundary reference contract endpoint has different access or is not a descriptor",
            );
        }
        Ok(())
    }
}
pub(crate) mod scalar_applications {

    use crate::codec_error::{CodecError, malformed};
    use semantic_vocabulary::{IntegerCarrier, IntegerSign, ScalarType};
    use terminal_psi::{
        LayoutMeasurement, ScalarApplication, ScalarConditionSubject, ScalarRangeCondition,
        ScalarRangeObservation, TargetMachineCallable, TerminalModule,
    };

    pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
        super::proof_scope::validate(&module.scalar_applications.proof_scope, &module.identities)?;
        let scope = &module.scalar_applications.proof_scope;
        for (_, row) in scope.callables.iter() {
            let terminal_psi::ProofCallableAssociation::Callable {
                declaration,
                callable,
            } = row
            else {
                return malformed("proof scope callable association is absent");
            };
            let emitted = match callable {
                TargetMachineCallable::Machine(body) => module
                    .machine(*body)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "proof scope names an absent emitted body",
                    ))?
                    .declaration
                    .as_ref(),
                TargetMachineCallable::Boundary(boundary) => {
                    let boundary = module.boundary_machine(*boundary).ok_or(
                        CodecError::MalformedStructuralFoundation(
                            "proof scope names an absent emitted boundary",
                        ),
                    )?;
                    if boundary.declaration.is_none()
                        && boundary.kind == terminal_psi::BoundaryRequirementKind::Operator
                    {
                        validate_operator_association(
                            module,
                            boundary,
                            scope.declaration(*declaration),
                        )?;
                    }
                    boundary.declaration.as_ref().map(|row| &row.source)
                }
            };
            if let Some(emitted) = emitted
                && !matches!(scope.declaration(*declaration), Some(terminal_psi::ProofScopeDeclaration::Callable(retained)) if retained == emitted)
            {
                return malformed("proof scope callable differs from its emitted declaration");
            }
        }
        for declaration in &module.structural_types {
            if let terminal_psi::StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Application(application),
                ..
            } = declaration.shape
            {
                let scalar_type = module
                    .scalar_applications
                    .declaration(application)
                    .and_then(|declaration| declaration.scalar_type());
                if !matches!(scalar_type, Some(ScalarType::Integer(_))) {
                    return malformed("array extent application has no integer result carrier");
                }
            }
        }
        for application in module.scalar_applications.declarations() {
            match *application {
                ScalarApplication::Missing => {
                    return malformed("scalar application catalog contains the absent sentinel");
                }
                ScalarApplication::LayoutObservation {
                    subject,
                    semantics_version,
                    scalar_type,
                    ..
                } => {
                    if module.structural_type(subject).is_none() {
                        return malformed(
                            "layout observation has no structural subject declaration",
                        );
                    }
                    if semantics_version != LayoutMeasurement::SEMANTICS_VERSION {
                        return malformed("layout observation semantics version is unsupported");
                    }
                    if !matches!(scalar_type, ScalarType::Integer(integer)
                        if integer.carrier() == IntegerCarrier::Fixed
                            && integer.sign() == IntegerSign::Unsigned
                            && integer.bits() == 64)
                    {
                        return malformed(
                            "layout observation result is not the byte-count carrier",
                        );
                    }
                }
                ScalarApplication::Evaluate { entry, scalar_type } => {
                    let signature_matches = match entry {
                        TargetMachineCallable::Machine(machine) => {
                            module.machine(machine).is_some_and(|machine| {
                                machine.parameters.is_empty()
                                    && machine.structural_parameters.is_empty()
                                    && machine
                                        .result
                                        .scalar_ref()
                                        .is_some_and(|result| result.scalar_type == scalar_type)
                            })
                        }
                        TargetMachineCallable::Boundary(boundary) => {
                            module.boundary_machine(boundary).is_some_and(|boundary| {
                                boundary.scalar_parameters.is_empty()
                                    && boundary.structural_parameters.is_empty()
                                    && boundary.result.scalar() == Some(scalar_type)
                            })
                        }
                    };
                    if !signature_matches {
                        return malformed(
                            "scalar application entry has a different closed signature",
                        );
                    }
                }
            }
        }
        let conditions = &module.scalar_applications.conditions;
        for condition in conditions.completions() {
            let terminal_psi::ScalarCompletionCondition::Complete {
                subject,
                application,
            } = condition
            else {
                return malformed("scalar completion is absent");
            };
            validate_subject(module, subject)?;
            if !matches!(
                module.scalar_applications.declaration(*application),
                Some(
                    ScalarApplication::Evaluate { .. }
                        | ScalarApplication::LayoutObservation { .. }
                )
            ) {
                return malformed("scalar completion has no defined application");
            }
        }
        for condition in conditions.ranges() {
            let ScalarRangeCondition::Range { subject, .. } = condition else {
                return malformed("scalar condition is absent");
            };
            validate_subject(module, subject)?;
            conditions
                .steps(condition)
                .map_err(CodecError::MalformedStructuralFoundation)?;
        }
        for condition in conditions.predicates() {
            validate_subject(module, &condition.subject)?;
        }
        for observation in conditions.observations() {
            match observation {
                ScalarRangeObservation::Missing => {
                    return malformed("scalar condition contains an absent observation");
                }
                ScalarRangeObservation::Literal(_) => {}
                ScalarRangeObservation::Application(application) => {
                    if !module
                        .scalar_applications
                        .declaration(*application)
                        .and_then(|application| application.scalar_type())
                        .is_some_and(|scalar_type| scalar_type.as_integer().is_some())
                    {
                        return malformed("scalar bound observation has no integer application");
                    }
                }
                ScalarRangeObservation::EntryResult {
                    application,
                    machine,
                    result,
                    scalar_type,
                } => {
                    let Some(ScalarApplication::Evaluate {
                        entry: TargetMachineCallable::Machine(entry),
                        ..
                    }) = module.scalar_applications.declaration(*application)
                    else {
                        return malformed("scalar bound result has no ordinary evaluation entry");
                    };
                    if entry != machine || scalar_type.as_integer().is_none() {
                        return malformed(
                            "scalar bound result differs from its exact entry or carrier",
                        );
                    }
                    let found = module.machine(*machine).and_then(|machine| {
                        machine
                            .operations()
                            .filter_map(|operation| operation.result.as_scalar())
                            .find(|value| value.id == *result)
                    });
                    if !found.is_some_and(|value| value.scalar_type == *scalar_type) {
                        return malformed(
                            "scalar bound result has no exact integer-producing occurrence",
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_operator_association(
        module: &TerminalModule,
        boundary: &terminal_psi::BoundaryMachineDeclaration,
        declaration: Option<&terminal_psi::ProofScopeDeclaration>,
    ) -> Result<(), CodecError> {
        let Some(terminal_psi::ProofScopeDeclaration::Operator(requirement)) = declaration else {
            return malformed("proof scope operator has no exact requirement declaration");
        };
        if requirement.declaration() != &boundary.owner
            || module.identities.text(boundary.identity) != Some(requirement.overload())
        {
            return malformed("proof scope operator differs from its emitted requirement");
        }
        for operation in module.operations() {
            if !matches!(operation.kind, terminal_psi::OperationKind::BoundaryCall { boundary: called, .. } if called == boundary.id)
            {
                continue;
            }
            if let Some(demand) = module
                .boundary_application_demands
                .iter()
                .find(|demand| demand.terminal_operation() == operation.id)
                && demand.requirement() != requirement
            {
                return malformed("proof scope operator differs from its application demand");
            }
        }
        Ok(())
    }

    fn validate_subject(
        module: &TerminalModule,
        subject: &ScalarConditionSubject,
    ) -> Result<(), CodecError> {
        let has_subject = match subject {
            ScalarConditionSubject::Module => true,
            ScalarConditionSubject::Declaration(declaration) => module
                .scalar_applications
                .proof_scope
                .declaration(*declaration)
                .is_some(),
            ScalarConditionSubject::Application(application) => module
                .scalar_applications
                .declaration(*application)
                .is_some(),
        };
        if !has_subject {
            return malformed("scalar condition lost its exact subject");
        }

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::sections::semantic_module::target_family_tests::{identity_table, unit_machine};
        use semantic_vocabulary::MachineId;
        use terminal_psi::{
            ProofCallableAssociation, ProofDeclarationId, ProofScopeDeclaration,
            TerminalIdentityIndex,
        };

        #[test]
        fn proof_scope_refuses_swapped_same_nominal_callable_descriptions() {
            let first = MachineId::new(1).unwrap();
            let second = MachineId::new(2).unwrap();
            let mut module = TerminalModule::for_entry(first);
            module.identities = identity_table(10);
            for (position, machine) in [(1, first), (2, second)] {
                let mut body = unit_machine(machine);
                let declaration = body.declaration.as_mut().unwrap();
                declaration.semantic_declaration = TerminalIdentityIndex(3);
                declaration.callable = TerminalIdentityIndex(4);
                declaration.result_type = TerminalIdentityIndex(5);
                declaration.specialization = Some(terminal_psi::CheckedMachineSpecialization {
                    template: declaration.machine.clone(),
                    template_callable: declaration.callable,
                    template_semantic_declaration: declaration.semantic_declaration,
                    commitment: [position as u8; 32],
                    capture: None,
                });
                module
                    .scalar_applications
                    .proof_scope
                    .declarations
                    .insert(ProofScopeDeclaration::Callable(declaration.clone()));
                module.scalar_applications.proof_scope.callables.insert(
                    ProofCallableAssociation::Callable {
                        declaration: ProofDeclarationId(position),
                        callable: TargetMachineCallable::Machine(machine),
                    },
                );
                module.machines.push(body);
            }
            validate(&module).unwrap();
            module
                .scalar_applications
                .proof_scope
                .callables
                .for_each_mut(|_, row| {
                    if let ProofCallableAssociation::Callable { declaration, .. } = row {
                        declaration.0 = 3 - declaration.0;
                    }
                });
            assert!(matches!(
                validate(&module),
                Err(CodecError::MalformedStructuralFoundation(
                    "proof scope callable differs from its emitted declaration"
                ))
            ));
        }

        #[test]
        fn proof_scope_refuses_swapped_operator_associations_without_machine_descriptions() {
            let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let overload = identities.intern("operator::choose(u64,u64)->u64");
            let owner =
                terminal_psi::NominalDeclarationIdentity::new(identities.intern("package::choose"));
            let other_owner =
                terminal_psi::NominalDeclarationIdentity::new(identities.intern("other::choose"));
            module.identities = identities.build();
            let boundary = terminal_psi::BoundaryMachineDeclaration {
                id: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
                declaration: None,
                numeric_intrinsic: None,
                role: terminal_psi::BoundaryDeclarationRole::Callable,
                kind: terminal_psi::BoundaryRequirementKind::Operator,
                completion: terminal_psi::BoundaryCompletion::NormalReturn,
                unconditional_termination: false,
                identity: overload,
                owner: owner.clone(),
                attachment: None,
                parameter_order: Vec::new(),
                scalar_parameters: Vec::new(),
                crash_routes: Vec::new(),
                structural_parameters: Vec::new(),
                result: terminal_psi::BoundaryMachineResult::Unit,
                requires: Vec::new(),
                normal_qualifications: Vec::new(),
                scalar_domain_requirements: Vec::new(),
                scalar_requires: Vec::new(),
                scalar_ensures: Vec::new(),
                result_reference_regions: Vec::new(),
                reference_outputs: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                fixed_service_reach: Vec::new(),
                owning_service: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
            };
            module.boundary_machines.push(boundary);
            let description = |owner, overload: &str| {
                ProofScopeDeclaration::Operator(
                    terminal_psi::BoundaryOperatorRequirement::new(owner, overload.into(), None)
                        .unwrap(),
                )
            };
            let scope = &mut module.scalar_applications.proof_scope;
            for row in [
                description(owner.clone(), "operator::choose(u64,u64)->u64"),
                description(other_owner, "operator::choose(u64,u64)->u64"),
                description(owner, "operator::choose(u32,u32)->u32"),
                ProofScopeDeclaration::Nominal(overload),
            ] {
                scope.declarations.insert(row);
            }
            let association = scope.callables.insert(ProofCallableAssociation::Callable {
                declaration: ProofDeclarationId(1),
                callable: TargetMachineCallable::Boundary(module.boundary_machines[0].id),
            });
            validate(&module).unwrap();
            for owner in 2..=4 {
                let ProofCallableAssociation::Callable { declaration, .. } = module
                    .scalar_applications
                    .proof_scope
                    .callables
                    .get_mut(association)
                else {
                    unreachable!()
                };
                *declaration = ProofDeclarationId(owner);
                assert!(
                    validate(&module).is_err(),
                    "wrong owner/overload or nominal fallback {owner}"
                );
            }
        }
    }
}
pub(crate) mod scalar_boundary_contracts {

    use super::{CodecError, TerminalMachine, TerminalModule, malformed};
    use semantic_vocabulary::{ScalarDomainId, ScalarQualificationSetId, ValueId};
    use terminal_psi::{BoundaryMachineDeclaration, ValueDeclaration};

    fn domains(
        module: &TerminalModule,
        set: ScalarQualificationSetId,
    ) -> Result<&[ScalarDomainId], CodecError> {
        super::qualification_domains(module, set).ok_or(CodecError::MalformedStructuralFoundation(
            "boundary scalar value names an absent qualification set",
        ))
    }

    pub(crate) fn validate_declarations(module: &TerminalModule) -> Result<(), CodecError> {
        for result in module
            .boundary_machines
            .iter()
            .map(|boundary| &boundary.result)
            .chain(
                module
                    .dynamic_dispatch
                    .interfaces
                    .iter()
                    .flat_map(|interface| interface.slots.iter().map(|slot| &slot.result)),
            )
        {
            if let terminal_psi::BoundaryMachineResult::Scalar(result) = result {
                for membership in domains(module, result.qualifications)? {
                    if !module.scalar_qualifications.domains.iter().any(|domain| {
                        domain.id == *membership && domain.carrier == result.scalar_type
                    }) {
                        return malformed(
                            "boundary scalar result qualification changes its carrier",
                        );
                    }
                }
            }
        }
        for boundary in &module.boundary_machines {
            for requirement in &boundary.scalar_domain_requirements {
                let carrier = boundary
                    .scalar_parameters
                    .get(requirement.argument_index as usize)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "boundary scalar domain requirement has no parameter",
                    ))?;
                let domain = super::scalar_domain(module, requirement.domain).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "boundary scalar requirement names an absent domain",
                    ),
                )?;
                if domain.carrier != *carrier {
                    return malformed("boundary scalar domain requirement changes its carrier");
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_arguments(
        module: &TerminalModule,
        machine: &TerminalMachine,
        boundary: &BoundaryMachineDeclaration,
        arguments: &[ValueId],
    ) -> Result<(), CodecError> {
        for requirement in &boundary.scalar_domain_requirements {
            let argument = arguments.get(requirement.argument_index as usize).ok_or(
                CodecError::MalformedStructuralFoundation(
                    "boundary scalar domain requirement has no actual argument",
                ),
            )?;
            let actual = super::values(machine)
                .find(|value| value.id == *argument)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "boundary scalar membership has no exact value declaration",
                ))?;
            if !domains(module, actual.qualifications)?.contains(&requirement.domain) {
                return malformed("boundary scalar argument lacks its required domain identity");
            }
        }
        Ok(())
    }

    pub(crate) fn validate_provider(
        module: &TerminalModule,
        boundary: &BoundaryMachineDeclaration,
        parameters: &[ValueDeclaration],
    ) -> Result<(), CodecError> {
        for (argument_index, parameter) in parameters.iter().enumerate() {
            for domain in domains(module, parameter.qualifications)? {
                if !boundary.scalar_domain_requirements.iter().any(|required| {
                    required.argument_index as usize == argument_index && required.domain == *domain
                }) {
                    return malformed(
                        "provider candidate requires scalar membership absent from its boundary",
                    );
                }
            }
        }
        Ok(())
    }
}
pub(crate) mod scalar_fields {

    use super::{CodecError, TerminalMachine, TerminalModule, malformed};
    use semantic_vocabulary::{
        ScalarDomainId, ScalarQualificationSetId, StructuralFieldId, ValueId,
    };
    use terminal_psi::{
        OperationKind, RecordFieldValue, StructuralFieldDeclaration, StructuralTypeShape,
    };
    fn fields(module: &TerminalModule) -> impl Iterator<Item = &StructuralFieldDeclaration> {
        module.structural_types.iter().flat_map(|owner| {
            let (fields, cases) = match &owner.shape {
                StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
                StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
                StructuralTypeShape::Mixed { fields, cases } => {
                    (fields.as_slice(), cases.as_slice())
                }
                _ => (&[][..], &[][..]),
            };
            fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
        })
    }

    fn domains(
        module: &TerminalModule,
        set: ScalarQualificationSetId,
    ) -> Result<&[ScalarDomainId], CodecError> {
        super::qualification_domains(module, set).ok_or(CodecError::MalformedStructuralFoundation(
            "stored scalar names an absent qualification set",
        ))
    }

    fn required(
        module: &TerminalModule,
        field: StructuralFieldId,
    ) -> Result<&[ScalarDomainId], CodecError> {
        module
            .scalar_qualifications
            .fields
            .iter()
            .find(|row| row.field == field)
            .map_or(Ok(&[][..]), |row| {
                domains(module, row.value_type.qualifications)
            })
    }

    fn validate_write(
        module: &TerminalModule,
        machine: &TerminalMachine,
        field: StructuralFieldId,
        value: ValueId,
    ) -> Result<(), CodecError> {
        let required = required(module, field)?;
        if required.is_empty() {
            return Ok(());
        }
        let value = super::values(machine).find(|row| row.id == value).ok_or(
            CodecError::MalformedStructuralFoundation(
                "scalar field write lost its exact SSA source",
            ),
        )?;
        let actual = domains(module, value.qualifications)?;
        if required.iter().any(|domain| !actual.contains(domain)) {
            return malformed("scalar field write lacks its required domain identity");
        }
        Ok(())
    }

    pub(crate) fn validate_primitive_reference_presentation(
        module: &TerminalModule,
        machine: &TerminalMachine,
        place: semantic_vocabulary::PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        access: terminal_psi::StructuralAccess,
    ) -> Result<(), CodecError> {
        if access == terminal_psi::StructuralAccess::SharedBorrow {
            return Ok(());
        }
        if let Some(field) = storage_field(module, machine, place, path)?
            && !required(module, field)?.is_empty()
        {
            return malformed("primitive reference loses scalar field membership");
        }
        Ok(())
    }

    fn storage_field(
        module: &TerminalModule,
        machine: &TerminalMachine,
        place: semantic_vocabulary::PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<Option<StructuralFieldId>, CodecError> {
        let root = super::structural_place_type(machine, place).ok_or(
            CodecError::MalformedStructuralFoundation(
                "scalar storage reference lost its declaration",
            ),
        )?;
        Ok(
            terminal_semantics::runtime_structural_field(
                module.structural_types.iter(),
                root,
                path,
            )
            .map(|field| field.id),
        )
    }

    pub(crate) fn validate_storage_interpretation(
        module: &TerminalModule,
        source: semantic_vocabulary::StructuralTypeId,
        reference: semantic_vocabulary::StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    ) -> Result<(), CodecError> {
        let Some(StructuralTypeShape::Reference { referent, .. }) =
            module.structural_type(reference).map(|row| &row.shape)
        else {
            return malformed("scalar field storage interpretation lost its reference type");
        };
        let read_only = access == terminal_psi::StructuralAccess::SharedBorrow
            && terminal_semantics::shared_mutation(module.structural_types.iter(), *referent, &[])
                == terminal_semantics::SharedMutation::None;
        if contains_membership(module, *referent)?
            || !read_only && contains_membership(module, source)?
        {
            return malformed(
                "storage reinterpretation lacks scalar field membership correspondence",
            );
        }
        Ok(())
    }

    fn contains_membership(
        module: &TerminalModule,
        root: semantic_vocabulary::StructuralTypeId,
    ) -> Result<bool, CodecError> {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(current) = pending.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.push(current);
            let declaration = module.structural_type(current).ok_or(
                CodecError::MalformedStructuralFoundation(
                    "scalar field storage interpretation lost a stored type",
                ),
            )?;
            let (fields, cases) = match &declaration.shape {
                StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
                StructuralTypeShape::Mixed { fields, cases } => {
                    (fields.as_slice(), cases.as_slice())
                }
                StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
                StructuralTypeShape::FixedArray { element, length } => {
                    if length.literal() != Some(0) {
                        pending.push(*element);
                    }
                    continue;
                }
                _ => continue,
            };
            for field in fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .filter(|field| !field.relevance.is_erased())
            {
                if !required(module, field.id)?.is_empty() {
                    return Ok(true);
                }
                if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                    pending.push(child);
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
        for row in &module.scalar_qualifications.fields {
            let mut matching = fields(module).filter(|field| field.id == row.field);
            let first = matching
                .next()
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "scalar field membership lost its declaration",
                ))?;
            for field in std::iter::once(first).chain(matching) {
                if field.relevance.is_erased()
                    || matches!(
                        field.field_type,
                        terminal_psi::StructuralFieldType::Atomic(_)
                            | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
                    )
                    || field.field_type.scalar_type() != Some(row.value_type.scalar_type)
                    || row.value_type.qualifications.is_empty()
                {
                    return malformed("scalar field membership changes its declared carrier");
                }
            }
            for domain in domains(module, row.value_type.qualifications)? {
                if super::scalar_domain(module, *domain)
                    .is_none_or(|entry| entry.carrier != row.value_type.scalar_type)
                {
                    return malformed("scalar field membership has a different domain carrier");
                }
            }
        }
        for machine in &module.machines {
            for operation in machine.operations() {
                match &operation.kind {
                    OperationKind::EstablishZeroedStructural {} => {
                        let result = operation.result.structural().ok_or(
                            CodecError::MalformedStructuralFoundation(
                                "zero provisioning lost its structural result",
                            ),
                        )?;
                        if !terminal_semantics::zeroed_structural_fields_satisfy(
                            module.structural_types.iter(),
                            result.structural_type,
                            |field| {
                                required(module, field.id).is_ok_and(|domains| {
                                    domains.iter().all(|id| {
                                        super::scalar_domain(module, *id).is_some_and(|domain| {
                                            domain.predicate.is_none()
                                                && domain.establishment_routes.is_empty()
                                                && domain.has_no_required_memberships()
                                        })
                                    })
                                })
                            },
                        ) {
                            return malformed(
                                "zero provisioning cannot establish scalar field membership",
                            );
                        }
                    }
                    OperationKind::EstablishRecord { fields }
                    | OperationKind::EstablishStructuralCase { fields, .. } => {
                        for field in fields {
                            if let RecordFieldValue::Scalar { value, .. } = field.value {
                                validate_write(module, machine, field.field, value)?;
                            }
                        }
                    }
                    OperationKind::StructuralScalarFieldStore { field, value, .. } => {
                        validate_write(module, machine, *field, *value)?;
                    }
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination,
                        path,
                        value,
                        ..
                    } => {
                        if let Some(field) = storage_field(module, machine, *destination, path)? {
                            validate_write(module, machine, field, *value)?;
                        }
                    }
                    _ => {}
                }
                let Some(result) = operation
                    .result
                    .scalar_ref()
                    .filter(|row| !row.qualifications.is_empty())
                else {
                    continue;
                };
                let field = match &operation.kind {
                    OperationKind::ScalarStructuralField { field, .. }
                    | OperationKind::BooleanStructuralField { field, .. } => Some(*field),
                    OperationKind::PrimitiveScalarRead { source, path } => {
                        storage_field(module, machine, *source, path)?
                    }
                    _ => continue,
                }
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "qualified scalar read lost its exact field declaration",
                ))?;
                let supplied = required(module, field)?;
                if domains(module, result.qualifications)?
                    .iter()
                    .any(|domain| !supplied.contains(domain))
                {
                    return malformed(
                        "scalar field read introduces membership absent from its declaration",
                    );
                }
            }
        }
        Ok(())
    }
}
pub(crate) mod scalar_observations {
    use crate::codec_error::{CodecError, malformed};
    use semantic_vocabulary::{
        PlaceId, Proposition, ScalarTerm, ScalarType, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        BoundaryMachineDeclaration, StructuralAccess, TerminalMachine, TerminalModule,
    };
    use terminal_semantics::{canonical_structural_path_tip, structural_read_scalar_type};

    pub(crate) fn machine(
        module: &TerminalModule,
        machine: &TerminalMachine,
    ) -> Result<(), CodecError> {
        let root = |place| {
            let parameter = machine
                .structural_parameters
                .iter()
                .chain(machine.blocks.iter().flat_map(|b| &b.structural_parameters))
                .find(|p| p.place == place);
            if parameter.is_some_and(|p| p.access == StructuralAccess::WriteOnlyBorrow) {
                return None;
            }
            super::structural_place_type(machine, place).or_else(|| match &machine.result {
                terminal_psi::TerminalMachineResult::Structural(r) if r.place == place => {
                    Some(r.structural_type)
                }
                _ => None,
            })
        };
        let value = |id| {
            super::declared_scalar_type(machine, id).or_else(|| {
                machine
                    .contract
                    .erased_scalar_formals
                    .iter()
                    .chain(machine.blocks.iter().flat_map(|b| &b.erased_scalar_formals))
                    .find(|v| v.id == id)
                    .map(|v| v.scalar_type)
            })
        };
        for proposition in machine
            .contract
            .requires
            .iter()
            .chain(machine.contract.ensures.iter().map(|row| &row.proposition))
            .chain(
                machine
                    .contract
                    .outcome_specific_ensures
                    .iter()
                    .map(|row| &row.proposition),
            )
            .chain(
                machine
                    .contract
                    .crash_routes
                    .iter()
                    .flat_map(|row| &row.alternatives)
                    .filter_map(|guard| match guard {
                        terminal_psi::CrashRouteGuard::Predicate(p) => Some(p.proposition()),
                        _ => None,
                    }),
            )
            .chain(
                module
                    .scalar_block_invariants
                    .iter()
                    .filter(|row| row.machine == machine.id)
                    .map(|row| &row.predicate),
            )
        {
            validate(module, proposition, &root, &value)?;
        }
        for operation in machine.operations() {
            use terminal_psi::OperationKind;
            let (arguments, routes): (&[ScalarTerm], &[terminal_psi::CrashRouteBucket]) =
                match &operation.kind {
                    OperationKind::Call {
                        erased_arguments,
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallUnit {
                        erased_arguments,
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallDynamic {
                        erased_arguments,
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallMachine {
                        erased_arguments,
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallStructuralScalar {
                        erased_arguments,
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        erased_arguments,
                        crash_continuations,
                        ..
                    } => (erased_arguments, crash_continuations),
                    OperationKind::CallStructural {
                        crash_continuations,
                        ..
                    } => (&[], crash_continuations),
                    _ => (&[], &[]),
                };
            validate_terms(
                module,
                arguments.iter().map(|term| (term, false)).collect(),
                &root,
                &value,
            )?;
            for route in routes.iter().flat_map(|row| &row.alternatives) {
                if let terminal_psi::CrashRouteGuard::Predicate(predicate) = route {
                    validate(module, predicate.proposition(), &root, &value)?;
                }
            }
        }
        for block in &machine.blocks {
            use terminal_psi::Terminator;
            if let Some(invocation) = block.terminator.module_invocation() {
                validate_terms(
                    module,
                    invocation
                        .erased_arguments
                        .iter()
                        .map(|term| (term, false))
                        .collect(),
                    &root,
                    &value,
                )?;
                for route in invocation
                    .crash_continuations
                    .iter()
                    .flat_map(|row| &row.alternatives)
                {
                    if let terminal_psi::CrashRouteGuard::Predicate(predicate) = route {
                        validate(module, predicate.proposition(), &root, &value)?;
                    }
                }
            }
            let arguments = match &block.terminator {
                Terminator::Jump {
                    erased_arguments, ..
                } => erased_arguments.iter().collect(),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => when_true
                    .erased_arguments
                    .iter()
                    .chain(&when_false.erased_arguments)
                    .collect(),
                Terminator::Crash { site_guard, .. } => {
                    for predicate in site_guard {
                        validate(module, predicate.proposition(), &root, &value)?;
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            };
            validate_terms(
                module,
                arguments.into_iter().map(|term| (term, false)).collect(),
                &root,
                &value,
            )?;
        }
        Ok(())
    }

    pub(crate) fn boundary(
        module: &TerminalModule,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<(), CodecError> {
        let root = |place| {
            boundary
                .structural_parameters
                .iter()
                .find(|p| p.place == place && p.access != StructuralAccess::WriteOnlyBorrow)
                .map(|p| p.structural_type)
        };
        let value = |id: ValueId| {
            usize::try_from(id.get())
                .ok()
                .and_then(|position| position.checked_sub(1))
                .and_then(|position| {
                    boundary
                        .scalar_parameters
                        .get(position)
                        .copied()
                        .or_else(|| {
                            (position == boundary.scalar_parameters.len())
                                .then(|| boundary.result.scalar())
                                .flatten()
                        })
                })
        };
        for proposition in boundary
            .scalar_requires
            .iter()
            .chain(&boundary.scalar_ensures)
            .chain(
                boundary
                    .crash_routes
                    .iter()
                    .flat_map(|row| &row.alternatives)
                    .filter_map(|guard| match guard {
                        terminal_psi::CrashRouteGuard::Predicate(p) => Some(p.proposition()),
                        _ => None,
                    }),
            )
        {
            validate(module, proposition, &root, &value)?;
        }
        Ok(())
    }

    fn validate(
        module: &TerminalModule,
        proposition: &Proposition,
        root_type: &impl Fn(PlaceId) -> Option<StructuralTypeId>,
        value_type: &impl Fn(ValueId) -> Option<ScalarType>,
    ) -> Result<(), CodecError> {
        let mut propositions = vec![proposition];
        let mut scalars = Vec::new();
        while let Some(proposition) = propositions.pop() {
            match proposition {
                semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                    let mut pending = vec![left, right];
                    while let Some(term) = pending.pop() {
                        if let semantic_vocabulary::IntegerMathTerm::Embed(term) = term {
                            scalars.push((term.as_ref(), false));
                        } else {
                            pending.extend(term.operands());
                        }
                    }
                }
                Proposition::StructuralIdentity { left, right } => {
                    if left.structural_type() != right.structural_type() {
                        return malformed(
                            "structural identity operands have different declared types",
                        );
                    }
                    for subject in [left, right] {
                        let Some(root) = root_type(subject.root()) else {
                            return malformed("structural identity has no readable declared root");
                        };
                        let Some(endpoint) = canonical_structural_path_tip(
                            module.structural_types.iter(),
                            root,
                            subject.path(),
                        ) else {
                            return malformed("structural identity has no exact declared endpoint");
                        };
                        if endpoint.id != subject.structural_type()
                            || !matches!(
                                endpoint.shape,
                                terminal_psi::StructuralTypeShape::Record { .. }
                                    | terminal_psi::StructuralTypeShape::Sum { .. }
                                    | terminal_psi::StructuralTypeShape::Mixed { .. }
                                    | terminal_psi::StructuralTypeShape::FixedArray { .. }
                            )
                        {
                            return malformed(
                                "structural identity disagrees with its exact data carrier",
                            );
                        }
                    }
                }
                Proposition::Equal(a, b)
                | Proposition::LessThan(a, b)
                | Proposition::LessOrEqual(a, b)
                | Proposition::ScalarIeeeFloatComparison {
                    left: a, right: b, ..
                } => {
                    scalars.push((a, false));
                    scalars.push((b, false));
                }
                Proposition::Conjunction(items) | Proposition::Disjunction(items) => {
                    propositions.extend(items)
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    propositions.push(premise);
                    propositions.push(conclusion);
                }
                _ => {}
            }
        }
        validate_terms(module, scalars, root_type, value_type)
    }

    fn validate_terms(
        module: &TerminalModule,
        mut scalars: Vec<(&ScalarTerm, bool)>,
        root_type: &impl Fn(PlaceId) -> Option<StructuralTypeId>,
        value_type: &impl Fn(ValueId) -> Option<ScalarType>,
    ) -> Result<(), CodecError> {
        while let Some((term, selector)) = scalars.pop() {
            if let ScalarTerm::Application {
                application,
                scalar_type,
            } = term
                && module
                    .scalar_applications
                    .declaration(*application)
                    .and_then(|declaration| declaration.scalar_type())
                    != Some(*scalar_type)
            {
                return malformed("scalar application term has no exact catalog carrier");
            }
            if let ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. }
            | ScalarTerm::IeeeFloatField { root, path, .. } = term
            {
                let Some(root) = root_type(*root) else {
                    return malformed("scalar field predicate has no readable declared root");
                };
                let projections = path
                    .iter()
                    .cloned()
                    .map(semantic_vocabulary::ScalarReadProjection::from)
                    .collect::<Vec<_>>();
                if structural_read_scalar_type(module.structural_types.iter(), root, &projections)
                    != Some(term.scalar_type())
                {
                    return malformed(
                        "scalar field predicate disagrees with its declaration path or scalar carrier",
                    );
                }
            }
            if let ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            } = term
            {
                let Some(root) = root_type(*root) else {
                    return malformed("structural read predicate has no readable declared root");
                };
                if structural_read_scalar_type(module.structural_types.iter(), root, path)
                    != Some(*scalar_type)
                {
                    return malformed(
                        "structural read predicate disagrees with its declaration path or scalar carrier",
                    );
                }
                scalars.extend(term.operands().into_iter().map(|term| (term, true)));
            } else {
                if selector
                    && let ScalarTerm::Value { id, scalar_type } = term
                    && value_type(*id) != Some(*scalar_type)
                {
                    return malformed("structural read selector has no exact scalar declaration");
                }
                scalars.extend(term.operands().into_iter().map(|term| (term, selector)));
            }
        }
        Ok(())
    }
}
pub(crate) mod scalar_qualifications {

    use super::{
        CodecError, TerminalModule, Terminator, malformed, qualification_domains, scalar_domain,
        values,
    };
    use semantic_vocabulary::{Proposition, PropositionContext};
    use terminal_psi::{ScalarDomainEstablishmentRoute, ScalarReturnOrigin, ScalarReturnSite};

    pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
        let catalog = &module.scalar_qualifications;
        for domain in &catalog.domains {
            if !domain.required_memberships_are_well_formed(&catalog.domains) {
                return malformed(
                    "scalar domain has ambiguous, cyclic or wrong-carrier prerequisites",
                );
            }
            if module
                .identities
                .text(domain.identity)
                .is_none_or(str::is_empty)
            {
                return malformed("scalar domain has no retained identity");
            }
            if !domain.establishment_routes.is_empty() && domain.predicate.is_none() {
                return malformed("routed scalar domain lost its predicate component");
            }
            if let Some(predicate) = &domain.predicate {
                PropositionContext::from_value_types([(predicate.subject, domain.carrier)])
                    .and_then(|context| context.validate(&predicate.proposition))
                    .map_err(CodecError::MalformedProposition)?;
                let mut exact = predicate.subject.get() == 1;
                exact &= predicate.proposition.visit_value_ids(|value| {
                    exact &= value == predicate.subject;
                });
                if !exact {
                    return malformed("scalar domain predicate escapes its formal subject");
                }
            }
            if domain.establishment_routes.iter().any(|route| {
                module
                    .identities
                    .text(route.identity())
                    .is_none_or(str::is_empty)
                    || module
                        .identities
                        .text(route.declaration().identity())
                        .is_none_or(str::is_empty)
            }) {
                return malformed("scalar domain lost an issuer identity");
            }
        }
        for set in &catalog.sets {
            if set
                .domains
                .iter()
                .any(|id| !catalog.domains.iter().any(|domain| domain.id == *id))
            {
                return malformed("scalar qualification set names an absent domain");
            }
        }
        for coercion in &catalog.coercions {
            let machine = module
                .machines
                .iter()
                .find(|machine| machine.id == coercion.machine)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "scalar coercion lost its machine",
                ))?;
            let source = values(machine)
                .find(|value| value.id == coercion.source)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "scalar coercion lost its source",
                ))?;
            let destination = values(machine)
                .find(|value| value.id == coercion.destination)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "scalar coercion lost its destination",
                ))?;
            let source_domains = qualification_domains(module, source.qualifications).ok_or(
                CodecError::MalformedStructuralFoundation("scalar coercion lost its source set"),
            )?;
            let destination_domains = qualification_domains(module, destination.qualifications)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "scalar coercion lost its destination set",
                ))?;
            for id in destination_domains {
                let domain = scalar_domain(module, *id).ok_or(
                    CodecError::MalformedStructuralFoundation("scalar coercion domain is absent"),
                )?;
                if domain
                    .missing_membership_requirements(&catalog.domains, source_domains)
                    .is_none_or(|missing| {
                        missing.iter().any(|node| {
                            node.predicate.is_some()
                                || !node.establishment_routes.is_empty()
                                || !node.has_no_required_memberships()
                        })
                    })
                {
                    return malformed(
                        "scalar coercion cannot establish a predicate or routed qualification",
                    );
                }
            }
        }
        for returned in &catalog.returns {
            let machine = module
                .machines
                .iter()
                .find(|machine| machine.id == returned.machine)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "scalar return lost its machine",
                ))?;
            let result = machine
                .result
                .scalar_ref()
                .into_iter()
                .chain(values(machine))
                .find(|result| result.id == returned.result)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "scalar return lost its result",
                ))?;
            let domain = scalar_domain(module, returned.domain).ok_or(
                CodecError::MalformedStructuralFoundation("scalar return lost its domain"),
            )?;
            let bound = match returned.site {
                ScalarReturnSite::NormalExit => machine.result.scalar_ref().is_some_and(|result| result.id == returned.result)
                    && machine.blocks.iter().any(|block| matches!(block.terminator,
                        Terminator::Return { edge, value, .. } if edge == returned.edge && value == returned.source)),
                ScalarReturnSite::Completion { argument_ordinal } => machine.blocks.iter().any(|block| {
                    let Terminator::Jump { edge, target, ref arguments, .. } = block.terminator else { return false; };
                    edge == returned.edge && arguments.get(argument_ordinal as usize) == Some(&returned.source)
                        && machine.blocks.iter().find(|block| block.id == target)
                            .and_then(|block| block.parameters.get(argument_ordinal as usize))
                            .is_some_and(|destination| destination.id == returned.result && destination.id != returned.source)
                }),
            };
            if result.scalar_type != domain.carrier
                || !catalog
                    .sets
                    .iter()
                    .any(|set| set.id == result.qualifications && set.domains.contains(&domain.id))
                || !bound
            {
                return malformed("scalar membership is not bound to its exact normal result");
            }
            if let Some(predicate) = &domain.predicate
                && returned.site == ScalarReturnSite::NormalExit
            {
                let mut expected = predicate.proposition.clone();
                expected.remap_value_ids(|_| result.id);
                let complete = match &expected {
                    Proposition::Conjunction(predicates) => predicates.iter().all(|predicate| {
                        machine
                            .contract
                            .ensures
                            .iter()
                            .any(|clause| retains_conjunct(&clause.proposition, predicate))
                    }),
                    Proposition::Truth => true,
                    _ => machine
                        .contract
                        .ensures
                        .iter()
                        .any(|clause| retains_conjunct(&clause.proposition, &expected)),
                };
                if !complete {
                    return malformed("scalar result contract lost its domain predicate");
                }
            }
            let source = values(machine)
                .find(|source| {
                    source.id == returned.source && source.scalar_type == result.scalar_type
                })
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "scalar return lost its same-carrier source",
                ))?;
            if matches!(returned.site, ScalarReturnSite::Completion { .. })
                && catalog
                    .sets
                    .iter()
                    .any(|set| set.id == source.qualifications && set.domains.contains(&domain.id))
            {
                return malformed("scalar return completion does not add its named membership");
            }
            for origin in &returned.origins {
                let authorized = match origin {
                    ScalarReturnOrigin::Vacuous => {
                        matches!(returned.site, ScalarReturnSite::Completion { .. })
                            && domain.predicate.is_none()
                            && domain.establishment_routes.is_empty()
                            && domain.has_no_required_memberships()
                    }
                    ScalarReturnOrigin::Forwarded => catalog.sets.iter().any(|set| {
                        set.id == source.qualifications
                            && domain
                                .missing_membership_requirements(&catalog.domains, &set.domains)
                                .is_some_and(|missing| missing.is_empty())
                    }),
                    ScalarReturnOrigin::Established { route } => {
                        let declaration = machine.declaration.as_ref().ok_or(
                            CodecError::MalformedStructuralFoundation(
                                "scalar issuer lost its checked declaration",
                            ),
                        )?;
                        match domain.establishment_routes.get(*route as usize) {
                            Some(ScalarDomainEstablishmentRoute::ExactMachine { declaration: issuer, machine_identity }) => {
                                (module.identities.text(*machine_identity).zip(module.identities.text(declaration.callable))
                                    .is_some_and(|(issued, callable)| issued == terminal_psi::CheckedMachineDeclaration::callable_evidence_identity(
                                        callable, declaration.specialization.as_ref().map(|application| &application.commitment),
                                    )) && issuer.identity() == declaration.semantic_declaration)
                                    || declaration.specialization.as_ref().is_some_and(|specialization| {
                                        *machine_identity == specialization.template_callable
                                            && issuer.identity() == specialization.template_semantic_declaration
                                            && module.identities.text(specialization.template_callable).is_some_and(|identity| !identity.is_empty())
                                    })
                            }
                            Some(ScalarDomainEstablishmentRoute::CheckedRequirement { declaration: issuer, requirement_identity }) => {
                                module.closed_conformance_applications.iter().any(|application| {
                                    application.rows.iter().any(|row| {
                                        row.public_requirement_identity == *requirement_identity
                                            && row.requirement_declaration == *issuer
                                            && row.realization_callable_identity == Some(declaration.callable)
                                            && application.realization_callables.iter().any(|callable| {
                                                callable.source_callable_identity == declaration.callable
                                                    && callable.callable == terminal_psi::TargetMachineCallable::Machine(machine.id)
                                            })
                                    })
                                })
                            }
                            _ => false,
                        }
                    }
                };
                let prerequisites = if matches!(origin, ScalarReturnOrigin::Established { .. }) {
                    let supplied = qualification_domains(module, source.qualifications).ok_or(
                        CodecError::MalformedStructuralFoundation(
                            "scalar issuer lost its source qualification set",
                        ),
                    )?;
                    domain
                        .missing_membership_requirements(&catalog.domains, supplied)
                        .is_some_and(|missing| {
                            missing.iter().all(|node| {
                                node.id == domain.id || node.establishment_routes.is_empty()
                            })
                        })
                } else {
                    true
                };
                if !authorized || !prerequisites {
                    let machine_identity = machine
                        .declaration
                        .as_ref()
                        .and_then(|declaration| module.identities.text(declaration.callable))
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("machine:{}", machine.id.get()));
                    let source_memberships: Option<Vec<String>> =
                        qualification_domains(module, source.qualifications).map(|memberships| {
                            memberships
                                .iter()
                                .map(|membership| {
                                    scalar_domain(module, *membership)
                                        .and_then(|domain| module.identities.text(domain.identity))
                                        .map(str::to_owned)
                                        .unwrap_or_else(|| format!("domain:{}", membership.get()))
                                })
                                .collect()
                        });
                    let issuer = match origin {
                        ScalarReturnOrigin::Established { route } => domain
                            .establishment_routes
                            .get(*route as usize)
                            .map(|route| {
                                let callable = match route {
                                    ScalarDomainEstablishmentRoute::ExactMachine {
                                        machine_identity,
                                        ..
                                    } => module.identities.text(*machine_identity),
                                    _ => None,
                                };
                                (
                                    callable,
                                    module.identities.text(route.declaration().identity()),
                                )
                            }),
                        _ => None,
                    };
                    let returned_declaration = machine.declaration.as_ref().map(|declaration| {
                        module.identities.text(declaration.semantic_declaration)
                    });
                    return Err(CodecError::MalformedScalarReturnFoundation {
                        machine: machine_identity,
                        domain: module
                            .identities
                            .text(domain.identity)
                            .unwrap_or("<missing domain identity>")
                            .to_owned(),
                        edge: returned.edge,
                        source: returned.source,
                        result: returned.result,
                        reason: format!(
                            "scalar return origin does not match its source or authorized issuer: site={:?}, origin={origin:?}, source memberships={source_memberships:?}, origin authorized={authorized}, prerequisites satisfied={prerequisites}, issuer={issuer:?}, returned declaration={returned_declaration:?}",
                            returned.site,
                        ),
                    });
                }
            }
        }
        for machine in &module.machines {
            for block in &machine.blocks {
                let successors = match &block.terminator {
                    Terminator::Jump {
                        edge,
                        target,
                        arguments,
                        ..
                    } => vec![(*edge, *target, arguments.as_slice())],
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => vec![
                        (
                            when_true.edge,
                            when_true.target,
                            when_true.arguments.as_slice(),
                        ),
                        (
                            when_false.edge,
                            when_false.target,
                            when_false.arguments.as_slice(),
                        ),
                    ],
                    _ => Vec::new(),
                };
                for (edge, target, arguments) in successors {
                    let target = machine
                        .blocks
                        .iter()
                        .find(|block| block.id == target)
                        .ok_or(CodecError::MalformedStructuralFoundation(
                            "scalar successor target is absent",
                        ))?;
                    for (ordinal, (argument, destination)) in
                        arguments.iter().zip(&target.parameters).enumerate()
                    {
                        let source = values(machine).find(|value| value.id == *argument).ok_or(
                            CodecError::MalformedStructuralFoundation(
                                "scalar successor source is absent",
                            ),
                        )?;
                        for domain in &catalog.domains {
                            let contains = |qualification| {
                                catalog.sets.iter().any(|set| {
                                    set.id == qualification && set.domains.contains(&domain.id)
                                })
                            };
                            if contains(destination.qualifications) && !contains(source.qualifications)
                                && !catalog.coercions.iter().any(|row| row.machine == machine.id && row.edge == edge
                                    && row.source == source.id && row.destination == destination.id && row.argument_ordinal as usize == ordinal)
                                && !catalog.returns.iter().any(|row| row.machine == machine.id && row.edge == edge
                                    && row.source == source.id && row.result == destination.id && row.domain == domain.id
                                    && matches!(row.site, ScalarReturnSite::Completion { argument_ordinal } if argument_ordinal as usize == ordinal))
                            {
                                return malformed("scalar successor introduces membership without its checked return completion");
                            }
                        }
                    }
                }
            }
            let Some(result) = machine.result.scalar() else {
                continue;
            };
            let Some(set) = catalog
                .sets
                .iter()
                .find(|set| set.id == result.qualifications)
            else {
                continue;
            };
            for domain in set.domains.iter().filter(|id| {
                catalog.domains.iter().any(|domain| {
                    domain.id == **id
                        && domain
                            .required_domain_closure(&catalog.domains)
                            .is_none_or(|required| {
                                required
                                    .iter()
                                    .any(|node| !node.establishment_routes.is_empty())
                            })
                })
            }) {
                for block in &machine.blocks {
                    if let Terminator::Return { edge, .. } = block.terminator
                        && !catalog.returns.iter().any(|row| {
                            row.machine == machine.id && row.edge == edge && row.domain == *domain
                        })
                    {
                        return malformed(
                            "routed scalar return has no establishment or forwarding row",
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn retains_conjunct(actual: &Proposition, expected: &Proposition) -> bool {
        actual == expected
            || matches!(actual, Proposition::Conjunction(children)
            if children.iter().any(|child| retains_conjunct(child, expected)))
    }
}
pub(crate) mod storage_foundations {

    use super::{declared_scalar_type, structural_place_type, validate_structural_path};
    use crate::codec_error::{CodecError, malformed};
    use semantic_vocabulary::{
        BlockId, IntegerSign, IntegerType, PlaceId, ScalarType, StructuralFieldId,
        StructuralPlaceKind, StructuralTypeId,
    };

    pub(crate) mod byte_sequences {
        use super::*;
        use semantic_vocabulary::{IntegerSign, IntegerType};
        use terminal_psi::StructuralAccess;
        use terminal_psi::{
            ByteSequenceCarrier, StructuralArgument, StructuralTypeShape,
            nonlinear_multiplicity_pair_pattern,
        };
        use terminal_semantics::{
            arrival_parameter, byte_owned_call_result, byte_parameter_predicates,
            descriptor_reborrow_matches, fixed_byte_extent, projected_field_declaration,
            runtime_structural_field,
        };

        pub(crate) fn validate_structural_byte_view_field_store(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::StructuralByteViewFieldStore {
                destination,
                path,
                field,
                source,
                ..
            } = &operation.kind
            else {
                unreachable!("dispatched validate_structural_byte_view_field_store")
            };
            if operation.result != OperationResult::Unit || destination == source {
                return malformed("byte view field store requires Unit and a distinct source");
            }
            let shared_view = ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            };
            let view_slot = match field {
                Some(field) => {
                    let (root_type, carrier_path) =
                        field_store_carrier(module, machine, *destination, path, *field)?;
                    projected_field_declaration(
                        module.structural_types.iter(),
                        root_type,
                        carrier_path,
                        *field,
                    )
                    .is_some_and(|candidate| {
                        candidate.field_type.byte_sequence_carrier() == Some(shared_view)
                    })
                }
                None => {
                    path.is_empty()
                        && structural_place_type(machine, *destination).is_some_and(|home_type| {
                            module.structural_types.iter().any(|declaration| {
                                declaration.id == home_type
                                    && matches!(
                                        declaration.shape,
                                        StructuralTypeShape::ByteSequence(
                                            ByteSequenceCarrier::BorrowedView {
                                                access: None | Some(StructuralAccess::SharedBorrow),
                                            }
                                        )
                                    )
                            })
                        })
                }
            };
            if !view_slot {
                return malformed("byte view field store does not select a shared view slot");
            }
            if !structural_place_type(machine, *source).is_some_and(|source_type| {
                module.structural_types.iter().any(|declaration| {
                    declaration.id == source_type && declaration.shape.is_borrowed_byte_view()
                })
            }) {
                return malformed("byte view field store source has no descriptor carrier");
            }
            Ok(())
        }

        pub(crate) fn validate_structural_byte_sequence_field_store(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                source,
                length,
                ..
            } = &operation.kind
            else {
                unreachable!("dispatched validate_structural_byte_sequence_field_store")
            };
            if operation.result != OperationResult::Unit {
                return malformed("byte field store requires Unit");
            }
            let reference = reference_store_carrier(module, machine, *destination, path, *field);
            let (destination_type, carrier_path) = if let Some(reference) = reference {
                reference
            } else if let Some(structural_type) =
                owned_store_carrier(module, machine, *destination, path)
            {
                (structural_type, path.as_slice())
            } else {
                let Some(parameter) = arrival_parameter(machine, *destination) else {
                    return malformed("byte field store destination is not arrival storage");
                };
                if !matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ) || !matches!(
                    parameter.multiplicity,
                    nonlinear_multiplicity_pair_pattern!()
                ) || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return malformed("byte field store has invalid destination custody");
                }
                (parameter.structural_type, path.as_slice())
            };
            if !is_static_structural_scalar_store_path(carrier_path)
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == *destination || claim.input == *source)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == *destination || claim.input.root == *source)
            {
                return malformed("byte field store has invalid destination custody");
            }
            let parent_type = validate_structural_path(module, destination_type, carrier_path)?;
            let bounded_field = module
                .structural_type(parent_type)
                .is_some_and(|declaration| match &declaration.shape {
                    StructuralTypeShape::Record { fields, .. } => fields.iter().any(|candidate| {
                        candidate.id == *field
                            && !candidate.relevance.is_erased()
                            && matches!(
                                candidate.field_type.byte_sequence_carrier(),
                                Some(ByteSequenceCarrier::BoundedOwned { .. })
                            )
                    }),
                    _ => false,
                });
            if !bounded_field {
                return malformed("byte field store does not select a bounded byte field");
            }
            let Some(source_place) = machine.structural_place(*source) else {
                return malformed("byte field store source is unknown");
            };
            let mut owned_source = false;
            let source_type = match source_place.kind {
                StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                } => structural_type,
                StructuralPlaceKind::Parameter { .. }
                | StructuralPlaceKind::BlockParameter { .. } => {
                    let Some(parameter) = arrival_parameter(machine, *source).filter(|parameter| {
                        parameter.multiplicity == StructuralMultiplicity::Unrestricted
                            && parameter.projected_qualifications.is_empty()
                            && ((parameter.access == StructuralAccess::SharedBorrow
                                && parameter.qualifications.is_empty())
                                || (parameter.access == StructuralAccess::Owned
                                    && byte_parameter_predicates(module, parameter).is_some()))
                    }) else {
                        return malformed(
                            "byte field store source is not a readable whole byte carrier",
                        );
                    };
                    owned_source = parameter.access == StructuralAccess::Owned;
                    parameter.structural_type
                }
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } => {
                    let mut producers = machine
                        .operations()
                        .filter(|operation| operation.id == producer);
                    let Some(producer) = producers.next() else {
                        return malformed("byte field store source has no producer");
                    };
                    let owned_call = byte_owned_call_result(module, machine, *source).is_some();
                    if producers.next().is_some()
                        || !(owned_call
                            || matches!(
                                producer.kind,
                                OperationKind::ByteSequenceSubslice { .. }
                                    | OperationKind::EstablishByteSequenceConcatenation { .. }
                            ))
                        || !producer.result.structural().is_some_and(|result| {
                            result.place == *source
                                && result.structural_type == structural_type
                                && result.multiplicity == StructuralMultiplicity::Unrestricted
                                && (owned_call || result.qualifications.is_empty())
                                && result.projected_qualifications.is_empty()
                                && result.claims.is_empty()
                        })
                    {
                        return malformed("byte field store source has inexact producer custody");
                    }
                    owned_source = owned_call
                        || matches!(
                            producer.kind,
                            OperationKind::EstablishByteSequenceConcatenation { .. }
                        );
                    structural_type
                }
                _ => return malformed("byte field store source is not an immutable whole view"),
            };
            if !module.structural_types.iter().any(|declaration| declaration.id == source_type
                && match declaration.shape {
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. }) => !owned_source,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => owned_source,
                    _ => false,
                })
                || !machine.operations().any(|candidate| {
                    matches!(&candidate.kind, OperationKind::ByteSequenceLength { source: measured } if measured.place == *source && measured.path.is_empty())
                        && candidate.result.scalar_ref().is_some_and(|result| result.id == *length
                            && matches!(result.scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64))
                })
            {
                return malformed("byte field store source or exact length is invalid");
            }
            Ok(())
        }

        pub(crate) fn validate_structural_byte_sequence_field_length(
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::StructuralByteSequenceFieldLength { .. } = &operation.kind else {
                unreachable!("dispatched validate_structural_byte_sequence_field_length")
            };
            if operation.result.scalar().is_none_or(|result| {
                !matches!(result.scalar_type, ScalarType::Integer(integer)
                    if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64)
            }) {
                return malformed("byte field length requires an unsigned 64-bit scalar result");
            }
            Ok(())
        }

        pub(crate) fn validate_structural_byte_sequence_field_read(
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::StructuralByteSequenceFieldRead { .. } = &operation.kind else {
                unreachable!("dispatched validate_structural_byte_sequence_field_read")
            };
            let expected = ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"),
            );
            if operation
                .result
                .scalar()
                .is_none_or(|result| result.scalar_type != expected)
            {
                return malformed("byte field read requires an unsigned 8-bit scalar result");
            }
            Ok(())
        }

        pub(crate) fn validate_byte_sequence_subslice(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::ByteSequenceSubslice { source, .. } = &operation.kind else {
                unreachable!("dispatched validate_byte_sequence_subslice")
            };
            let Some(result) = operation.result.structural() else {
                return malformed("byte-sequence subslice requires a structural result");
            };
            if !matches!(result.multiplicity, nonlinear_multiplicity_pair_pattern!())
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !module.structural_types.iter().any(|row| {
                    row.id == result.structural_type
                        && matches!(
                            row.shape,
                            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
                        )
                }) || !machine.structural_places.iter().any(|row| {
                row.id == result.place
                    && matches!(row.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                        if producer == operation.id && structural_type == result.structural_type)
            }) {
                return malformed("byte-sequence subslice requires its exact borrowed result place");
            }
            validate_byte_descriptor_source(module, machine, source, true)?;
            let root = place_structural_type(machine, source.place).expect("source checked above");
            let selected = validate_structural_path(module, root, &source.path)?;
            let source_occurrence_access = if source.path.is_empty() {
                arrival_parameter(machine, source.place)
                    .and_then(|parameter| {
                        if parameter.access != StructuralAccess::Owned {
                            Some(parameter.access)
                        } else {
                            terminal_semantics::descriptor_results::descriptor_value_access(
                                module.structural_types.iter(),
                                selected,
                                parameter.multiplicity,
                            )
                        }
                    })
                    .or_else(|| {
                        machine.operations().find_map(|operation| {
                            operation
                                .result
                                .structural()
                                .filter(|result| result.place == source.place)
                                .and_then(|result| {
                                    terminal_semantics::descriptor_results::descriptor_value_access(
                                        module.structural_types.iter(),
                                        selected,
                                        result.multiplicity,
                                    )
                                })
                        })
                    })
                    .or_else(|| {
                        machine.structural_place(source.place).and_then(|place| {
                            matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. })
                                .then_some(StructuralAccess::SharedBorrow)
                        })
                    })
            } else {
                runtime_structural_field(module.structural_types.iter(), root, &source.path)
                    .and_then(|field| field.field_type.byte_sequence_carrier())
                    .and_then(|carrier| match carrier {
                        ByteSequenceCarrier::BorrowedView { access }
                        | ByteSequenceCarrier::BoundedView { access, .. } => access,
                        _ => None,
                    })
            };
            let Some(result_access) =
                terminal_semantics::descriptor_results::descriptor_value_access(
                    module.structural_types.iter(),
                    result.structural_type,
                    result.multiplicity,
                )
            else {
                return malformed("byte subslice result lacks descriptor permission");
            };
            if !descriptor_reborrow_matches(
                module.structural_types.iter(),
                selected,
                source.access,
                source_occurrence_access,
                result.structural_type,
                result_access,
            ) {
                return malformed("byte subslice must preserve its selected descriptor and access");
            }
            Ok(())
        }

        pub(crate) fn validate_byte_sequence_read(operation: &Operation) -> Result<(), CodecError> {
            let OperationKind::ByteSequenceRead { .. } = &operation.kind else {
                unreachable!("dispatched validate_byte_sequence_read")
            };
            let expected = ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"),
            );
            if operation
                .result
                .scalar()
                .is_none_or(|result| result.scalar_type != expected)
            {
                return malformed("byte-sequence read requires an unsigned 8-bit scalar result");
            }
            Ok(())
        }

        pub(crate) fn validate_byte_sequence_length(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::ByteSequenceLength { source } = &operation.kind else {
                unreachable!("dispatched validate_byte_sequence_length")
            };
            super::validate_u64_length_result(
                operation,
                "byte-sequence length requires an unsigned 64-bit scalar result",
            )?;
            validate_byte_descriptor_source(module, machine, source, false)?;
            Ok(())
        }

        fn validate_byte_descriptor_source(
            module: &TerminalModule,
            machine: &TerminalMachine,
            source: &StructuralArgument,
            borrowed_only: bool,
        ) -> Result<(), CodecError> {
            if source.access == StructuralAccess::WriteOnlyBorrow {
                return malformed("byte descriptor requires readable access");
            }
            let Some(root) = place_structural_type(machine, source.place) else {
                return malformed("byte descriptor has no source declaration");
            };
            let selected = validate_structural_path(module, root, &source.path)?;
            if !module.structural_type(selected).is_some_and(|declaration| {
                declaration.shape.is_borrowed_byte_carrier()
                    || (!borrowed_only
                        && (declaration.shape.is_bounded_owned_byte_sequence()
                            || matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
                                && fixed_byte_extent(module.structural_types.iter(), selected)
                                    .is_some()))
            }) {
                return malformed("byte operation requires its exact selected byte descriptor");
            }
            Ok(())
        }
    }
    pub(crate) mod element_views {
        use super::*;
        use terminal_semantics::descriptor_results::{
            DescriptorAnalysis, ElementPayloadMemberships, element_payload_memberships,
            terminal_graph_from,
        };
        use terminal_semantics::{
            collection_backing, collection_view_descriptor, element_view_leaf_scalar,
        };

        fn element_view_result_place(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
            result: &terminal_psi::StructuralOperationResult,
            multiplicity: StructuralMultiplicity,
        ) -> bool {
            result.multiplicity == multiplicity
                && result.qualifications.is_empty()
                && result.projected_qualifications.iter().all(|row| {
                    matches!(
                        row.path.first(),
                        Some(terminal_psi::StructuralPathSegment::AllElements)
                    )
                })
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty()
                && module.structural_types.iter().any(|row| {
                    row.id == result.structural_type
                        && (matches!(row.shape, StructuralTypeShape::ElementView { access, .. }
                            if access.is_none_or(|access| match multiplicity {
                                StructuralMultiplicity::Unrestricted => access == terminal_psi::StructuralAccess::SharedBorrow,
                                StructuralMultiplicity::Affine => access == terminal_psi::StructuralAccess::MutableBorrow,
                                StructuralMultiplicity::Linear => false,
                            }))
                            || row.shape.is_borrowed_byte_view()
                            || (row.shape.is_borrowed_byte_carrier()
                                && fixed_descriptor_establishment(module, machine, operation, &row.shape)))
                }) && machine.structural_places.iter().any(|row| {
                row.id == result.place
                    && matches!(row.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                            if producer == operation.id && structural_type == result.structural_type)
            })
        }

        fn fixed_descriptor_establishment(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
            shape: &StructuralTypeShape,
        ) -> bool {
            let (source, element, region) = match &operation.kind {
                OperationKind::EstablishElementView {
                    source,
                    element,
                    region,
                    ..
                }
                | OperationKind::EstablishMutableElementView {
                    source,
                    element,
                    region,
                    ..
                } => (source, *element, *region),
                _ => return false,
            };
            let extent = place_structural_type(machine, source.place)
                .and_then(|root| validate_structural_path(module, root, &source.path).ok())
                .and_then(|collection| {
                    collection_backing(module.structural_types.iter(), collection, element)
                })
                .map(|backing| backing.extent);
            collection_view_descriptor(
                shape,
                extent,
                region,
                &source.path,
                module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == element),
            )
            .is_some()
        }

        pub(crate) fn validate_establish_element_view(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::EstablishElementView { .. } = &operation.kind else {
                unreachable!("dispatched validate_establish_element_view")
            };
            let Some(result) = operation.result.structural() else {
                return malformed("element-view establishment requires a structural result");
            };
            if !element_view_result_place(
                module,
                machine,
                operation,
                result,
                StructuralMultiplicity::Unrestricted,
            ) {
                return malformed(
                    "element-view establishment requires its exact borrowed result place",
                );
            }
            validate_element_memberships(module, machine, operation)?;
            Ok(())
        }

        pub(crate) fn validate_establish_mutable_element_view(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::EstablishMutableElementView { .. } = &operation.kind else {
                unreachable!("dispatched validate_establish_mutable_element_view")
            };
            let Some(result) = operation.result.structural() else {
                return malformed(
                    "mutable element-view establishment requires a structural result",
                );
            };
            if !element_view_result_place(
                module,
                machine,
                operation,
                result,
                StructuralMultiplicity::Affine,
            ) {
                return malformed(
                    "mutable element-view establishment requires its exact borrowed result place",
                );
            }
            validate_element_memberships(module, machine, operation)?;
            Ok(())
        }

        pub(crate) fn validate_element_view_subslice(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::ElementViewSubslice { source, .. } = &operation.kind else {
                unreachable!("dispatched validate_element_view_subslice")
            };
            let Some(result) = operation.result.structural() else {
                return malformed("element-view subslice requires a structural result");
            };
            if !matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            ) || !element_view_result_place(
                module,
                machine,
                operation,
                result,
                result.multiplicity,
            ) {
                return malformed("element-view subslice requires its exact borrowed result place");
            }
            if source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return malformed("element subslice requires readable descriptor access");
            }
            let Some(root) = place_structural_type(machine, source.place) else {
                return malformed("element subslice has no source declaration");
            };
            let selected = validate_structural_path(module, root, &source.path)?;
            let same_element = module
                .structural_type(selected)
                .zip(module.structural_type(result.structural_type))
                .is_some_and(|(source, result)| {
                    matches!((&source.shape, &result.shape),
                    (StructuralTypeShape::ElementView { element: source, .. },
                     StructuralTypeShape::ElementView { element: result, .. }) if source == result)
                });
            if !same_element
                || (result.multiplicity == StructuralMultiplicity::Affine
                    && !matches!(
                        source.access,
                        terminal_psi::StructuralAccess::Owned
                            | terminal_psi::StructuralAccess::MutableBorrow
                    ))
            {
                return malformed(
                    "element subslice must preserve its selected descriptor and access",
                );
            }
            validate_element_memberships(module, machine, operation)?;
            Ok(())
        }

        pub(crate) fn validate_element_view_length(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::ElementViewLength { source } = &operation.kind else {
                unreachable!("dispatched validate_element_view_length")
            };
            if source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return malformed("element-view length requires readable descriptor access");
            }
            let Some(root) = place_structural_type(machine, source.place) else {
                return malformed("element-view length has no source declaration");
            };
            let selected = validate_structural_path(module, root, &source.path)?;
            if !module.structural_type(selected).is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            }) {
                return malformed(
                    "element-view length requires the exact selected element descriptor",
                );
            }
            super::validate_u64_length_result(
                operation,
                "element-view length requires an unsigned 64-bit scalar result",
            )
        }

        pub(crate) fn validate_element_view_read(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::ElementViewRead { source, path, .. } = &operation.kind else {
                unreachable!("dispatched validate_element_view_read")
            };
            let Some(source_type) = place_structural_type(machine, *source) else {
                return malformed("element-view read has an undeclared source place");
            };
            let Some(element) = module.structural_types.iter().find_map(|row| {
                match (row.id == source_type, &row.shape) {
                    (true, StructuralTypeShape::ElementView { element, .. }) => Some(*element),
                    _ => None,
                }
            }) else {
                return malformed("element-view read requires an element-view source");
            };
            let Some(expected) = element_view_leaf_scalar(&module.structural_types, element, path)
            else {
                return malformed("element-view read requires a scalar element leaf");
            };
            if operation
                .result
                .scalar()
                .is_none_or(|result| result.scalar_type != expected)
            {
                return malformed("element-view read requires the element leaf's scalar result");
            }
            Ok(())
        }

        fn validate_element_memberships(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let Some(result) = operation.result.structural() else {
                return malformed("element membership result is absent");
            };
            if result.projected_qualifications.is_empty() {
                return Ok(());
            }
            let Some(StructuralTypeShape::ElementView { element, .. }) = module
                .structural_type(result.structural_type)
                .map(|row| &row.shape)
            else {
                return malformed("universal membership requires an element-view carrier");
            };
            let source = match &operation.kind {
                OperationKind::EstablishElementView { source, .. }
                | OperationKind::EstablishMutableElementView { source, .. }
                | OperationKind::ElementViewSubslice { source, .. } => source,
                _ => return malformed("universal membership has no view introduction"),
            };
            let position = machine
                .blocks
                .iter()
                .find_map(|block| {
                    block
                        .operations
                        .iter()
                        .position(|row| row.id == operation.id)
                        .and_then(|ordinal| {
                            u32::try_from(ordinal)
                                .ok()
                                .map(|ordinal| (block.id, ordinal))
                        })
                })
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "view introduction position is absent",
                ))?;
            let graph = terminal_graph_from(module, [machine.id]);
            let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graph);
            let evidence = element_payload_memberships(
                &module.structural_types,
                &module.structural_domains,
                machine,
                &mut analysis,
                position,
                source,
                *element,
            )
            .ok_or(CodecError::MalformedStructuralFoundation(
                "view element memberships have unknown source payloads",
            ))?;
            let ElementPayloadMemberships::Present(contract) = evidence else {
                return Ok(());
            };
            let (whole, projected) = terminal_psi::StructuralPathQualification::select(
                &[],
                &result.projected_qualifications,
                &[terminal_psi::StructuralPathSegment::AllElements],
            );
            if whole
                .iter()
                .any(|domain| !contract.qualifications.contains(domain))
                || projected
                    .iter()
                    .any(|row| !contract.projected_qualifications.contains(row))
            {
                return malformed(
                    "view element memberships do not hold for every actual source element",
                );
            }
            Ok(())
        }
    }
    pub(crate) mod representation_geometry {

        use crate::codec_error::{CodecError, malformed};
        use semantic_vocabulary::{ScalarType, StructuralTypeId};
        use terminal_psi::{
            LayoutApplicationId, StructuralPathSegment, StructuralRecordStorage,
            StructuralTypeShape, TargetLayoutApplication, TargetLayoutInput,
            TargetLayoutRealization, TerminalModule,
        };
        use terminal_semantics::{
            RecordStorageResolver, ReferenceStorageGeometry, StorageGeometryError,
        };

        pub(crate) fn fixed_region_is_compatible(
            module: &TerminalModule,
            source: StructuralTypeId,
            path: &[StructuralPathSegment],
            reference: StructuralTypeId,
            stored: Option<ScalarType>,
        ) -> Result<bool, CodecError> {
            let Some(StructuralTypeShape::Reference { referent, .. }) =
                module.structural_type(reference).map(|row| &row.shape)
            else {
                return Ok(false);
            };
            let mut pending = vec![source, *referent];
            let mut visited = std::collections::BTreeSet::new();
            let mut applications = Vec::<&TargetLayoutApplication>::new();
            while let Some(carrier) = pending.pop() {
                if !visited.insert(carrier) {
                    continue;
                }
                let declaration = module.structural_type(carrier).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "reference geometry lost its declaration",
                    ),
                )?;
                match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
                    StructuralTypeShape::Record { fields, storage } => {
                        if let StructuralRecordStorage::TargetLayout(id) = storage {
                            let application = module
                                .target_layout_applications
                                .iter()
                                .find(|row| row.id == *id && row.record == carrier)
                                .ok_or(CodecError::MalformedStructuralFoundation(
                                    "reference geometry lost its exact layout application",
                                ))?;
                            applications.push(application);
                        }
                        pending.extend(
                            fields
                                .iter()
                                .filter(|field| !field.relevance.is_erased())
                                .filter_map(|field| field.field_type.as_structural()),
                        );
                    }
                    _ => {}
                }
            }
            let Some(first) = applications.first() else {
                return Ok(ReferenceStorageGeometry::new(
                    module.structural_types.iter(),
                    |application| Err(StorageGeometryError::UnresolvedExtent(application)),
                )
                .region_source_compatible(source, path, reference, stored));
            };
            if first.target_inputs.is_empty()
                || applications
                    .iter()
                    .any(|row| row.target_inputs != first.target_inputs)
            {
                return malformed("reference geometry has no common complete layout input roster");
            }
            for input in &first.target_inputs {
                let mut geometry =
                    ReferenceStorageGeometry::new(module.structural_types.iter(), |application| {
                        Err(StorageGeometryError::UnresolvedExtent(application))
                    })
                    .with_record_storage(RetainedLayout { module, input });
                if !geometry.region_source_compatible(source, path, reference, stored) {
                    return Ok(false);
                }
            }
            Ok(true)
        }

        struct RetainedLayout<'module> {
            module: &'module TerminalModule,
            input: &'module TargetLayoutInput,
        }

        impl RecordStorageResolver for RetainedLayout<'_> {
            fn realization(
                &self,
                record: StructuralTypeId,
                application: LayoutApplicationId,
            ) -> Result<&TargetLayoutRealization, StorageGeometryError> {
                self.module
                    .target_layout_applications
                    .iter()
                    .find(|row| row.id == application && row.record == record)
                    .and_then(|application| {
                        application.realizations.iter().find(|row| {
                            row.target == self.input.target
                                && row.pointer_size == self.input.pointer_size
                                && row.pointer_alignment == self.input.pointer_alignment
                                && row.case_tag_size == self.input.case_tag_size
                                && row.case_tag_alignment == self.input.case_tag_alignment
                                && row.little_endian == self.input.little_endian
                        })
                    })
                    .ok_or(StorageGeometryError::UnresolvedLayout(record))
            }
        }
    }
    pub(crate) mod structural_fields {
        use super::*;
        use semantic_vocabulary::{
            CanonicalStructuralPathSegment, PlaceId, ScalarType, StructuralDomainId,
            StructuralFieldId, StructuralTypeId,
        };
        use terminal_psi::{
            StructuralAccess, StructuralPathQualification, StructuralPathSegment,
            StructuralStoreDisposition, StructuralTypeShape, element_view_establish_pair_pattern,
            mutating_access_pair_pattern, nonlinear_multiplicity_pair_pattern,
        };
        use terminal_semantics::{
            observed_field_backing_access, observed_field_declaration, primitive_projection_type,
            primitive_store_preserves_scalar_memberships, primitive_store_restriction,
            projected_field_declaration, scalar_field_requires_evidence,
            structural_store_value_admitted, structural_window_projection,
            structural_window_selects_child,
        };

        pub(crate) fn validate_structural_case_membership(
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::StructuralCaseMembership { .. } = &operation.kind else {
                unreachable!("dispatched validate_structural_case_membership")
            };
            if operation.result.scalar().is_none_or(|result| {
                result.scalar_type != ScalarType::Boolean || !result.qualifications.is_empty()
            }) {
                return malformed("case membership requires an unqualified Boolean result");
            }
            Ok(())
        }

        pub(crate) fn validate_write_only_primitive_store(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::WriteOnlyPrimitiveStore {
                destination,
                value,
                path,
                range_obligation,
            } = &operation.kind
            else {
                unreachable!("dispatched validate_write_only_primitive_store")
            };
            if operation.result != OperationResult::Unit {
                return malformed("write-only primitive store declares a non-Unit result");
            }
            let projected = !path.is_empty();
            let (destination_type, qualifications, projected_qualifications, additional_input) =
                if let Some(parameter) = machine.structural_parameter(*destination) {
                    let declaration_matches = machine.structural_places.iter().any(|place| {
                        if place.id != *destination {
                            return false;
                        }
                        match place.kind {
                            StructuralPlaceKind::Parameter { position } => {
                                position == parameter.position
                                    && machine.structural_parameters.contains(parameter)
                            }
                            StructuralPlaceKind::BlockParameter { block, position } => {
                                position == parameter.position
                                    && machine.block(&block).and_then(|block| {
                                        block.structural_parameters.get(position as usize)
                                    }) == Some(parameter)
                            }
                            _ => false,
                        }
                    });
                    let writable = matches!(
                        parameter.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                    ) || (projected && parameter.access == StructuralAccess::Owned);
                    let multiplicity = parameter.multiplicity
                        == StructuralMultiplicity::Unrestricted
                        || (projected && parameter.multiplicity == StructuralMultiplicity::Affine);
                    if !writable
                        || !multiplicity
                        || !declaration_matches
                        || machine
                            .entry_claims
                            .iter()
                            .any(|claim| claim.input == *destination)
                        || machine
                            .content_entry_claims
                            .iter()
                            .any(|claim| claim.input.root == *destination)
                    {
                        return malformed(
                            "write-only primitive store has invalid destination custody",
                        );
                    }
                    (
                        parameter.structural_type,
                        parameter.qualifications.as_slice(),
                        parameter.projected_qualifications.as_slice(),
                        parameter.additional_input_qualifications.as_slice(),
                    )
                } else {
                    let Some(result) = machine
                        .operations()
                        .filter(|producer| {
                            projected
                                || matches!(
                                    producer.kind,
                                    OperationKind::EstablishPrimitiveLocal { .. }
                                )
                        })
                        .filter_map(|producer| producer.result.structural())
                        .find(|result| {
                            result.place == *destination
                                && result.claims.is_empty()
                                && (result.multiplicity == StructuralMultiplicity::Unrestricted
                                    || (projected
                                        && result.multiplicity == StructuralMultiplicity::Affine))
                        })
                    else {
                        return malformed(
                            "primitive store destination is neither a parameter nor an initialized local",
                        );
                    };
                    if machine
                        .entry_claims
                        .iter()
                        .any(|claim| claim.input == *destination)
                        || machine
                            .content_entry_claims
                            .iter()
                            .any(|claim| claim.input.root == *destination)
                    {
                        return malformed("primitive local store cannot carry entry claims");
                    }
                    (
                        result.structural_type,
                        result.qualifications.as_slice(),
                        result.projected_qualifications.as_slice(),
                        result.additional_input_qualifications.as_slice(),
                    )
                };
            let Some(expected) =
                primitive_projection_type(module.structural_types.iter(), destination_type, path)
            else {
                return malformed("write-only primitive store requires a primitive-scalar root");
            };
            let Some(restriction) = primitive_store_restriction(
                module.structural_types.iter(),
                module.structural_domains.iter(),
                destination_type,
                qualifications,
                projected_qualifications,
                additional_input,
                path,
            ) else {
                return malformed("primitive store does not preserve enclosing qualification");
            };
            if restriction.is_some() != range_obligation.is_some() {
                return malformed(
                    "primitive store receiving obligation does not match its declaration",
                );
            }
            let Some(source) = machine
                .parameters
                .iter()
                .chain(machine.block_parameters())
                .chain(
                    machine
                        .operations()
                        .filter_map(|operation| operation.result.scalar_ref()),
                )
                .find(|source| source.id == *value)
            else {
                return malformed("primitive store lost its completed SSA source");
            };
            if !primitive_store_preserves_scalar_memberships(
                module.structural_types.iter(),
                module.structural_domains.iter(),
                &module.scalar_qualifications,
                destination_type,
                qualifications,
                projected_qualifications,
                additional_input,
                path,
                source,
            ) {
                return malformed(
                    "primitive store value lacks its exact receiving domain provenance",
                );
            }
            let actual = declared_scalar_type(machine, *value);
            if actual != Some(expected) {
                return malformed("write-only primitive store value type does not match referent");
            }
            Ok(())
        }

        pub(crate) fn validate_structural_scalar_field_store(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                range_obligation,
            } = &operation.kind
            else {
                unreachable!("dispatched validate_structural_scalar_field_store")
            };
            if operation.result != OperationResult::Unit {
                return malformed("structural scalar field store declares a non-Unit result");
            }
            let (root_type, carrier_path) =
                field_store_carrier(module, machine, *destination, path, *field)?;
            let Some(declaration) = projected_field_declaration(
                module.structural_types.iter(),
                root_type,
                carrier_path,
                *field,
            ) else {
                return malformed(
                    "structural scalar field store does not select a relevant scalar field",
                );
            };
            let expected = match &declaration.field_type {
                StructuralFieldType::Scalar(scalar_type) if range_obligation.is_none() => {
                    Some(*scalar_type)
                }
                field_type
                    if scalar_field_requires_evidence(field_type) && range_obligation.is_some() =>
                {
                    field_type.scalar_type()
                }
                StructuralFieldType::IeeeFloat(format) if range_obligation.is_none() => {
                    Some(ScalarType::IeeeFloat(*format))
                }
                _ => None,
            };
            let Some(expected) = expected else {
                return malformed(
                    "structural scalar field store does not select a relevant scalar field",
                );
            };
            let actual = declared_scalar_type(machine, *value);
            if actual != Some(expected) {
                return malformed("structural scalar field store value type does not match field");
            }
            Ok(())
        }

        pub(crate) fn validate_move_structural_field(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::MoveStructuralField {
                source,
                path,
                field,
            } = &operation.kind
            else {
                unreachable!("dispatched validate_move_structural_field")
            };
            let Some(result) = operation.result.structural() else {
                return malformed("structural field move has no structural result");
            };
            let Some(root_type) = window_root_type(
                module,
                machine,
                *source,
                path,
                *field,
                StructuralAccess::MutableBorrow,
            ) else {
                return malformed("structural field move requires a mutable-borrowed parameter");
            };
            let field_type = window_field_type(module, root_type, path, *field)?;
            if result.structural_type != field_type {
                return malformed("structural field move result type does not match field");
            }
            let (qualifications, projected) =
                window_memberships(module, machine, *source, root_type, path, *field)?;
            if result.qualifications != qualifications
                || result.projected_qualifications != projected
                || !result.qualification_establishments.is_empty()
            {
                return malformed(
                    "structural field move does not retain the selected source memberships",
                );
            }
            Ok(())
        }

        pub(crate) fn validate_store_structural_field(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let OperationKind::StoreStructuralField {
                disposition,
                destination,
                path,
                field,
                value,
            } = &operation.kind
            else {
                unreachable!("dispatched validate_store_structural_field")
            };
            if operation.result != OperationResult::Unit {
                return malformed("structural field store declares a non-Unit result");
            }
            if value.access != StructuralAccess::Owned
                || !value.path.is_empty()
                || !structural_store_value_admitted(
                    machine,
                    module.structural_types.iter(),
                    *disposition,
                    value.place,
                )
            {
                return malformed("structural field store value must be a whole owned place");
            }
            let owned_root = (*disposition == StructuralStoreDisposition::Vacant
                && path.is_empty()
                && field.is_none())
            .then(|| owned_store_root_type(machine, *destination))
            .flatten();
            let Some(root_type) = owned_root.or_else(|| {
                window_root_type(
                    module,
                    machine,
                    *destination,
                    path,
                    *field,
                    disposition.required_access(),
                )
            }) else {
                return malformed(
                    "structural field store requires declared owned vacant storage or an accessible borrowed window",
                );
            };
            let field_type = window_field_type(module, root_type, path, *field)?;
            if structural_place_type(machine, value.place) != Some(field_type) {
                return malformed("structural field store value type does not match field");
            }
            let (required, required_projected) =
                window_memberships(module, machine, *destination, root_type, path, *field)?;
            let Some((offered, offered_projected)) = declared_memberships(machine, value.place)
            else {
                return malformed("structural field store value has no membership declaration");
            };
            if !required.iter().all(|domain| offered.contains(domain))
                || !required_projected
                    .iter()
                    .all(|row| offered_projected.contains(row))
            {
                return malformed(
                    "structural field store does not restore the selected memberships",
                );
            }
            Ok(())
        }

        fn owned_store_root_type(
            machine: &TerminalMachine,
            place: PlaceId,
        ) -> Option<StructuralTypeId> {
            let declaration = machine.structural_place(place)?;
            match declaration.kind {
                StructuralPlaceKind::Parameter { position } => {
                    let parameter = machine.structural_parameters.get(position as usize)?;
                    (parameter.place == place
                        && parameter.position == position
                        && parameter.access == StructuralAccess::Owned)
                        .then_some(parameter.structural_type)
                }
                StructuralPlaceKind::BlockParameter { block, position } => {
                    let parameter = machine
                        .block(&block)?
                        .structural_parameters
                        .get(position as usize)?;
                    (block != machine.entry
                        && parameter.place == place
                        && parameter.position == position
                        && parameter.access == StructuralAccess::Owned
                        && !machine
                            .content_entry_claims
                            .iter()
                            .any(|claim| claim.input.root == place))
                    .then_some(parameter.structural_type)
                }
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } => {
                    let result = machine.operation(producer)?.result.structural()?;
                    (result.place == place
                        && result.structural_type == structural_type
                        && !machine
                            .content_entry_claims
                            .iter()
                            .any(|claim| claim.input.root == place))
                    .then_some(structural_type)
                }
                _ => None,
            }
        }

        fn window_root_type(
            module: &TerminalModule,
            machine: &TerminalMachine,
            place: PlaceId,
            path: &[StructuralPathSegment],
            field: Option<StructuralFieldId>,
            required_access: StructuralAccess,
        ) -> Option<StructuralTypeId> {
            if !structural_window_selects_child(path, field) {
                if path.is_empty() {
                    return mutable_borrowed_parameter_type(machine, place, required_access);
                }
                let (StructuralPathSegment::Referent, carrier_path) = path.split_last()? else {
                    return None;
                };
                let root_type = borrowed_field_store_root_type(machine, place, required_access)?;
                let carrier = structural_window_projection(
                    module.structural_types.iter(),
                    root_type,
                    carrier_path,
                    None,
                )?;
                module
                    .structural_types
                    .iter()
                    .any(|declaration| {
                        declaration.id == carrier.structural_type
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::Reference {
                                    access,
                                    ..
                                } if required_access.admits_referent(access)
                            )
                    })
                    .then_some(root_type)
            } else {
                borrowed_field_store_root_type(machine, place, required_access)
            }
        }

        fn borrowed_field_store_root_type(
            machine: &TerminalMachine,
            place: PlaceId,
            required_access: StructuralAccess,
        ) -> Option<StructuralTypeId> {
            if let Some(parameter) = machine
                .structural_parameters
                .iter()
                .chain(machine.block_structural_parameters())
                .find(|parameter| parameter.place == place)
                && required_access.admits(parameter.access)
                && (parameter.access == StructuralAccess::Owned
                    || matches!(
                        parameter.multiplicity,
                        nonlinear_multiplicity_pair_pattern!()
                    ))
            {
                return Some(parameter.structural_type);
            }
            machine
                .operations()
                .filter_map(|producer| producer.result.structural())
                .find(|result| result.place == place)
                .map(|result| result.structural_type)
        }

        fn mutable_borrowed_parameter_type(
            machine: &TerminalMachine,
            place: PlaceId,
            required_access: StructuralAccess,
        ) -> Option<StructuralTypeId> {
            machine
                .structural_parameters
                .iter()
                .chain(machine.block_structural_parameters())
                .find(|parameter| {
                    parameter.place == place
                        && required_access.admits_referent(parameter.access)
                        && matches!(
                            parameter.multiplicity,
                            nonlinear_multiplicity_pair_pattern!()
                        )
                        && !machine
                            .entry_claims
                            .iter()
                            .any(|claim| claim.input == place)
                        && !machine
                            .content_entry_claims
                            .iter()
                            .any(|claim| claim.input.root == place)
                })
                .map(|parameter| parameter.structural_type)
        }

        fn window_memberships(
            module: &TerminalModule,
            machine: &TerminalMachine,
            place: PlaceId,
            root_type: StructuralTypeId,
            path: &[StructuralPathSegment],
            field: Option<StructuralFieldId>,
        ) -> Result<(Vec<StructuralDomainId>, Vec<StructuralPathQualification>), CodecError>
        {
            let Some(projection) = structural_window_projection(
                module.structural_types.iter(),
                root_type,
                path,
                field,
            ) else {
                return malformed("qualified structural window has no declared projection");
            };
            let Some((whole, projected)) = declared_memberships(machine, place) else {
                return malformed("structural window has no membership declaration");
            };
            StructuralPathQualification::select_restorable(whole, projected, &projection.path)
                .ok_or({
                    CodecError::MalformedStructuralFoundation(
                        "structural window cannot restore an ancestor's qualification",
                    )
                })
        }

        pub(crate) fn declared_memberships(
            machine: &TerminalMachine,
            place: PlaceId,
        ) -> Option<(&[StructuralDomainId], &[StructuralPathQualification])> {
            machine
                .structural_parameter(place)
                .map(|parameter| {
                    (
                        parameter.qualifications.as_slice(),
                        parameter.projected_qualifications.as_slice(),
                    )
                })
                .or_else(|| {
                    machine
                        .operations()
                        .filter_map(|operation| operation.result.structural())
                        .find(|result| result.place == place)
                        .map(|result| {
                            (
                                result.qualifications.as_slice(),
                                result.projected_qualifications.as_slice(),
                            )
                        })
                })
        }

        fn window_field_type(
            module: &TerminalModule,
            root_type: StructuralTypeId,
            path: &[StructuralPathSegment],
            field: Option<StructuralFieldId>,
        ) -> Result<StructuralTypeId, CodecError> {
            let Some(projection) = structural_window_projection(
                module.structural_types.iter(),
                root_type,
                path,
                field,
            ) else {
                return malformed("structural field move does not select a structural field");
            };
            Ok(projection.structural_type)
        }

        fn owns_observed_storage(
            module: &TerminalModule,
            structural_type: StructuralTypeId,
            access: StructuralAccess,
        ) -> bool {
            access == StructuralAccess::Owned
                && module.structural_types.iter().any(|declaration| {
                    declaration.id == structural_type && (declaration.shape).is_composite_shape()
                })
        }

        pub(crate) fn validate_scalar_structural_field(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let (OperationKind::ScalarStructuralField {
                source,
                path,
                field,
            }
            | OperationKind::BooleanStructuralField {
                source,
                path,
                field,
            }) = &operation.kind
            else {
                unreachable!("dispatched validate_scalar_structural_field")
            };
            let Some(result) = operation.result.scalar_ref() else {
                return malformed("scalar structural field has no scalar result");
            };
            if !matches!(
                (&operation.kind, result.scalar_type),
                (
                    OperationKind::ScalarStructuralField { .. },
                    ScalarType::Integer(_) | ScalarType::IeeeFloat(_)
                ) | (
                    OperationKind::BooleanStructuralField { .. },
                    ScalarType::Boolean
                )
            ) {
                return malformed("scalar structural field has an invalid result type");
            }
            if let Some((producer, local)) = machine
                .operations()
                .filter_map(|producer| producer.result.structural().map(|local| (producer, local)))
                .find(|(_, local)| local.place == *source)
            {
                let (observed_root, access) = match module
                    .structural_type(local.structural_type)
                    .map(|declaration| &declaration.shape)
                {
                    Some(StructuralTypeShape::Reference { referent, access }) => {
                        (*referent, *access)
                    }
                    _ => (
                        local.structural_type,
                        match &producer.kind {
                            OperationKind::EstablishReference { source, .. }
                            | OperationKind::EstablishAccessReference { source, .. }
                            | element_view_establish_pair_pattern!(source, ..) => source.access,
                            _ => StructuralAccess::Owned,
                        },
                    ),
                };
                let matching = observed_field_declaration(
                    module.structural_types.iter(),
                    observed_root,
                    path,
                    *field,
                )
                .is_some_and(|candidate| {
                    candidate.field_type.scalar_type() == Some(result.scalar_type)
                        && scalar_observation_authorized(
                            module,
                            observed_root,
                            path,
                            &candidate.field_type,
                            access,
                        )
                });
                if !matching
                    || (!owns_observed_storage(module, local.structural_type, access)
                        && (local.multiplicity == StructuralMultiplicity::Linear
                            || !local.claims.is_empty()))
                {
                    return malformed("scalar record field has invalid local result custody");
                }
                return Ok(());
            }
            let Some(parameter) = machine.signature_structural_parameter(*source).or_else(|| {
                let declaration = machine.structural_place(*source)?;
                let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind
                else {
                    return None;
                };
                machine
                    .block(&block)?
                    .structural_parameters
                    .get(position as usize)
                    .filter(|parameter| {
                        parameter.place == *source && parameter.position == position
                    })
            }) else {
                return malformed("scalar structural field source is not a parameter");
            };
            let matching = observed_field_declaration(
                module.structural_types.iter(),
                parameter.structural_type,
                path,
                *field,
            )
            .filter(|candidate| {
                candidate.field_type.scalar_type() == Some(result.scalar_type)
                    && scalar_observation_authorized(
                        module,
                        parameter.structural_type,
                        path,
                        &candidate.field_type,
                        parameter.access,
                    )
            });
            let owned_observation =
                owns_observed_storage(module, parameter.structural_type, parameter.access);
            if (!owned_observation && parameter.multiplicity == StructuralMultiplicity::Linear)
                || !matches!(
                    parameter.access,
                    StructuralAccess::Owned
                        | StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                )
                || (!owned_observation
                    && (machine
                        .entry_claims
                        .iter()
                        .any(|claim| claim.input == *source)
                        || machine
                            .content_entry_claims
                            .iter()
                            .any(|claim| claim.input.root == *source)))
                || matching.is_none()
            {
                return malformed("scalar structural field has invalid source custody");
            }
            Ok(())
        }

        fn scalar_observation_authorized(
            module: &TerminalModule,
            root: StructuralTypeId,
            path: &[CanonicalStructuralPathSegment],
            field: &StructuralFieldType,
            access: StructuralAccess,
        ) -> bool {
            let Some(access) =
                observed_field_backing_access(module.structural_types.iter(), root, path, access)
            else {
                return false;
            };
            access != StructuralAccess::WriteOnlyBorrow
                && (!matches!(
                    field,
                    StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
                ) || matches!(access, mutating_access_pair_pattern!()))
        }
    }
    pub(crate) mod structural_transfers {

        use super::*;
        use terminal_semantics::descriptor_results::{
            DescriptorAnalysis, DescriptorMachine, current_payload_contract,
            current_selected_payload_contract, descriptor_contents_query, payload_memberships,
            terminal_graph_from, terminal_payload_contract,
        };
        use terminal_semantics::{canonical_static_projection, is_copyable_carrier};

        pub(crate) struct StructuralTransferInputs<'a> {
            module: &'a TerminalModule,
            descriptors: Option<(semantic_vocabulary::MachineId, Vec<DescriptorMachine>)>,
        }

        impl<'a> StructuralTransferInputs<'a> {
            pub(crate) fn new(module: &'a TerminalModule) -> Self {
                Self {
                    module,
                    descriptors: None,
                }
            }

            fn descriptors(
                &mut self,
                machine: semantic_vocabulary::MachineId,
            ) -> &[DescriptorMachine] {
                if self
                    .descriptors
                    .as_ref()
                    .is_none_or(|(owner, _)| *owner != machine)
                {
                    self.descriptors = Some((machine, terminal_graph_from(self.module, [machine])));
                }
                &self.descriptors.as_ref().expect("prepared body closure").1
            }
        }

        pub(crate) fn validate_move_structural_value(
            inputs: &mut StructuralTransferInputs<'_>,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let module = inputs.module;
            let OperationKind::MoveStructuralValue { source } = &operation.kind else {
                return malformed("owned value transfer dispatch differs");
            };
            let Some(result) = operation.result.structural() else {
                return malformed("owned value transfer has no structural result");
            };
            let whole_multiplicity = machine
                .structural_parameter(source.place)
                .map(|parameter| parameter.multiplicity)
                .or_else(|| {
                    machine
                        .producing_operation(source.place)
                        .and_then(|(producer, _)| producer.result.structural())
                        .map(|result| result.multiplicity)
                });
            if source.access != terminal_psi::StructuralAccess::Owned
                || result.multiplicity == StructuralMultiplicity::Unrestricted
                || whole_multiplicity == Some(StructuralMultiplicity::Unrestricted)
                || (!source.path.is_empty()
                    && result.multiplicity != StructuralMultiplicity::Linear
                    && result.claims.iter().any(|claim| claim.path.is_empty()))
                || !result.qualification_establishments.is_empty()
                || source.place == result.place
                || (source.path.is_empty() && whole_multiplicity != Some(result.multiplicity))
                || machine
                    .structural_parameter(source.place)
                    .is_some_and(|parameter| {
                        parameter.access != terminal_psi::StructuralAccess::Owned
                    })
            {
                return malformed(
                    "owned value transfer requires owned custody and cannot issue authority",
                );
            }
            let root = structural_place_type(machine, source.place).ok_or(
                CodecError::MalformedStructuralFoundation("owned transfer source is absent"),
            )?;
            let selected =
                canonical_static_projection(module.structural_types.iter(), root, &source.path)
                    .map(|(_, selected)| selected)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "owned transfer requires a finite owned path",
                    ))?;
            if selected.id != result.structural_type {
                return malformed("owned value transfer changes its carrier");
            }
            if !matches!(machine.structural_place(result.place).map(|place| place.kind),
                Some(StructuralPlaceKind::OperationResult { producer, structural_type })
                    if producer == operation.id && structural_type == result.structural_type)
            {
                return malformed("owned value transfer result is not a fresh occurrence");
            }
            let position = machine
                .blocks
                .iter()
                .find_map(|block| {
                    block
                        .operations
                        .iter()
                        .position(|candidate| candidate.id == operation.id)
                        .and_then(|ordinal| {
                            u32::try_from(ordinal)
                                .ok()
                                .map(|ordinal| (block.id, ordinal))
                        })
                })
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "owned transfer position is absent",
                ))?;
            let graph = inputs.descriptors(machine.id);
            let contract = current_selected_payload_contract(
                &module.structural_types,
                &module.structural_domains,
                graph,
                machine.id,
                position,
                source.place,
                &source.path,
                &|place| terminal_payload_contract(machine, place),
            )
            .map_err(CodecError::MalformedStructuralFoundation)?;
            if contract.carrier != result.structural_type
                || contract
                    .qualifications
                    .iter()
                    .any(|domain| !result.qualifications.contains(domain))
                || contract
                    .projected_qualifications
                    .iter()
                    .any(|row| !result.projected_qualifications.contains(row))
            {
                return malformed("owned value transfer loses source memberships");
            }
            validate_transfer_domain_additions(
                module,
                &contract.qualifications,
                &contract.projected_qualifications,
                result,
            )
        }

        fn validate_transfer_domain_additions(
            module: &TerminalModule,
            source: &[semantic_vocabulary::StructuralDomainId],
            projected: &[terminal_psi::StructuralPathQualification],
            result: &terminal_psi::StructuralOperationResult,
        ) -> Result<(), CodecError> {
            let additions = result
                .qualifications
                .iter()
                .map(|domain| (&[][..], domain))
                .chain(
                    result
                        .projected_qualifications
                        .iter()
                        .map(|row| (row.path.as_slice(), &row.domain)),
                );
            for (path, domain) in additions {
                let declaration = module
                    .structural_domains
                    .iter()
                    .find(|row| row.id == *domain)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "value transfer domain is absent",
                    ))?;
                let (supplied, _) =
                    terminal_psi::StructuralPathQualification::select(source, projected, path);
                let missing = declaration
                    .missing_membership_requirements(&module.structural_domains, &supplied)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "value transfer has unknown prerequisites",
                    ))?;
                if missing
                    .iter()
                    .any(|row| !row.establishment_routes.is_empty())
                {
                    return malformed(
                        "value transfer cannot establish unsupplied routed prerequisites",
                    );
                }
            }
            Ok(())
        }

        pub(crate) fn validate_structural_leaf_copy(
            inputs: &mut StructuralTransferInputs<'_>,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> Result<(), CodecError> {
            let module = inputs.module;
            let (terminal_psi::structural_leaf_copy_pair_pattern!()) = &operation.kind else {
                unreachable!("dispatched validate_structural_leaf_copy")
            };
            if operation.result.structural().is_none_or(|result| {
                (result.multiplicity != StructuralMultiplicity::Unrestricted
                    && !(result.multiplicity == StructuralMultiplicity::Affine
                        && is_copyable_carrier(
                            module.structural_types.iter(),
                            result.structural_type,
                        )))
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
            }) {
                return malformed("leaf copy has invalid multiplicity or qualification custody");
            }
            let result = operation
                .result
                .structural()
                .expect("checked structural copy result");
            if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() {
                let OperationKind::StructuralLeafCopy { source, path } = &operation.kind else {
                    return malformed("case leaf copy does not authenticate memberships");
                };
                let descriptors = inputs.descriptors(machine.id);
                let mut analysis = DescriptorAnalysis::new(&module.structural_types, descriptors);
                let mut history_failures = Vec::new();
                let mut query_phase = "root";
                let memberships = (|| {
                    let root = terminal_payload_contract(machine, *source)?;
                    query_phase = "query";
                    let query = descriptor_contents_query(
                        &module.structural_types,
                        *source,
                        root.carrier,
                        path,
                        None,
                    )?;
                    query_phase = "position";
                    let position = machine.blocks.iter().find_map(|block| {
                        block
                            .operations
                            .iter()
                            .position(|candidate| candidate.id == operation.id)
                            .and_then(|ordinal| {
                                u32::try_from(ordinal)
                                    .ok()
                                    .map(|ordinal| (block.id, ordinal))
                            })
                    })?;
                    query_phase = "targets";
                    let targets = analysis.payload_targets(machine.id, position, &query)?;
                    query_phase = "memberships";
                    payload_memberships(
                        &module.structural_types,
                        &module.structural_domains,
                        targets,
                        |place| {
                            current_payload_contract(
                                &module.structural_types,
                                &module.structural_domains,
                                descriptors,
                                machine.id,
                                position,
                                place,
                                &|source| terminal_payload_contract(machine, source),
                            )
                            .map_err(|reason| history_failures.push((place, reason)))
                            .ok()
                        },
                    )
                })()
                .and_then(|contract| {
                    query_phase = "presentation";
                    contract.presented(
                        &module.structural_types,
                        &module.structural_domains,
                        result.structural_type,
                    )
                });
                let Some(contract) = memberships
                    .as_ref()
                    .filter(|contract| contract.carrier == result.structural_type)
                else {
                    return Err(CodecError::MalformedStructuralOperation {
                        machine: machine
                            .declaration
                            .as_ref()
                            .and_then(|declaration| module.identities.text(declaration.callable))
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("{:?}", machine.id)),
                        operation: operation.id,
                        reason: format!(
                            "leaf copy memberships do not follow from every current payload source: source={source:?}, path={path:?}, requested={:?}/{:?}/{:?}, current={memberships:?}, history={history_failures:?}, phase={query_phase}, machine_present={}",
                            result.structural_type,
                            result.qualifications,
                            result.projected_qualifications,
                            descriptors.iter().any(|row| row.id == machine.id),
                        ),
                    });
                };
                validate_transfer_domain_additions(
                    module,
                    &contract.qualifications,
                    &contract.projected_qualifications,
                    result,
                )
                .inspect_err(|_| {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE structural-leaf-transfer machine={:?} operation={:?} source={:?} path={:?} requested={:?}/{:?}/{:?} current={:?}",
                            machine.id,
                            operation.id,
                            source,
                            path,
                            result.structural_type,
                            result.qualifications,
                            result.projected_qualifications,
                            contract,
                        );
                        trace_payload_producer_chain(machine, operation);
                    }
                })?;
            }
            Ok(())
        }

        fn trace_payload_producer_chain(machine: &TerminalMachine, failing: &Operation) {
            const MAX_BLOCKS: usize = 64;
            const MAX_OPERATIONS: usize = 256;
            const MAX_DETAIL_CHARACTERS: usize = 4096;
            eprintln!(
                "CHECKSITE payload-chain-inputs machine={:?} failing={:?} entry={:?} structural={:?} requires={:?}",
                machine.id,
                failing.id,
                machine.entry,
                machine.structural_parameters,
                machine.contract.requires,
            );
            let mut remaining = MAX_OPERATIONS;
            for block in machine.blocks.iter().take(MAX_BLOCKS) {
                let edge = format!("{:?}", block.terminator);
                let edge_cut = edge.chars().count() > MAX_DETAIL_CHARACTERS;
                let edge: String = edge.chars().take(MAX_DETAIL_CHARACTERS).collect();
                eprintln!(
                    "CHECKSITE payload-chain-block machine={:?} block={:?} structural={:?} terminator={} terminator_cut={}",
                    machine.id, block.id, block.structural_parameters, edge, edge_cut,
                );
                for (ordinal, operation) in block.operations.iter().enumerate().take(remaining) {
                    let detail = format!("{:?}", operation.kind);
                    let detail_cut = detail.chars().count() > MAX_DETAIL_CHARACTERS;
                    let detail: String = detail.chars().take(MAX_DETAIL_CHARACTERS).collect();
                    eprintln!(
                        "CHECKSITE payload-chain-operation machine={:?} block={:?} ordinal={} operation={:?} failing={} result={:?} kind={} kind_cut={}",
                        machine.id,
                        block.id,
                        ordinal,
                        operation.id,
                        operation.id == failing.id,
                        operation.result,
                        detail,
                        detail_cut,
                    );
                }
                remaining = remaining.saturating_sub(block.operations.len());
            }
            eprintln!(
                "CHECKSITE payload-chain-end machine={:?} blocks={} operations={} blocks_cut={} operations_cut={}",
                machine.id,
                machine.blocks.len().min(MAX_BLOCKS),
                MAX_OPERATIONS - remaining,
                machine.blocks.len() > MAX_BLOCKS,
                machine.operations().count() > MAX_OPERATIONS,
            );
        }
    }
    use self::structural_fields::declared_memberships;
    use super::scalar_fields::{
        validate_primitive_reference_presentation, validate_storage_interpretation,
    };
    pub(crate) use byte_sequences::*;
    pub(crate) use element_views::*;
    pub(crate) use structural_fields::*;
    pub(crate) use structural_transfers::*;
    use terminal_psi::{
        ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralAccess,
        StructuralFieldType, StructuralMultiplicity, StructuralPathQualification,
        StructuralPathSegment, StructuralTypeShape, TerminalMachine, TerminalModule,
        is_static_structural_scalar_store_path, is_structural_scalar_store_path,
    };
    use terminal_semantics::{
        access_reference_correspondence, arrival_parameter,
        borrowed_primitive_projection_declaration, projected_field_declaration,
        reference_byte_field_compatible, reference_carrier_memberships,
        reference_preserves_ancestor_qualifications, reference_source_projection_accessible,
        reference_source_projection_compatible, reference_source_storage_path,
        reference_storage_source_compatible, resident_reference_plan, runtime_structural_field,
        scalar_field_store_preserves_content, scalar_field_store_preserves_domain_content,
        scalar_projection_entails_membership, store_preserves_content,
        structural_memberships_disjoint_from_write,
    };

    fn reference_store_carrier<'path>(
        module: &TerminalModule,
        machine: &TerminalMachine,
        destination: PlaceId,
        path: &'path [StructuralPathSegment],
        field: StructuralFieldId,
    ) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
        let crossing = path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
        let suffix = &path[crossing + 1..];
        let structural_type = structural_place_type(machine, destination)?;
        let member = projected_field_declaration(
            module.structural_types.iter(),
            structural_type,
            path,
            field,
        )?;
        let mut written = path.to_vec();
        written.push(StructuralPathSegment::Field(member.identity));
        let preserves = |whole: &[_], projected: &[_]| {
            structural_memberships_disjoint_from_write(whole, projected, &written)
        };
        let (multiplicity, access) =
            if let Some(parameter) = machine.structural_parameter(destination) {
                if !preserves(
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                ) {
                    return None;
                }
                (parameter.multiplicity, parameter.access)
            } else {
                let result = machine
                    .operations()
                    .filter_map(|operation| operation.result.structural())
                    .find(|result| {
                        result.place == destination
                            && result.structural_type == structural_type
                            && preserves(&result.qualifications, &result.projected_qualifications)
                            && result.claims.is_empty()
                    })?;
                (result.multiplicity, StructuralAccess::Owned)
            };
        if !reference_source_projection_accessible(
            module.structural_types.iter(),
            structural_type,
            multiplicity,
            access,
            path,
            StructuralAccess::WriteOnlyBorrow,
        ) {
            return None;
        }
        let StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow,
        } = module
            .structural_type(
                validate_structural_path(module, structural_type, &path[..crossing]).ok()?,
            )?
            .shape
        else {
            return None;
        };
        module
            .structural_types
            .iter()
            .any(|declaration| declaration.id == referent)
            .then_some((referent, suffix))
    }

    fn owned_store_carrier(
        module: &TerminalModule,
        machine: &TerminalMachine,
        destination: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        let (structural_type, multiplicity) =
            if let Some(parameter) = arrival_parameter(machine, destination) {
                if parameter.access != StructuralAccess::Owned
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return None;
                }
                (parameter.structural_type, parameter.multiplicity)
            } else {
                let StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = machine.structural_place(destination)?.kind
                else {
                    return None;
                };
                let mut producers = machine
                    .operations()
                    .filter(|operation| operation.id == producer);
                let result = producers.next()?.result.structural()?;
                if producers.next().is_some()
                    || result.place != destination
                    || result.structural_type != structural_type
                {
                    return None;
                }
                (structural_type, result.multiplicity)
            };
        reference_source_projection_accessible(
            module.structural_types.iter(),
            structural_type,
            multiplicity,
            StructuralAccess::Owned,
            path,
            StructuralAccess::WriteOnlyBorrow,
        )
        .then_some(structural_type)
    }

    pub(crate) fn validate_u64_length_result(
        operation: &Operation,
        malformed_label: &'static str,
    ) -> Result<(), CodecError> {
        let expected =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
        if operation
            .result
            .scalar()
            .is_none_or(|result| result.scalar_type != expected)
        {
            return malformed(malformed_label);
        }
        Ok(())
    }

    pub(crate) fn validate_establish_access_reference(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishAccessReference { source, resident } = &operation.kind else {
            unreachable!("dispatched validate_establish_access_reference")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("placed establishment requires a structural reference result");
        };
        let Some(source_type) = structural_place_type(machine, source.place) else {
            return malformed("placed establishment source is unknown");
        };
        let source_type = validate_structural_path(module, source_type, &source.path)?;
        if result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return malformed(
                "placed establishment cannot introduce owned content or qualifications",
            );
        }
        if let Some(resident) = resident {
            return resident_reference_plan(
                module,
                source_type,
                result.structural_type,
                source.access,
                *resident,
            )
            .map(|_| ())
            .map_err(CodecError::MalformedStructuralFoundation);
        }
        if let Err(reason) = access_reference_correspondence(
            module,
            source_type,
            result.structural_type,
            source.access,
        ) {
            return Err(CodecError::AccessRamCorrespondence(reason));
        }
        Ok(())
    }

    pub(crate) fn validate_establish_reference(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishReference { source, storage } = &operation.kind else {
            unreachable!("dispatched validate_establish_reference")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("reference establishment requires a structural result");
        };
        let Some(StructuralTypeShape::Reference { access, .. }) = module
            .structural_type(result.structural_type)
            .map(|row| &row.shape)
        else {
            return malformed("reference establishment requires a reference carrier type");
        };
        let Some(source_type) = structural_place_type(machine, source.place) else {
            return malformed("reference establishment source is unknown");
        };
        let source_path = reference_source_storage_path(
            module.structural_types.iter(),
            source_type,
            &source.path,
        );
        validate_primitive_reference_presentation(
            module,
            machine,
            source.place,
            source_path,
            *access,
        )?;
        if let Some(storage) = storage {
            let offset_type = declared_scalar_type(machine, storage.byte_offset);
            let offset_is_u64 = matches!(offset_type, Some(ScalarType::Integer(integer))
                if !integer.is_address() && integer.bits() == 64
                    && integer.sign() == IntegerSign::Unsigned);
            if !offset_is_u64 {
                return malformed("reference storage offset is not a u64 scalar");
            }
        }
        let compatible = match storage {
            Some(storage)
                if matches!(
                    source_path.last(),
                    Some(StructuralPathSegment::FixedRange { .. })
                ) =>
            {
                let (_, prefix) = source_path.split_last().expect("matched range");
                let array = validate_structural_path(module, source_type, prefix)?;
                validate_storage_interpretation(module, array, result.structural_type, *access)?;
                representation_geometry::fixed_region_is_compatible(
                    module,
                    source_type,
                    source_path,
                    result.structural_type,
                    storage.stored_scalar,
                )?
            }
            Some(storage) => {
                let observed = (*access == StructuralAccess::SharedBorrow)
                    .then(|| {
                        borrowed_primitive_projection_declaration(
                            module.structural_types.iter(),
                            source_type,
                            source_path,
                            None,
                        )
                    })
                    .flatten();
                let source_type = match observed {
                    Some(observed) => observed,
                    None => validate_structural_path(module, source_type, source_path)?,
                };
                validate_storage_interpretation(
                    module,
                    source_type,
                    result.structural_type,
                    *access,
                )?;
                reference_storage_source_compatible(
                    module.structural_types.iter(),
                    source_type,
                    result.structural_type,
                    storage.stored_scalar,
                )
            }
            None => reference_source_projection_compatible(
                module.structural_types.iter(),
                source_type,
                source_path,
                result.structural_type,
            ),
        };
        let ancestors_preserved = *access == StructuralAccess::SharedBorrow
            || declared_memberships(machine, source.place).is_some_and(|(whole, projected)| {
                reference_preserves_ancestor_qualifications(
                    module.structural_types.iter(),
                    module.structural_domains.iter(),
                    &module.identities,
                    whole,
                    projected,
                    source_path,
                    *access,
                ) && store_preserves_content(
                    module.structural_types.iter(),
                    module.structural_domains.iter(),
                    &module.identities,
                    source_type,
                    source.place,
                    machine
                        .content_entry_claims
                        .iter()
                        .filter(|claim| claim.input.segments.len() < source_path.len()),
                    source_path,
                )
            });
        if !ancestors_preserved {
            return malformed(
                "reference establishment cannot preserve source ancestor obligations",
            );
        }
        if !compatible {
            return malformed("reference establishment has incompatible source storage geometry");
        }
        if result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.claims.is_empty()
            || *access != source.access
            || source.access == StructuralAccess::Owned
        {
            return malformed(
                "reference establishment has inconsistent carrier ownership or access",
            );
        }
        Ok(())
    }

    pub(crate) fn validate_reference_memberships(
        module: &TerminalModule,
    ) -> Result<(), CodecError> {
        let querying_machines = module
            .machines
            .iter()
            .filter(|machine| {
                machine.operations().any(|operation| {
                    matches!(&operation.kind, OperationKind::EstablishReference { .. })
                })
            })
            .collect::<Vec<_>>();
        if querying_machines.is_empty() {
            return Ok(());
        }
        let graph = terminal_semantics::descriptor_results::terminal_graph_from(
            module,
            querying_machines.iter().map(|machine| machine.id),
        );
        let mut analysis = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
            &module.structural_types,
            &graph,
        );
        for machine in querying_machines {
            for block in &machine.blocks {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    if !matches!(&operation.kind, OperationKind::EstablishReference { .. }) {
                        continue;
                    }
                    let ordinal = u32::try_from(ordinal).map_err(|_| {
                        super::operation_foundation_error(
                            module,
                            machine,
                            block.id,
                            operation,
                            CodecError::MalformedStructuralFoundation(
                                "operation ordinal exceeds u32",
                            ),
                        )
                    })?;
                    validate_reference_membership(
                        module,
                        machine,
                        operation,
                        (block.id, ordinal),
                        &graph,
                        &mut analysis,
                    )
                    .map_err(|error| {
                        super::operation_foundation_error(
                            module, machine, block.id, operation, error,
                        )
                    })?;
                }
            }
        }
        Ok(())
    }

    fn validate_reference_membership(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
        position: (BlockId, u32),
        reference_graph: &[terminal_semantics::descriptor_results::DescriptorMachine],
        reference_analysis: &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'_>,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishReference { source, storage } = &operation.kind else {
            return malformed("reference membership admission has no reference operation");
        };
        let Some(result) = operation.result.structural() else {
            return malformed("reference membership admission has no structural result");
        };
        let Some(StructuralTypeShape::Reference { access, .. }) = module
            .structural_type(result.structural_type)
            .map(|row| &row.shape)
        else {
            return malformed("reference membership admission has no reference carrier");
        };
        let Some(source_type) = structural_place_type(machine, source.place) else {
            return malformed("reference membership admission has no source declaration");
        };
        let source_path = reference_source_storage_path(
            module.structural_types.iter(),
            source_type,
            &source.path,
        );
        let inline_owner = storage.is_none()
            && runtime_structural_field(module.structural_types.iter(), source_type, source_path)
                .is_some_and(|field| {
                    matches!(
                        field.field_type.byte_sequence_carrier(),
                        Some(ByteSequenceCarrier::BoundedOwned { .. })
                    )
                });
        let field_compatible = inline_owner
            && reference_byte_field_compatible(
                module.structural_types.iter(),
                module.structural_domains.iter(),
                source_type,
                source_path,
                result.structural_type,
                &result.projected_qualifications,
            );
        let memberships_match = if inline_owner {
            field_compatible
        } else {
            declared_memberships(machine, source.place).is_some_and(|(whole, projected)| {
                let Ok(mut captured) = reference_carrier_memberships(
                    module.structural_types.iter(),
                    source_type,
                    whole,
                    projected,
                    source_path,
                    *access,
                    result.structural_type,
                    storage.as_ref(),
                ) else {
                    return false;
                };
                if let Ok(backing) =
                    terminal_semantics::descriptor_results::reference_backing_memberships(
                        &module.structural_types,
                        &module.structural_domains,
                        &module.identities,
                        reference_analysis,
                        machine.id,
                        position,
                        source,
                        storage.as_ref(),
                        result,
                        |place| {
                            terminal_semantics::descriptor_results::terminal_payload_contract(
                                machine, place,
                            )
                        },
                    )
                {
                    captured.extend(backing);
                }
                if *access == StructuralAccess::SharedBorrow
                    && result
                        .projected_qualifications
                        .iter()
                        .any(|row| !captured.contains(row))
                {
                    let Ok(current) =
                        terminal_semantics::descriptor_results::current_selected_payload_contract(
                            &module.structural_types,
                            &module.structural_domains,
                            reference_graph,
                            machine.id,
                            position,
                            source.place,
                            source_path,
                            &|place| {
                                terminal_semantics::descriptor_results::terminal_payload_contract(
                                    machine, place,
                                )
                            },
                        )
                    else {
                        return false;
                    };
                    StructuralPathQualification::append_under(
                        &current.qualifications,
                        &current.projected_qualifications,
                        &[StructuralPathSegment::Referent],
                        &mut captured,
                    );
                }
                result.projected_qualifications.iter().all(|row| {
                    captured.contains(row)
                        || row
                            .path
                            .strip_prefix(&[StructuralPathSegment::Referent])
                            .is_some_and(|relative| {
                                let mut path = source_path.to_vec();
                                path.extend_from_slice(relative);
                                scalar_projection_entails_membership(
                                    module.structural_types.iter(),
                                    module.structural_domains.iter(),
                                    source_type,
                                    &path,
                                    row.domain,
                                )
                            })
                }) && (*access == StructuralAccess::SharedBorrow
                    || captured
                        .iter()
                        .all(|row| result.projected_qualifications.contains(row)))
            })
        };
        if (inline_owner
            || !result.projected_qualifications.is_empty()
            || *access != StructuralAccess::SharedBorrow)
            && !memberships_match
        {
            return malformed(
                "reference establishment cannot retain source membership obligations",
            );
        }
        Ok(())
    }

    pub(crate) fn validate_release_reference(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::ReleaseReference { source, path } = &operation.kind else {
            unreachable!("dispatched validate_release_reference")
        };
        let Some(source_type) = structural_place_type(machine, *source) else {
            return malformed("reference release source is absent");
        };
        if operation.result != OperationResult::Unit
            || path.iter().any(|segment| {
                !matches!(
                    segment,
                    StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                )
            })
        {
            return malformed(
                "reference release requires an owned descriptor path and Unit result",
            );
        }
        let leaf_type = validate_structural_path(module, source_type, path)?;
        if !module.structural_types.iter().any(|row| {
            row.id == leaf_type && matches!(row.shape, StructuralTypeShape::Reference { .. })
        }) {
            return malformed("reference release requires a reference carrier");
        }
        Ok(())
    }

    fn field_store_carrier<'path>(
        module: &TerminalModule,
        machine: &TerminalMachine,
        destination: PlaceId,
        path: &'path [StructuralPathSegment],
        field: StructuralFieldId,
    ) -> Result<(StructuralTypeId, &'path [StructuralPathSegment]), CodecError> {
        let preserves = |root, whole: &[_], projected: &[_]| {
            scalar_field_store_preserves_domain_content(
                module.structural_types.iter(),
                module.structural_domains.iter(),
                &module.identities,
                root,
                whole,
                projected,
                path,
                field,
            )
        };
        let reference = reference_store_carrier(module, machine, destination, path, field);
        let destination_type = if let Some((referent, _)) = reference {
            referent
        } else if let Some(parameter) = machine.structural_parameter(destination) {
            if !matches!(
                parameter.access,
                StructuralAccess::Owned
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            ) || !preserves(
                parameter.structural_type,
                &parameter.qualifications,
                &parameter.projected_qualifications,
            ) {
                return malformed("structural field store cannot preserve destination custody");
            }
            parameter.structural_type
        } else {
            let Some(result) = machine
                .operations()
                .filter_map(|producer| producer.result.structural())
                .find(|result| result.place == destination)
            else {
                return malformed("structural field store destination has no record home");
            };
            if !preserves(
                result.structural_type,
                &result.qualifications,
                &result.projected_qualifications,
            ) {
                return malformed("structural field store has invalid local custody");
            }
            result.structural_type
        };
        let Some(content_root) = structural_place_type(machine, destination) else {
            return malformed("structural field store has no declared content root");
        };
        if !scalar_field_store_preserves_content(
            module.structural_types.iter(),
            module.structural_domains.iter(),
            &module.identities,
            content_root,
            destination,
            machine.content_entry_claims.iter(),
            path,
            field,
        ) {
            return malformed("structural scalar field store changes conserved content");
        }
        let carrier_path = reference.map_or(path, |(_, suffix)| suffix);
        if !is_structural_scalar_store_path(carrier_path) {
            return malformed("structural field store has invalid destination custody");
        }
        Ok((destination_type, carrier_path))
    }

    fn place_structural_type(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        if let Some(parameter) = machine.signature_structural_parameter(place) {
            return Some(parameter.structural_type);
        }
        if let Some(parameter) = machine
            .block_structural_parameters()
            .find(|parameter| parameter.place == place)
        {
            return Some(parameter.structural_type);
        }
        machine.structural_places.iter().find_map(|row| {
            if row.id != place {
                return None;
            }
            match row.kind {
                StructuralPlaceKind::OperationResult {
                    structural_type, ..
                }
                | StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                } => Some(structural_type),
                _ => None,
            }
        })
    }

    pub(crate) fn validate_global_descriptor_table(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::GlobalDescriptorTable {
            service,
            operation: action,
            source,
        } = &operation.kind
        else {
            return malformed("descriptor-table validator received another operation");
        };
        let writes = *action == terminal_psi::GlobalDescriptorTableOperation::Store;
        let access_matches = if writes {
            matches!(
                source.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            )
        } else {
            matches!(
                source.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
        };
        if operation.result != OperationResult::Unit
            || !super::has_service(module, *service)
            || !access_matches
        {
            return malformed(
                "descriptor-table operation requires Unit, service authority, and the declared byte borrow",
            );
        }
        validate_hardware_array_loan(module, machine, source, writes, 8, 10)
    }

    fn validate_hardware_array_loan(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: &terminal_psi::StructuralArgument,
        writes: bool,
        bits: u16,
        extent: u64,
    ) -> Result<(), CodecError> {
        let root = structural_place_type(machine, source.place).ok_or(
            CodecError::MalformedStructuralFoundation(
                "descriptor-table operand has no declared storage",
            ),
        )?;
        let preserves = |whole: &[_], projected: &[_]| {
            !writes || structural_memberships_disjoint_from_write(whole, projected, &source.path)
        };
        let (multiplicity, access) = if let Some(parameter) =
            arrival_parameter(machine, source.place)
        {
            if !preserves(
                &parameter.qualifications,
                &parameter.projected_qualifications,
            ) {
                return malformed(
                    "descriptor-table store cannot preserve a predicate on the overwritten bytes",
                );
            }
            (parameter.multiplicity, parameter.access)
        } else {
            let result = machine
                .operations()
                .filter_map(|row| row.result.structural())
                .find(|result| result.place == source.place && result.structural_type == root)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "descriptor-table operand has no retained producer",
                ))?;
            if !preserves(&result.qualifications, &result.projected_qualifications)
                || (writes && !result.claims.is_empty())
            {
                return malformed(
                    "descriptor-table store cannot overwrite retained content obligations",
                );
            }
            (result.multiplicity, StructuralAccess::Owned)
        };
        if writes
            && !store_preserves_content(
                module.structural_types.iter(),
                module.structural_domains.iter(),
                &module.identities,
                root,
                source.place,
                machine.content_entry_claims.iter(),
                &source.path,
            )
        {
            return malformed("descriptor-table store changes conserved content");
        }
        if !terminal_semantics::descriptor_source_projection_accessible(
            module.structural_types.iter(),
            root,
            multiplicity,
            access,
            &source.path,
            source.access,
        ) {
            return malformed("descriptor-table byte borrow exceeds its source access");
        }
        if writes
            && runtime_structural_field(module.structural_types.iter(), root, &source.path)
                .is_some_and(|field| !field.field_type.byte_predicates().is_empty())
        {
            return malformed(
                "descriptor-table store cannot preserve a predicate on its byte field",
            );
        }
        let selected = validate_structural_path(module, root, &source.path)?;
        let declaration =
            module
                .structural_type(selected)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "descriptor-table byte array declaration is absent",
                ))?;
        let plain = |declaration: &terminal_psi::StructuralTypeDeclaration| {
            declaration.quotient_carrier.is_none()
                && declaration.scalar_restriction.is_none()
                && !declaration.owns_linear_claim
        };
        let exact_bytes = plain(declaration)
            && match &declaration.shape {
                StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BoundedOwned { capacity }
                    | ByteSequenceCarrier::BoundedView { capacity, .. },
                ) => bits == 8 && *capacity == extent,
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } if *length == extent => module.structural_type(*element).is_some_and(|element| {
                    plain(element)
                        && matches!(
                            element.shape,
                            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                                if !integer.is_address()
                                    && integer.bits() == bits
                                    && integer.sign() == IntegerSign::Unsigned
                        )
                }),
                _ => false,
            };
        if !exact_bytes {
            return malformed(if bits == 8 && extent == 10 {
                "descriptor-table operand must be an exact unqualified ten-byte array"
            } else {
                "hardware operand must be an exact unqualified array of the declared element width and extent"
            });
        }
        Ok(())
    }

    pub(crate) fn validate_descriptor_operation(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let unsigned = |bits| {
            ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, bits).expect("fixed scalar width"),
            )
        };
        let (service, result_bits) = match &operation.kind {
            OperationKind::ReadTaskRegister { service }
            | OperationKind::ReadSegmentSelector { service, .. } => (*service, 16),
            OperationKind::ReloadSegmentDescriptors {
                service,
                entries,
                gdt,
                selector,
                scratch,
                ..
            } => {
                if !(1..=8192).contains(entries)
                    || gdt.access != StructuralAccess::MutableBorrow
                    || scratch.access != StructuralAccess::MutableBorrow
                    || declared_scalar_type(machine, *selector) != Some(unsigned(16))
                {
                    return malformed(
                        "selector reload requires a mutable GDT, mutable ten-byte scratch and u16 selector",
                    );
                }
                validate_hardware_array_loan(module, machine, gdt, true, 64, u64::from(*entries))?;
                validate_hardware_array_loan(module, machine, scratch, true, 8, 10)?;
                (*service, 8)
            }
            OperationKind::ActivateTaskRegister {
                service,
                entries,
                gdt,
                task_state,
                selector,
                scratch,
            } => {
                if !(2..=8192).contains(entries)
                    || gdt.access != StructuralAccess::MutableBorrow
                    || scratch.access != StructuralAccess::MutableBorrow
                    || !matches!(
                        task_state.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
                    || declared_scalar_type(machine, *selector) != Some(unsigned(16))
                {
                    return malformed(
                        "task activation requires a mutable GDT, readable 104-byte task state, mutable ten-byte scratch and u16 selector",
                    );
                }
                validate_hardware_array_loan(module, machine, gdt, true, 64, u64::from(*entries))?;
                validate_hardware_array_loan(module, machine, task_state, false, 8, 104)?;
                validate_hardware_array_loan(module, machine, scratch, true, 8, 10)?;
                (*service, 8)
            }
            _ => return malformed("descriptor validator received another operation"),
        };
        if !super::has_service(module, service)
            || operation
                .result
                .scalar()
                .is_none_or(|result| result.value_type() != unsigned(result_bits).into())
        {
            return malformed(
                "descriptor operation requires service authority and its exact unsigned result",
            );
        }
        Ok(())
    }
}
pub(crate) mod structural_type_foundations {

    use super::require_unique_nonempty_identities;
    use crate::codec_error::{CodecError, malformed};
    use semantic_vocabulary::AuthoredFieldStorage;
    use semantic_vocabulary::StructuralTypeId;
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        StructuralFieldType, StructuralTypeShape, TerminalModule, structural_children,
    };
    use terminal_psi::{StructuralRecordStorage, StructuralTypeDeclaration};

    macro_rules! extent {
        ($module:expr => $field:expr => $storage:expr => $size:expr) => {
            validate_authored_field_extent($module, $field, $storage, $size)
        };
    }

    pub(crate) struct StructuralCatalog<'a> {
        pub(crate) structural_types: &'a [StructuralTypeDeclaration],
        pub(crate) structural_referents: &'a [terminal_psi::StructuralReferentIdentity],
    }
    impl<'a> From<&'a TerminalModule> for StructuralCatalog<'a> {
        fn from(module: &'a TerminalModule) -> Self {
            Self {
                structural_types: &module.structural_types,
                structural_referents: &module.structural_referents,
            }
        }
    }
    fn has_structural_type(catalog: &StructuralCatalog<'_>, id: StructuralTypeId) -> bool {
        terminal_psi::find_structural_type(catalog.structural_types, id).is_some()
    }
    pub(crate) fn validate_type_shape(
        module: &TerminalModule,
        declaration: &StructuralTypeDeclaration,
        attachment_storage: &BTreeSet<StructuralTypeId>,
    ) -> Result<(), CodecError> {
        validate_catalog_type_shape(
            &StructuralCatalog::from(module),
            declaration,
            attachment_storage,
        )
    }

    fn check_field_identity_relevance(
        field: &terminal_psi::StructuralFieldDeclaration,
        erased_message: &'static str,
    ) -> Result<(), CodecError> {
        match field.field_type {
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::Atomic(_)
            | StructuralFieldType::AtomicBoundedInteger(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_)
            | StructuralFieldType::IeeeFloat(_)
            | StructuralFieldType::Structural(_)
                if field.relevance.is_erased() =>
            {
                malformed(erased_message)
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn validate_catalog_type_shape(
        module: &StructuralCatalog<'_>,
        declaration: &StructuralTypeDeclaration,
        attachment_storage: &BTreeSet<StructuralTypeId>,
    ) -> Result<(), CodecError> {
        terminal_semantics::validate_inline_carriers(module.structural_types, declaration)
            .map_err(|error| CodecError::MalformedStructuralFoundation(error.reason))?;
        if declaration.copyable && declaration.owns_linear_claim {
            return malformed("a copyable type cannot own a conserved linear claim");
        }
        if let Some(restriction) = &declaration.scalar_restriction {
            let StructuralTypeShape::PrimitiveScalar(carrier) = declaration.shape else {
                return malformed("scalar restriction requires a primitive declaration");
            };
            if !terminal_semantics::scalar_field_requires_evidence(restriction)
                || restriction.scalar_type() != Some(carrier)
            {
                return malformed("scalar restriction differs from its declared carrier");
            }
        }
        validate_field_storage_envelope(&declaration.shape)?;
        match &declaration.shape {
            StructuralTypeShape::Reference { referent, access } => {
                if !(has_structural_type(module, *referent)
                    || module
                        .structural_referents
                        .iter()
                        .any(|identity| identity.id == *referent))
                    || *access == terminal_psi::StructuralAccess::Owned
                {
                    return malformed(
                        "reference type requires a closed referent identity and borrowed access",
                    );
                }
            }
            StructuralTypeShape::PrimitiveScalar(_) => {}
            StructuralTypeShape::AtomicCell(cell) => {
                if !cell.is_supported() || declaration.scalar_restriction.is_some() {
                    return malformed("atomic storage has an invalid resident contract");
                }
            }
            StructuralTypeShape::ByteSequence(_) => {}
            StructuralTypeShape::Record { fields, storage } => {
                if let StructuralRecordStorage::Authored(storage) = storage
                    && (storage.byte_alignment == 0 || !storage.byte_alignment.is_power_of_two())
                {
                    return malformed(
                        "authored record storage requires positive power-of-two alignment",
                    );
                }
                require_unique_nonempty_identities(
                    fields.iter().map(|field| field.identity),
                    "structural field identity",
                )?;
                let mut destinations = Vec::new();
                for field in fields {
                    match (storage, field.storage.as_ref()) {
                        (StructuralRecordStorage::Authored(_), None)
                            if !field.relevance.is_erased() =>
                        {
                            return malformed(
                                "authored record field has no storage interpretation",
                            );
                        }
                        (StructuralRecordStorage::Authored(record), Some(field_storage)) => {
                            if field.relevance.is_erased()
                                || field_storage
                                    .byte_offset()
                                    .is_some_and(|offset| offset > record.byte_size)
                                || field_storage.element_stride() == Some(0)
                            {
                                return malformed(
                                    "authored field storage has invalid offset, stride or relevance",
                                );
                            }
                            let extent =
                                extent!(module => field => field_storage => record.byte_size)?;
                            if let AuthoredFieldStorage::Fragmented(recipe) = field_storage {
                                for fragment in recipe.fragments() {
                                    let start = u128::from(fragment.container_byte_offset) * 8
                                        + u128::from(fragment.destination_lsb);
                                    destinations.push((start, start + u128::from(fragment.width)));
                                }
                            } else if let Some((offset, extent)) = extent
                                && extent != 0
                            {
                                let start = u128::from(offset) * 8;
                                destinations.push((start, start + u128::from(extent) * 8));
                            }
                            if let Some(stored) = field_storage.stored_scalar() {
                                let logical = field
                                    .field_type
                                    .scalar_type()
                                    .or_else(|| {
                                        let identity = field.field_type.as_structural()?;
                                        let StructuralTypeShape::PrimitiveScalar(scalar) =
                                            terminal_psi::find_structural_type(
                                                module.structural_types,
                                                identity,
                                            )?
                                            .shape
                                        else {
                                            return None;
                                        };
                                        Some(scalar)
                                    })
                                    .ok_or(CodecError::MalformedStructuralFoundation(
                                        "encoded field requires a scalar referent",
                                    ))?;
                                let width =
                                    terminal_semantics::reference_storage_byte_width(stored)
                                        .ok_or(CodecError::MalformedStructuralFoundation(
                                            "encoded field has no byte-aligned scalar footprint",
                                        ))?;
                                if !terminal_semantics::reference_storage_view_is_compatible(
                                    stored, logical,
                                ) || field_storage
                                    .byte_offset()
                                    .and_then(|offset| offset.checked_add(u64::from(width)))
                                    .is_none_or(|end| end > record.byte_size)
                                {
                                    return malformed(
                                        "encoded field footprint or interpretation is incompatible",
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                    match &field.field_type {
                        StructuralFieldType::Atomic(_)
                        | StructuralFieldType::AtomicBoundedInteger(_)
                            if field.field_type.atomic_scalar_type().is_none() =>
                        {
                            return malformed(
                                "atomic field requires a fixed integer or Boolean cell",
                            );
                        }
                        StructuralFieldType::Structural(field_type)
                            if !has_structural_type(module, *field_type) =>
                        {
                            return malformed(
                                "structural field references an unknown structural type",
                            );
                        }
                        StructuralFieldType::Erased { type_identity }
                            if type_identity.is_absent() =>
                        {
                            return malformed(
                                "opaque structural field type must have a nonempty type identity",
                            );
                        }
                        StructuralFieldType::Erased { .. }
                            if !field.relevance.is_erased()
                                && !attachment_storage.contains(&declaration.id) =>
                        {
                            return malformed(
                                "provider-backed attachment specialization is incomplete",
                            );
                        }
                        _ => check_field_identity_relevance(
                            field,
                            "erased structural field must use its opaque semantic type identity",
                        )?,
                    }
                }
                validate_authored_destination_intervals(destinations)?;
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                if !has_structural_type(module, *element) {
                    return malformed("fixed array references an unknown structural element type");
                }
            }
            StructuralTypeShape::ElementView { element, access } => {
                if *access == Some(terminal_psi::StructuralAccess::Owned) {
                    return malformed("element view cannot declare owned backing access");
                }
                if !has_structural_type(module, *element) {
                    return malformed("element view references an unknown structural element type");
                }
            }
            StructuralTypeShape::Sum { cases } => {
                require_unique_nonempty_identities(
                    cases.iter().map(|case| case.identity),
                    "structural case identity",
                )?;
                if cases.is_empty() {
                    return malformed("structural sum must declare at least one case");
                }
                for case in cases {
                    require_unique_nonempty_identities(
                        case.fields.iter().map(|field| field.identity),
                        "structural case payload field identity",
                    )?;
                    for field in &case.fields {
                        match &field.field_type {
                            StructuralFieldType::Atomic(_)
                            | StructuralFieldType::AtomicBoundedInteger(_)
                                if field.field_type.atomic_scalar_type().is_none() =>
                            {
                                return malformed(
                                    "atomic field requires a fixed integer or Boolean cell",
                                );
                            }
                            StructuralFieldType::Structural(field_type)
                                if !has_structural_type(module, *field_type) =>
                            {
                                return malformed(
                                    "structural case payload references an unknown structural type",
                                );
                            }
                            StructuralFieldType::Erased { type_identity }
                                if !field.relevance.is_erased() || type_identity.is_absent() =>
                            {
                                return malformed(
                                    "opaque structural case payload must have erased relevance and a nonempty type identity",
                                );
                            }
                            _ => check_field_identity_relevance(
                                field,
                                "erased structural case payload must use its opaque semantic type identity",
                            )?,
                        }
                    }
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                require_unique_nonempty_identities(
                    fields.iter().map(|field| field.identity),
                    "mixed structural field identity",
                )?;
                require_unique_nonempty_identities(
                    cases.iter().map(|case| case.identity),
                    "mixed structural case identity",
                )?;
                if cases.is_empty() {
                    return malformed("mixed structural type must declare at least one case");
                }
                for case in cases {
                    require_unique_nonempty_identities(
                        case.fields.iter().map(|field| field.identity),
                        "mixed structural case payload field identity",
                    )?;
                }
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    match &field.field_type {
                        StructuralFieldType::Atomic(_)
                        | StructuralFieldType::AtomicBoundedInteger(_)
                            if field.field_type.atomic_scalar_type().is_none() =>
                        {
                            return malformed(
                                "atomic field requires a fixed integer or Boolean cell",
                            );
                        }
                        StructuralFieldType::Structural(field_type)
                            if !has_structural_type(module, *field_type) =>
                        {
                            return malformed(
                                "mixed structural field references an unknown structural type",
                            );
                        }
                        StructuralFieldType::Erased { type_identity }
                            if !field.relevance.is_erased() || type_identity.is_absent() =>
                        {
                            return malformed(
                                "opaque mixed structural field must have erased relevance and a nonempty type identity",
                            );
                        }
                        _ => check_field_identity_relevance(
                            field,
                            "erased mixed structural field must use its opaque semantic type identity",
                        )?,
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_authored_field_extent(
        module: &StructuralCatalog<'_>,
        field: &terminal_psi::StructuralFieldDeclaration,
        storage: &AuthoredFieldStorage,
        record_size: u64,
    ) -> Result<Option<(u64, u64)>, CodecError> {
        terminal_semantics::authored_field_storage_extent(
            module.structural_types.iter(),
            field,
            storage,
            record_size,
            |child| {
                terminal_semantics::reference_storage_layout(module.structural_types.iter(), child)
            },
        )
        .map_err(CodecError::MalformedStructuralFoundation)
    }

    fn validate_authored_destination_intervals(
        destinations: Vec<(u128, u128)>,
    ) -> Result<(), CodecError> {
        terminal_semantics::validate_storage_destination_intervals(destinations)
            .map_err(CodecError::MalformedStructuralFoundation)
    }

    fn validate_field_storage_envelope(shape: &StructuralTypeShape) -> Result<(), CodecError> {
        if matches!(
            shape,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Authored(_)
                    | StructuralRecordStorage::AccessView(_),
                ..
            }
        ) {
            return Ok(());
        }
        let direct_storage = shape
            .direct_fields()
            .is_some_and(|fields| fields.iter().any(|field| field.storage.is_some()));
        let case_storage = match shape {
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .any(|field| field.storage.is_some()),
            _ => false,
        };
        if direct_storage || case_storage {
            return malformed("field storage requires an authored record envelope");
        }
        Ok(())
    }

    pub(crate) fn attachment_storage_types(
        types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        attachments: impl Iterator<Item = StructuralTypeId>,
    ) -> BTreeSet<StructuralTypeId> {
        use terminal_psi::StructuralTypeShape;
        let mut pending: Vec<_> = attachments.collect();
        let mut storage = BTreeSet::new();
        while let Some(identity) = pending.pop() {
            if !storage.insert(identity) {
                continue;
            }
            let Some(declaration) = types.get(&identity) else {
                continue;
            };
            let mut retain_fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
                pending.extend(structural_children(fields));
            };
            match &declaration.shape {
                StructuralTypeShape::Record { fields, .. } => retain_fields(fields),
                StructuralTypeShape::Mixed { fields, cases } => {
                    retain_fields(fields);
                    for case in cases {
                        retain_fields(&case.fields);
                    }
                }
                StructuralTypeShape::Sum { cases } => {
                    for case in cases {
                        retain_fields(&case.fields);
                    }
                }
                StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
                StructuralTypeShape::PrimitiveScalar(_)
                | StructuralTypeShape::AtomicCell(_)
                | StructuralTypeShape::ByteSequence(_)
                | StructuralTypeShape::Reference { .. }
                | StructuralTypeShape::ElementView { .. } => {}
            }
        }
        storage
    }

    #[cfg(test)]
    mod authored_storage_envelope_tests {
        use semantic_vocabulary::{
            AuthoredBitFragment, AuthoredBitFragments, IntegerSign, IntegerType, MachineId,
            StructuralTypeId,
        };
        use terminal_psi::{
            ArrayExtent, StructuralTypeDeclaration, TerminalIdentityIndex, TerminalModule,
        };

        use super::{validate_authored_field_extent, validate_field_storage_envelope};
        use semantic_vocabulary::{
            AuthoredFieldStorage, AuthoredRecordStorage, ScalarType, StructuralCaseId,
            StructuralFieldId,
        };
        use terminal_psi::{
            BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
            StructuralFieldType, StructuralRecordStorage, StructuralTypeShape,
        };

        #[test]
        fn field_geometry_requires_the_record_envelope_in_every_field_roster() {
            let field = StructuralFieldDeclaration {
                inline_carrier: None,
                id: StructuralFieldId::new(1).expect("field identity"),
                identity: TerminalIdentityIndex(291),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                storage: Some(AuthoredFieldStorage::Fragmented(AuthoredBitFragments::new(
                    [AuthoredBitFragment {
                        container_byte_offset: 0,
                        container_width_bits: 8,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 1,
                    }],
                ))),
            };
            let case = StructuralCaseDeclaration {
                id: StructuralCaseId::new(1).expect("case identity"),
                identity: TerminalIdentityIndex(826),
                fields: vec![field.clone()],
            };
            for shape in [
                StructuralTypeShape::Record {
                    fields: vec![field.clone()],
                    storage: StructuralRecordStorage::Packed,
                },
                StructuralTypeShape::Sum {
                    cases: vec![case.clone()],
                },
                StructuralTypeShape::Mixed {
                    fields: vec![field.clone()],
                    cases: Vec::new(),
                },
                StructuralTypeShape::Mixed {
                    fields: Vec::new(),
                    cases: vec![case],
                },
            ] {
                assert!(validate_field_storage_envelope(&shape).is_err());
            }
            let authored = StructuralTypeShape::Record {
                fields: vec![field],
                storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                    byte_size: 1,
                    byte_alignment: 1,
                }),
            };
            assert!(validate_field_storage_envelope(&authored).is_ok());
        }

        #[test]
        fn contiguous_fields_must_fit_the_whole_footprint() {
            let mut module = TerminalModule::for_entry(MachineId::new(1).expect("entry"));
            let scalar_type = StructuralTypeId::new(1).expect("scalar type");
            let integer = ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 64).expect("signed integer"),
            );
            module.structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: scalar_type,
                identity: TerminalIdentityIndex(77),
                shape: StructuralTypeShape::PrimitiveScalar(integer),
            });
            let field = StructuralFieldDeclaration {
                inline_carrier: None,
                id: StructuralFieldId::new(1).expect("field"),
                identity: TerminalIdentityIndex(291),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(scalar_type),
                storage: None,
            };
            for storage in [
                AuthoredFieldStorage::Scalar {
                    byte_offset: 0,
                    stored_scalar: None,
                },
                AuthoredFieldStorage::Direct { byte_offset: 0 },
            ] {
                assert!(
                    extent!(&super::StructuralCatalog::from(&module) => &field => &storage => 1)
                        .is_err()
                );
                assert!(
                    extent!(&super::StructuralCatalog::from(&module) => &field => &storage => 8)
                        .is_ok()
                );
            }
            let offset = AuthoredFieldStorage::Direct {
                byte_offset: u64::MAX - 3,
            };
            assert!(
                extent!(&super::StructuralCatalog::from(&module) => &field => &offset => u64::MAX)
                    .is_err()
            );
        }

        #[test]
        fn repeated_fields_require_aligned_nonoverlapping_bounded_extents() {
            let mut module = TerminalModule::for_entry(MachineId::new(1).expect("entry"));
            let element = StructuralTypeId::new(1).expect("element");
            let array = StructuralTypeId::new(2).expect("array");
            module.structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: element,
                identity: TerminalIdentityIndex(599),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 64).expect("signed integer"),
                )),
            });
            module.structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: array,
                identity: TerminalIdentityIndex(941),
                shape: StructuralTypeShape::FixedArray {
                    element,
                    length: ArrayExtent::Literal(2),
                },
            });
            let field = StructuralFieldDeclaration {
                inline_carrier: None,
                id: StructuralFieldId::new(1).expect("field"),
                identity: TerminalIdentityIndex(535),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(array),
                storage: None,
            };
            let storage = AuthoredFieldStorage::Repeated {
                byte_offset: 0,
                element_stride: 8,
            };
            assert!(
                extent!(&super::StructuralCatalog::from(&module) => &field => &storage => 16)
                    .is_ok()
            );
            assert!(
                extent!(&super::StructuralCatalog::from(&module) => &field => &storage => 15)
                    .is_err()
            );
            for storage in [
                AuthoredFieldStorage::Repeated {
                    byte_offset: 0,
                    element_stride: 1,
                },
                AuthoredFieldStorage::Repeated {
                    byte_offset: 4,
                    element_stride: 8,
                },
            ] {
                assert!(
                    extent!(&super::StructuralCatalog::from(&module) => &field => &storage => 24)
                        .is_err()
                );
            }
            module.structural_types[1].shape = StructuralTypeShape::FixedArray {
                element,
                length: ArrayExtent::Literal(u64::MAX),
            };
            assert!(
                extent!(&super::StructuralCatalog::from(&module) => &field => &storage => u64::MAX)
                    .is_err()
            );
        }
    }
}
pub(crate) mod target_layouts {
    use super::super::wire::strictly_increasing;
    use super::structural_type_foundations::validate_type_shape;
    use crate::codec_error::{CodecError, malformed};
    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeSet;
    use terminal_psi::{
        StructuralRecordStorage, StructuralTypeShape, TargetLayoutFieldLayoutSource,
        TargetLayoutSchemaFieldKind, TerminalIdentityIndex, TerminalModule,
    };
    use terminal_semantics::reference_storage_byte_width;

    pub(crate) fn validate(
        module: &TerminalModule,
        attachments: &BTreeSet<StructuralTypeId>,
    ) -> Result<(), CodecError> {
        if !strictly_increasing(
            module
                .target_layout_applications
                .iter()
                .map(|application| application.id),
        ) {
            return malformed("layout applications are not strictly ordered");
        }
        let mut records = BTreeSet::new();
        for application in &module.target_layout_applications {
            if application.id.0 == 0
                || application.policy_identity == TerminalIdentityIndex::ABSENT
                || application.plan_identity == TerminalIdentityIndex::ABSENT
                || module
                    .identities
                    .text(application.policy_identity)
                    .is_none()
                || module.identities.text(application.plan_identity).is_none()
                || !records.insert(application.record)
            {
                return malformed("layout application has absent or duplicate identity");
            }
            let Some(record) = module.structural_type(application.record) else {
                return malformed("layout application has no record");
            };
            let StructuralTypeShape::Record {
                fields,
                storage: StructuralRecordStorage::TargetLayout(id),
            } = &record.shape
            else {
                return malformed("layout application has no exact pending record marker");
            };
            if *id != application.id {
                return malformed("layout application record marker differs");
            }
            let Some(schema) = module.structural_type(application.schema) else {
                return malformed("layout application has no semantic schema");
            };
            let StructuralTypeShape::Record {
                fields: schema_fields,
                storage,
            } = &schema.shape
            else {
                return malformed("layout schema ABI supports records only");
            };
            if matches!(storage, StructuralRecordStorage::TargetLayout(_))
                || record.owns_linear_claim != schema.owns_linear_claim
                || record.copyable != schema.copyable
            {
                return malformed("layout schema differs from the semantic custody shape");
            }
            let live_fields = fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            let live_schema = schema_fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            if application.target_inputs.len() != application.realizations.len()
                || !strictly_increasing(
                    application
                        .target_inputs
                        .iter()
                        .map(|input| input.target.as_str()),
                )
            {
                return malformed(
                    "layout realizations do not cover the independent complete target input roster",
                );
            }
            let mut targets = BTreeSet::new();
            for (realization, input) in application
                .realizations
                .iter()
                .zip(&application.target_inputs)
            {
                if realization.target != input.target
                    || realization.pointer_size != input.pointer_size
                    || realization.pointer_alignment != input.pointer_alignment
                    || realization.case_tag_size != input.case_tag_size
                    || realization.case_tag_alignment != input.case_tag_alignment
                    || realization.little_endian != input.little_endian
                {
                    return malformed(
                        "layout realization changed its independent issuer target input",
                    );
                }
                if realization.schema_argument.abi_version != 4
                    || realization.schema_argument.fields.len() > 32
                    || live_fields.len() != realization.schema_argument.fields.len()
                    || live_schema.len() != live_fields.len()
                {
                    return malformed("layout schema argument does not cover exact live fields");
                }
                let retired = &realization.schema_argument.retired_field_identities;
                if retired.len() > 32
                    || retired.iter().enumerate().any(|(index, identity)| {
                        retired[..index].contains(identity)
                            || realization
                                .schema_argument
                                .fields
                                .iter()
                                .any(|field| field.identity == Some(*identity))
                    })
                {
                    return malformed(
                        "layout schema argument has conflicting retired field identities",
                    );
                }
                let inputs = &realization.schema_argument.fields;
                let numbered = inputs.iter().any(|field| field.identity.is_some());
                if (numbered
                    && (inputs.iter().any(|field| field.identity.is_none())
                        || !strictly_increasing(inputs.iter().filter_map(|field| field.identity))))
                    || !strictly_increasing(retired.iter().copied())
                {
                    return malformed("layout schema argument is not in canonical identity order");
                }
                let mut keys = BTreeSet::new();
                let mut stable = BTreeSet::new();
                let mut captured = BTreeSet::new();
                for (ordinal, input) in inputs.iter().enumerate() {
                    let Some(position) = live_fields
                        .iter()
                        .position(|field| field.id == input.record_field)
                    else {
                        return malformed("layout schema argument names an absent record field");
                    };
                    let field = live_fields[position];
                    let source = live_schema[position];
                    if !captured.insert(input.record_field)
                        || (!numbered && position != ordinal)
                        || input.schema_field != source.id
                        || field.field_type != source.field_type
                        || field.inline_carrier != source.inline_carrier
                        || input.key == 0
                        || !keys.insert(input.key)
                        || input
                            .identity
                            .is_some_and(|identity| !stable.insert(identity))
                    {
                        return malformed("layout schema argument has stale field correspondence");
                    }
                    let geometry = if let Some(scalar) = source.field_type.scalar_type() {
                        reference_storage_byte_width(scalar).map(|width| {
                            (
                                u64::from(width),
                                u64::from(width),
                                TargetLayoutSchemaFieldKind::Scalar,
                            )
                        })
                    } else if let Some(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                        capacity,
                    }) = source.field_type.byte_sequence_carrier()
                    {
                        Some((capacity, 1, TargetLayoutSchemaFieldKind::Repeated))
                    } else {
                        source
                            .field_type
                            .as_structural()
                            .map(|child| schema_storage_layout(module, child, realization))
                            .transpose()?
                            .map(|(size, align)| {
                                (
                                    size,
                                    align,
                                    if source
                                        .field_type
                                        .as_structural()
                                        .and_then(|id| module.structural_type(id))
                                        .is_some_and(|ty| {
                                            matches!(
                                                ty.shape,
                                                StructuralTypeShape::FixedArray { .. }
                                            )
                                        })
                                    {
                                        TargetLayoutSchemaFieldKind::Repeated
                                    } else {
                                        TargetLayoutSchemaFieldKind::Nested
                                    },
                                )
                            })
                    };
                    if geometry != Some((input.byte_size, input.byte_alignment, input.kind))
                        || input.byte_size == 0
                    {
                        return malformed(
                            "layout schema argument differs from invariant field geometry",
                        );
                    }
                    if schema_layout_source(module, &source.field_type, realization)?
                        != input.layout_source
                    {
                        return malformed(
                            "layout schema argument changed its field geometry source",
                        );
                    }
                }
                if realization.target.is_empty()
                    || !targets.insert(&realization.target)
                    || realization.pointer_size == 0
                    || !realization.pointer_alignment.is_power_of_two()
                    || realization.pointer_size % realization.pointer_alignment != 0
                    || realization.case_tag_size == 0
                    || !realization.case_tag_alignment.is_power_of_two()
                    || realization.case_tag_size % realization.case_tag_alignment != 0
                    || realization.fields.len() != live_fields.len()
                    || realization
                        .fields
                        .iter()
                        .zip(&live_fields)
                        .any(|(stored, field)| stored.field != field.id)
                {
                    return malformed(
                        "fixed representation changed its target input or exact semantic fields",
                    );
                }
                let mut resolved = record.clone();
                let StructuralTypeShape::Record { fields, storage } = &mut resolved.shape else {
                    unreachable!()
                };
                *storage = StructuralRecordStorage::Authored(realization.storage);
                for field in fields
                    .iter_mut()
                    .filter(|field| !field.relevance.is_erased())
                {
                    field.storage = realization
                        .fields
                        .iter()
                        .find(|stored| stored.field == field.id)
                        .map(|stored| stored.storage.clone());
                }
                validate_type_shape(module, &resolved, attachments)?;
            }
        }
        for declaration in &module.structural_types {
            if let StructuralTypeShape::Record {
                storage: StructuralRecordStorage::TargetLayout(application),
                ..
            } = declaration.shape
                && !module
                    .target_layout_applications
                    .iter()
                    .any(|row| row.id == application && row.record == declaration.id)
            {
                return malformed("pending record has no exact layout application");
            }
        }
        Ok(())
    }

    fn schema_storage_layout(
        module: &TerminalModule,
        identity: StructuralTypeId,
        target: &terminal_psi::TargetLayoutRealization,
    ) -> Result<(u64, u64), CodecError> {
        terminal_semantics::ReferenceStorageGeometry::new(
            module.structural_types.iter(),
            |application| {
                Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                    application,
                ))
            },
        )
        .with_target_carriers(Some(semantic_vocabulary::TargetCarrierLayout {
            pointer_byte_size: target.pointer_size,
            pointer_byte_alignment: target.pointer_alignment,
            descriptor_byte_size: target.pointer_size.checked_mul(2).ok_or(
                CodecError::MalformedStructuralFoundation("target descriptor footprint overflows"),
            )?,
            descriptor_byte_alignment: target.pointer_alignment,
            case_tag_byte_size: target.case_tag_size,
            case_tag_byte_alignment: target.case_tag_alignment,
        }))
        .with_record_storage(SchemaStorage { module, target })
        .layout(identity)
        .map_err(|error| {
            CodecError::NativeLayout(format!(
                "schema {identity:?} under target {} has no represented geometry: {error:?}",
                target.target,
            ))
        })
    }

    struct SchemaStorage<'module> {
        module: &'module TerminalModule,
        target: &'module terminal_psi::TargetLayoutRealization,
    }

    impl terminal_semantics::RecordStorageResolver for SchemaStorage<'_> {
        fn realization(
            &self,
            record: StructuralTypeId,
            application: terminal_psi::LayoutApplicationId,
        ) -> Result<&terminal_psi::TargetLayoutRealization, terminal_semantics::StorageGeometryError>
        {
            selected_layout_realization(self.module, record, application, self.target).ok_or(
                terminal_semantics::StorageGeometryError::UnresolvedLayout(record),
            )
        }
    }

    fn schema_layout_source(
        module: &TerminalModule,
        field: &terminal_psi::StructuralFieldType,
        target: &terminal_psi::TargetLayoutRealization,
    ) -> Result<TargetLayoutFieldLayoutSource, CodecError> {
        match terminal_semantics::schema_field_layout_source(
            module,
            field,
            &SchemaStorage { module, target },
        ) {
            Ok(source) => Ok(source),
            Err(terminal_semantics::StorageGeometryError::RecursiveType(_)) => {
                malformed("layout schema geometry source contains an array cycle")
            }
            Err(terminal_semantics::StorageGeometryError::MissingType(_)) => {
                malformed("layout schema geometry source has an absent child")
            }
            Err(terminal_semantics::StorageGeometryError::UnresolvedLayout(_)) => {
                malformed("layout schema explicit child has no matching issuer target")
            }
            Err(error) => Err(CodecError::NativeLayout(format!(
                "layout schema geometry source: {error}"
            ))),
        }
    }

    fn selected_layout_realization<'module>(
        module: &'module TerminalModule,
        record: StructuralTypeId,
        id: terminal_psi::LayoutApplicationId,
        target: &terminal_psi::TargetLayoutRealization,
    ) -> Option<&'module terminal_psi::TargetLayoutRealization> {
        module
            .target_layout_applications
            .iter()
            .find(|application| application.id == id && application.record == record)?
            .realizations
            .iter()
            .find(|row| {
                row.target == target.target
                    && row.pointer_size == target.pointer_size
                    && row.pointer_alignment == target.pointer_alignment
                    && row.case_tag_size == target.case_tag_size
                    && row.case_tag_alignment == target.case_tag_alignment
                    && row.little_endian == target.little_endian
            })
    }
}
pub(crate) mod task_activation {
    use super::structural_type_foundations::{StructuralCatalog, validate_catalog_type_shape};
    use super::validate_structural_type_graph;
    use crate::codec_error::{CodecError, malformed};
    use std::collections::BTreeSet;
    use terminal_psi::*;

    pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
        let bundle = &module.task_activation_demands;
        if bundle.demands.is_empty()
            && bundle.structural_types.is_empty()
            && bundle.structural_referents.is_empty()
            && bundle.target_layout_applications.is_empty()
        {
            return Ok(());
        }
        let identity = |index| {
            if module.identities.text(index).is_none_or(str::is_empty) {
                malformed("task activation has an unresolved identity")
            } else {
                Ok(())
            }
        };
        let catalog = StructuralCatalog {
            structural_types: &bundle.structural_types,
            structural_referents: &bundle.structural_referents,
        };
        let mut type_ids = BTreeSet::new();
        let mut type_names = BTreeSet::new();
        for (id, name) in bundle
            .structural_types
            .iter()
            .map(|row| (row.id, row.identity))
            .chain(
                bundle
                    .structural_referents
                    .iter()
                    .map(|row| (row.id, row.identity)),
            )
        {
            identity(name)?;
            if !type_ids.insert(id) || !type_names.insert(name) {
                return malformed("task activation structural catalog repeats an identity");
            }
        }
        for declaration in catalog.structural_types {
            validate_catalog_type_shape(&catalog, declaration, &BTreeSet::new())?;
        }
        validate_structural_type_graph(catalog.structural_types)?;
        let value = |value_type| match value_type {
            TerminalTaskValueType::Unit | TerminalTaskValueType::Scalar(_) => Ok(()),
            TerminalTaskValueType::Structural(id)
                if bundle.structural_types.iter().any(|row| row.id == id) =>
            {
                Ok(())
            }
            _ => malformed("task activation value has no storage declaration"),
        };
        let resident = |row: &TerminalTaskResidentValue| {
            identity(row.normalized_identity)?;
            value(row.value_type)
        };
        let mut layouts = BTreeSet::new();
        for layout in &bundle.target_layout_applications {
            if !layouts.insert(layout.id)
                || !bundle.structural_types.iter().any(|row| {
                    row.id == layout.record
                        && matches!(row.shape, StructuralTypeShape::Record {
                        storage: StructuralRecordStorage::TargetLayout(id), ..
                    } if id == layout.id)
                })
            {
                return malformed("task activation layout does not bind one exact record");
            }
            let mut targets = BTreeSet::new();
            for row in &layout.realizations {
                if row.target.is_empty()
                    || !targets.insert(&row.target)
                    || !row.storage.byte_alignment.is_power_of_two()
                {
                    return malformed("task activation layout has an invalid target or alignment");
                }
            }
        }
        for declaration in &bundle.structural_types {
            if let StructuralTypeShape::Record {
                storage: StructuralRecordStorage::TargetLayout(id),
                ..
            } = declaration.shape
                && !bundle
                    .target_layout_applications
                    .iter()
                    .any(|layout| layout.id == id && layout.record == declaration.id)
            {
                return malformed("task activation record has no target layout alternatives");
            }
        }
        for demand in &bundle.demands {
            for index in [
                demand.requirement_callable_identity,
                demand.requirement_owner_path,
                demand.authored_requirement_identity,
                demand.machine_callable_identity,
                demand.entry,
            ] {
                identity(index)?;
            }
            if demand.report_fingerprint == 0
                || demand.entry_report_fingerprint == 0
                || demand.contract_report_fingerprint == 0
                || demand.specialization_report_fingerprint == 0
                || demand.specialization_commitment == [0; 32]
            {
                return malformed("task activation has an absent commitment");
            }
            for argument in demand
                .signature_arguments
                .iter()
                .chain([&demand.signature_outcome])
            {
                identity(argument.normalized_identity)?;
                value(argument.value_type)?;
            }
            let frames = demand
                .frames
                .iter()
                .map(|frame| frame.identity)
                .collect::<BTreeSet<_>>();
            if frames.len() != demand.frames.len() || frames.contains(&0) {
                return malformed("task activation frame roster repeats an identity");
            }
            let root = demand
                .frames
                .iter()
                .find(|frame| frame.identity == demand.root_frame)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "task activation root frame is absent",
                ))?;
            if root.machine_callable_identity != demand.machine_callable_identity
                || root.entry != demand.entry
            {
                return malformed("task activation root frame differs from its selected entry");
            }
            for frame in &demand.frames {
                identity(frame.machine_callable_identity)?;
                identity(frame.entry)?;
                let mut hash = semantic_vocabulary::Fnv1a::new();
                hash.byte(0x57);
                hash.string_terminated(
                    module
                        .identities
                        .text(frame.machine_callable_identity)
                        .unwrap_or_default(),
                );
                hash.string_terminated(module.identities.text(frame.entry).unwrap_or_default());
                if frame.identity != hash.finish_or(1) {
                    return malformed(
                        "task activation frame identity differs from its callable and entry",
                    );
                }
                let mut states = BTreeSet::new();
                if frame.states.is_empty() {
                    return malformed("task activation frame has no resident states");
                }
                for state in &frame.states {
                    identity(state.state)?;
                    if !states.insert(state.state) {
                        return malformed("task activation frame repeats a state");
                    }
                    for row in state
                        .resident
                        .iter()
                        .chain(state.crossing_arguments.iter().flatten())
                    {
                        resident(row)?;
                    }
                }
                if !states.contains(&frame.entry) {
                    return malformed("task activation frame omits its entry state");
                }
                for call in &frame.calls {
                    if !frames.contains(&call.callee) {
                        return malformed("task activation call has no callee frame");
                    }
                }
                let mut coordinates = BTreeSet::new();
                for call in &frame.unresolved_calls {
                    if !states.contains(&call.state)
                        || !coordinates.insert((
                            call.state,
                            call.statement_index,
                            call.call_ordinal,
                        ))
                    {
                        return malformed(
                            "task activation unresolved call has no unique state coordinate",
                        );
                    }
                }
            }
            for crossing in &demand.crossings {
                if crossing.identity == 0 {
                    return malformed("task activation has a zero crossing identity");
                }
                let mut places = BTreeSet::new();
                for live in &crossing.live_carry {
                    if live.place == 0
                        || live.value_type == 0
                        || !places.insert(live.place)
                        || live.claims.contains(&0)
                    {
                        return malformed("task activation carry has an invalid live identity");
                    }
                }
            }
        }
        Ok(())
    }
}
pub(crate) mod unoptimized_behavior {

    use crate::codec_error::{CodecError, malformed};
    use semantic_vocabulary::InvocationSite;
    use terminal_psi::{TerminalModule, UnoptimizedBehaviorOperation as Operation};

    pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
        super::super::wire::require_increasing(
            module.unoptimized_behavior.iter().map(|body| body.machine),
            "unoptimized semantic bodies",
        )?;
        for body in &module.unoptimized_behavior {
            let known_machine = |machine| {
                module.machine(machine).is_some()
                    || module.unoptimized_machine_behavior(machine).is_some()
            };
            for operation in &body.operations {
                match operation {
                    Operation::Invocation { site, callee, .. } => {
                        if !known_machine(*callee)
                            || matches!(site, InvocationSite::ExternalCompletion(_))
                        {
                            return malformed(
                                "unoptimized invocation has an unknown callee or wrong site role",
                            );
                        }
                    }
                    Operation::NominalCleanup { callee, .. } if !known_machine(*callee) => {
                        return malformed("unoptimized cleanup has an unknown callee");
                    }
                    Operation::BoundaryInvocation { site, boundary, .. } => {
                        if module.boundary_machine(*boundary).is_none()
                            || matches!(site, InvocationSite::InvokeNoNormal(_))
                        {
                            return malformed(
                                "unoptimized boundary invocation has an unknown declaration or wrong site role",
                            );
                        }
                    }
                    Operation::InstructionService { service, .. }
                        if module.service(*service).is_none() =>
                    {
                        return malformed("unoptimized instruction names an unknown service");
                    }
                    _ => {}
                }
            }
            let Some(machine) = module.machine(body.machine) else {
                continue;
            };
            for current in terminal_psi::UnoptimizedMachineBehavior::capture(machine).operations {
                if !body
                    .operations
                    .iter()
                    .any(|retained| same_contribution(*retained, current))
                {
                    return malformed(
                        "unoptimized behavior omits an executable semantic contribution",
                    );
                }
            }
        }
        Ok(())
    }

    fn same_contribution(left: Operation, right: Operation) -> bool {
        match (left, right) {
            (
                Operation::Invocation { callee: left, .. },
                Operation::Invocation { callee: right, .. },
            )
            | (
                Operation::NominalCleanup { callee: left, .. },
                Operation::NominalCleanup { callee: right, .. },
            ) => left == right,
            (
                Operation::BoundaryInvocation { boundary: left, .. },
                Operation::BoundaryInvocation {
                    boundary: right, ..
                },
            ) => left == right,
            (
                Operation::InstructionService { service: left, .. },
                Operation::InstructionService { service: right, .. },
            ) => left == right,
            (Operation::Crash { cause: left, .. }, Operation::Crash { cause: right, .. }) => {
                left == right
            }
            _ => false,
        }
    }
}
pub(crate) mod value_foundations {

    use super::super::wire::strictly_increasing;
    use crate::codec_error::{CodecError, malformed};
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, Operation, OperationKind, OperationResult,
        RecordFieldInitializer, RecordFieldValue, StructuralAccess, StructuralArgument,
        StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralOperationResult, StructuralPlaceDeclaration, StructuralTypeShape,
        TerminalMachine, TerminalModule, find_structural_case, nonlinear_multiplicity_pair_pattern,
    };
    use terminal_semantics::{
        canonical_static_projection, contains_access_carrier, machine_scalar_constant_origin,
        primitive_scalar_memberships, scalar_array_leaf_declaration, scalar_array_leaf_shape,
        scalar_array_qualifications_hold, scalar_constant_value,
        scalar_field_requires_initialization_evidence, structural_claim_paths_may_overlap,
        zeroed_projected_byte_qualifications_hold, zeroed_structural_shape,
    };

    pub(crate) fn validate_establish_scalar_array(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishScalarArray { elements } = &operation.kind else {
            unreachable!("dispatched validate_establish_scalar_array")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("scalar array establishment has no structural result");
        };
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !result.qualification_establishments.is_empty()
            || !scalar_array_qualifications_hold(
                module.structural_types.iter(),
                module.structural_domains.iter(),
                result.structural_type,
                &result.qualifications,
                elements,
                |element| {
                    scalar_constant_value(element, |value| {
                        machine_scalar_constant_origin(machine, value)
                    })
                },
            )
            || !matches!(
                machine.structural_place(result.place),
                Some(StructuralPlaceDeclaration {
                    kind: StructuralPlaceKind::OperationResult { producer, structural_type }, ..
                }) if *producer == operation.id && *structural_type == result.structural_type
            )
            || scalar_array_leaf_shape(module.structural_types.iter(), result.structural_type)
                .is_none_or(|(_, count)| u64::try_from(elements.len()).ok() != Some(count))
        {
            return malformed("scalar array establishment has an invalid result shape");
        }
        if scalar_array_leaf_declaration(module.structural_types.iter(), result.structural_type)
            .is_some_and(|(leaf, count)| count != 0 && leaf.scalar_restriction.is_some())
        {
            return malformed("scalar array initializer lacks receiving restriction obligations");
        }
        Ok(())
    }

    pub(crate) fn validate_establish_scalar_case(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishScalarCase {
            result_case,
            fields,
        } = &operation.kind
        else {
            unreachable!("dispatched validate_establish_scalar_case")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("scalar case establishment has no structural result");
        };
        if result.multiplicity == StructuralMultiplicity::Linear
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return malformed("scalar case establishment has an invalid result surface");
        }
        if !matches!(
            machine.structural_place(result.place),
            Some(StructuralPlaceDeclaration {
                kind: StructuralPlaceKind::OperationResult { producer, structural_type },
                ..
            }) if *producer == operation.id && *structural_type == result.structural_type
        ) {
            return malformed("scalar case establishment has no matching result place");
        }
        let Some(declaration) = module.structural_type(result.structural_type) else {
            return malformed("scalar case establishment has an unknown structural type");
        };
        let StructuralTypeShape::Sum { cases } = &declaration.shape else {
            return malformed("scalar case establishment requires a sum type");
        };
        let Some(selected) = find_structural_case(cases, *result_case) else {
            return malformed("scalar case establishment requires an exact member");
        };
        if selected.fields.len() != fields.len() {
            return malformed("scalar case establishment requires every selected field");
        }
        for (declaration, field) in selected.fields.iter().zip(fields) {
            if declaration.id != field.field
                || declaration.relevance.is_erased()
                || declaration.field_type.scalar_type().is_none()
                || scalar_field_requires_initialization_evidence(&declaration.field_type)
                    != field.range_obligation.is_some()
            {
                return malformed("scalar case establishment has an invalid field binding");
            }
        }
        Ok(())
    }

    pub(crate) fn validate_establish_structural_case(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishStructuralCase {
            result_case,
            fields,
        } = &operation.kind
        else {
            unreachable!("dispatched validate_establish_structural_case")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("structural case establishment has no structural result");
        };
        validate_constructor_claims(module, result)?;
        if !matches!(
            machine.structural_place(result.place),
            Some(StructuralPlaceDeclaration {
                kind: StructuralPlaceKind::OperationResult { producer, structural_type },
                ..
            }) if *producer == operation.id && *structural_type == result.structural_type
        ) {
            return malformed("structural case establishment has no matching result place");
        }
        let Some(declaration) = module.structural_type(result.structural_type) else {
            return malformed("structural case establishment has an unknown structural type");
        };
        let (common, cases) = match &declaration.shape {
            StructuralTypeShape::Sum { cases } => (&[][..], cases),
            StructuralTypeShape::Mixed { fields, cases }
                if fields.iter().all(|field| {
                    matches!(field.field_type, StructuralFieldType::Erased { .. })
                        || !field.relevance.is_erased()
                }) =>
            {
                (fields.as_slice(), cases)
            }
            _ => return malformed("structural case establishment requires a sum type"),
        };
        let Some(selected) = find_structural_case(cases, *result_case) else {
            return malformed("structural case establishment requires an exact member");
        };
        let roster = common
            .iter()
            .chain(&selected.fields)
            .filter(|field| !matches!(field.field_type, StructuralFieldType::Erased { .. }))
            .collect::<Vec<_>>();
        if roster.len() != fields.len() {
            return malformed("structural case establishment requires every selected field");
        }
        for (declaration, field) in roster.into_iter().zip(fields) {
            if declaration.id != field.field || declaration.relevance.is_erased() {
                return malformed("structural case establishment has an invalid field binding");
            }
            validate_constructor_field(module, machine, field, &declaration.field_type)?;
        }
        Ok(())
    }

    pub(crate) fn validate_establish_byte_sequence_literal(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishByteSequenceLiteral { destination, .. } = &operation.kind
        else {
            unreachable!("dispatched validate_establish_byte_sequence_literal")
        };
        if operation.result != OperationResult::Unit {
            return malformed("byte-sequence literal establishment declares a scalar result");
        }
        let Some(StructuralPlaceDeclaration {
            kind:
                StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                },
            ..
        }) = machine.structural_place(*destination)
        else {
            return malformed("byte-sequence literal establishment has no literal declaration");
        };
        let Some(declaration) = module.structural_type(*structural_type) else {
            return malformed("byte-sequence literal has an unknown structural type");
        };
        if !matches!(
            declaration.shape,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        ) {
            return malformed("byte-sequence literal must use a borrowed byte-sequence type");
        }
        Ok(())
    }

    pub(crate) fn validate_establish_zeroed_structural(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishZeroedStructural {} = &operation.kind else {
            unreachable!("dispatched validate_establish_zeroed_structural")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("zeroed structural establishment has no structural result");
        };
        if !matches!(result.multiplicity, nonlinear_multiplicity_pair_pattern!())
            || !zeroed_projected_byte_qualifications_hold(
                module.structural_types.iter(),
                module.structural_domains.iter(),
                result.structural_type,
                &result.projected_qualifications,
            )
            || !result.claims.is_empty()
            || !result.qualification_establishments.is_empty()
            || !matches!(
                machine.structural_place(result.place),
                Some(StructuralPlaceDeclaration {
                    kind: StructuralPlaceKind::OperationResult { producer, structural_type }, ..
                }) if *producer == operation.id && *structural_type == result.structural_type
            )
            || !zeroed_structural_shape(module.structural_types.iter(), result.structural_type)
        {
            return malformed("zeroed structural establishment has an invalid result shape");
        }
        Ok(())
    }

    pub(crate) fn validate_establish_structural_local(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishStructuralLocal {} = &operation.kind else {
            unreachable!("dispatched validate_establish_structural_local")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("structural local establishment has no structural result");
        };
        terminal_semantics::validate_vacant_structural_local(
            module.structural_types.iter(),
            result,
        )
        .map_err(CodecError::MalformedStructuralFoundation)?;
        if !matches!(
            machine.structural_place(result.place),
            Some(StructuralPlaceDeclaration {
                kind: StructuralPlaceKind::OperationResult { producer, structural_type }, ..
            }) if *producer == operation.id && *structural_type == result.structural_type
        ) {
            return malformed("structural local establishment has an invalid result shape");
        }
        Ok(())
    }

    pub(crate) fn validate_establish_trivial_affine_local(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishTrivialAffineLocal { destination } = &operation.kind else {
            unreachable!("dispatched validate_establish_trivial_affine_local")
        };
        if operation.result != OperationResult::Unit {
            return malformed("trivial affine local establishment declares a scalar result");
        }
        let Some(StructuralPlaceDeclaration {
            kind:
                StructuralPlaceKind::TrivialAffineLocal {
                    structural_type, ..
                },
            ..
        }) = machine.structural_place(*destination)
        else {
            return malformed("trivial affine local establishment has no local declaration");
        };
        if contains_access_carrier(module, *structural_type) {
            return malformed("ordinary local construction cannot establish placed custody");
        }
        let Some(declaration) = module.structural_type(*structural_type) else {
            return malformed("trivial affine local has an unknown structural type");
        };
        if !matches!(
            &declaration.shape,
            StructuralTypeShape::Record { fields, .. } if fields.is_empty()
        ) {
            return malformed("trivial affine local must have an empty record type");
        }
        Ok(())
    }

    pub(crate) fn validate_establish_record(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishRecord { fields } = &operation.kind else {
            unreachable!("dispatched validate_establish_record")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("record has no structural result");
        };
        validate_constructor_claims(module, result)?;
        if !machine.structural_places.iter().any(|place| place.id == result.place && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type } if producer == operation.id && structural_type == result.structural_type)) {
            return malformed("record result custody is noncanonical");
        }
        let Some(declaration) = module.structural_type(result.structural_type) else {
            return malformed("record type is absent");
        };
        let StructuralTypeShape::Record {
            fields: declarations,
            ..
        } = &declaration.shape
        else {
            return malformed("record result is not a record");
        };
        let declarations = declarations
            .iter()
            .filter(|field| !field.relevance.is_erased());
        if fields.len() != declarations.clone().count() {
            return malformed("record field roster differs");
        }
        for (field, declaration) in fields.iter().zip(declarations) {
            if field.field != declaration.id || declaration.relevance != BindingRelevance::Relevant
            {
                return malformed("record field identity differs");
            }
            validate_constructor_field(module, machine, field, &declaration.field_type)?;
        }
        Ok(())
    }

    fn validate_constructor_field(
        module: &TerminalModule,
        machine: &TerminalMachine,
        field: &RecordFieldInitializer,
        field_type: &StructuralFieldType,
    ) -> Result<(), CodecError> {
        match (&field.value, field_type) {
            (
                RecordFieldValue::Scalar {
                    range_obligation, ..
                },
                field_type,
            ) if field_type.scalar_type().is_some() => {
                if range_obligation.is_some()
                    != scalar_field_requires_initialization_evidence(field_type)
                {
                    return malformed("constructor field range obligation differs");
                }
            }
            (RecordFieldValue::Structural(argument), field_type)
                if (matches!(field_type, StructuralFieldType::Structural(_))
                    || matches!(
                        field_type.byte_sequence_carrier(),
                        Some(
                            ByteSequenceCarrier::BoundedOwned { .. }
                                | ByteSequenceCarrier::BorrowedView { .. }
                        )
                    ))
                    && argument.access == StructuralAccess::Owned =>
            {
                validate_constructor_projection(module, machine, argument, field_type)?;
            }
            _ => return malformed("constructor field operand differs"),
        }
        Ok(())
    }

    fn validate_constructor_claims(
        module: &TerminalModule,
        result: &StructuralOperationResult,
    ) -> Result<(), CodecError> {
        if !strictly_increasing(result.claims.iter().map(|binding| binding.claim)) {
            return malformed("aggregate claim bindings disagree with canonical order");
        }
        for claim in &result.claims {
            super::validate_structural_path(module, result.structural_type, &claim.path)?;
            canonical_static_projection(
                module.structural_types.iter(),
                result.structural_type,
                &claim.path,
            )
            .ok_or(CodecError::MalformedStructuralFoundation(
                "aggregate claim path is not static owned storage",
            ))?;
        }
        for (position, claim) in result.claims.iter().enumerate() {
            for other in &result.claims[position + 1..] {
                let overlap = structural_claim_paths_may_overlap(
                    module.structural_types.iter(),
                    result.structural_type,
                    &claim.path,
                    &other.path,
                )
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "aggregate claim path is not static owned storage",
                ))?;
                if overlap {
                    return malformed("aggregate claim bindings overlap");
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_establish_primitive_local(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishPrimitiveLocal {
            value,
            range_obligation,
        } = &operation.kind
        else {
            unreachable!("dispatched validate_establish_primitive_local")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("primitive local has no structural result");
        };
        let source = machine
            .parameters
            .iter()
            .chain(machine.block_parameters())
            .chain(
                machine
                    .operations()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
            .find(|declaration| declaration.id == *value)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "primitive initializer lost its exact SSA declaration",
            ))?;
        let qualifications = primitive_scalar_memberships(
            module.structural_types.iter(),
            module.structural_domains.iter(),
            &module.scalar_qualifications,
            result.structural_type,
            source,
        )
        .ok_or(CodecError::MalformedStructuralFoundation(
            "primitive initializer lost its exact scalar membership correspondence",
        ))?;
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.claims.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || result.qualifications != qualifications
            || !machine.structural_places.iter().any(|place| {
                place.id == result.place
                    && matches!(place.kind,
                    StructuralPlaceKind::OperationResult { producer, structural_type }
                        if producer == operation.id && structural_type == result.structural_type)
            })
            || !module.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && declaration.scalar_restriction.is_some() == range_obligation.is_some()
                    && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(carrier)
                        if super::declared_scalar_type(machine, *value) == Some(carrier))
            })
        {
            return malformed("primitive local result custody is noncanonical");
        }
        Ok(())
    }

    pub(crate) fn validate_establish_byte_sequence_concatenation(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let Some(result) = operation.result.structural() else {
            return malformed("byte concatenation requires an owned structural result");
        };
        if !matches!(machine.structural_place(result.place).map(|row| row.kind),
            Some(StructuralPlaceKind::OperationResult { producer, structural_type })
                if producer == operation.id && structural_type == result.structural_type)
            || !module.structural_types.iter().any(|row| {
                row.id == result.structural_type && row.shape.is_bounded_owned_byte_sequence()
            })
        {
            return malformed(
                "byte concatenation result differs from its owned carrier declaration",
            );
        }
        Ok(())
    }

    fn validate_constructor_projection(
        module: &TerminalModule,
        machine: &TerminalMachine,
        argument: &StructuralArgument,
        field_type: &StructuralFieldType,
    ) -> Result<(), CodecError> {
        if argument.path.is_empty() {
            return Ok(());
        }
        let Some(expected) = field_type.as_structural() else {
            return malformed("projected constructor operand requires a structural field");
        };
        let Some(root) = super::structural_place_type(machine, argument.place) else {
            return malformed("projected constructor operand has no declared root");
        };
        let Some((_, selected)) =
            canonical_static_projection(module.structural_types.iter(), root, &argument.path)
        else {
            return malformed("projected constructor operand requires a finite owned path");
        };
        if selected.id != expected {
            return malformed("projected constructor operand differs from its field carrier");
        }
        Ok(())
    }

    pub(crate) fn validate_establish_structural_array(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishStructuralArray { elements } = &operation.kind else {
            unreachable!("array dispatch")
        };
        let Some(result) = operation.result.structural() else {
            return malformed("array has no structural result");
        };
        validate_constructor_claims(module, result)?;
        if !result.qualifications.is_empty()
            || !machine.structural_places.iter().any(|place| place.id == result.place && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type } if producer == operation.id && structural_type == result.structural_type)) {
            return malformed("array result custody is noncanonical");
        }
        let Some(StructuralTypeShape::FixedArray { element, length }) = module
            .structural_type(result.structural_type)
            .map(|row| &row.shape)
        else {
            return malformed("structural array result is not a fixed array");
        };
        let Some(length) = length.literal() else {
            return malformed("array construction requires a closed element count");
        };
        if u64::try_from(elements.len()).ok() != Some(length) {
            return malformed("array element count differs");
        }
        for argument in elements {
            if argument.access != StructuralAccess::Owned || argument.place == result.place {
                return malformed("array element is not an independent owned source");
            }
            let Some(root) = super::structural_place_type(machine, argument.place) else {
                return malformed("array element source is absent");
            };
            if super::validate_structural_path(module, root, &argument.path)? != *element {
                return malformed("array element type differs");
            }
            validate_constructor_projection(
                module,
                machine,
                argument,
                &StructuralFieldType::Structural(*element),
            )?;
        }
        Ok(())
    }

    pub(crate) fn validate_native_code_layout(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<(), CodecError> {
        let OperationKind::EstablishNativeCodeLayout { machine: selected } = operation.kind else {
            unreachable!()
        };
        let Some(result) = operation.result.structural() else {
            return malformed("native code layout has no structural result");
        };
        if module.machine(selected).is_none()
            || result.multiplicity != StructuralMultiplicity::Affine
            || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
        {
            return malformed(
                "native code layout has unavailable executable entry or noncanonical custody",
            );
        }
        if !machine.structural_places.iter().any(|place| place.id == result.place && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type } if producer == operation.id && structural_type == result.structural_type)) { return malformed("native code layout result has no exact producer home"); }
        let Some(declaration) = module.structural_type(result.structural_type) else {
            return malformed("native code layout result type is absent");
        };
        let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            return malformed("native code layout result is not a record");
        };
        let address = |field: &StructuralFieldDeclaration| matches!(field.field_type, StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(ty)) if ty.is_address());
        if fields.len() != 2
            || fields.iter().any(|field| field.relevance.is_erased())
            || !address(&fields[0])
        {
            return malformed("native code layout needs an entry address and region sequence");
        }
        let StructuralFieldType::Structural(view) = fields[1].field_type else {
            return malformed("native code layout region sequence is absent");
        };
        let Some(StructuralTypeShape::ElementView { element, access }) =
            module.structural_type(view).map(|ty| &ty.shape)
        else {
            return malformed("native code layout regions are not a finite element view");
        };
        if access.is_some_and(|access| access != StructuralAccess::SharedBorrow) {
            return malformed("native code layout regions cannot grant mutable authority");
        }
        let Some(StructuralTypeShape::Record {
            fields: regions, ..
        }) = module.structural_type(*element).map(|ty| &ty.shape)
        else {
            return malformed("native code layout region type is absent");
        };
        if regions.len() != 2
            || regions.iter().any(|field| field.relevance.is_erased())
            || !address(&regions[0])
            || !matches!(regions[1].field_type, StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(ty)) if !ty.is_address() && ty.sign() == semantic_vocabulary::IntegerSign::Unsigned && ty.bits() == 64)
        {
            return malformed("native code layout region needs base addr and length u64");
        }
        Ok(())
    }
}

use super::wire::strictly_increasing;
use crate::codec_error::{CodecError, malformed};
use terminal_psi::TerminalIdentityIndex;

use self::call_foundations::{
    validate_boundary_call, validate_call_dynamic, validate_call_machine, validate_call_structural,
    validate_call_structural_scalar, validate_call_unit, validate_port_write,
    validate_register_access,
};
use semantic_vocabulary::{
    BlockId, ClaimId, IntegerCarrier, IntegerSign, PlaceId, ScalarDomainId,
    ScalarQualificationSetId, ScalarType, ServiceId, StructuralDomainId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use std::collections::BTreeSet;
use terminal_psi::types::atomic_once_outcome_cases;
use terminal_psi::{
    AtomicAccessEvent, BoundaryDeclarationRole, ByteSequenceCarrier, Operation, OperationKind,
    OperationResult, ReferenceOriginProjection, ScalarDomainDeclaration, StructuralAccess,
    StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
    TerminalRootServiceReach, Terminator, ValueDeclaration, call_structural_pair_pattern,
    field_by_identity, find_structural_case, find_structural_type, record_or_mixed_shape_pattern,
    structural_leaf_copy_pair_pattern,
};
use terminal_semantics::descriptor_results::terminal_payload_contract;
use terminal_semantics::{
    access_view_input_parameter, borrowed_primitive_projection_declaration,
    borrowed_referent_matches, boundary_buffer_capacity, bounded_byte_owner_capacity,
    byte_owned_result_predicates, byte_parameter_predicates, byte_sequence_source_predicates,
    contains_access_carrier, fixed_byte_array_loan, fixed_element_array_extent,
    primitive_projection_declaration, pure_byte_qualifications, qualification_path_carrier,
    reference_origin_type, representation_view_declaration, runtime_structural_path_tip,
    scalar_array_leaf_shape, shared_boundary_buffer_capacity, stored_view_field_presentation,
    structural_memberships_disjoint_from_write, validate_access_lifecycle_plans,
    validate_access_view_plans, validate_dynamic_reference_regions,
    validate_runtime_evidence_inputs,
};

pub(crate) fn declared_scalar_type(
    machine: &TerminalMachine,
    value: ValueId,
) -> Option<ScalarType> {
    machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(machine.block_parameters())
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|candidate| candidate.result.scalar_ref())
        }))
        .find(|declaration| declaration.id == value)
        .map(|declaration| declaration.scalar_type)
}

pub(crate) fn qualification_domains(
    module: &TerminalModule,
    set: ScalarQualificationSetId,
) -> Option<&[ScalarDomainId]> {
    if set.is_empty() {
        return Some(&[]);
    }
    module
        .scalar_qualifications
        .sets
        .iter()
        .find(|row| row.id == set)
        .map(|row| row.domains.as_slice())
}

pub(crate) fn scalar_domain(
    module: &TerminalModule,
    id: ScalarDomainId,
) -> Option<&ScalarDomainDeclaration> {
    module
        .scalar_qualifications
        .domains
        .iter()
        .find(|domain| domain.id == id)
}

pub(crate) fn values(machine: &TerminalMachine) -> impl Iterator<Item = &ValueDeclaration> {
    machine
        .parameters
        .iter()
        .chain(machine.block_parameters())
        .chain(
            machine
                .operations()
                .filter_map(|operation| operation.result.scalar_ref()),
        )
}

fn validate_callable_scalar_types(module: &TerminalModule) -> Result<(), CodecError> {
    use terminal_psi::{BoundaryMachineResult, StructuralTypeShape};
    let check = |scalar| {
        module
            .callables
            .validate_scalar_type(scalar)
            .map_err(CodecError::MalformedStructuralFoundation)
    };
    let boundary_result = |result: &BoundaryMachineResult| {
        if let BoundaryMachineResult::Scalar(result) = result {
            check(result.scalar_type)?;
        }
        Ok::<(), CodecError>(())
    };
    let field = |field: &terminal_psi::StructuralFieldDeclaration| {
        if let Some(scalar) = field
            .field_type
            .scalar_type()
            .or_else(|| field.field_type.atomic_scalar_type())
        {
            check(scalar)?;
        }
        if let Some(stored) = field
            .storage
            .as_ref()
            .and_then(|storage| storage.stored_scalar())
        {
            check(stored)?;
        }
        Ok::<(), CodecError>(())
    };
    for declaration in &module.structural_types {
        if let Some(restriction) = &declaration.scalar_restriction
            && let Some(scalar) = restriction
                .scalar_type()
                .or_else(|| restriction.atomic_scalar_type())
        {
            check(scalar)?;
        }
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => check(*scalar)?,
            StructuralTypeShape::Record { fields, .. } => {
                for value in fields {
                    field(value)?;
                }
            }
            StructuralTypeShape::Sum { cases } => {
                for value in cases.iter().flat_map(|case| &case.fields) {
                    field(value)?;
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                for value in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    field(value)?;
                }
            }
            _ => {}
        }
    }
    for machine in &module.machines {
        for value in values(machine)
            .chain(machine.result.scalar_ref())
            .chain(&machine.contract.erased_scalar_formals)
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.erased_scalar_formals),
            )
        {
            check(value.scalar_type)?;
        }
    }
    for boundary in &module.boundary_machines {
        for scalar in &boundary.scalar_parameters {
            check(*scalar)?;
        }
        boundary_result(&boundary.result)?;
    }
    for slot in module
        .dynamic_dispatch
        .interfaces
        .iter()
        .flat_map(|interface| &interface.slots)
    {
        for scalar in &slot.scalar_parameters {
            check(*scalar)?;
        }
        boundary_result(&slot.result)?;
    }
    for (ordinal, contract) in module.callables.contracts.iter().enumerate() {
        if usize::try_from(contract.id.get() - 1).ok() != Some(ordinal)
            || module
                .identities
                .text(contract.identity)
                .is_none_or(str::is_empty)
        {
            return malformed(
                "callable contract catalog has a stale coordinate or missing identity",
            );
        }
        for value in contract
            .scalar_parameters
            .iter()
            .chain(&contract.proof.erased_scalar_formals)
            .chain(contract.proof_result.scalar_ref())
        {
            check(value.scalar_type)?;
        }
        boundary_result(&contract.result)?;
    }
    for (ordinal, row) in module.callables.machine_envelopes.iter().enumerate() {
        if let Some(row) = row
            && (usize::try_from(row.machine.get() - 1).ok() != Some(ordinal)
                || module
                    .machine(row.machine)
                    .is_none_or(|machine| machine.declaration.is_none()))
        {
            return malformed("named callable envelope has no exact checked machine declaration");
        }
    }
    let boundaries: std::collections::BTreeMap<_, _> = module
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary))
        .collect();
    for (ordinal, row) in module.callables.boundary_envelopes.iter().enumerate() {
        let Some(row) = row else {
            continue;
        };
        if usize::try_from(row.boundary.get() - 1).ok() != Some(ordinal) {
            return malformed("boundary callable envelope has a stale catalog coordinate");
        }
        let Some(boundary) = boundaries.get(&row.boundary) else {
            return malformed("boundary callable envelope has no exact boundary declaration");
        };
        let Some(declaration) = boundary.declaration.as_ref() else {
            return malformed("boundary callable envelope has no checked source declaration");
        };
        let entries: Vec<_> = row
            .envelope
            .invocations
            .iter()
            .filter_map(|entry| match entry {
                terminal_psi::TerminalCallableInvocation::Service(service) => Some(*service),
                terminal_psi::TerminalCallableInvocation::Parameter(_) => None,
            })
            .collect();
        let unconditional_termination = matches!(
            &row.envelope.termination,
            terminal_psi::TerminalCallableTermination::Terminates { premises } if premises.is_empty()
        );
        if row.envelope.service_reach != boundary.published_service_ceiling
            || entries != boundary.published_synchronous_entries
            || row.envelope.suspends != declaration.source.published_may_suspend
            || row.envelope.blocks != declaration.source.published_may_block
            || unconditional_termination != boundary.unconditional_termination
        {
            return malformed("boundary callable envelope differs from its checked publication");
        }
    }
    for boundary in boundaries.values() {
        if boundary.declaration.is_some()
            && module.callables.boundary_envelope(boundary.id).is_none()
        {
            return malformed("checked boundary declaration has no complete callable envelope");
        }
    }
    for domain in &module.scalar_qualifications.domains {
        check(domain.carrier)?;
    }
    for application in module.scalar_applications.declarations() {
        if let Some(scalar) = application.scalar_type() {
            check(scalar)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_structural_foundation(module: &TerminalModule) -> Result<(), CodecError> {
    validate_callable_scalar_types(module)?;
    terminal_semantics::calls::callable_refinement::validate_descriptor_refinement_evidence(module)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    terminal_semantics::callable_envelopes::validate_callable_operational_refinements(module)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    scalar_applications::validate(module)?;
    task_activation::validate(module)?;
    module
        .folded_constants
        .validate(module)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    scalar_qualifications::validate(module)?;
    scalar_boundary_contracts::validate_declarations(module)?;
    scalar_fields::validate(module)?;
    terminal_semantics::quotients::validate_quotient_catalog(module)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    terminal_semantics::closed_reach::validate_closed_reach_applications(module)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    terminal_semantics::closed_conformances::validate_closed_conformances(module)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    let mut placed_coordinates = BTreeSet::new();
    for row in &module.access_view_inputs {
        if !placed_coordinates.insert((
            row.machine,
            row.source_state_identity,
            row.runtime_position,
        )) {
            return malformed("access-view inputs repeat one structural formal");
        }
        access_view_input_parameter(module, row).map_err(CodecError::AccessViewInput)?;
    }
    validate_access_view_plans(module).map_err(CodecError::AccessViewPlan)?;
    validate_access_lifecycle_plans(module).map_err(CodecError::MalformedStructuralFoundation)?;
    validate_target_families(module)?;
    validate_boundary_application_operations(module)?;
    validate_callback_registrars(module)?;
    installed_interrupts::validate(module)?;
    require_unique_nonempty_identities(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.identity)
            .chain(
                module
                    .structural_referents
                    .iter()
                    .map(|referent| referent.identity),
            ),
        "structural type identity",
    )?;
    let mut type_ids = BTreeSet::new();
    for id in module
        .structural_types
        .iter()
        .map(|declaration| declaration.id)
        .chain(
            module
                .structural_referents
                .iter()
                .map(|referent| referent.id),
        )
    {
        if !type_ids.insert(id) {
            return malformed("structural identity and storage catalogs overlap");
        }
    }
    for referent in &module.structural_referents {
        if !module.structural_types.iter().any(|declaration| {
            matches!(
                declaration.shape, StructuralTypeShape::Reference { referent: target, .. }
                    if target == referent.id
            )
        }) {
            return malformed("identity-only structural type is not a reference target");
        }
    }
    require_unique_nonempty_identities(
        module
            .structural_domains
            .iter()
            .map(|declaration| declaration.identity),
        "structural domain identity",
    )?;
    require_unique_nonempty_identities(
        module
            .services
            .iter()
            .map(|declaration| declaration.identity),
        "service identity",
    )?;
    require_unique_nonempty_identities(
        module
            .boundary_machines
            .iter()
            .map(|declaration| declaration.identity),
        "boundary machine identity",
    )?;

    let types = module
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let attachment_storage = structural_type_foundations::attachment_storage_types(
        &types,
        module
            .machines
            .iter()
            .filter_map(|machine| machine.attachment),
    );
    target_layouts::validate(module, &attachment_storage)?;
    native_layouts::validate(module)?;
    for declaration in &module.structural_types {
        structural_type_foundations::validate_type_shape(module, declaration, &attachment_storage)?;
    }
    validate_structural_type_graph(&module.structural_types)?;
    for interface in &module.dynamic_dispatch.interfaces {
        for slot in &interface.slots {
            validate_dynamic_reference_regions(module.structural_types.iter(), slot)
                .map_err(CodecError::MalformedStructuralFoundation)?;
        }
    }
    declaration_foundations::validate_domains(module)?;
    declaration_foundations::validate_services(module)?;
    if !strictly_increasing(module.external_providers.iter()) {
        return malformed("external provider conformances must be canonical and unique");
    }
    for candidate in &module.external_providers {
        candidate
            .validate(&module.identities)
            .map_err(CodecError::MalformedStructuralFoundation)?;
        for boundary in module
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.identity == candidate.requirement_identity)
        {
            if boundary.numeric_intrinsic != candidate.numeric_intrinsic {
                return malformed("external provider disagrees with boundary numeric meaning");
            }
        }
    }
    unoptimized_behavior::validate(module)?;
    declaration_foundations::validate_calling_signatures(module)?;
    for boundary in &module.boundary_machines {
        declaration_foundations::validate_boundary_machine(module, boundary)?;
    }
    for candidate in &module.provider_candidates {
        declaration_foundations::validate_provider_candidate(module, candidate)?;
    }
    for row in &module.provider_instance_requirements {
        if std::iter::once(row.template)
            .chain(
                row.required_instances
                    .iter()
                    .map(|instance| instance.requirement_identity),
            )
            .any(|identity| module.identities.text(identity).is_none_or(str::is_empty))
        {
            return malformed(
                "provider instance requirement has an empty or unknown callable identity",
            );
        }
        if let Some(owner) = &row.selecting_declaration {
            if module
                .identities
                .text(owner.identity())
                .is_none_or(str::is_empty)
                || module
                    .identities
                    .text(row.source_candidate_identity)
                    .is_none_or(str::is_empty)
            {
                return malformed(
                    "provider instance obligation lost its choosing declaration or realization template",
                );
            }
        } else if !row.source_candidate_identity.is_absent() {
            return malformed(
                "requirement-only provider obligation has a realization template without a choosing owner",
            );
        }
        for instance in &row.required_instances {
            if row.selecting_declaration.is_some() {
                if module
                    .identities
                    .text(instance.candidate_identity)
                    .is_none_or(str::is_empty)
                {
                    return malformed(
                        "checked provider obligation lost its closed realization identity",
                    );
                }
            } else if !instance.candidate_identity.is_absent() {
                return malformed(
                    "requirement-only provider obligation carries a closed realization",
                );
            }
            if (!instance.receiving_application.is_absent()
                && (module
                    .identities
                    .text(instance.declaration_application)
                    .is_none_or(str::is_empty)
                    || module
                        .identities
                        .text(instance.receiving_application)
                        .is_none_or(str::is_empty)))
                || instance
                    .receiving_region_arguments
                    .iter()
                    .any(|identity| module.identities.text(*identity).is_none_or(str::is_empty))
                || (instance.receiving_application.is_absent()
                    && (!instance.declaration_application.is_absent()
                        || !instance.receiving_region_arguments.is_empty()))
            {
                return malformed(
                    "provider instance obligation has an invalid receiving application or region tuple",
                );
            }
        }
    }
    let mut transfers = storage_foundations::StructuralTransferInputs::new(module);
    for machine in &module.machines {
        machine_foundations::validate_machine(module, machine, &mut transfers)?;
    }
    terminal_semantics::activation_storage::validate_module(module)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    storage_foundations::validate_reference_memberships(module)?;
    validate_runtime_evidence_inputs(module).map_err(CodecError::MalformedStructuralFoundation)?;
    Ok(())
}

fn validate_boundary_application_operations(module: &TerminalModule) -> Result<(), CodecError> {
    let demands = &module.boundary_application_demands;
    if demands.is_empty() {
        return Ok(());
    }
    let mut found = vec![false; demands.len()];
    for operation in module.operations() {
        if let Ok(ordinal) =
            demands.binary_search_by_key(&operation.id, |demand| demand.terminal_operation())
        {
            if found[ordinal] {
                return malformed("boundary application names a repeated operation");
            }
            found[ordinal] = true;
        }
    }
    if found.iter().any(|found| !found) {
        return malformed("boundary application names an absent operation");
    }
    Ok(())
}

fn validate_callback_registrars(module: &TerminalModule) -> Result<(), CodecError> {
    let mut registrars = module
        .callback_applications
        .iter()
        .chain(
            module
                .installed_interrupt_registrations
                .iter()
                .flat_map(|registration| &registration.callbacks),
        )
        .map(|application| application.registrar)
        .collect::<BTreeSet<_>>();
    if registrars.is_empty() {
        return Ok(());
    }
    for operation in module.operations() {
        if matches!(operation.kind, OperationKind::BoundaryCall { .. }) {
            registrars.remove(&operation.id);
        }
    }
    if !registrars.is_empty() {
        return malformed("callback registrar is not a boundary call");
    }
    Ok(())
}

fn validate_target_families(module: &TerminalModule) -> Result<(), CodecError> {
    use terminal_psi::{TargetMachineCallable, TargetMachineSupply};

    match module.entry {
        None if module.root_service_reach == TerminalRootServiceReach::default() => {}
        Some(TargetMachineCallable::Machine(machine)) if module.machine(machine).is_some() => {}
        Some(head @ TargetMachineCallable::Boundary(boundary))
            if module.target_machine_families.iter().any(|family| {
                family.canonical == head
                    && !family.members.iter().any(|member| member.callable == head)
            }) && module
                .boundary_machine(boundary)
                .is_some_and(|declaration| {
                    declaration.kind == semantic_vocabulary::BoundaryRequirementKind::Machine
                        && declaration.role == BoundaryDeclarationRole::Callable
                }) => {}
        _ => return malformed("portable entry has no retained machine or target-family head"),
    }
    let mut owners = BTreeSet::new();
    for family in &module.target_machine_families {
        if family.head_contract == [0; 32] {
            return malformed("target-family head has no checked contract commitment");
        }
        if family.members.is_empty() {
            return malformed("target family has no members");
        }
        if !family
            .members
            .iter()
            .any(|member| member.callable == family.canonical)
        {
            let TargetMachineCallable::Boundary(head) = family.canonical else {
                return malformed("independent target-family head is not a machine contract");
            };
            if module.boundary_machine(head).is_none_or(|declaration| {
                declaration.kind != semantic_vocabulary::BoundaryRequirementKind::Machine
                    || declaration.role != BoundaryDeclarationRole::Callable
            }) {
                return malformed("independent target-family head has no machine declaration");
            }
        }
        for member in &family.members {
            if member.target.is_empty() || !owners.insert(member.callable) {
                return malformed("target-family member has an empty tag or repeated callable");
            }
            if member.callable != family.canonical
                && module
                    .target_machine_families
                    .binary_search_by_key(&member.callable, |family| family.canonical)
                    .is_ok()
            {
                return malformed("target-family head is a member of another family");
            }
            match member.callable {
                TargetMachineCallable::Machine(machine) => {
                    if !matches!(
                        member.supply,
                        TargetMachineSupply::CheckedBody | TargetMachineSupply::Boundary
                    ) || !module.machine(machine).is_some()
                    {
                        return malformed(
                            "target-family checked member has no matching body supply",
                        );
                    }
                }
                TargetMachineCallable::Boundary(boundary) => {
                    if member.supply == TargetMachineSupply::CheckedBody
                        || module.boundary_machine(boundary).is_none_or(|declaration| {
                            declaration.role != BoundaryDeclarationRole::Callable
                        })
                    {
                        return malformed(
                            "target-family opaque member has no matching declaration supply",
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_provider_attachment_foundation(
    _module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    if machine
        .structural_places
        .iter()
        .any(|place| matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. }))
    {
        return malformed("provider-backed attachment specialization is incomplete");
    }
    Ok(())
}

pub(crate) fn validate_structural_path(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, CodecError> {
    if path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::RepresentationView { .. }))
    {
        return malformed("returned-origin interpretation is not an operand or owned path");
    }
    validate_reference_origin_path(module, structural_type, path)
}

pub(crate) fn validate_reference_origin_family(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    path: &[ReferenceOriginProjection],
) -> Result<StructuralTypeId, CodecError> {
    reference_origin_type(module.structural_types.iter(), structural_type, path).ok_or(
        CodecError::MalformedStructuralFoundation(
            "reference origin has an invalid typed projection",
        ),
    )
}

fn validate_reference_origin_path(
    module: &TerminalModule,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, CodecError> {
    if let Some(endpoint) =
        runtime_structural_path_tip(module.structural_types.iter(), structural_type, path)
    {
        return Ok(endpoint.id);
    }
    if let Some(endpoint) =
        primitive_projection_declaration(module.structural_types.iter(), structural_type, path)
    {
        return Ok(endpoint);
    }
    let mut payload_fields = None;
    for segment in path {
        let Some(declaration) = module.structural_type(structural_type) else {
            return malformed("structural path has an unknown structural type");
        };
        if let StructuralPathSegment::Case(case) = segment {
            if payload_fields.is_some() {
                return malformed("structural case path requires its payload field");
            }
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return malformed("structural case path requires a sum");
            };
            let Some(selected) = find_structural_case(cases, *case) else {
                return malformed("structural path has an unknown case");
            };
            payload_fields = Some(selected.fields.as_slice());
            continue;
        }
        if payload_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return malformed("structural case path requires its payload field");
        }
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::AllElements, _) => {
                return malformed("universal qualification is not an executable storage path");
            }
            (
                StructuralPathSegment::RepresentationView {
                    structural_type: view,
                    stored_scalar,
                },
                shape,
            ) => {
                if matches!(
                    shape,
                    StructuralTypeShape::Reference { .. } | StructuralTypeShape::AtomicCell(_)
                ) {
                    return malformed(
                        "storage interpretation cannot replace descriptor or atomic storage authority",
                    );
                }
                if let Some(scalar) = stored_scalar {
                    module
                        .callables
                        .validate_scalar_type(*scalar)
                        .map_err(CodecError::MalformedStructuralFoundation)?;
                }
                let Some(view) = representation_view_declaration(
                    module.structural_types.iter(),
                    *view,
                    *stored_scalar,
                ) else {
                    return malformed(
                        "storage interpretation has an unknown or incompatible declaration",
                    );
                };
                view.id
            }
            (StructuralPathSegment::Case(_), _) => {
                return malformed("structural case path requires its payload field");
            }
            (StructuralPathSegment::FixedRange { .. }, _) => {
                return malformed("fixed byte ranges require call-scoped byte-view custody");
            }
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
            (StructuralPathSegment::Referent, _) => structural_type,
            (_, StructuralTypeShape::Reference { .. }) => {
                return malformed("referent projection requires a reference carrier");
            }
            (StructuralPathSegment::Field(identity), shape) => {
                let fields = match payload_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        record_or_mixed_shape_pattern!(fields) => fields.as_slice(),
                        _ => {
                            return malformed(
                                "structural field path requires a record or case payload",
                            );
                        }
                    },
                };
                if identity.is_absent() {
                    return malformed("structural path field identity cannot be empty");
                }
                let Some(field) = field_by_identity(fields, *identity) else {
                    return malformed("structural path has an unknown structural field");
                };
                if field.relevance.is_erased() {
                    return malformed("structural path cannot select an erased structural field");
                }
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let Some(declaration) = leaf
                            .canonical_leaf_type()
                            .and_then(|leaf| leaf.leaf_declaration(module.structural_types.iter()))
                        else {
                            return malformed("structural path must retain structural custody");
                        };
                        declaration.id
                    }
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) => {
                let Some(length) = length.literal() else {
                    return malformed("fixed-index validation requires a closed array extent");
                };
                if *index >= length {
                    return malformed("structural path fixed index is out of bounds");
                }
                *element
            }
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Record { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
            }
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Mixed { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
            }
            (StructuralPathSegment::RuntimeIndex { .. }, StructuralTypeShape::Record { .. }) => {
                return malformed("structural path runtime index requires a fixed-array type");
            }
            (StructuralPathSegment::RuntimeIndex { .. }, StructuralTypeShape::Mixed { .. }) => {
                return malformed("structural path runtime index requires a fixed-array type");
            }
            (_, StructuralTypeShape::Sum { .. }) => {
                return malformed("structural path cannot traverse a payload-less sum");
            }
            (_, StructuralTypeShape::PrimitiveScalar(_)) => {
                return malformed("primitive-scalar structural type has no projected children");
            }
            (_, StructuralTypeShape::AtomicCell(_)) => {
                return malformed("atomic-cell structural type has no projected children");
            }
            (_, StructuralTypeShape::ByteSequence(_)) => {
                return malformed("byte-sequence structural type has no projected children");
            }
            (
                StructuralPathSegment::FixedIndex(_),
                StructuralTypeShape::ElementView { element, .. },
            ) => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { element, .. },
            ) => *element,
        };
    }
    if payload_fields.is_some() {
        return malformed("structural case path requires its payload field");
    }
    Ok(structural_type)
}

fn validate_structural_parameters(
    module: &TerminalModule,
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    let mut places = BTreeSet::new();
    for parameter in parameters {
        if !places.insert(parameter.place) {
            return malformed("structural parameters reuse a place identity");
        }
        if !has_structural_type(module, parameter.structural_type) {
            return malformed("structural parameter references an unknown type");
        }
        if parameter.access == StructuralAccess::Owned
            && contains_access_carrier(module, parameter.structural_type)
        {
            return malformed("ordinary owned parameters cannot receive placed custody");
        }
        validate_structural_qualifications(
            module,
            parameter.structural_type,
            &parameter.qualifications,
            &parameter.projected_qualifications,
        )?;
    }
    Ok(())
}

pub(crate) fn operation_foundation_error(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: BlockId,
    operation: &Operation,
    error: CodecError,
) -> CodecError {
    let CodecError::MalformedStructuralFoundation(reason) = error else {
        return error;
    };
    let declaration = machine.declaration.as_ref();
    CodecError::MalformedOperationFoundation {
        machine: declaration
            .and_then(|declaration| module.identities.text(declaration.machine.identity()))
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{:?}", machine.id)),
        state: declaration
            .and_then(|declaration| module.identities.text(declaration.state.identity()))
            .unwrap_or("synthetic")
            .to_owned(),
        block,
        operation: operation.id,
        reason,
    }
}

fn validate_operation_foundation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    transfers: &mut storage_foundations::StructuralTransferInputs<'_>,
) -> Result<(), CodecError> {
    if let OperationKind::SelectMachine { descriptor } = operation.kind {
        let selected = module.callables.descriptor(descriptor).ok_or(
            CodecError::MalformedStructuralFoundation(
                "machine selection has no retained descriptor",
            ),
        )?;
        if operation.result.scalar().map(|result| result.scalar_type)
            != Some(semantic_vocabulary::ScalarType::Callable(selected.contract))
            || module.callables.contract(selected.contract).is_none()
            || module.machine(selected.machine).is_none()
        {
            return malformed(
                "machine selection changes its retained callable contract or named body",
            );
        }
    }
    if let OperationKind::ReadScalarApplication { application } = operation.kind {
        let Some(carrier) = module
            .scalar_applications
            .declaration(application)
            .and_then(|declaration| declaration.scalar_type())
        else {
            return malformed("scalar application read has no catalog declaration");
        };
        if operation
            .result
            .scalar_ref()
            .is_none_or(|result| result.scalar_type != carrier)
        {
            return malformed("scalar application read result differs from its declared carrier");
        }
    }
    if operation
        .kind
        .structural_projections()
        .iter()
        .any(|projection| {
            projection
                .path
                .iter()
                .any(|segment| matches!(segment, StructuralPathSegment::RepresentationView { .. }))
        })
    {
        return malformed("returned-origin interpretation is not an executable projection");
    }
    if let Some(result) = operation.result.structural() {
        if contains_access_carrier(module, result.structural_type) {
            return malformed("ordinary value production cannot establish placed custody");
        }
        validate_structural_qualifications(
            module,
            result.structural_type,
            &result.qualifications,
            &result.projected_qualifications,
        )?;
    }
    match &operation.kind {
        OperationKind::EstablishAccessReference { .. } => {
            storage_foundations::validate_establish_access_reference(module, machine, operation)?
        }
        OperationKind::EstablishReference { .. } => {
            storage_foundations::validate_establish_reference(module, machine, operation)?
        }
        OperationKind::ReleaseReference { .. } => {
            storage_foundations::validate_release_reference(module, machine, operation)?
        }
        OperationKind::ByteSequenceStore { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("whole byte store requires Unit");
            }
        }
        OperationKind::StructuralByteSequenceFieldStore { .. } => {
            storage_foundations::validate_structural_byte_sequence_field_store(
                module, machine, operation,
            )?
        }
        OperationKind::StructuralByteViewFieldStore { .. } => {
            storage_foundations::validate_structural_byte_view_field_store(
                module, machine, operation,
            )?
        }
        OperationKind::StructuralByteSequenceFieldLength { .. } => {
            storage_foundations::validate_structural_byte_sequence_field_length(operation)?
        }
        OperationKind::StructuralByteSequenceFieldByteStore { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte field byte store requires Unit");
            }
        }
        OperationKind::StructuralByteSequenceFieldRead { .. } => {
            storage_foundations::validate_structural_byte_sequence_field_read(operation)?
        }
        OperationKind::ByteSequenceSubslice { .. } => {
            storage_foundations::validate_byte_sequence_subslice(module, machine, operation)?
        }
        OperationKind::EstablishElementView { .. } => {
            storage_foundations::validate_establish_element_view(module, machine, operation)?
        }
        OperationKind::EstablishActivationClaim { committed, .. } => {
            if !matches!(declared_scalar_type(machine, *committed), Some(ScalarType::Integer(integer))
                if integer.bits() == 64 && integer.sign() == IntegerSign::Unsigned
                    && integer.carrier() == IntegerCarrier::Fixed)
            {
                return malformed(
                    "activation committed extent must be an unsigned fixed 64-bit value",
                );
            }
            terminal_semantics::activation_storage::activation_claim_shape(module, operation)
                .map_err(CodecError::MalformedStructuralFoundation)?;
        }
        OperationKind::ReleaseActivationClaim { source, .. } => {
            if operation.result != OperationResult::Unit || source.access != StructuralAccess::Owned
            {
                return malformed(
                    "activation release requires one owned claim and Unit completion",
                );
            }
        }
        OperationKind::SplitActivationClaim {
            source,
            committed_at,
            ..
        } => {
            if operation.result.structural().is_none()
                || source.access != StructuralAccess::Owned
                || !matches!(declared_scalar_type(machine, *committed_at), Some(ScalarType::Integer(integer))
                    if integer.bits() == 64 && integer.sign() == IntegerSign::Unsigned
                        && integer.carrier() == IntegerCarrier::Fixed)
            {
                return malformed(
                    "activation split requires one owned claim and an unsigned fixed 64-bit cut",
                );
            }
        }
        OperationKind::MergeActivationClaim { lower, upper, .. } => {
            if operation.result.structural().is_none()
                || lower.access != StructuralAccess::Owned
                || upper.access != StructuralAccess::Owned
            {
                return malformed("activation merge requires two owned claims");
            }
        }
        OperationKind::EstablishMutableElementView { .. } => {
            storage_foundations::validate_establish_mutable_element_view(
                module, machine, operation,
            )?
        }
        OperationKind::ElementViewLength { .. } => {
            storage_foundations::validate_element_view_length(module, machine, operation)?
        }
        OperationKind::ElementViewRead { .. } => {
            storage_foundations::validate_element_view_read(module, machine, operation)?
        }
        OperationKind::ElementViewSubslice { .. } => {
            storage_foundations::validate_element_view_subslice(module, machine, operation)?
        }
        OperationKind::ByteSequenceWrite { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte-sequence write requires a Unit result");
            }
        }
        OperationKind::ElementViewWrite {
            destination,
            path,
            index,
            obligation,
            ..
        } => {
            if operation.result != OperationResult::Unit {
                return malformed("element-view write requires a Unit result");
            }
            let root = terminal_payload_contract(machine, destination.place).ok_or(
                CodecError::MalformedStructuralFoundation("element-view write root is absent"),
            )?;
            let mut written = destination.path.clone();
            written.push(StructuralPathSegment::RuntimeIndex {
                index: *index,
                obligation: *obligation,
            });
            written.extend(path.iter().cloned());
            if !structural_memberships_disjoint_from_write(
                &root.qualifications,
                &root.projected_qualifications,
                &written,
            ) {
                return malformed(
                    "element-view scalar write overlaps qualifications it does not restore",
                );
            }
        }
        OperationKind::StructuralCaseMembership { .. } => {
            storage_foundations::validate_structural_case_membership(operation)?
        }
        structural_leaf_copy_pair_pattern!() => {
            storage_foundations::validate_structural_leaf_copy(transfers, machine, operation)?
        }
        OperationKind::MoveStructuralValue { .. } => {
            storage_foundations::validate_move_structural_value(transfers, machine, operation)?
        }
        OperationKind::TransformContent { .. } => {
            terminal_semantics::content_transformations::validate_shape(module, machine, operation)
                .map_err(CodecError::MalformedStructuralFoundation)?;
        }
        OperationKind::ByteSequenceRead { .. } => {
            storage_foundations::validate_byte_sequence_read(operation)?
        }
        OperationKind::ByteSequenceLength { .. } => {
            storage_foundations::validate_byte_sequence_length(module, machine, operation)?
        }
        OperationKind::WriteOnlyPrimitiveStore { .. } => {
            storage_foundations::validate_write_only_primitive_store(module, machine, operation)?
        }
        OperationKind::StructuralScalarFieldStore { .. } => {
            storage_foundations::validate_structural_scalar_field_store(module, machine, operation)?
        }
        OperationKind::MoveStructuralField { .. } => {
            storage_foundations::validate_move_structural_field(module, machine, operation)?
        }
        OperationKind::StoreStructuralField { .. } => {
            storage_foundations::validate_store_structural_field(module, machine, operation)?
        }
        OperationKind::ScalarStructuralField { .. }
        | OperationKind::BooleanStructuralField { .. } => {
            storage_foundations::validate_scalar_structural_field(module, machine, operation)?
        }
        OperationKind::EstablishScalarArray { .. } => {
            value_foundations::validate_establish_scalar_array(module, machine, operation)?
        }
        OperationKind::EstablishScalarCase { .. } => {
            value_foundations::validate_establish_scalar_case(module, machine, operation)?
        }
        OperationKind::EstablishByteSequenceConcatenation { .. } => {
            value_foundations::validate_establish_byte_sequence_concatenation(
                module, machine, operation,
            )?
        }
        OperationKind::EstablishByteSequenceLiteral { .. } => {
            value_foundations::validate_establish_byte_sequence_literal(module, machine, operation)?
        }
        OperationKind::CallUnit { .. } => validate_call_unit(module, operation)?,
        OperationKind::CallDynamic { .. } => validate_call_dynamic(module, machine, operation)?,
        OperationKind::CallMachine { .. } => validate_call_machine(module, machine, operation)?,
        OperationKind::CallStructuralScalar { .. } => {
            validate_call_structural_scalar(module, operation)?
        }
        call_structural_pair_pattern!(..) => validate_call_structural(module, machine, operation)?,
        OperationKind::BoundaryCall { .. } => validate_boundary_call(module, machine, operation)?,
        OperationKind::PortWrite { .. } => validate_port_write(module, operation)?,
        OperationKind::ActivateTaskRegister { .. }
        | OperationKind::ReadTaskRegister { .. }
        | OperationKind::ReloadSegmentDescriptors { .. }
        | OperationKind::ReadSegmentSelector { .. } => {
            storage_foundations::validate_descriptor_operation(module, machine, operation)?
        }
        OperationKind::GlobalDescriptorTable { .. } => {
            storage_foundations::validate_global_descriptor_table(module, machine, operation)?
        }
        OperationKind::RegisterAccess { .. } => {
            validate_register_access(module, machine, operation)?
        }
        OperationKind::EstablishZeroedStructural { .. } => {
            value_foundations::validate_establish_zeroed_structural(module, machine, operation)?
        }
        OperationKind::EstablishStructuralLocal { .. } => {
            value_foundations::validate_establish_structural_local(module, machine, operation)?
        }
        OperationKind::EstablishTrivialAffineLocal { .. } => {
            value_foundations::validate_establish_trivial_affine_local(module, machine, operation)?
        }
        OperationKind::EstablishStructuralArray { .. } => {
            value_foundations::validate_establish_structural_array(module, machine, operation)?
        }
        OperationKind::EstablishNativeCodeLayout { .. } => {
            value_foundations::validate_native_code_layout(module, machine, operation)?;
        }
        OperationKind::EstablishRecord { .. } => {
            value_foundations::validate_establish_record(module, machine, operation)?
        }
        OperationKind::EstablishStructuralCase { .. } => {
            value_foundations::validate_establish_structural_case(module, machine, operation)?
        }
        OperationKind::EstablishPrimitiveLocal { .. } => {
            value_foundations::validate_establish_primitive_local(module, machine, operation)?
        }
        OperationKind::Call { .. } => {
            if !matches!(operation.result, OperationResult::Scalar(_)) {
                return malformed("scalar call declares a Unit result");
            }
        }
        OperationKind::AtomicFence { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("atomic fence must define no value");
            }
        }
        OperationKind::AtomicAccess {
            place,
            path,
            event,
            resident_obligation,
        } => {
            if !event.ordering_is_legal() {
                return malformed("atomic access has an illegal ordering");
            }
            let root = structural_place_type(machine, *place).ok_or({
                CodecError::MalformedStructuralFoundation("atomic location has no declared root")
            })?;
            let cell =
                terminal_semantics::atomic_cell_type(module.structural_types.iter(), root, path)
                    .ok_or({
                        CodecError::MalformedStructuralFoundation(
                            "atomic location is not an atomic cell",
                        )
                    })?;
            let resident = cell.scalar_type();
            let bounded = cell.bounds().is_some();
            if resident_obligation.is_some() != (bounded && event.modifies_resident()) {
                return malformed(
                    "atomic resident establishment obligation disagrees with its declaration and event",
                );
            }
            if event
                .operands()
                .iter()
                .any(|value| declared_scalar_type(machine, *value) != Some(resident))
            {
                return malformed("atomic operand differs from the exact resident type");
            }
            let once = matches!(event, AtomicAccessEvent::CompareExchangeOnce { .. });
            let shape_matches = match &operation.result {
                OperationResult::Scalar(result) => {
                    !once && event.observes_resident() && result.scalar_type == resident
                }
                OperationResult::Unit => !event.observes_resident(),
                OperationResult::Structural(result) => {
                    event.single_attempt_custody().is_some_and(|custody| {
                        terminal_psi::types::atomic_once_result_is_plain(result, custody)
                    }) && module.structural_type(result.structural_type).is_some_and(
                        |declaration| {
                            event.single_attempt_custody().is_some_and(|custody| {
                                atomic_once_outcome_cases(declaration, resident, custody).is_some()
                            })
                        },
                    )
                }
            };
            if !shape_matches {
                return malformed("atomic access result disagrees with its event");
            }
        }
        _ => {
            if !matches!(operation.result, OperationResult::Scalar(_)) {
                return malformed("scalar operation declares a Unit result");
            }
        }
    }
    Ok(())
}

fn validate_structural_qualifications(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
) -> Result<(), CodecError> {
    if !strictly_increasing(whole.iter().copied()) || !strictly_increasing(projected.iter()) {
        return malformed("structural qualifications are noncanonical");
    }
    for (domain, carrier) in whole
        .iter()
        .map(|domain| (*domain, Ok(structural_type)))
        .chain(projected.iter().map(|row| {
            (
                row.domain,
                if row.path.is_empty() {
                    malformed("projected qualification requires a child path")
                } else {
                    qualification_path_carrier(
                        module.structural_types.iter(),
                        structural_type,
                        &row.path,
                    )
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "qualification pattern has no exact carrier",
                    ))
                },
            )
        }))
    {
        let carrier = carrier?;
        let Some(declaration) = module.structural_domain(domain) else {
            return malformed("structural qualification references an unknown domain");
        };
        if declaration.carrier != carrier {
            return malformed("structural qualification has the wrong carrier");
        }
    }
    Ok(())
}

fn callee_exact_payloadless_return(callee: &TerminalMachine) -> bool {
    let Some(result) = callee.result.structural() else {
        return false;
    };
    let mut has_return = false;
    for block in &callee.blocks {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &block.terminator
        else {
            continue;
        };
        has_return = true;
        if !returned_claims.is_empty() {
            return false;
        }
        let Some(producer) = callee.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == result.structural_type => Some(producer),
                    _ => None,
                })
        }) else {
            return false;
        };
        let Some(operation) = callee.operation(producer) else {
            return false;
        };
        if !matches!(&operation.kind, OperationKind::EstablishScalarCase { fields, .. } if fields.is_empty())
            || !operation
                .result
                .structural()
                .is_some_and(|operation_result| {
                    operation_result.place == *source
                        && operation_result.structural_type == result.structural_type
                        && operation_result.multiplicity == StructuralMultiplicity::Unrestricted
                        && operation_result.qualifications.is_empty()
                        && operation_result.claims.is_empty()
                })
        {
            return false;
        }
    }
    has_return
        && callee.operations().all(|operation| {
            !matches!(
                operation.kind,
                OperationKind::Call { .. }
                    | OperationKind::CallUnit { .. }
                    | OperationKind::CallStructuralScalar { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::BoundaryCall { .. }
            )
        })
}

#[derive(Clone, Copy)]
enum StructuralArgumentPresentation {
    Ordinary,
    Boundary,
}

fn validate_structural_arguments(
    module: &TerminalModule,
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    presentation: StructuralArgumentPresentation,
) -> Result<(), CodecError> {
    for (argument, expected) in arguments.iter().zip(expected) {
        scalar_fields::validate_primitive_reference_presentation(
            module,
            machine,
            argument.place,
            &argument.path,
            expected.access,
        )?;
        if expected.access == StructuralAccess::Owned
            && contains_access_carrier(module, expected.structural_type)
        {
            return malformed("ordinary owned call transport cannot carry placed custody");
        }
        let Some(actual_type) = structural_place_type(machine, argument.place) else {
            return malformed("structural argument references an unknown structural place");
        };
        let parameter = machine.signature_structural_parameter(argument.place);
        let result = machine
            .operations()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place);
        let owned_array_payload =
            scalar_array_leaf_shape(module.structural_types.iter(), actual_type).is_some()
                && (parameter.is_some_and(|parameter| {
                    parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                }) || result.is_some());
        if owned_array_payload {
            let payload_loan =
                argument.access == expected.access && argument.access != StructuralAccess::Owned;
            let supported = |carrier, whole: &[_], projected: &[_]| {
                projected.is_empty()
                    || payload_loan
                        && pure_byte_qualifications(
                            module.structural_types.iter(),
                            module.structural_domains.iter(),
                            carrier,
                            whole,
                            projected,
                        )
            };
            let plain_source = parameter.is_some_and(|parameter| {
                parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && supported(
                        actual_type,
                        &parameter.qualifications,
                        &parameter.projected_qualifications,
                    )
            }) || result.is_some_and(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && supported(
                        actual_type,
                        &result.qualifications,
                        &result.projected_qualifications,
                    )
                    && result.claims.is_empty()
            });
            let admitted_access = matches!(
                (argument.access, expected.access),
                (StructuralAccess::Owned, StructuralAccess::Owned)
            ) || payload_loan;
            if !plain_source
                || !admitted_access
                || (!payload_loan
                    && argument
                        .path
                        .iter()
                        .any(|segment| !matches!(segment, StructuralPathSegment::FixedIndex(_))))
                || expected.multiplicity != StructuralMultiplicity::Unrestricted
                || !supported(
                    expected.structural_type,
                    &expected.qualifications,
                    &expected.projected_qualifications,
                )
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == argument.place)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == argument.place)
            {
                return malformed(
                    "primitive-array argument requires owned element selection or loan custody",
                );
            }
        }
        let result_backing = result.map(|result| StructuralParameterDeclaration {
            place: argument.place,
            position: expected.position,
            structural_type: actual_type,
            multiplicity: result.multiplicity,
            access: StructuralAccess::Owned,
            qualifications: result.qualifications.clone(),
            projected_qualifications: result.projected_qualifications.clone(),
            additional_input_qualifications: result.additional_input_qualifications.clone(),
        });
        let backing = parameter
            .or_else(|| block_parameter(machine, argument.place))
            .or(result_backing.as_ref());
        if backing.is_some_and(|actual| {
            (fixed_byte_array_loan(
                module.structural_types.iter(),
                module.structural_domains.iter(),
                actual,
                argument,
                expected,
            )
            .is_some()
                || fixed_element_array_extent(
                    module.structural_types.iter(),
                    actual,
                    argument,
                    expected,
                )
                .is_some())
                && !result.is_some_and(|result| {
                    !result.claims.is_empty() || !result.qualification_establishments.is_empty()
                })
                && !machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == argument.place)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == argument.place)
        }) {
            continue;
        }
        if matches!(presentation, StructuralArgumentPresentation::Ordinary)
            && backing.is_some_and(|actual| {
                bounded_byte_owner_capacity(module, actual, argument, expected).is_some()
            })
            && !result.is_some_and(|result| {
                !result.claims.is_empty() || !result.qualification_establishments.is_empty()
            })
            && !machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == argument.place)
            && !machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
        {
            continue;
        }
        let inline_byte_view = match presentation {
            StructuralArgumentPresentation::Boundary => true,
            StructuralArgumentPresentation::Ordinary => {
                expected.multiplicity == StructuralMultiplicity::Unrestricted
                    && !argument.path.is_empty()
                    && argument
                        .path
                        .iter()
                        .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
                    && machine.structural_parameters.iter().any(|parameter| {
                        parameter.place == argument.place
                            && parameter.access == StructuralAccess::MutableBorrow
                            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    })
            }
        };
        if inline_byte_view
            && boundary_buffer_capacity(
                module.structural_types.iter(),
                actual_type,
                argument,
                expected,
            )
            .is_some()
        {
            continue;
        }
        let shared_byte_field_loan = shared_boundary_buffer_capacity(
            module.structural_types.iter(),
            actual_type,
            argument,
            expected,
        )
        .is_some()
            && expected.multiplicity == StructuralMultiplicity::Unrestricted;
        if shared_byte_field_loan {
            continue;
        }
        if stored_view_field_presentation(
            module.structural_types.iter(),
            actual_type,
            argument,
            expected,
        )
        .is_some()
        {
            continue;
        }
        if owned_byte_view_presentation(module, machine, argument, expected) {
            continue;
        }
        let borrowed_endpoint = (argument.access == expected.access
            && argument.access != StructuralAccess::Owned)
            .then(|| {
                borrowed_primitive_projection_declaration(
                    module.structural_types.iter(),
                    actual_type,
                    &argument.path,
                    Some(expected.structural_type),
                )
            })
            .flatten();
        let actual_type = match borrowed_endpoint {
            Some(endpoint) => endpoint,
            None => validate_structural_path(module, actual_type, &argument.path)?,
        };
        if actual_type != expected.structural_type
            && !(argument.access == expected.access
                && argument.access != StructuralAccess::Owned
                && borrowed_referent_matches(
                    module.structural_types.iter(),
                    actual_type,
                    expected.structural_type,
                ))
        {
            return malformed("structural argument has the wrong concrete type");
        }
    }
    Ok(())
}

fn owned_byte_view_presentation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> bool {
    if !argument.path.is_empty()
        || argument.access != StructuralAccess::SharedBorrow
        || expected.access != StructuralAccess::SharedBorrow
        || expected.multiplicity != StructuralMultiplicity::Unrestricted
        || !expected.projected_qualifications.is_empty()
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == argument.place)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == argument.place)
        || byte_owned_result_predicates(module, machine, argument.place).is_none()
        || !module.structural_types.iter().any(|declaration| {
            declaration.id == expected.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: None | Some(StructuralAccess::SharedBorrow),
                    })
                )
        })
    {
        return false;
    }
    let Some(required) = byte_parameter_predicates(module, expected) else {
        return false;
    };
    byte_sequence_source_predicates(module, machine, argument.place)
        .is_some_and(|available| available.implies(required))
}

fn structural_place_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .find_map(|parameter| (parameter.place == place).then_some(parameter.structural_type))
        .or_else(|| {
            machine.structural_places.iter().find_map(|declaration| {
                if declaration.id != place {
                    return None;
                }
                match declaration.kind {
                    StructuralPlaceKind::BlockParameter { .. } => {
                        block_parameter(machine, place).map(|parameter| parameter.structural_type)
                    }
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } => Some(structural_type),
                    StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
                    | StructuralPlaceKind::Result => None,
                }
            })
        })
}

fn block_parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    machine.structural_places.iter().find_map(|declaration| {
        let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
            return None;
        };
        (declaration.id == place).then_some(())?;
        machine
            .block(&block)?
            .structural_parameters
            .get(position as usize)
            .filter(|parameter| parameter.place == place)
    })
}

fn is_owned_cleanup_operation_result(machine: &TerminalMachine, place: PlaceId) -> bool {
    let mut places = machine
        .structural_places
        .iter()
        .filter(|candidate| candidate.id == place);
    let Some(declaration) = places.next() else {
        return false;
    };
    if places.next().is_some() {
        return false;
    }
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return false;
    };
    let mut operations = machine
        .operations()
        .filter(|operation| operation.id == producer);
    let Some(operation) = operations.next() else {
        return false;
    };
    if operations.next().is_some() {
        return false;
    }
    operation.result.as_structural().is_some_and(|result| {
        result.place == place
            && result.structural_type == structural_type
            && matches!(
                result.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Linear
            )
    })
}

fn validate_claim_actions(
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    claims: impl Iterator<Item = (ClaimId, u32)>,
) -> Result<(), CodecError> {
    let mut transferred = BTreeSet::new();
    for (claim, argument_index) in claims {
        if arguments.get(argument_index as usize).is_none() {
            return malformed("claim action has an unknown structural argument index");
        }
        if !transferred.insert(claim) {
            return malformed("claim action repeats a claim identity");
        }
        let declared = machine
            .entry_claims
            .iter()
            .chain(&machine.block_entry_claims)
            .any(|entry| entry.claim == claim)
            || machine.loan_completions.iter().any(|completion| {
                completion
                    .claims
                    .iter()
                    .any(|binding| binding.claim == claim)
            })
            || machine.operations().any(|operation| {
                let Some(result) = operation.result.structural() else {
                    return false;
                };
                machine.structural_places.iter().any(|declaration| {
                    declaration.id == result.place
                        && matches!(declaration.kind,
                        StructuralPlaceKind::OperationResult { producer, structural_type }
                        if producer == operation.id && structural_type == result.structural_type)
                }) && result.claims.iter().any(|binding| binding.claim == claim)
            });
        if !declared {
            return malformed("claim action has an undeclared structural claim identity");
        }
    }
    Ok(())
}

fn has_structural_type(module: &TerminalModule, id: StructuralTypeId) -> bool {
    module.structural_type(id).is_some()
}

fn validate_structural_type_graph(
    declarations: &[StructuralTypeDeclaration],
) -> Result<(), CodecError> {
    fn visit(
        declarations: &[StructuralTypeDeclaration],
        id: StructuralTypeId,
        active: &mut BTreeSet<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), CodecError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return malformed("structural type graph contains a by-value cycle");
        }
        let declaration = find_structural_type(declarations, id)
            .expect("structural field targets were validated before graph traversal");
        match &declaration.shape {
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::AtomicCell(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. } => {}
            StructuralTypeShape::Record { fields, .. } => {
                for field in fields {
                    if let Some(target) = field.field_type.as_structural() {
                        visit(declarations, target, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(declarations, *element, active, complete)?;
            }
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let Some(target) = field.field_type.as_structural() {
                        visit(declarations, target, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let Some(target) = field.field_type.as_structural() {
                        visit(declarations, target, active, complete)?;
                    }
                }
            }
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for declaration in declarations {
        visit(declarations, declaration.id, &mut active, &mut complete)?;
    }
    Ok(())
}

fn validate_service_parent_graph(module: &TerminalModule) -> Result<(), CodecError> {
    fn visit(
        module: &TerminalModule,
        id: ServiceId,
        active: &mut BTreeSet<ServiceId>,
        complete: &mut BTreeSet<ServiceId>,
    ) -> Result<(), CodecError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return malformed("service parent graph contains a cycle");
        }
        let declaration = module
            .service(id)
            .expect("service parent targets were validated before graph traversal");
        for parent in &declaration.parents {
            visit(module, *parent, active, complete)?;
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for declaration in &module.services {
        visit(module, declaration.id, &mut active, &mut complete)?;
    }
    for declaration in &module.services {
        for parent in &declaration.parents {
            let parent = module
                .service(*parent)
                .expect("service parent targets were validated before closure validation");
            if parent
                .parents
                .iter()
                .any(|ancestor| !declaration.parents.contains(ancestor))
            {
                return malformed("service parent closure is incomplete");
            }
        }
    }
    Ok(())
}

fn has_service(module: &TerminalModule, id: ServiceId) -> bool {
    module
        .services
        .iter()
        .any(|declaration| declaration.id == id)
}

fn require_known_services(
    module: &TerminalModule,
    services: &[ServiceId],
) -> Result<(), CodecError> {
    if services
        .iter()
        .any(|service| !has_service(module, *service))
    {
        return malformed("published service row references an unknown service");
    }
    Ok(())
}

fn require_unique_nonempty_identities(
    identities: impl Iterator<Item = TerminalIdentityIndex>,
    label: &'static str,
) -> Result<(), CodecError> {
    let mut seen = BTreeSet::new();
    for identity in identities {
        if identity.is_absent() || !seen.insert(identity) {
            return Err(CodecError::MalformedStructuralFoundation(label));
        }
    }
    Ok(())
}
