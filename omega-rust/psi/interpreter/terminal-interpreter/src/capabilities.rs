//! Capability decisions on the admitted execution projection, before effects.
//!
//! A valid selected reference result can retain several possible ingress
//! backings; the current reference runtime transports only singleton origins.
//! External completions also need a closed terminal invocation; the transitional
//! Unit result is not a normal return the runtime can commit. Reject these
//! missing capabilities before any entry operation, rather than after
//! a preceding boundary effect. This is not semantic verification: admission
//! already authenticated these signatures and dispatch tables. Walk possible
//! executable calls from the chosen entry, preserving descriptor arguments and
//! installed providers, so unrelated retained declarations and unused dynamic
//! requirement rows do not impose runtime capabilities.
//! The existing selected-backing refusal takes priority over external completion:
//! collect external demands while inspecting the rest of the closure, without
//! following them as normal provider calls. This keeps independent transport
//! limitations visible regardless of when an external call is encountered.

use crate::effects::AdmittedProviderInstallation;
use crate::errors::TerminalInterpretError;
use crate::program::AdmittedTerminalProgram;
use semantic_vocabulary::{BoundaryMachineId, ExternalCompletionKind, MachineId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    OperationKind, StructuralPathSegment, TerminalAffineCleanupAction,
    TerminalDynamicDescriptorSource, Terminator,
};

/// A precise implementation limitation on an otherwise admitted program.
/// Driver task names and display diagnostics belong to the driver, not Psi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalUnsupportedCapability {
    ExternalCompletionInvocation {
        boundary: BoundaryMachineId,
        kind: ExternalCompletionKind,
    },
    SelectedReferenceResultBacking {
        machine: MachineId,
        result_path: Vec<StructuralPathSegment>,
    },
}

/// Exact host/provider provisions of the possible execution closure. Unused
/// catalog declarations do not demand bindings; installed provider bodies do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExecutionRequirements {
    pub imports: Vec<BoundaryMachineId>,
    pub missing_providers: Vec<BoundaryMachineId>,
}

type DescriptorParameters = BTreeMap<u32, Vec<Option<MachineId>>>;

