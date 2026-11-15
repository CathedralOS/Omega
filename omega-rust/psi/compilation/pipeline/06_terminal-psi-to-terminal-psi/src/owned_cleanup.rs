//! Generated cleanup entries use ordinary invocation and requirement rows.
//! Only the source producer pass mints them; discharge checks the exact rows.

use semantic_vocabulary::InvocationSite;
use terminal_check_tables::edges::{Actual, Callee, InvocationRow};
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::program::Elaboration;
use terminal_check_tables::{
    Checked, Construct, ExactPlaceId, MachineTables, ProgramPoint, RefusalReason,
};
use terminal_psi::{OwnedCleanupInvocation, StructuralArgument, TerminalAffineCleanupAction};

pub(crate) struct Catalog<'a> {
    machines: std::collections::BTreeMap<
        semantic_vocabulary::MachineId,
        &'a terminal_psi::TerminalMachine,
    >,
    types: std::collections::BTreeMap<
        semantic_vocabulary::StructuralTypeId,
        &'a terminal_psi::StructuralTypeDeclaration,
    >,
}
impl<'a> Catalog<'a> {
    pub(crate) fn new(module: &'a terminal_psi::TerminalModule) -> Result<Self, &'static str> {
        let machines = module
            .machines
            .iter()
            .map(|machine| (machine.id, machine))
            .collect::<std::collections::BTreeMap<_, _>>();
        let types = module
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<std::collections::BTreeMap<_, _>>();
        if machines.len() != module.machines.len() || types.len() != module.structural_types.len() {
            return Err("cleanup catalog contains ambiguous declaration identities");
        }
        Ok(Self { machines, types })
    }
    pub(crate) fn target(
        &self,
        cleanup: &OwnedCleanupInvocation,
    ) -> Result<&'a terminal_psi::TerminalMachine, &'static str> {
        let declaration = self
            .types
            .get(&cleanup.structural_type)
            .copied()
            .ok_or("cleanup subject type is absent")?;
        let target = self
            .machines
            .get(&cleanup.cleanup_machine)
            .copied()
            .ok_or("cleanup selected target is absent")?;
        terminal_semantics::nominal_cleanup::owned_target(
            std::iter::once(declaration),
            std::iter::once(target),
            cleanup,
        )
    }
}

pub(crate) fn inputs(
    cleanup: &OwnedCleanupInvocation,
) -> terminal_psi::ModuleInvocationInputs<'_, semantic_vocabulary::TargetMachineCallable> {
    cleanup.module_invocation().declared()
}

fn subject_path(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    argument: &StructuralArgument,
) -> Checked<Vec<terminal_check_tables::bindings::Segment>> {
    let fail = || {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            machine.id,
            Construct::Machine,
            "cleanup actual has no exact inline owned path",
        )
    };
    let root = machine
        .structural_place_type(argument.place)
        .ok_or_else(fail)?;
    terminal_semantics::canonical_inline_region(
        module.structural_types.iter(),
        root,
        &argument.path,
    )
    .ok_or_else(fail)?
    .into_iter()
    .map(|segment| {
        use semantic_vocabulary::CanonicalStructuralPathSegment as C;
        use terminal_check_tables::bindings::Segment;
        match segment {
            C::Field(field) => Ok(Segment::Field(field)),
            C::Case(case) => Ok(Segment::Case(case)),
            C::FixedIndex(index) => Ok(Segment::FixedIndex(index)),
            C::Referent => Err(fail()),
        }
    })
    .collect()
}

pub(crate) fn exact_place(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    tables: &MachineTables,
    argument: &StructuralArgument,
) -> Checked<ExactPlaceId> {
    let fail = || {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            machine.id,
            Construct::Machine,
            "cleanup actual has no exact interned owned place",
        )
    };
    let mut place = *tables.roots.get(argument.place).ok_or_else(fail)?;
    for segment in subject_path(module, machine, argument)? {
        let row = tables.places.get(place).ok_or_else(fail)?;
        place = row
            .children
            .binary_search_by_key(&segment, |(segment, _)| *segment)
            .ok()
            .map(|index| row.children[index].1)
            .ok_or_else(fail)?;
    }
    Ok(place)
}

