//! Retain one private root and prove commit/publication use the checked operands.
use super::expressions::Expressions;
use super::{Contract, Error, Expr, Graph, Inputs, Word, Writes, unique};
use runtime_abi::static_interrupts::root;
use selected_instructions::{
    LocalStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind as Kind, SelectedMemoryAccessOrigin,
    SelectedMemoryAccessRole as Role, SelectedTerminator,
};
use semantic_vocabulary::OperationId;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    selected: &SelectedFunction,
    operation: OperationId,
    input: &Inputs,
    contract: &Contract,
    address: Expr,
    completion: u64,
    work: SelectedInstructionId,
    graph: &Graph<'_>,
    expressions: &mut Expressions<'_>,
    writes: &mut Writes<'_>,
    refusal: SelectedBlockId,
) -> Result<SelectedInstructionId, Error> {
    let slot = LocalStorageSlotId::Boundary { operation };
    let allocation = unique(
        selected
            .local_storage_slots
            .iter()
            .filter(|row| row.id == slot),
        "installation must own exactly one control allocation",
    )?;
    if allocation.byte_size != 128 || allocation.alignment != 16 {
        return Err(Error::Mismatch(
            "installation root and scratch allocation changed geometry",
        ));
    }
    let check = unique(
        selected.instructions().filter(|row| {
            row.provenance.operations.contains(&operation)
                && matches!(row.kind, Kind::InstalledInterruptCpuCheck { .. })
        }),
        "installation must have one CPU check",
    )?;
    let validator = unique(
        selected.calls.iter().filter(|call| {
            call.owner
                == target_operations::CallSiteOwner::Installation {
                    operation,
                    role: target_operations::InstallationCallRole::Validate,
                }
        }),
        "installation CPU check lost its exact validator",
    )?;
    if !graph.precedes(validator.instruction, check.id)? {
        return Err(Error::Mismatch(
            "installation CPU check precedes its selected validator",
        ));
    }
    let commit = unique(
        selected.instructions().filter(|row| {
            row.provenance.operations.contains(&operation)
                && matches!(row.kind, Kind::InstalledInterruptTaskCommit { .. })
        }),
        "installation must have one task-register commit",
    )?;
    let publication = unique(
        selected.instructions().filter(|row| {
            row.provenance.operations.contains(&operation)
                && matches!(row.kind, Kind::InstalledInterruptTablePublication { .. })
        }),
        "installation must have one table publication",
    )?;
    if commit.kind
        != (Kind::InstalledInterruptTaskCommit {
            gdt_bytes: input.layout.gdt_bytes,
        })
        || publication.kind
            != (Kind::InstalledInterruptTablePublication {
                table_bytes: input.layout.table_bytes,
            })
    {
        return Err(Error::Mismatch(
            "installation commit changed the checked descriptor extents",
        ));
    }
    for (index, expected) in [
        input.pointers[1].clone(),
        input.pointers[2].clone(),
        contract.home.clone().offset(48),
    ]
    .iter()
    .enumerate()
    {
        if expressions.input(commit, index as u16)? != *expected {
            return Err(Error::Mismatch(
                "task commit does not use the checked GDT, TSS and scratch",
            ));
        }
    }
    if expressions.input(publication, 0)? != input.pointers[0]
        || expressions.input(publication, 1)? != contract.home.clone().offset(48)
    {
        return Err(Error::Mismatch(
            "IDT publication does not use the checked table and scratch",
        ));
    }
    let mut initial = Vec::new();
    for (offset, value) in [
        (root::STATE, Expr::constant(0)),
        (root::INVOCATION, Expr::constant(0)),
        (root::DEBT, Expr::constant(0)),
        (root::COMPLETION, Expr::constant(completion)),
        (root::PROVIDER, address),
        (root::CONTEXT, input.pointers[5].clone()),
    ] {
        initial.extend(writes.take_bytes(contract.home.clone().offset(offset), value, 8)?);
    }
    let stack_writes = writes.take_bytes(
        contract.home.clone().offset(96).load(8),
        contract.home.clone(),
        8,
    )?;
    let active = writes.take_bytes(
        contract.home.clone().offset(root::STATE),
        Expr::constant(root::ACTIVE),
        8,
    )?;
    let first = *initial
        .first()
        .ok_or(Error::Mismatch("installation has no root initialization"))?;
    let root_published = *stack_writes.first().ok_or(Error::Mismatch(
        "installation has no leased root-word write",
    ))?;
    let active_first = *active
        .first()
        .ok_or(Error::Mismatch("installation never activates its root"))?;
    for write in &initial {
        if !graph.precedes(*write, root_published)? {
            return Err(Error::Mismatch(
                "installation exposes a partially initialized root",
            ));
        }
    }
    for write in &stack_writes {
        if !graph.precedes(*write, active_first)? {
            return Err(Error::Mismatch(
                "installation activates before publishing its leased root word",
            ));
        }
    }
    for write in &active {
        if !graph.precedes(*write, commit.id)? {
            return Err(Error::Mismatch(
                "task commit precedes complete root activation",
            ));
        }
    }
    if !graph.precedes(commit.id, publication.id)? || !graph.precedes(publication.id, work)? {
        return Err(Error::Mismatch(
            "installation reordered task commit, IDT publication or work",
        ));
    }
    if !graph.precedes(check.id, first)? {
        return Err(Error::Mismatch(
            "installation begins publication before checking CPU state",
        ));
    }
    graph.only_success_path(first, work, refusal)?;
    let (work_block, _) = graph.position(work)?;
    if !matches!(&selected.blocks[work_block].terminator,SelectedTerminator::OperationInvokeNoNormal {instruction,operation:owner} if instruction.id==work && *owner==operation)
    {
        return Err(Error::Mismatch(
            "installation work does not keep its root activation alive",
        ));
    }
    Ok(first)
}

