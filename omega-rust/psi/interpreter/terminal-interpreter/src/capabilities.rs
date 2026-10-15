//! Capability decisions on the admitted execution projection, before effects.
//!
//! External completions demand a distinct host delegation roster; the
//! transitional Unit boundary operation is not a terminal invocation and stays
//! unsupported. Reject missing capabilities before any entry operation, rather than after
//! a preceding boundary effect. This is not semantic verification: admission
//! already authenticated these signatures. Walk possible executable calls from
//! the chosen entry, preserving installed providers, so unrelated retained
//! declarations do not impose runtime capabilities.
//! A dynamic call contributes every executable row for its exact interface
//! slot. The selector's runtime value is not available during preflight; neither
//! one representative implementation nor every slot of every table describes
//! this call's possible execution. Empty rows trap without a callee demand.
//! Reference results transport their complete possible backing set in the
//! runtime; source-count is not a missing execution capability. Collect external
//! demands without following them as normal provider calls.

use crate::effects::{AdmittedBoundarySupply, TerminalRuntimeSupplies};
use crate::errors::TerminalInterpretError;
use crate::program::AdmittedTerminalProgram;
use semantic_vocabulary::{BoundaryMachineId, ExternalCompletionKind};
use std::collections::BTreeSet;
use terminal_psi::{OperationKind, Terminator};

/// A precise implementation limitation on an otherwise admitted program.
/// Driver task names and display diagnostics belong to the driver, not Psi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalUnsupportedCapability {
    /// Opaque members require separately admitted execution and result custody.
    TargetMachineMemberSupply {
        head: terminal_psi::TargetMachineCallable,
        callable: terminal_psi::TargetMachineCallable,
        supply: terminal_psi::TargetMachineSupply,
    },
    ExternalCompletionInvocation {
        boundary: BoundaryMachineId,
        kind: ExternalCompletionKind,
    },
}

/// Exact host/provider provisions of the possible execution closure. Unused
/// catalog declarations do not demand bindings; installed provider bodies do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExecutionRequirements {
    pub imports: Vec<BoundaryMachineId>,
    pub missing_providers: Vec<BoundaryMachineId>,
    /// Closed terminal invocations demand host-delegated domain authority,
    /// independently of normal import adapters and provider selection.
    pub external_completions: Vec<TerminalExternalCompletionDemand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalExternalCompletionDemand {
    pub boundary: BoundaryMachineId,
    pub kind: ExternalCompletionKind,
}

pub(crate) fn check(
    program: &AdmittedTerminalProgram,
    installation: Option<&TerminalRuntimeSupplies>,
) -> Result<TerminalExecutionRequirements, TerminalInterpretError> {
    check_with_private_entry_ram(program, installation, &[])
}