fn intern_actual(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    tables: &mut MachineTables,
    argument: &StructuralArgument,
) -> Checked<ExactPlaceId> {
    let mut place = *tables.roots.get(argument.place).ok_or_else(|| {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            machine.id,
            Construct::Machine,
            "cleanup actual has no owned root",
        )
    })?;
    for segment in subject_path(module, machine, argument)? {
        place = terminal_check_tables::bindings::intern_projection(
            machine.id,
            &mut tables.places,
            place,
            segment,
        )?;
    }
    Ok(place)
}

/// Source-side completion of generated rows, before private inference. A
/// partially supplied invocation is never repaired; the receiving checker
/// validates the same rows without calling this producer routine.
pub(crate) fn materialize(elaboration: &mut Elaboration) -> Checked<()> {
    let catalog = Catalog::new(&elaboration.module).map_err(|detail| {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            elaboration
                .module
                .machines
                .first()
                .map(|machine| machine.id)
                .unwrap_or_else(|| semantic_vocabulary::MachineId::new(1).expect("one is nonzero")),
            Construct::Machine,
            detail,
        )
    })?;
    let machine_positions = elaboration
        .module
        .machines
        .iter()
        .enumerate()
        .map(|(index, machine)| (machine.id, index))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut pending = Vec::new();
    for machine in &elaboration.module.machines {
        for (block_index, block) in machine.blocks.iter().enumerate() {
            for (edge, actions) in block.terminator.cleanup_rosters() {
                for (ordinal, action) in actions.iter().enumerate() {
                    let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action else {
                        continue;
                    };
                    let ordinal = crate::position(ordinal, machine.id)?;
                    let target = catalog.target(cleanup).map_err(|detail| {
                        crate::refusal(
                            RefusalReason::ForeignIdentity,
                            machine.id,
                            Construct::Edge(edge),
                            detail,
                        )
                    })?;
                    if !target.contract.erased_scalar_formals.is_empty()
                        || !target.contract.erased_proof_formals.is_empty()
                    {
                        return Err(crate::refusal(
                            RefusalReason::Unsupported,
                            machine.id,
                            Construct::Edge(edge),
                            "owned cleanup needs its explicit erased argument correspondence",
                        ));
                    }
                    pending.push((
                        machine.id,
                        block_index,
                        edge,
                        ordinal,
                        cleanup.clone(),
                        target.contract.requires.len(),
                    ));
                }
            }
        }
    }
    drop(catalog);
    let mut site_counts = elaboration.tables.machines.map(|_, tables| {
        let mut counts = std::collections::BTreeMap::new();
        for call in tables.invocations.values() {
            *counts.entry(call.site).or_insert(0usize) += 1;
        }
        counts
    });
    for (id, block, edge, action_ordinal, cleanup, clauses) in pending {
        let site = InvocationSite::Cleanup {
            edge,
            action_ordinal,
        };
        let tables = elaboration.tables.machines.get_mut(id).ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Machine,
                "cleanup caller has no tables",
            )
        })?;
        let existing = site_counts
            .get(id)
            .and_then(|counts| counts.get(&site))
            .copied()
            .unwrap_or(0);
        if existing != 0 {
            if existing != 1 {
                return Err(crate::refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Edge(edge),
                    "cleanup has duplicate invocation rows",
                ));
            }
            continue;
        }
        if !cleanup.requirement_obligations.is_empty() {
            return Err(crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Edge(edge),
                "cleanup requirements have no owning invocation row",
            ));
        }
        let machine_index = *machine_positions.get(&id).ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Machine,
                "cleanup caller is absent",
            )
        })?;
        let caller = &elaboration.module.machines[machine_index];
        let actual = intern_actual(&elaboration.module, caller, tables, &cleanup.subject)?;
        site_counts
            .get_mut(id)
            .expect("the machine was indexed")
            .insert(site, 1);
        let invocation = tables.invocations.push(InvocationRow {
            site,
            callee: Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(
                cleanup.cleanup_machine,
            )),
            required_contract: None,
            actuals: vec![Actual::Place(actual)],
            requirements: Vec::new(),
        });
        let sources = elaboration.sources.get_mut(id).ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Edge(edge),
                "cleanup caller has no source correspondence",
            )
        })?;
        let source = *terminal_check_tables::row(&sources.edges, edge, id, Construct::Edge(edge))?;
        let mut requirements = Vec::new();
        for clause in 0..clauses {
            let obligation = tables.obligations.push(ObligationRow {
                kind: ObligationKind::CallRequirement {
                    invocation,
                    clause: crate::position(clause, id)?,
                },
                created_at: ProgramPoint::BeforeCleanup {
                    edge,
                    action_ordinal,
                },
            });
            if sources.obligations.push(source) != obligation {
                return Err(crate::refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Edge(edge),
                    "cleanup obligation lost its source correspondence",
                ));
            }
            requirements.push(obligation);
        }
        tables
            .invocations
            .get_mut(invocation)
            .expect("the invocation was minted")
            .requirements
            .clone_from(&requirements);
        let caller = &mut elaboration.module.machines[machine_index];
        let actions = actions_mut(&mut caller.blocks[block].terminator, edge).ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Edge(edge),
                "cleanup edge disappeared",
            )
        })?;
        let TerminalAffineCleanupAction::InvokeOwned(actual) =
            &mut actions[action_ordinal as usize]
        else {
            return Err(crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Edge(edge),
                "cleanup owner changed while minting its call",
            ));
        };
        actual.requirement_obligations = requirements;
    }
    materialize_writes(elaboration)?;
    Ok(())
}

