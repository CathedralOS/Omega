//! Retain the exact helper operands/results after replacing a boundary call.
//! Legalization independently checks its authenticated kind; this audit checks
//! that physical source attribution did not substitute another token or home.

/// Prepare this function's helper operations, accesses and instructions once.
/// Duplicate rows remain visible to the same exact uniqueness checks.
pub(super) struct PreparedHelperReplay<'a> {
    instructions: Vec<Option<&'a selected_instructions::SelectedInstruction>>,
    operations: Vec<(
        semantic_vocabulary::OperationId,
        Vec<&'a target_operations::TargetUnitOperation>,
    )>,
    accesses: Vec<(
        semantic_vocabulary::OperationId,
        Vec<&'a selected_instructions::SelectedMemoryAccess>,
    )>,
}
impl<'a> PreparedHelperReplay<'a> {
    pub(super) fn new(
        selected: &'a selected_instructions::SelectedFunction,
        targeted: &'a target_operations::TargetFunction,
    ) -> Result<Self, super::Error> {
        let operations = group_operations(targeted.graph.operations().filter_map(|row| {
            if let target_operations::TargetUnitOperation::StaticInterrupt {
                psi_operation, ..
            } = row
            {
                Some((*psi_operation, row))
            } else {
                None
            }
        }));
        let mut result = Self {
            instructions: Vec::new(),
            operations,
            accesses: Vec::new(),
        };
        if result.operations.is_empty() {
            return Ok(result);
        }
        result.accesses = group_operations(selected.memory_accesses.iter().filter_map(|access| {
            if let selected_instructions::SelectedMemoryAccessOrigin::Operation(operation) =
                access.origin
            {
                Some((operation, access))
            } else {
                None
            }
        }));
        for row in selected.instructions() {
            let index = row.id.0 as usize;
            if result.instructions.len() <= index {
                result.instructions.resize(index + 1, None);
            }
            if result.instructions[index].replace(row).is_some() {
                return Err(super::Error::Mismatch(
                    "installed interrupt replay found duplicate instruction identities",
                ));
            }
        }
        Ok(result)
    }
    fn instruction(
        &self,
        id: selected_instructions::SelectedInstructionId,
    ) -> Option<&'a selected_instructions::SelectedInstruction> {
        self.instructions.get(id.0 as usize).copied().flatten()
    }
    fn operations(
        &self,
        operation: semantic_vocabulary::OperationId,
    ) -> &[&'a target_operations::TargetUnitOperation] {
        operation_rows(&self.operations, operation)
    }
    fn accesses(
        &self,
        operation: semantic_vocabulary::OperationId,
    ) -> &[&'a selected_instructions::SelectedMemoryAccess] {
        operation_rows(&self.accesses, operation)
    }
}

/// Compact groups for opaque operation identities. SelectedInstructionId is
/// separately a dense per-function ID and keeps its direct lookup above.
fn group_operations<T>(
    rows: impl Iterator<Item = (semantic_vocabulary::OperationId, T)>,
) -> Vec<(semantic_vocabulary::OperationId, Vec<T>)> {
    let mut rows: Vec<_> = rows.collect();
    rows.sort_unstable_by_key(|(operation, _)| *operation);
    let mut groups: Vec<(semantic_vocabulary::OperationId, Vec<T>)> = Vec::new();
    for (operation, row) in rows {
        if let Some((_, values)) = groups.last_mut().filter(|(last, _)| *last == operation) {
            values.push(row);
        } else {
            groups.push((operation, vec![row]));
        }
    }
    groups
}
fn operation_rows<T>(
    groups: &[(semantic_vocabulary::OperationId, Vec<T>)],
    operation: semantic_vocabulary::OperationId,
) -> &[T] {
    groups
        .binary_search_by_key(&operation, |(id, _)| *id)
        .map_or(&[], |index| groups[index].1.as_slice())
}

pub(super) fn retained(
    operation: &abstract_operations::AbstractOperation,
    helpers: &target_operations_to_selected_instructions::PreparedInstalledInterruptHelpers,
    replay: &PreparedHelperReplay<'_>,
) -> Result<bool, super::Error> {
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
    let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        arguments,
        structural_arguments,
        result,
        ..
    } = operation
    else {
        return Ok(false);
    };
    if !arguments.is_empty() {
        return Ok(false);
    }
    let mut rows = replay
        .operations(*psi_operation)
        .iter()
        .copied()
        .filter_map(|row| match row {
            target_operations::TargetUnitOperation::StaticInterrupt {
                psi_operation: actual,
                boundary: actual_boundary,
                token,
                result_home,
                operation: kind,
            } if actual == psi_operation => Some((actual_boundary, token, result_home, kind)),
            _ => None,
        });
    let (Some((actual_boundary, token, home, kind)), None) = (rows.next(), rows.next()) else {
        return Ok(false);
    };
    helpers
        .verify(*psi_operation, *kind)
        .map_err(super::Error::Mismatch)?;
    if !root_accesses(replay, *psi_operation, *kind, token.place) {
        return Err(super::Error::Mismatch(
            "installed interrupt helper changed its selected root pointer or field accesses",
        ));
    }
    Ok(actual_boundary == boundary
        && structural_arguments.as_slice() == std::slice::from_ref(token)
        && match (result, home) {
            (AbstractBoundaryResult::Unit, None) => true,
            (AbstractBoundaryResult::Structural(expected), Some(home)) => home
                .operation_result()
                .is_some_and(|(actual, result)| actual == *psi_operation && result == expected),
            _ => false,
        })
}

