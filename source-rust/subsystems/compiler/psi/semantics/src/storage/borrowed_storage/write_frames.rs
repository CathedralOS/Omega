//! Infer which parameter fields a machine can write and check any frame it publishes.

use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorGraphError, DescriptorInvocationTarget, DescriptorMachine,
    terminal_flow,
};
use arena::IdTable;
use semantic_vocabulary::MachineId;

/// A bodyless contract belongs to the program. Every error in a body names its machine.
#[derive(Debug)]
pub struct WriteFrameError {
    pub machine: Option<MachineId>,
    pub detail: &'static str,
}

impl WriteFrameError {
    fn at(machine: MachineId, detail: &'static str) -> Self {
        Self {
            machine: Some(machine),
            detail,
        }
    }
}

impl std::fmt::Display for WriteFrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.detail)?;
        if let Some(machine) = self.machine {
            write!(f, " in {machine:?}")?;
        }
        Ok(())
    }
}

/// An unreadable body cannot publish a precise write frame. Independent bodies
/// can still publish the paths they write through their parameters.
pub fn inferred_write_frames(
    module: &terminal_psi::TerminalModule,
) -> Result<IdTable<MachineId, terminal_psi::PublishedWriteFrame>, WriteFrameError> {
    derive_write_frames(module, &|_| Ok(()))
}

fn derive_write_frames(
    module: &terminal_psi::TerminalModule,
    unreadable: &dyn Fn(DescriptorGraphError) -> Result<(), WriteFrameError>,
) -> Result<IdTable<MachineId, terminal_psi::PublishedWriteFrame>, WriteFrameError> {
    let mut frames = IdTable::default();
    for machine in &module.machines {
        if frames.push(None) != machine.id {
            return Err(WriteFrameError::at(
                machine.id,
                "write-frame machines are not at their declared positions",
            ));
        }
    }
    let mut graph = Vec::new();
    for row in write_frame_rows(module) {
        match row {
            Ok(body) => graph.push(body),
            Err(error @ DescriptorGraphError::IncompleteMachine { .. }) => unreadable(error)?,
            Err(DescriptorGraphError::MissingOrAmbiguousMachine(machine)) => {
                return Err(WriteFrameError::at(
                    machine,
                    "write frame has no unique machine",
                ));
            }
        }
    }
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.types.structural_types, &graph);
    for body in &graph {
        let machine = frame_owner(module, body.id)?;
        let frame = machine_write_frame(machine, &mut analysis)
            .map_err(|detail| WriteFrameError::at(machine.id, detail))?;
        *frames
            .get_mut(machine.id)
            .ok_or_else(|| WriteFrameError::at(machine.id, "write-frame owner disappeared"))? =
            frame;
        analysis.release_storage_snapshots();
    }
    Ok(frames)
}

// Frame inference visits every body in its checked table order. A dynamic
// caller resolves stored selectors from actual writes and unchanged storage
// in the initial rows. Both passes keep those reads, writes and edge bindings.
fn write_frame_rows(
    module: &terminal_psi::TerminalModule,
) -> Vec<Result<DescriptorMachine, DescriptorGraphError>> {
    let roster: Vec<_> = module.machines.iter().collect();
    let zeroed = crate::module_analyses::ZeroedCases::new(&module.types.structural_types);
    let tracks_native_addresses = crate::storage::native_borrows::tracks_native_addresses(module);
    let adapt = |body, selectors: Option<&[DescriptorMachine]>| {
        terminal_flow::adapt(
            tracks_native_addresses,
            module,
            &roster,
            body,
            terminal_flow::RowPurpose::WriteFrame,
            &[],
            selectors,
            Default::default(),
            &zeroed,
        )
    };
    let mut rows: Vec<_> = roster.iter().map(|body| adapt(body, None)).collect();
    // The closure also refuses a known callee absent from the module. The
    // caller can publish an effects body or multiple possible realizations.
    let mut missing = None;
    let mut check_target = |id: &MachineId| {
        if id.get() > roster.len() as u64 {
            missing = Some(missing.map_or(*id, |old: MachineId| old.min(*id)));
        }
    };
    for row in rows.iter().flatten() {
        for (_, call) in row.known_invocations() {
            match &call.target {
                DescriptorInvocationTarget::Machine(id) => check_target(id),
                DescriptorInvocationTarget::Public {
                    effects_machine,
                    realizations,
                    ..
                } => {
                    effects_machine
                        .iter()
                        .chain(realizations)
                        .for_each(&mut check_target);
                }
                DescriptorInvocationTarget::Provider { .. }
                | DescriptorInvocationTarget::PrepareWorker { .. } => {}
            }
        }
    }
    if let Some(id) = missing {
        rows.push(Err(DescriptorGraphError::MissingOrAmbiguousMachine(id)));
    }
    // Direct and forwarded selectors resolve on the first pass. The stored
    // selector lookup starts with a scalar read in the calling body.
    let callers: Vec<_> = rows
        .iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, row)| {
            let body = roster[row.id.get() as usize - 1];
            (!row.scalar_payload_reads.is_empty()
                && body
                    .operations()
                    .any(|op| matches!(op.kind, terminal_psi::OperationKind::CallDynamic { .. })))
            .then_some((index, body))
        })
        .collect();
    if callers.is_empty() {
        return rows;
    }
    let mut initial = Vec::new();
    let mut failures = Vec::new();
    for row in rows {
        match row {
            Ok(row) => initial.push(row),
            Err(error) => failures.push(Err(error)),
        }
    }
    let replacements: Vec<_> = callers
        .into_iter()
        .map(|(index, body)| (index, adapt(body, Some(&initial))))
        .collect();
    let mut rows: Vec<_> = initial.into_iter().map(Ok).chain(failures).collect();
    for (index, replacement) in replacements {
        rows[index] = replacement;
    }
    rows
}