fn actions_mut(
    terminator: &mut terminal_psi::Terminator,
    selected: semantic_vocabulary::EdgeId,
) -> Option<&mut Vec<TerminalAffineCleanupAction>> {
    use terminal_psi::Terminator as T;
    match terminator {
        T::Jump {
            edge,
            cleanup_actions,
            ..
        }
        | T::Return {
            edge,
            cleanup_actions,
            ..
        }
        | T::ReturnUnit {
            edge,
            cleanup_actions,
        }
        | T::ReturnStructural {
            edge,
            cleanup_actions,
            ..
        } => (*edge == selected).then_some(cleanup_actions),
        T::Conditional {
            when_true,
            when_false,
            ..
        } => {
            if when_true.edge == selected {
                Some(&mut when_true.cleanup_actions)
            } else if when_false.edge == selected {
                Some(&mut when_false.cleanup_actions)
            } else {
                None
            }
        }
        T::StructuralCase { cases, .. } => cases
            .iter_mut()
            .find(|case| case.edge == selected)
            .map(|case| &mut case.cleanup_actions),
        _ => None,
    }
}

#[derive(Clone)]
pub(crate) struct Write {
    pub edge: semantic_vocabulary::EdgeId,
    pub action_ordinal: u32,
    pub captured: terminal_semantics::descriptor_lifetimes::DescriptorLocation,
    pub backing: terminal_semantics::descriptor_lifetimes::DescriptorLocation,
    pub content: terminal_check_tables::bindings::Content,
}