fn offset(address: &Expr, base: &Expr) -> Option<u64> {
    fn split(value: &Expr) -> (&Expr, u64) {
        if let Word::Binary(super::Binary::Add, root, displacement) = value.word()
            && let Word::Constant(displacement) = displacement.word()
        {
            return (root, *displacement);
        }
        (value, 0)
    }
    let (address_root, address_offset) = split(address);
    let (base_root, base_offset) = split(base);
    (address_root == base_root)
        .then(|| address_offset.checked_sub(base_offset))
        .flatten()
}
pub(super) fn within(address: &Expr, bytes: u8, base: &Expr, extent: u32) -> bool {
    offset(address, base).is_some_and(|offset| {
        offset
            .checked_add(u64::from(bytes))
            .is_some_and(|end| end <= u64::from(extent))
    })
}
fn load_word(row: &SelectedInstruction) -> Option<(u32, u8)> {
    match row.kind {
        Kind::Load8 { byte_offset } => Some((byte_offset, 1)),
        Kind::Load16 { byte_offset } => Some((byte_offset, 2)),
        Kind::Load32 { byte_offset } => Some((byte_offset, 4)),
        Kind::Load64 { byte_offset } => Some((byte_offset, 8)),
        _ => None,
    }
}

fn source_loads(value: &Expr, loads: &mut Vec<(Expr, u8)>) {
    match value.word() {
        Word::Load { address, bytes, .. } => {
            loads.push((address.clone(), *bytes));
            source_loads(address, loads);
        }
        Word::Binary(_, left, right) => {
            source_loads(left, loads);
            source_loads(right, loads);
        }
        _ => {}
    }
}