// Frame inference checked every machine ID against its table position before reading bodies.
fn frame_owner(
    module: &terminal_psi::TerminalModule,
    id: MachineId,
) -> Result<&terminal_psi::TerminalMachine, WriteFrameError> {
    usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| module.machines.get(position))
        .ok_or_else(|| WriteFrameError::at(id, "write frame has no source machine"))
}

fn machine_write_frame(
    machine: &terminal_psi::TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
) -> Result<terminal_psi::PublishedWriteFrame, &'static str> {
    use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;

    let Some(written) = analysis.write_frame(machine.id)? else {
        return Ok(None);
    };
    let mut paths = Vec::with_capacity(written.len());
    for target in written {
        use crate::descriptors::replay::StorageWriteTarget;
        let StorageWriteTarget::Storage(region) = target else {
            let StorageWriteTarget::NativeAddress(formal) = target else {
                return Ok(None);
            };
            let scalar = machine
                .parameters
                .iter()
                .position(|parameter| parameter.id == formal)
                .ok_or("native write frame has no corresponding scalar formal")?;
            let ordinal = machine
                .structural_parameters
                .len()
                .checked_add(scalar)
                .and_then(|index| u32::try_from(index).ok())
                .ok_or("native write formal exceeds ordinal range")?;
            paths.push((ordinal, Vec::new()));
            continue;
        };
        let mut parameters = machine
            .structural_parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.place == region.root);
        let (position, _) = parameters
            .next()
            .ok_or("write frame names storage outside its structural formals")?;
        if parameters.next().is_some() {
            return Err("write frame has an ambiguous structural formal");
        }
        let position = u32::try_from(position)
            .map_err(|_| "write-frame formal ordinal cannot be represented")?;
        let mut fields = Vec::with_capacity(region.path.len());
        for segment in &region.path {
            let DescriptorPathSegment::Field(field) = segment else {
                return Err("published write frame contains a non-field step");
            };
            fields.push(*field);
        }
        paths.push((position, fields));
    }
    paths.sort_unstable();
    paths.dedup();
    Ok(Some(paths))
}