/// A by-value consumer can write through mutable references its value carries.
/// Without a representable narrower frame, their exact backing is the ordinary
/// ownership ceiling. An unresolved backing is never an empty set of writes.
pub(crate) fn writes(
    module: &terminal_psi::TerminalModule,
    caller: &terminal_psi::TerminalMachine,
    analysis: &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'_>,
) -> Checked<Vec<Write>> {
    use terminal_check_tables::bindings::Content;
    use terminal_semantics::descriptor_lifetimes::descriptor_location;
    use terminal_semantics::descriptor_results::DescriptorSlotKind;
    let mut writes = Vec::new();
    if !caller
        .blocks
        .iter()
        .flat_map(|block| block.terminator.cleanup_rosters())
        .flat_map(|(_, actions)| actions)
        .any(|action| matches!(action, TerminalAffineCleanupAction::InvokeOwned(_)))
    {
        return Ok(writes);
    }
    let catalog = Catalog::new(module).map_err(|detail| {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            caller.id,
            Construct::Machine,
            detail,
        )
    })?;
    for block in &caller.blocks {
        let point = (
            block.id,
            crate::position(block.operations.len(), caller.id)?,
        );
        for (edge, actions) in block.terminator.cleanup_rosters() {
            let fail = |detail| {
                crate::refusal(
                    RefusalReason::Unsupported,
                    caller.id,
                    Construct::Edge(edge),
                    detail,
                )
            };
            for (ordinal, action) in actions.iter().enumerate() {
                let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action else {
                    continue;
                };
                let action_ordinal = crate::position(ordinal, caller.id)?;
                let owner = terminal_psi::InvocationOwner::Cleanup {
                    edge,
                    action_ordinal,
                };
                let root = caller
                    .structural_place_type(cleanup.subject.place)
                    .ok_or_else(|| fail("cleanup subject has no type"))?;
                let subject = descriptor_location(
                    &module.structural_types,
                    cleanup.subject.place,
                    root,
                    &cleanup.subject.path,
                    None,
                )
                .ok_or_else(|| fail("cleanup subject has no exact inline storage"))?;
                writes.push(Write {
                    edge,
                    action_ordinal,
                    captured: subject.clone(),
                    backing: subject,
                    content: Content::Vacated,
                });
                let target = catalog.target(cleanup).map_err(fail)?;
                if target
                    .contract
                    .published_writes
                    .as_ref()
                    .is_some_and(Vec::is_empty)
                {
                    continue;
                }
                let views = DescriptorSlotKind::Views
                    .slots(
                        &module.structural_types,
                        cleanup.subject.place,
                        cleanup.structural_type,
                    )
                    .ok_or_else(|| fail("cleanup has no finite borrowed-payload inventory"))?;
                if !views.is_empty() {
                    return Err(fail(
                        "owned cleanup of view payload needs its exact write correspondence",
                    ));
                }
                let inventory = DescriptorSlotKind::References
                    .inventory(
                        &module.structural_types,
                        cleanup.subject.place,
                        cleanup.structural_type,
                    )
                    .ok_or_else(|| fail("cleanup has no finite reachable-reference inventory"))?;
                for opaque in &inventory.opaque_referents {
                    let path = terminal_semantics::descriptor_lifetimes::descriptor_path(
                        &module.structural_types,
                        cleanup.structural_type,
                        &opaque.path,
                    )
                    .ok_or_else(|| fail("cleanup opaque backing has no typed permission path"))?;
                    if [
                        terminal_psi::StructuralAccess::MutableBorrow,
                        terminal_psi::StructuralAccess::WriteOnlyBorrow,
                    ]
                    .into_iter()
                    .any(|access| {
                        terminal_semantics::reference_source_projection_accessible(
                            module.structural_types.iter(),
                            cleanup.structural_type,
                            terminal_psi::StructuralMultiplicity::Affine,
                            terminal_psi::StructuralAccess::Owned,
                            &path,
                            access,
                        )
                    }) {
                        return Err(fail(
                            "cleanup write ceiling reaches unresolved reference storage",
                        ));
                    }
                }
                for leaf in inventory.slots {
                    let path = terminal_semantics::descriptor_lifetimes::descriptor_path(
                        &module.structural_types,
                        cleanup.structural_type,
                        &leaf.path,
                    )
                    .ok_or_else(|| fail("cleanup reference has no typed permission path"))?;
                    let leaf_type = terminal_semantics::runtime_structural_path_tip(
                        module.structural_types.iter(),
                        cleanup.structural_type,
                        &path,
                    )
                    .ok_or_else(|| fail("cleanup reference has no declared leaf type"))?
                    .id;
                    let declaration = catalog
                        .types
                        .get(&leaf_type)
                        .copied()
                        .ok_or_else(|| fail("cleanup carried reference type is absent"))?;
                    let terminal_psi::StructuralTypeShape::Reference { access, .. } =
                        declaration.shape
                    else {
                        return Err(fail("cleanup carried reference inventory changed type"));
                    };
                    if access == terminal_psi::StructuralAccess::SharedBorrow {
                        continue;
                    }
                    if access == terminal_psi::StructuralAccess::Owned {
                        return Err(fail("cleanup reference leaf has invalid owned access"));
                    }
                    let mut actual_path = cleanup.subject.path.clone();
                    actual_path.extend(path);
                    let mut path = actual_path;
                    path.push(terminal_psi::StructuralPathSegment::Referent);
                    if !terminal_semantics::reference_source_projection_accessible(
                        module.structural_types.iter(),
                        root,
                        terminal_psi::StructuralMultiplicity::Affine,
                        terminal_psi::StructuralAccess::Owned,
                        &path,
                        access,
                    ) {
                        continue;
                    }
                    let captured = descriptor_location(
                        &module.structural_types,
                        cleanup.subject.place,
                        root,
                        &path,
                        None,
                    )
                    .ok_or_else(|| fail("cleanup reference write has no exact source path"))?;
                    let backing = analysis
                        .location_at(caller.id, point, owner, &captured)
                        .ok_or_else(|| {
                            fail("cleanup reference write has no actual owner-prestate backing")
                        })?;
                    writes.push(Write {
                        edge,
                        action_ordinal,
                        captured,
                        backing,
                        content: Content::Overwritten,
                    });
                }
            }
        }
    }
    Ok(writes)
}