/// Every data read must stay in the typed source window, a checked descriptor
/// entry, or the unique compiler home. Dynamic membership is proved by the
/// actual selected guard edge before the read, not by its memory-role label.
pub(super) fn reads(
    selected: &SelectedFunction,
    operation: OperationId,
    input: &Inputs,
    contract: &Contract,
    validate: SelectedInstructionId,
    graph: &Graph<'_>,
    expressions: &mut Expressions<'_>,
) -> Result<(), Error> {
    let mut inputs = Vec::new();
    for value in input
        .pointers
        .iter()
        .chain([&input.stack_length, &input.arrival_length])
    {
        source_loads(value, &mut inputs);
    }
    for row in selected
        .instructions()
        .filter(|row| row.provenance.operations.contains(&operation))
    {
        let Some((displacement, bytes)) = load_word(row) else {
            continue;
        };
        if expressions.private_reload(row)? {
            // The shared spill audit accounts for this read against the exact
            // full-word writer on every incoming path, not a source interval.
            continue;
        }
        let address = expressions.input(row, 0)?.offset(displacement);
        if inputs
            .iter()
            .any(|(expected, width)| *width == bytes && *expected == address)
        {
            continue;
        }
        if [64, 72, 80, 88, 96]
            .iter()
            .any(|offset| address == contract.home.clone().offset(*offset) && bytes == 8)
            || (!contract.pairs.is_empty()
                && [112, 120]
                    .iter()
                    .any(|offset| address == contract.home.clone().offset(*offset) && bytes == 8))
            || within(
                &address,
                bytes,
                &input.pointers[0],
                input.layout.table_bytes,
            )
        {
            continue;
        }
        fresh_observations(&address, input, validate, graph)?;
        let interval = contract
            .read_intervals
            .iter()
            .find(|interval| within(&address, bytes, &interval.address, interval.bytes))
            .ok_or(Error::Mismatch(
                "installation read is outside every authenticated operand interval",
            ))?;
        let refusal = super::refusal(selected, graph, expressions, validate)?;
        match interval.kind {
            super::guards::ReadKind::Gate
            | super::guards::ReadKind::Stack
            | super::guards::ReadKind::Arrival
            | super::guards::ReadKind::Task => {
                let (counter, limit) = if matches!(interval.kind, super::guards::ReadKind::Gate) {
                    (&contract.table_index, &contract.table_limit)
                } else if matches!(interval.kind, super::guards::ReadKind::Arrival) {
                    (&contract.arrival_index, &contract.arrival_limit)
                } else {
                    (&contract.stack_index, &contract.stack_limit)
                };
                let condition = Expr::binary(super::Binary::Less, counter.clone(), limit.clone());
                let header = graph.require_edge(expressions, &condition, row.id)?;
                let Word::Load { address: cell, .. } = counter.word() else {
                    return Err(Error::Mismatch("installation loop has no counter home"));
                };
                address.visit_loads(&mut |address, bytes, instruction| {
                    if address == cell && bytes == 8 {
                        let load = instruction
                            .ok_or(Error::Mismatch("installation index has no observation"))?;
                        if graph.position(load)?.0 != header
                            && graph.require_edge(expressions, &condition, load)? != header
                        {
                            return Err(Error::Mismatch(
                                "installation read uses another scan's index observation",
                            ));
                        }
                    }
                    Ok(())
                })?;
                if matches!(interval.kind, super::guards::ReadKind::Task) {
                    for predicate in &contract.each_stack {
                        graph.require_guard(expressions, predicate, row.id, refusal)?;
                    }
                }
            }
            super::guards::ReadKind::Code(entry) => {
                for predicate in &contract.entries[entry].selector_guards {
                    graph.require_guard(expressions, predicate, row.id, refusal)?;
                }
            }
            super::guards::ReadKind::Table => {}
            super::guards::ReadKind::Control => {
                return Err(Error::Mismatch(
                    "installation reads an undeclared control word",
                ));
            }
        }
    }
    footprints(selected, operation, input, contract, expressions)
}