pub(crate) fn check_with_private_entry_ram(
    program: &AdmittedTerminalProgram,
    installation: Option<&TerminalRuntimeSupplies>,
    private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
) -> Result<TerminalExecutionRequirements, TerminalInterpretError> {
    let entry = program.resolve_entry(installation)?;
    if private_entry_ram.iter().any(|recipe| {
        installation
            .and_then(|supplies| supplies.target_layouts.as_deref())
            .is_none_or(|layouts| layouts != recipe.layouts())
            || program
                .target_layouts
                .as_deref()
                .is_none_or(|layouts| layouts != recipe.layouts())
    }) {
        return Err(TerminalInterpretError::TargetLayoutSelectionIdentityMismatch);
    }
    if private_entry_ram.iter().any(|recipe| {
        recipe.terminal() != program.identity
            || recipe.input().machine != entry
            || !program.catalogs.placed_view_inputs.contains(recipe.input())
    }) {
        return Err(crate::errors::malformed());
    }
    let mut imports = BTreeSet::new();
    let mut missing_providers = BTreeSet::new();
    let mut placed_candidates = Vec::new();
    let mut external_completion = None;
    let mut external_demands = BTreeSet::new();
    let mut pending = vec![entry];
    let mut visited = BTreeSet::new();
    while let Some(machine_id) = pending.pop() {
        if !visited.insert(machine_id) {
            continue;
        }
        let machine = program.verified_machine(machine_id)?;
        let mut blocks = vec![machine.entry];
        let mut visited_blocks = BTreeSet::new();
        while let Some(block_id) = blocks.pop() {
            if !visited_blocks.insert(block_id) {
                continue;
            }
            let block = machine
                .block(&block_id)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let callee = match &operation.kind {
                    OperationKind::CallDynamic {
                        interface, slot, ..
                    } => {
                        let catalog = &program.catalogs.dynamic_dispatch;
                        let slot_index =
                            usize::try_from(*slot).map_err(|_| crate::errors::malformed())?;
                        if catalog
                            .interface(*interface)
                            .and_then(|interface| interface.slots.get(slot_index))
                            .is_none()
                        {
                            return Err(crate::errors::malformed());
                        }
                        for table in &catalog.tables {
                            if table.interface == *interface {
                                let row = table
                                    .rows
                                    .get(slot_index)
                                    .ok_or_else(crate::errors::malformed)?;
                                pending.extend(*row);
                            }
                        }
                        None
                    }
                    OperationKind::BoundaryCall { boundary, .. } => {
                        if program.is_target_family_head(*boundary) {
                            let member = crate::calls::target_families::selected_member(
                                program,
                                installation
                                    .and_then(|installation| installation.target_machines.as_ref()),
                                *boundary,
                            )?;
                            if let terminal_psi::TargetMachineCallable::Machine(member) = member {
                                crate::calls::target_families::validate_result_transport(
                                    operation,
                                    program.verified_machine(member)?,
                                )?;
                                pending.push(member);
                                continue;
                            }
                            // An opaque member contributes the head's exact
                            // provider or host demand, never an executable body
                            // or an adapter supplied by catalog membership.
                        }
                        let declaration = program.verified_boundary_machine(boundary)?;
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
                        if installation
                            .and_then(|installation| installation.supplies.get(boundary))
                            .is_none()
                            && let OperationKind::BoundaryCall {
                                structural_arguments,
                                ..
                            } = &operation.kind
                        {
                            placed_candidates.push((
                                machine_id,
                                (
                                    block.id,
                                    u32::try_from(operation_index).map_err(|_| {
                                        TerminalInterpretError::VerifiedOperationMalformed
                                    })?,
                                ),
                                *boundary,
                                structural_arguments.clone(),
                            ));
                        }
                        match installation
                            .and_then(|installation| installation.supplies.get(boundary))
                        {
                            Some(AdmittedBoundarySupply::CheckedProvider(selected)) => {
                                Some(*selected)
                            }
                            Some(AdmittedBoundarySupply::HostImport) => {
                                imports.insert(*boundary);
                                None
                            }
                            None if program.has_provider(boundary) => {
                                missing_providers.insert(*boundary);
                                None
                            }
                            None => {
                                imports.insert(*boundary);
                                None
                            }
                        }
                    }
                    kind => kind.direct_callee(),
                };
                if let Some(callee) = callee {
                    pending.push(callee);
                }
            }
            match &block.terminator {
                Terminator::ExternalCompletion { boundary, kind, .. } => {
                    external_demands.insert(TerminalExternalCompletionDemand {
                        boundary: *boundary,
                        kind: *kind,
                    });
                }
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
                    .filter_map(|action| action.invoked_machine()),
            );
        }
    }
    // Keep installed-provider selection first. Only a complete entry-relative
    // formation proof discharges the adapter obligation for an internal RAM
    // handler. Host, mixed and unresolved alternatives remain in the roster.
    if !placed_candidates.is_empty() {
        let graph =
            terminal_semantics::descriptor_results::terminal_placed_origin_graph(&program.catalogs);
        if let Some(graph) = graph {
            let mut origins = terminal_semantics::PlacedRamOriginAnalysis::new(
                &program.catalogs,
                &graph,
                &visited,
                entry,
            )
            .with_private_entry_roots(
                private_entry_ram
                    .iter()
                    .map(|recipe| recipe.input().clone()),
            );
            let mut internally_bound = BTreeSet::new();
            let mut unresolved = BTreeSet::new();
            for (machine, position, boundary, arguments) in placed_candidates {
                let formations =
                    origins.boundary_formations(machine, position, boundary, &arguments);
                if formations.is_some() {
                    internally_bound.insert(boundary);
                } else {
                    unresolved.insert(boundary);
                }
            }
            for boundary in internally_bound.difference(&unresolved) {
                missing_providers.remove(boundary);
                imports.remove(boundary);
            }
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
        external_completions: external_demands.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    // These tests exercise the projection graph algorithm, not program admission.
    // End-to-end acceptance uses genuinely produced artifacts in the corpus.
    use super::*;
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, PlaceId, StructuralTypeId};
    use std::collections::BTreeMap;
    use terminal_psi::{
        Block, Operation, OperationResult, StructuralAccess, StructuralArgument,
        StructuralMultiplicity, StructuralReferenceResultSource, StructuralResultDeclaration,
        TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult, TerminalModule,
    };
    fn machine(selected_origins: usize, operations: Vec<OperationKind>) -> TerminalMachine {
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
                        .map(Into::into)
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
        TerminalMachine {
            id: MachineId::new(1).unwrap(),
            attachment: None,
            declaration: None,
            nominal_cleanup_subject: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_entry_claims: Vec::new(),
            result,
            entry: block_id,
            blocks: vec![block],
            ranked_scc: None,
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            contract: terminal_psi::MachineContract::empty(
                semantic_vocabulary::ContractId::new(1).unwrap(),
            ),
        }
    }

    fn program() -> AdmittedTerminalProgram<'static> {
        let entry = MachineId::new(1).unwrap();
        let mut module = TerminalModule::for_entry(entry);
        module.machines.push(machine(0, Vec::new()));
        let identity = terminal_codec::terminal_psi_identity(&module).unwrap();
        let mut program = AdmittedTerminalProgram {
            identity,
            catalogs: std::sync::Arc::new(std::borrow::Cow::Owned(module)),
            target_layouts: None,
        };
        set_machine(&mut program, entry, machine(1, Vec::new()));
        let boundary = BoundaryMachineId::new(1).unwrap();
        let catalogs = std::sync::Arc::make_mut(&mut program.catalogs).to_mut();
        let declaration = boundary_declaration(catalogs, boundary);
        catalogs.boundary_machines.push(declaration);
        program
    }

    // Graph fixtures deliberately omit unrelated typing evidence. Keep their
    // mutation in test setup; production programs expose no mutable declarations.
    fn set_machine(
        program: &mut AdmittedTerminalProgram<'_>,
        identity: MachineId,
        mut machine: TerminalMachine,
    ) {
        machine.id = identity;
        let machines = &mut std::sync::Arc::make_mut(&mut program.catalogs)
            .to_mut()
            .machines;
        match machines.binary_search_by_key(&identity, |machine| machine.id) {
            Ok(position) => machines[position] = machine,
            Err(position) => machines.insert(position, machine),
        }
    }

    // Fixture declarations intern authored identities through the module
    // table the same way the producer does, so identity joins stay index-true.
    fn intern_identity(
        catalogs: &mut terminal_psi::TerminalModule,
        text: &str,
    ) -> terminal_psi::TerminalIdentityIndex {
        let mut builder = std::mem::take(&mut catalogs.identities).into_builder();
        let index = builder.intern(text);
        catalogs.identities = builder.snapshot();
        index
    }

    fn add_provider(program: &mut AdmittedTerminalProgram<'_>, boundary: BoundaryMachineId) {
        let catalogs = std::sync::Arc::make_mut(&mut program.catalogs).to_mut();
        let requirement_identity = intern_identity(catalogs, "graph boundary");
        let provider_identity = intern_identity(catalogs, "graph provider");
        let candidate_identity = intern_identity(catalogs, "graph candidate");
        catalogs
            .provider_candidates
            .push(terminal_psi::ProviderCandidateConformance {
                boundary,
                requirement_identity,
                source_requirement_identity: requirement_identity,
                receiving_application: terminal_psi::TerminalIdentityIndex::default(),
                receiving_region_arguments: Vec::new(),
                provider_identity,
                provider_declaration: terminal_psi::NominalDeclarationIdentity::new(
                    provider_identity,
                ),
                candidate_identity,
                source_candidate_identity: candidate_identity,
                source_candidate_package_identity: None,
                candidate: MachineId::new(2).unwrap(),
                signature: terminal_psi::ProviderSignature::empty(),
                refinement: terminal_psi::ProviderRefinement::empty(),
            });
    }

    fn boundary_declaration(
        catalogs: &mut terminal_psi::TerminalModule,
        boundary: BoundaryMachineId,
    ) -> terminal_psi::BoundaryMachineDeclaration {
        terminal_psi::BoundaryMachineDeclaration {
            declaration: None,
            role: terminal_psi::BoundaryDeclarationRole::Callable,
            id: boundary,
            kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: intern_identity(catalogs, "graph boundary"),
            owner: terminal_psi::NominalDeclarationIdentity::new(intern_identity(
                catalogs,
                "graph schema",
            )),
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
            owning_service: None,
            published_service_ceiling: Vec::new(),
        }
    }

    fn install_process_exit_caller(
        program: &mut AdmittedTerminalProgram<'_>,
        caller: u64,
    ) -> BoundaryMachineId {
        let boundary = BoundaryMachineId::new(1).unwrap();
        mark_boundary_external_completion(program, boundary, ExternalCompletionKind::ProcessExit);
        set_machine(
            program,
            MachineId::new(caller).unwrap(),
            machine(0, vec![boundary_call(boundary)]),
        );
        boundary
    }

    fn mark_boundary_external_completion(
        program: &mut AdmittedTerminalProgram<'_>,
        boundary: BoundaryMachineId,
        kind: ExternalCompletionKind,
    ) {
        std::sync::Arc::make_mut(&mut program.catalogs)
            .to_mut()
            .boundary_machines
            .iter_mut()
            .find(|declaration| declaration.id == boundary)
            .unwrap()
            .completion = terminal_psi::BoundaryCompletion::External(kind);
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
        let boundary = install_process_exit_caller(&mut program, 1);
        add_provider(&mut program, boundary);
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
        install_process_exit_caller(&mut program, 2);
        assert!(check(&program, None).unwrap().imports.is_empty());
    }

    #[test]
    fn capability_graph_missing_declaration_remains_malformed() {
        let mut program = program();
        let boundary = BoundaryMachineId::new(2).unwrap();
        set_machine(
            &mut program,
            MachineId::new(1).unwrap(),
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
    fn capability_graph_selected_backing_keeps_external_completion_demand() {
        let mut program = program();
        let boundary = BoundaryMachineId::new(1).unwrap();
        let selected = MachineId::new(2).unwrap();
        mark_boundary_external_completion(
            &mut program,
            boundary,
            ExternalCompletionKind::ProcessExit,
        );
        set_machine(
            &mut program,
            MachineId::new(1).unwrap(),
            machine(0, vec![boundary_call(boundary), unit_call(selected)]),
        );
        set_machine(&mut program, selected, machine(2, Vec::new()));
        assert!(
            matches!(check(&program, None), Err(TerminalInterpretError::UnsupportedCapability(
            TerminalUnsupportedCapability::ExternalCompletionInvocation { boundary: demanded, .. }
        )) if demanded == boundary)
        );
    }

    #[test]
    fn capability_graph_external_completion_does_not_hide_malformed_closure() {
        let mut program = program();
        let external = BoundaryMachineId::new(1).unwrap();
        let missing = BoundaryMachineId::new(2).unwrap();
        std::sync::Arc::make_mut(&mut program.catalogs)
            .to_mut()
            .boundary_machines
            .iter_mut()
            .find(|declaration| declaration.id == external)
            .unwrap()
            .completion =
            terminal_psi::BoundaryCompletion::External(ExternalCompletionKind::ProcessExit);
        set_machine(
            &mut program,
            MachineId::new(1).unwrap(),
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

    fn dynamic_call(interface: u32, slot: u32) -> OperationKind {
        OperationKind::CallDynamic {
            selector: semantic_vocabulary::ValueId::new(1).unwrap(),
            interface,
            slot,
            arguments: Vec::new(),
            family_actuals: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    }

    fn dynamic_program() -> AdmittedTerminalProgram<'static> {
        let mut program = program();
        let catalogs = std::sync::Arc::make_mut(&mut program.catalogs).to_mut();
        let slot =
            |catalogs: &mut terminal_psi::TerminalModule, slot| terminal_psi::TerminalDynamicSlot {
                slot,
                declaring_trait_identity: intern_identity(catalogs, "graph interface"),
                requirement_identity: intern_identity(catalogs, &format!("requirement {slot}")),
                family_tuple: Vec::new(),
                scalar_parameters: Vec::new(),
                structural_parameters: Vec::new(),
                entry_claim_paths: Vec::new(),
                result: terminal_psi::BoundaryMachineResult::Unit,
                result_claim_paths: Vec::new(),
                result_reference_regions: Vec::new(),
            };
        catalogs.dynamic_dispatch.interfaces = (0..2)
            .map(|id| terminal_psi::TerminalDynamicInterface {
                id,
                identity: intern_identity(catalogs, &format!("interface {id}")),
                slots: vec![slot(catalogs, 0), slot(catalogs, 1)],
            })
            .collect();
        catalogs.dynamic_dispatch.tables = vec![
            terminal_psi::TerminalDynamicTable {
                id: 0,
                interface: 0,
                implementation_identity: intern_identity(catalogs, "first implementation"),
                rows: vec![None, Some(MachineId::new(3).unwrap())],
            },
            terminal_psi::TerminalDynamicTable {
                id: 1,
                interface: 0,
                implementation_identity: intern_identity(catalogs, "second implementation"),
                rows: vec![Some(MachineId::new(2).unwrap()), None],
            },
            terminal_psi::TerminalDynamicTable {
                id: 2,
                interface: 1,
                implementation_identity: intern_identity(catalogs, "unrelated implementation"),
                rows: vec![Some(MachineId::new(3).unwrap()), None],
            },
        ];
        set_machine(
            &mut program,
            MachineId::new(1).unwrap(),
            machine(0, vec![dynamic_call(0, 0)]),
        );
        // A recursive row must terminate the closure walk. Missing declarations
        // in an unused slot or another interface must not become demands.
        set_machine(
            &mut program,
            MachineId::new(2).unwrap(),
            machine(0, vec![dynamic_call(0, 0)]),
        );
        set_machine(
            &mut program,
            MachineId::new(3).unwrap(),
            machine(0, vec![boundary_call(BoundaryMachineId::new(2).unwrap())]),
        );
        program
    }

    #[test]
    fn capability_graph_dynamic_call_demands_exact_slot_imports_and_providers() {
        let mut program = dynamic_program();
        let boundary = BoundaryMachineId::new(1).unwrap();
        set_machine(
            &mut program,
            MachineId::new(2).unwrap(),
            machine(0, vec![dynamic_call(0, 0), boundary_call(boundary)]),
        );
        assert_eq!(check(&program, None).unwrap().imports, vec![boundary]);
        add_provider(&mut program, boundary);
        assert_eq!(
            check(&program, None).unwrap().missing_providers,
            vec![boundary]
        );
        let provider = MachineId::new(4).unwrap();
        let provider_import = BoundaryMachineId::new(2).unwrap();
        let catalogs = std::sync::Arc::make_mut(&mut program.catalogs).to_mut();
        let declaration = boundary_declaration(catalogs, provider_import);
        catalogs.boundary_machines.push(declaration);
        set_machine(
            &mut program,
            provider,
            machine(0, vec![boundary_call(provider_import)]),
        );
        let installation = TerminalRuntimeSupplies {
            terminal_psi: program.identity,
            entry: None,
            supplies: BTreeMap::from([(
                boundary,
                AdmittedBoundarySupply::CheckedProvider(provider),
            )]),
            assumed_bindings: Vec::new(),
            target_machines: None,
            target_layouts: None,
        };
        assert_eq!(
            check(&program, Some(&installation)),
            Ok(TerminalExecutionRequirements {
                imports: vec![provider_import],
                missing_providers: Vec::new(),
                external_completions: Vec::new(),
            })
        );
    }

    #[test]
    fn capability_graph_dynamic_call_refuses_external_completion_in_later_table() {
        let mut program = dynamic_program();
        std::sync::Arc::make_mut(&mut program.catalogs)
            .to_mut()
            .dynamic_dispatch
            .tables[0]
            .rows[0] = Some(program.resolve_entry(None).unwrap());
        let boundary = install_process_exit_caller(&mut program, 2);
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
    fn capability_graph_dynamic_call_demands_closed_external_completion_in_later_table() {
        let mut program = dynamic_program();
        let boundary = BoundaryMachineId::new(1).unwrap();
        let kind = ExternalCompletionKind::ProcessExit;
        std::sync::Arc::make_mut(&mut program.catalogs)
            .to_mut()
            .boundary_machines
            .iter_mut()
            .find(|declaration| declaration.id == boundary)
            .unwrap()
            .completion = terminal_psi::BoundaryCompletion::External(kind);
        let mut external = machine(0, Vec::new());
        external.blocks.first_mut().unwrap().terminator = Terminator::ExternalCompletion {
            edge: EdgeId::new(2).unwrap(),
            boundary,
            kind,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            frontier_lower_bound: Vec::new(),
        };
        set_machine(&mut program, MachineId::new(2).unwrap(), external);
        assert_eq!(
            check(&program, None),
            Ok(TerminalExecutionRequirements {
                imports: Vec::new(),
                missing_providers: Vec::new(),
                external_completions: vec![TerminalExternalCompletionDemand { boundary, kind }],
            })
        );
    }

    #[test]
    fn capability_graph_dynamic_call_ignores_trap_and_unreachable_rows() {
        let program = dynamic_program();
        assert_eq!(
            check(&program, None),
            Ok(TerminalExecutionRequirements {
                imports: Vec::new(),
                missing_providers: Vec::new(),
                external_completions: Vec::new(),
            })
        );
    }

    fn import_cleanup_program() -> (
        AdmittedTerminalProgram<'static>,
        TerminalAffineCleanupAction,
    ) {
        let mut program = program();
        let cleanup_machine = MachineId::new(2).unwrap();
        set_machine(
            &mut program,
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
                retains_descendants: false,
                place: PlaceId::new(1).unwrap(),
                path: Vec::new(),
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
        let entry = std::sync::Arc::make_mut(&mut program.catalogs)
            .to_mut()
            .machines
            .first_mut()
            .unwrap();
        let successor = BlockId::new(2).unwrap();
        let mut destination = entry.blocks[0].clone();
        destination.id = successor;
        entry.blocks.push(destination);
        entry.blocks.first_mut().unwrap().terminator = Terminator::Jump {
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
        let entry = std::sync::Arc::make_mut(&mut program.catalogs)
            .to_mut()
            .machines
            .first_mut()
            .unwrap();
        let mut unreachable = entry.blocks[0].clone();
        unreachable.id = BlockId::new(2).unwrap();
        unreachable.terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            cleanup_actions: vec![action],
        };
        entry.blocks.push(unreachable);
        assert!(check(&program, None).unwrap().imports.is_empty());
    }

    #[test]
    fn capability_graph_ignores_unused_selected_result() {
        let mut program = program();
        set_machine(
            &mut program,
            MachineId::new(2).unwrap(),
            machine(2, Vec::new()),
        );
        assert_eq!(check(&program, None).unwrap().imports, Vec::new());
    }

    #[test]
    fn capability_graph_reaches_called_selected_result() {
        let mut program = program();
        let selected = MachineId::new(2).unwrap();
        set_machine(
            &mut program,
            MachineId::new(1).unwrap(),
            machine(0, vec![unit_call(selected)]),
        );
        set_machine(&mut program, selected, machine(2, Vec::new()));
        assert_eq!(check(&program, None).unwrap().imports, Vec::new());
    }

    #[test]
    fn capability_graph_does_not_demand_a_crashed_result() {
        let mut program = program();
        let selected = MachineId::new(2).unwrap();
        set_machine(
            &mut program,
            MachineId::new(1).unwrap(),
            machine(0, vec![unit_call(selected)]),
        );
        let mut never_returns = machine(2, Vec::new());
        never_returns.blocks.first_mut().unwrap().terminator = Terminator::Crash {
            edge: EdgeId::new(1).unwrap(),
            cause: terminal_psi::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        };
        set_machine(&mut program, selected, never_returns);
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
        set_machine(
            &mut program,
            MachineId::new(1).unwrap(),
            machine(0, vec![kind]),
        );
        assert_eq!(check(&program, None).unwrap().imports, vec![boundary]);
        add_provider(&mut program, boundary);
        let requirements = check(&program, None).unwrap();
        assert!(requirements.imports.is_empty());
        assert_eq!(requirements.missing_providers, vec![boundary]);
        set_machine(&mut program, target, machine(2, Vec::new()));
        let mut installation = TerminalRuntimeSupplies {
            terminal_psi: program.identity,
            entry: None,
            supplies: BTreeMap::from([(boundary, AdmittedBoundarySupply::CheckedProvider(target))]),
            assumed_bindings: Vec::new(),
            target_machines: None,
            target_layouts: None,
        };
        // Multi-source result transport no longer adds a capability demand.
        // Following the installed provider satisfies this graph's missing
        // provider requirement; the source corpus exercises runtime transport.
        assert_eq!(
            check(&program, Some(&installation)).unwrap(),
            TerminalExecutionRequirements {
                imports: Vec::new(),
                missing_providers: Vec::new(),
                external_completions: Vec::new(),
            }
        );
        installation.terminal_psi.program_fingerprint =
            terminal_psi::SemanticFingerprint::from_bytes([1; 32]);
        assert_eq!(
            check(&program, Some(&installation)),
            Err(TerminalInterpretError::ProviderInstallationIdentityMismatch)
        );
    }
}