fn materialize_writes(elaboration: &mut Elaboration) -> Checked<()> {
    let mut pending = Vec::new();
    for caller in &elaboration.module.machines {
        let graph = crate::places::reference_graph(&elaboration.module, caller.id)?;
        let mut analysis =
            terminal_semantics::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
                &elaboration.module.structural_types,
                &graph,
            );
        pending.push((
            caller.id,
            writes(&elaboration.module, caller, &mut analysis)?,
        ));
    }
    for (id, writes) in pending {
        let tables = elaboration.tables.machines.get_mut(id).ok_or_else(|| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Machine,
                "cleanup writes have no caller tables",
            )
        })?;
        if tables
            .writes
            .values()
            .any(|row| matches!(row.at, ProgramPoint::AfterCleanup { .. }))
        {
            continue;
        }
        for write in writes {
            let mut place = *tables.roots.get(write.captured.root).ok_or_else(|| {
                crate::refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Edge(write.edge),
                    "cleanup write has no actual root",
                )
            })?;
            for segment in write.captured.path {
                place = terminal_check_tables::bindings::intern_projection(
                    id,
                    &mut tables.places,
                    place,
                    segment,
                )?;
            }
            terminal_check_tables::bindings::record_write(
                id,
                &mut tables.places,
                &mut tables.writes,
                terminal_check_tables::bindings::WriteRow {
                    place,
                    at: ProgramPoint::AfterCleanup {
                        edge: write.edge,
                        action_ordinal: write.action_ordinal,
                    },
                    content: write.content,
                },
            )?;
        }
    }
    Ok(())
}

pub(crate) fn edge_entry_points(
    machine: &terminal_psi::TerminalMachine,
) -> std::collections::BTreeMap<semantic_vocabulary::EdgeId, ProgramPoint> {
    machine
        .blocks
        .iter()
        .flat_map(|block| block.terminator.cleanup_rosters())
        .map(|(edge, actions)| {
            (
                edge,
                if actions.is_empty() {
                    ProgramPoint::Edge(edge)
                } else {
                    ProgramPoint::BeforeCleanup {
                        edge,
                        action_ordinal: 0,
                    }
                },
            )
        })
        .collect()
}