fn footprints(
    selected: &SelectedFunction,
    operation: OperationId,
    input: &Inputs,
    contract: &Contract,
    expressions: &mut Expressions<'_>,
) -> Result<(), Error> {
    let slot = LocalStorageSlotId::Boundary { operation };
    for row in selected
        .instructions()
        .filter(|row| row.provenance.operations.contains(&operation))
    {
        let expected = match row.kind {
            Kind::InstalledInterruptCpuCheck { gdt_bytes } => Some((gdt_bytes, false, false)),
            Kind::InstalledInterruptTaskCommit { gdt_bytes } => Some((gdt_bytes, true, false)),
            Kind::InstalledInterruptTablePublication { table_bytes } => {
                Some((table_bytes, false, true))
            }
            _ => None,
        };
        let Some((extent, write, publish)) = expected else {
            continue;
        };
        let footprints = selected
            .memory_accesses
            .iter()
            .filter(|access| {
                access.instruction == row.id
                    && access.origin == SelectedMemoryAccessOrigin::Operation(operation)
            })
            .collect::<Vec<_>>();
        if footprints.len()
            != if publish {
                2
            } else if write {
                4
            } else {
                3
            }
        {
            return Err(Error::Mismatch(
                "installed CPU operation lost its complete memory effects",
            ));
        }
        let scratch = footprints
            .iter()
            .filter(|access| {
                access.role == (Role::WriteLocal { slot })
                    && access.place == input.places[0]
                    && access.byte_offset == 48
                    && access.byte_count == 10
            })
            .count();
        let gdt_or_table = footprints
            .iter()
            .filter(|access| {
                access.place == input.places[if publish { 0 } else { 1 }]
                    && access.byte_offset == 0
                    && access.byte_count == extent
                    && access.role
                        == if write {
                            Role::WriteBacking
                        } else {
                            Role::ReadBacking
                        }
            })
            .count();
        let task = publish
            || footprints.iter().any(|access| {
                access.place == input.places[2]
                    && access.byte_offset == 0
                    && access.byte_count == 104
                    && access.role == Role::ReadBacking
            });
        let commit_read = !write
            || footprints
                .iter()
                .filter(|access| {
                    access.place == input.places[1]
                        && access.byte_offset == 0
                        && access.byte_count == extent
                        && access.role == Role::ReadBacking
                })
                .count()
                == 1;
        if scratch != 1 || gdt_or_table != 1 || !task || !commit_read {
            return Err(Error::Mismatch(
                "installed CPU memory ranges differ from the checked geometry",
            ));
        }
        if publish
            && (expressions.input(row, 0)? != input.pointers[0]
                || expressions.input(row, 1)? != contract.home.clone().offset(48))
        {
            return Err(Error::Mismatch(
                "IDT publication has different physical operands",
            ));
        }
    }
    Ok(())
}

/// Source pointer preparation may precede Validate, but observations of the
/// table and descriptor contents used by its guards must follow that call.
pub(super) fn fresh_observations(
    value: &Expr,
    input: &Inputs,
    validate: SelectedInstructionId,
    graph: &Graph<'_>,
) -> Result<(), Error> {
    let mut exempt = Vec::new();
    for value in input
        .pointers
        .iter()
        .chain([&input.stack_length, &input.arrival_length])
    {
        source_loads(value, &mut exempt);
    }
    fn visit(
        value: &Expr,
        exempt: &[(Expr, u8)],
        validate: SelectedInstructionId,
        graph: &Graph<'_>,
    ) -> Result<(), Error> {
        match value.word() {
            Word::RootBoundary { instruction, .. } => {
                let observed = instruction.ok_or(Error::Mismatch(
                    "root interval has no actual address observation",
                ))?;
                graph.require_owned_observation(observed)?;
                if !graph.precedes(validate, observed)? {
                    return Err(Error::Mismatch(
                        "root interval reuses an address observed before its validator",
                    ));
                }
            }
            Word::Load {
                address,
                bytes,
                instruction,
            } => {
                if !exempt
                    .iter()
                    .any(|(input, width)| input == address && width == bytes)
                {
                    let observed = instruction.ok_or(Error::Mismatch(
                        "installation guard has no actual load observation",
                    ))?;
                    if !graph.precedes(validate, observed)? {
                        return Err(Error::Mismatch(
                            "installation guard reuses bytes observed before its validator",
                        ));
                    }
                }
                visit(address, exempt, validate, graph)?;
            }
            Word::Binary(_, left, right) => {
                visit(left, exempt, validate, graph)?;
                visit(right, exempt, validate, graph)?;
            }
            Word::Cpu {
                gdt, task, scratch, ..
            } => {
                visit(gdt, exempt, validate, graph)?;
                visit(task, exempt, validate, graph)?;
                visit(scratch, exempt, validate, graph)?;
            }
            _ => {}
        }
        Ok(())
    }
    visit(value, &exempt, validate, graph)
}