pub(crate) fn check(
    program: &AdmittedTerminalProgram,
    installation: Option<&AdmittedProviderInstallation>,
) -> Result<TerminalExecutionRequirements, TerminalInterpretError> {
    if installation.is_some_and(|installation| installation.terminal_psi != program.identity) {
        return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
    }
    let mut imports = BTreeSet::new();
    let mut missing_providers = BTreeSet::new();
    let mut external_completion = None;
    let mut pending = vec![(program.catalogs.entry, DescriptorParameters::new())];
    let mut visited = BTreeSet::new();
    while let Some((machine_id, parameters)) = pending.pop() {
        if !visited.insert((machine_id, parameters.clone())) {
            continue;
        }
        let machine = program.machines.get(&machine_id).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(machine_id),
        )?;
        let mut blocks = vec![machine.entry];
        let mut visited_blocks = BTreeSet::new();
        while let Some(block_id) = blocks.pop() {
            if !visited_blocks.insert(block_id) {
                continue;
            }
            let block = machine
                .blocks
                .get(&block_id)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
            // Backing transport is demanded only by a possible normal
            // structural return. An all-crash machine's declared result is
            // metadata, not a runtime result the engine must transport.
            if matches!(block.terminator, Terminator::ReturnStructural { .. }) {
                if let Some(result) = machine.result.structural() {
                    for origin in &result.reference_sources {
                        // Empty or otherwise invalid source rosters are verifier errors,
                        // never missing runtime coverage. Only valid alternatives need
                        // the transport that reference.rs currently cannot supply.
                        if origin.sources.len() > 1 {
                            return Err(TerminalInterpretError::UnsupportedCapability(
                                TerminalUnsupportedCapability::SelectedReferenceResultBacking {
                                    machine: machine_id,
                                    result_path: origin.path.clone(),
                                },
                            ));
                        }
                    }
                }
            }
            for operation in &block.operations {
                let callee = match &operation.kind {
                    OperationKind::Call { callee, .. }
                    | OperationKind::CallUnit { callee, .. }
                    | OperationKind::CallStructuralScalar { callee, .. }
                    | OperationKind::CallStructural { callee, .. }
                    | OperationKind::CallStructuralWithScalarArguments { callee, .. } => {
                        Some(*callee)
                    }
                    OperationKind::CallDynamicScalar {
                        descriptor_ordinal, ..
                    }
                    | OperationKind::CallDynamicUnit {
                        descriptor_ordinal, ..
                    } => Some(
                        program
                            .dynamic_scalar_calls
                            .get(&(machine_id, *descriptor_ordinal))
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?
                            .0,
                    ),
                    OperationKind::CallDynamicParameterScalar {
                        parameter_ordinal,
                        requirement_slot,
                        ..
                    }
                    | OperationKind::CallDynamicParameterUnit {
                        parameter_ordinal,
                        requirement_slot,
                        ..
                    } => Some(
                        parameters
                            .get(parameter_ordinal)
                            .ok_or(TerminalInterpretError::DynamicDescriptorInputMissing {
                                machine: machine_id,
                                parameter_ordinal: *parameter_ordinal,
                            })?
                            .get(*requirement_slot as usize)
                            .copied()
                            .flatten()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?,
                    ),
                    OperationKind::BoundaryCall { boundary, .. } => {
                        let declaration = program.boundary_machines.get(boundary).ok_or(
                            TerminalInterpretError::VerifiedBoundaryMachineMissing(*boundary),
                        )?;
                        if let terminal_psi::BoundaryCompletion::External(kind) =
                            declaration.completion
                        {
                            if external_completion.is_none_or(|(found, _)| *boundary < found) {
                                external_completion = Some((*boundary, kind));
                            }
                            continue;
                        }
                        // An admitted assumed binding may also resolve a
                        // required boundary to a checked provider. Follow the
                        // exact installation map, not only direct candidates.
                        if let Some(selected) = installation
                            .and_then(|installation| installation.installed.get(boundary))
                            .copied()
                        {
                            Some(selected)
                        } else if program.provider_candidates.contains(boundary) {
                            missing_providers.insert(*boundary);
                            None
                        } else {
                            imports.insert(*boundary);
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(callee) = callee {
                    pending.push((
                        callee,
                        call_parameters(program, machine_id, operation.id, &parameters)?,
                    ));
                }
            }
            match &block.terminator {
                Terminator::Jump { target, .. } => blocks.push(*target),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    blocks.extend([when_true.target, when_false.target]);
                }
                Terminator::StructuralCase { cases, .. } => {
                    blocks.extend(cases.iter().map(|edge| edge.target))
                }
                _ => {}
            }
            // Every possible successor and return owns an ordered cleanup
            // roster. Its nominal calls belong to this same execution closure;
            // a Unit return or branch edge must not hide their host demands.
            pending.extend(
                block
                    .terminator
                    .cleanup_rosters()
                    .flat_map(|(_, actions)| actions)
                    .filter_map(|action| match action {
                        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                            Some((cleanup.cleanup_machine, DescriptorParameters::new()))
                        }
                        _ => None,
                    }),
            );
        }
    }
    if let Some((boundary, kind)) = external_completion {
        return Err(TerminalInterpretError::UnsupportedCapability(
            TerminalUnsupportedCapability::ExternalCompletionInvocation { boundary, kind },
        ));
    }
    Ok(TerminalExecutionRequirements {
        imports: imports.into_iter().collect(),
        missing_providers: missing_providers.into_iter().collect(),
    })
}

fn call_parameters(
    program: &AdmittedTerminalProgram,
    owner: MachineId,
    operation: OperationId,
    parameters: &DescriptorParameters,
) -> Result<DescriptorParameters, TerminalInterpretError> {
    let mut supplied = DescriptorParameters::new();
    for argument in program
        .dynamic_descriptor_arguments
        .get(&(owner, operation))
        .into_iter()
        .flatten()
    {
        let table = match argument.source {
            TerminalDynamicDescriptorSource::Selection { ordinal } => program
                .dynamic_selection_templates
                .get(&(owner, ordinal))
                .map(|template| &template.callables),
            TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } => program
                .dynamic_descriptor_templates
                .get(&(owner, ordinal))
                .map(|template| &template.callables),
            TerminalDynamicDescriptorSource::Parameter { ordinal } => {
                Some(parameters.get(&ordinal).ok_or(
                    TerminalInterpretError::DynamicDescriptorInputMissing {
                        machine: owner,
                        parameter_ordinal: ordinal,
                    },
                )?)
            }
        }
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        supplied.insert(argument.parameter_ordinal, table.clone());
    }
    Ok(supplied)
}