/// A supplied frame is usable only when the canonical body derives it.
/// Omitting a summary keeps the ceiling and grants no precision. A
/// bodyless header cannot invent an empty or narrowed write promise.
pub fn validate_write_frames(module: &terminal_psi::TerminalModule) -> Result<(), WriteFrameError> {
    if module
        .execution
        .declaration_contracts
        .iter()
        .any(|contract| contract.published_writes.is_some())
    {
        return Err(WriteFrameError {
            machine: None,
            detail: "a bodyless contract claims an inferred write frame",
        });
    }
    if module
        .machines
        .iter()
        .all(|machine| machine.contract.published_writes.is_none())
    {
        return Ok(());
    }
    let inferred = derive_write_frames(module, &|error| {
        let machine = error.machine();
        if frame_owner(module, machine)?
            .contract
            .published_writes
            .is_some()
        {
            return Err(WriteFrameError::at(
                machine,
                error.detail().unwrap_or(
                    "cannot read the storage used by a body with a published write frame",
                ),
            ));
        }
        Ok(())
    })?;
    for machine in &module.machines {
        let Some(published) = &machine.contract.published_writes else {
            continue;
        };
        let Some(writes) = inferred.get(machine.id).and_then(Option::as_ref) else {
            return Err(WriteFrameError::at(
                machine.id,
                "cannot infer the writes of a body with a published write frame",
            ));
        };
        if writes != published {
            return Err(WriteFrameError::at(
                machine.id,
                "published write frame differs from its body",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        IeeeFloatFormat, Proposition, ScalarQualificationSetId, ScalarType, ValueId,
    };
    use terminal_psi::{TerminalModule, ValueDeclaration};

    #[test]
    fn a_stored_dynamic_selector_uses_the_actual_write_in_both_row_modes() {
        use crate::descriptor_results::{DescriptorSlotKind, terminal_graph_rows};
        use crate::module_analyses::tests::machine;
        use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId};
        use terminal_psi::{
            BoundaryMachineResult, Operation, OperationKind, OperationResult,
            StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
            StructuralTypeDeclaration, StructuralTypeShape, TerminalDynamicInterface,
            TerminalDynamicSlot, TerminalDynamicTable, TerminalIdentityIndex,
        };
        let callee = machine(2, vec![]);
        let callee_id = callee.id;
        let place = PlaceId::new(1).unwrap();
        let ty = StructuralTypeId::new(1).unwrap();
        let scalar = |id| ValueDeclaration {
            id: ValueId::new(id).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: ScalarQualificationSetId::ZERO,
        };
        let op =
            |id, result, kind| Operation::unsuspended(OperationId::new(id).unwrap(), result, kind);
        let mut body = machine(
            1,
            vec![
                op(
                    1,
                    OperationResult::Scalar(scalar(1)),
                    OperationKind::SelectImplementation { table: 0 },
                ),
                op(
                    2,
                    OperationResult::Structural(StructuralOperationResult {
                        place,
                        structural_type: ty,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: vec![],
                        projected_qualifications: vec![],
                        additional_input_qualifications: vec![],
                        qualification_establishments: vec![],
                        claims: vec![],
                    }),
                    OperationKind::EstablishPrimitiveLocal {
                        value: scalar(1).id,
                        range_obligation: None,
                    },
                ),
                op(
                    3,
                    OperationResult::Scalar(scalar(2)),
                    OperationKind::PrimitiveScalarRead {
                        source: place,
                        path: vec![],
                    },
                ),
                op(
                    4,
                    OperationResult::Unit,
                    OperationKind::CallDynamic {
                        selector: scalar(2).id,
                        interface: 0,
                        slot: 0,
                        arguments: vec![],
                        family_actuals: vec![],
                        erased_arguments: vec![],
                        erased_proof_arguments: vec![],
                        structural_arguments: vec![],
                        claim_transfers: vec![],
                        requirement_obligations: vec![],
                        crash_continuations: vec![],
                    },
                ),
            ],
        );
        body.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(2).unwrap(),
                structural_type: ty,
            },
        });
        let mut module = TerminalModule::for_entry(body.id);
        module.machines = vec![body, callee];
        module
            .types
            .structural_types
            .push(StructuralTypeDeclaration {
                id: ty,
                identity: TerminalIdentityIndex::ABSENT,
                declaration_identity: Default::default(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                multiplicity: StructuralMultiplicity::Unrestricted,
                requires: vec![],
                quotient_carrier: None,
                owns_linear_claim: false,
                descriptor_cleanup: true,
                no_code_affine_cleanup: true,
                nominal_cleanup: None,
                scalar_restriction: None,
            });
        let dispatch = &mut module.execution.dynamic_dispatch;
        dispatch.interfaces.push(TerminalDynamicInterface {
            id: 0,
            identity: TerminalIdentityIndex(7),
            slots: vec![TerminalDynamicSlot {
                slot: 0,
                declaring_trait_identity: TerminalIdentityIndex(7),
                requirement_identity: TerminalIdentityIndex(8),
                family_tuple: vec![],
                scalar_parameters: vec![],
                structural_parameters: vec![],
                entry_claim_paths: vec![],
                result: BoundaryMachineResult::Unit,
                result_claim_paths: vec![],
                result_reference_regions: vec![],
            }],
        });
        dispatch.tables.push(TerminalDynamicTable {
            id: 0,
            interface: 0,
            implementation_identity: TerminalIdentityIndex(9),
            rows: vec![Some(callee_id)],
        });
        for expected in [Some(callee_id), None] {
            if expected.is_none() {
                module.machines[0].blocks[0].operations[0].kind =
                    OperationKind::BooleanConstant { value: true };
            }
            let full = terminal_graph_rows(
                &module,
                module.machines.iter().map(|body| body.id),
                DescriptorSlotKind::BorrowedStorage,
                &[],
            );
            let frames = write_frame_rows(&module);
            for rows in [&full, &frames] {
                let row = rows[0].as_ref().unwrap();
                let (_, call) = row.known_invocations().next().unwrap();
                let DescriptorInvocationTarget::Public {
                    effects_machine, ..
                } = &call.target
                else {
                    panic!("expected a dynamic call")
                };
                assert_eq!(*effects_machine, expected);
            }
            assert!(frames[0].as_ref().unwrap().scalar_payload_writes.is_empty());
        }
    }

    #[test]
    fn omitting_scalar_premises_still_refuses_conflicting_formal_types() {
        let mut body = crate::module_analyses::tests::machine(1, Vec::new());
        let id = body.id;
        let formal = ValueDeclaration {
            id: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: ScalarQualificationSetId::ZERO,
        };
        body.parameters.push(formal);
        body.erased_scalar_formals.push(formal);
        body.requires.push(Proposition::Truth);
        body.published_writes = Some(Vec::new());
        let mut module = TerminalModule::for_entry(id);
        module.machines.push(body);
        assert!(validate_write_frames(&module).is_ok());
        let row = write_frame_rows(&module).pop().unwrap().unwrap();
        assert!(row.payload_entry_predicates.is_none());

        module.machines[0].erased_scalar_formals[0].scalar_type =
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
        assert_eq!(inferred_write_frames(&module).unwrap().get(id), Some(&None));
        let error = validate_write_frames(&module).unwrap_err();
        assert_eq!(error.machine, Some(id));
        assert_eq!(
            error.detail,
            "cannot read the storage used by a body with a published write frame"
        );
    }
}