/// Backing footprints describe the separate root, never the token's own extent.
/// Join their actual pointer operand to the first token word and retain exact
/// permitted fields. Allocation/lease authority remains installation admission.
fn root_accesses(
    replay: &PreparedHelperReplay<'_>,
    operation: semantic_vocabulary::OperationId,
    kind: language_core::static_interrupts::StaticInterruptOperation,
    token: semantic_vocabulary::PlaceId,
) -> bool {
    use language_core::static_interrupts::StaticInterruptOperation as Operation;
    use runtime_abi::static_interrupts::{WORD_BYTES, root};
    use selected_instructions::{
        SelectedInstructionKind as Kind, SelectedMemoryAccessOrigin as Origin,
        SelectedMemoryAccessRole as Role,
    };
    let instruction = |id| replay.instruction(id);
    let accesses = replay.accesses(operation);
    let Some(root_access) = accesses.iter().find(|access| {
        access.origin == Origin::Operation(operation) && access.role == Role::ReadBacking
    }) else {
        return false;
    };
    let Some(root_pointer) = instruction(root_access.instruction).and_then(|row| {
        row.operands
            .iter()
            .find(|operand| operand.operand == 0)
            .map(|operand| operand.virtual_register)
    }) else {
        return false;
    };
    // Other ReadPlace rows may load projection descriptors. Match the actual
    // root pointer's definition rather than guessing the first such load.
    let mut token_roots = accesses.iter().filter(|access| {
        if access.origin != Origin::Operation(operation)
            || access.place != token
            || access.role != Role::ReadPlace
            || access.byte_count != WORD_BYTES
        {
            return false;
        }
        instruction(access.instruction).is_some_and(|row| {
            matches!(row.kind, Kind::Load64 { byte_offset: 0 })
                && row.provenance.operations.contains(&operation)
                && row
                    .operands
                    .iter()
                    .any(|operand| operand.operand == 1 && operand.virtual_register == root_pointer)
        })
    });
    if !matches!((token_roots.next(), token_roots.next()), (Some(_), None)) {
        return false;
    }
    let expected: &[u32] = match kind {
        Operation::Context => &[
            root::STATE,
            root::INVOCATION,
            root::DEBT,
            root::PROVIDER,
            root::COMPLETION,
            root::CONTEXT,
        ],
        Operation::BeginCompletion => &[
            root::STATE,
            root::INVOCATION,
            root::DEBT,
            root::PROVIDER,
            root::COMPLETION,
        ],
        Operation::FinishCompletion => &[root::STATE, root::INVOCATION, root::DEBT],
        Operation::Install => return false,
    };
    let mut reads = Vec::new();
    let mut writes = 0;
    for access in accesses.iter().filter(|access| {
        access.origin == Origin::Operation(operation)
            && matches!(access.role, Role::ReadBacking | Role::WriteBacking)
    }) {
        let Some(row) = instruction(access.instruction) else {
            return false;
        };
        if access.place != token
            || access.byte_count != WORD_BYTES
            || access
                .byte_offset
                .checked_add(WORD_BYTES)
                .is_none_or(|end| end > root::BYTES)
            || !row.provenance.operations.contains(&operation)
            || !row
                .operands
                .iter()
                .any(|operand| operand.operand == 0 && operand.virtual_register == root_pointer)
        {
            return false;
        }
        match (&access.role, &row.kind) {
            (Role::ReadBacking, Kind::Load64 { byte_offset })
                if *byte_offset == access.byte_offset
                    && expected.contains(byte_offset)
                    && !reads.contains(byte_offset) =>
            {
                reads.push(*byte_offset)
            }
            (
                Role::WriteBacking,
                Kind::Store {
                    byte_offset,
                    byte_size,
                },
            ) if kind == Operation::FinishCompletion
                && *byte_offset == root::DEBT
                && access.byte_offset == root::DEBT
                && u32::from(*byte_size) == WORD_BYTES =>
            {
                writes += 1
            }
            _ => return false,
        }
    }
    reads.len() == expected.len() && writes == usize::from(kind == Operation::FinishCompletion)
}