#[cfg(test)]
mod tests {
    // These tests exercise the projection graph algorithm, not program admission.
    // End-to-end acceptance uses genuinely produced artifacts in the corpus.
    use super::*;
    use crate::execution::{ExecutableMachine, RuntimeDynamicDescriptorTemplate};
    use semantic_vocabulary::{BlockId, EdgeId, PlaceId, StructuralTypeId};
    use terminal_psi::{
        Block, Operation, OperationResult, StructuralAccess, StructuralArgument,
        StructuralMultiplicity, StructuralReferenceResultSource, StructuralResultDeclaration,
        TerminalMachineResult, TerminalModule,
    };

    fn machine(selected_origins: usize, operations: Vec<OperationKind>) -> ExecutableMachine {
        let result = if selected_origins == 0 {
            TerminalMachineResult::Unit
        } else {
            TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: PlaceId::new(1).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                reference_sources: vec![StructuralReferenceResultSource {
                    path: Vec::new(),
                    sources: (1..=selected_origins)
                        .map(|index| StructuralArgument {
                            place: PlaceId::new(index as u64).unwrap(),
                            path: Vec::new(),
                            access: StructuralAccess::MutableBorrow,
                        })
                        .collect(),
                }],
            })
        };
        let block_id = BlockId::new(1).unwrap();
        let block = Block {
            id: block_id,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: operations
                .into_iter()
                .enumerate()
                .map(|(index, kind)| Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: OperationId::new(index as u64 + 1).unwrap(),
                    result: OperationResult::Unit,
                    kind,
                })
                .collect(),
            terminator: if selected_origins > 0 {
                Terminator::ReturnStructural {
                    edge: EdgeId::new(1).unwrap(),
                    source: PlaceId::new(1).unwrap(),
                    returned_claims: Vec::new(),
                    cleanup_actions: Vec::new(),
                }
            } else {
                Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: Vec::new(),
                }
            },
        };
        ExecutableMachine {
            attachment: None,
            nominal_cleanup_subject: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            content_entry_claims: Vec::new(),
            result,
            entry: block_id,
            blocks: BTreeMap::from([(block_id, block)]),
        }
    }

    fn program() -> AdmittedTerminalProgram {
        let entry = MachineId::new(1).unwrap();
        let mut program = AdmittedTerminalProgram::admit(TerminalModule::for_entry(entry)).unwrap();
        program.machines.insert(entry, machine(1, Vec::new()));
        let boundary = BoundaryMachineId::new(1).unwrap();
        program
            .boundary_machines
            .insert(boundary, boundary_declaration(boundary));
        program
    }

    fn boundary_declaration(
        boundary: BoundaryMachineId,
    ) -> terminal_psi::BoundaryMachineDeclaration {
        terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: "graph boundary".into(),
            attachment: None,
            parameter_order: Vec::new(),
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            published_service_ceiling: Vec::new(),
        }
    }

    fn boundary_call(boundary: BoundaryMachineId) -> OperationKind {
        OperationKind::BoundaryCall {
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        }
    }

    #[test]
    fn capability_graph_refuses_external_completion_before_provider_dispatch() {
        let mut program = program();
        let boundary = BoundaryMachineId::new(1).unwrap();
        program
            .boundary_machines
            .get_mut(&boundary)
            .unwrap()
            .completion =
            terminal_psi::BoundaryCompletion::External(ExternalCompletionKind::ProcessExit);
        program.machines.insert(
            program.catalogs.entry,
            machine(0, vec![boundary_call(boundary)]),
        );
        program.provider_candidates.insert(boundary);
        assert_eq!(
            check(&program, None),
            Err(TerminalInterpretError::UnsupportedCapability(
                TerminalUnsupportedCapability::ExternalCompletionInvocation {
                    boundary,
                    kind: ExternalCompletionKind::ProcessExit,
                },
            ))
        );
    }

    #[test]
    fn capability_graph_ignores_unreachable_external_completion() {
        let mut program = program();
        let boundary = BoundaryMachineId::new(1).unwrap();
        program
            .boundary_machines
            .get_mut(&boundary)
            .unwrap()
            .completion =
            terminal_psi::BoundaryCompletion::External(ExternalCompletionKind::ProcessExit);
        program.machines.insert(
            MachineId::new(2).unwrap(),
            machine(0, vec![boundary_call(boundary)]),
        );
        assert!(check(&program, None).unwrap().imports.is_empty());
    }

    #[test]
    fn capability_graph_missing_declaration_remains_malformed() {
        let mut program = program();
        let boundary = BoundaryMachineId::new(2).unwrap();
        program.machines.insert(
            program.catalogs.entry,
            machine(0, vec![boundary_call(boundary)]),
        );
        assert_eq!(
            check(&program, None),
            Err(TerminalInterpretError::VerifiedBoundaryMachineMissing(
                boundary
            ))
        );
    }

    #[test]
    fn capability_graph_selected_backing_precedes_external_completion() {
        let mut program = program();
        let boundary = BoundaryMachineId::new(1).unwrap();
        let selected = MachineId::new(2).unwrap();
        program
            .boundary_machines
            .get_mut(&boundary)
            .unwrap()
            .completion =
            terminal_psi::BoundaryCompletion::External(ExternalCompletionKind::ProcessExit);
        program.machines.insert(
            program.catalogs.entry,
            machine(0, vec![boundary_call(boundary), unit_call(selected)]),
        );
        program.machines.insert(selected, machine(2, Vec::new()));
        assert!(
            matches!(check(&program, None), Err(TerminalInterpretError::UnsupportedCapability(
            TerminalUnsupportedCapability::SelectedReferenceResultBacking { machine, .. }
        )) if machine == selected)
        );
    }

    #[test]
    fn capability_graph_external_completion_does_not_hide_malformed_closure() {
        let mut program = program();
        let external = BoundaryMachineId::new(1).unwrap();
        let missing = BoundaryMachineId::new(2).unwrap();
        program
            .boundary_machines
            .get_mut(&external)
            .unwrap()
            .completion =
            terminal_psi::BoundaryCompletion::External(ExternalCompletionKind::ProcessExit);
        program.machines.insert(
            program.catalogs.entry,
            machine(0, vec![boundary_call(external), boundary_call(missing)]),
        );
        assert_eq!(
            check(&program, None),
            Err(TerminalInterpretError::VerifiedBoundaryMachineMissing(
                missing
            ))
        );
    }

    fn unit_call(callee: MachineId) -> OperationKind {
        OperationKind::CallUnit {
            callee,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    }

    fn import_cleanup_program() -> (AdmittedTerminalProgram, TerminalAffineCleanupAction) {
        let mut program = program();
        let cleanup_machine = MachineId::new(2).unwrap();
        program.machines.insert(
            cleanup_machine,
            machine(
                0,
                vec![OperationKind::BoundaryCall {
                    boundary: BoundaryMachineId::new(1).unwrap(),
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                    requirement_obligations: Vec::new(),
                }],
            ),
        );
        let action =
            TerminalAffineCleanupAction::InvokeNominal(terminal_psi::NominalAffineCleanup {
                place: PlaceId::new(1).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                cleanup_machine,
                consumed_claims: Vec::new(),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            });
        (program, action)
    }

    #[test]
    fn capability_graph_demands_successor_cleanup_import() {
        let (mut program, action) = import_cleanup_program();
        let entry = program.machines.get_mut(&program.catalogs.entry).unwrap();
        let successor = BlockId::new(2).unwrap();
        let mut destination = entry.blocks[&entry.entry].clone();
        destination.id = successor;
        entry.blocks.insert(successor, destination);
        entry.blocks.get_mut(&entry.entry).unwrap().terminator = Terminator::Jump {
            edge: EdgeId::new(1).unwrap(),
            target: successor,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: vec![action],
        };
        assert_eq!(
            check(&program, None).unwrap().imports,
            vec![BoundaryMachineId::new(1).unwrap()]
        );
    }

    #[test]
    fn capability_graph_ignores_unreachable_cleanup_import() {
        let (mut program, action) = import_cleanup_program();
        let entry = program.machines.get_mut(&program.catalogs.entry).unwrap();
        let mut unreachable = entry.blocks[&entry.entry].clone();
        unreachable.id = BlockId::new(2).unwrap();
        unreachable.terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            cleanup_actions: vec![action],
        };
        entry.blocks.insert(unreachable.id, unreachable);
        assert!(check(&program, None).unwrap().imports.is_empty());
    }

    #[test]
    fn capability_graph_ignores_unused_selected_result() {
        let mut program = program();
        program
            .machines
            .insert(MachineId::new(2).unwrap(), machine(2, Vec::new()));
        assert_eq!(check(&program, None).unwrap().imports, Vec::new());
    }

    #[test]
    fn capability_graph_reaches_called_selected_result() {
        let mut program = program();
        let selected = MachineId::new(2).unwrap();
        program.machines.insert(
            program.catalogs.entry,
            machine(0, vec![unit_call(selected)]),
        );
        program.machines.insert(selected, machine(2, Vec::new()));
        assert!(
            matches!(check(&program, None), Err(TerminalInterpretError::UnsupportedCapability(
            TerminalUnsupportedCapability::SelectedReferenceResultBacking { machine, .. })) if machine == selected)
        );
    }

    #[test]
    fn capability_graph_does_not_demand_a_crashed_result() {
        let mut program = program();
        let selected = MachineId::new(2).unwrap();
        program.machines.insert(
            program.catalogs.entry,
            machine(0, vec![unit_call(selected)]),
        );
        let mut never_returns = machine(2, Vec::new());
        never_returns
            .blocks
            .get_mut(&never_returns.entry)
            .unwrap()
            .terminator = Terminator::Crash {
            edge: EdgeId::new(1).unwrap(),
            cause: terminal_psi::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        };
        program.machines.insert(selected, never_returns);
        assert!(check(&program, None).is_ok());
    }

    #[test]
    fn capability_graph_uses_called_dynamic_slot_not_unused_rows() {
        let mut program = program();
        let target = MachineId::new(2).unwrap();
        let unused = MachineId::new(3).unwrap();
        let parameter = MachineId::new(4).unwrap();
        program.machines.insert(
            program.catalogs.entry,
            machine(0, vec![unit_call(parameter)]),
        );
        program.machines.insert(
            parameter,
            machine(
                0,
                vec![OperationKind::CallDynamicParameterUnit {
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }],
            ),
        );
        program.machines.insert(target, machine(1, Vec::new()));
        program.machines.insert(unused, machine(2, Vec::new()));
        program.dynamic_selection_templates.insert(
            (program.catalogs.entry, 0),
            RuntimeDynamicDescriptorTemplate {
                source: StructuralArgument {
                    place: PlaceId::new(1).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                },
                callables: vec![Some(target), Some(unused)],
            },
        );
        program.dynamic_descriptor_arguments.insert(
            (program.catalogs.entry, OperationId::new(1).unwrap()),
            vec![terminal_psi::TerminalDynamicDescriptorArgument {
                owner: program.catalogs.entry,
                operation: OperationId::new(1).unwrap(),
                parameter_ordinal: 0,
                source: TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
            }],
        );
        assert!(check(&program, None).is_ok());
    }

    #[test]
    fn capability_graph_separates_imports_and_provider_installations() {
        let mut program = program();
        let boundary = BoundaryMachineId::new(1).unwrap();
        let target = MachineId::new(2).unwrap();
        // Boundary operation payload is irrelevant to this graph query.
        let kind = OperationKind::BoundaryCall {
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        };
        program
            .machines
            .insert(program.catalogs.entry, machine(0, vec![kind]));
        assert_eq!(check(&program, None).unwrap().imports, vec![boundary]);
        program.provider_candidates.insert(boundary);
        let requirements = check(&program, None).unwrap();
        assert!(requirements.imports.is_empty());
        assert_eq!(requirements.missing_providers, vec![boundary]);
        program.machines.insert(target, machine(2, Vec::new()));
        let mut installation = AdmittedProviderInstallation {
            terminal_psi: program.identity,
            installed: BTreeMap::from([(boundary, target)]),
            assumed_bindings: Vec::new(),
        };
        assert!(matches!(
            check(&program, Some(&installation)),
            Err(TerminalInterpretError::UnsupportedCapability(_))
        ));
        installation.terminal_psi.program_fingerprint =
            terminal_psi::SemanticFingerprint::from_bytes([1; 32]);
        assert_eq!(
            check(&program, Some(&installation)),
            Err(TerminalInterpretError::ProviderInstallationIdentityMismatch)
        );
    }
}
