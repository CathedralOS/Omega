//! Exact semantic sites and spans; ordinal lookup is an input predicate.

#[cfg(test)]
mod tests;

use super::{Error, host};
use abstract_operations::{AbstractFunction, AbstractOperation};
use machine_code::{
    FunctionFragment, FunctionFragmentBranchEvidence, FunctionFragmentControlProvenance as Control,
    SemanticCodeAttribution, SemanticCodeSite,
};
use selected_instructions::SelectedSuccessorRole;

fn semantic_edge(
    successor: &machine_code::FunctionFragmentSuccessorProvenance,
) -> Result<semantic_vocabulary::EdgeId, Error> {
    successor
        .psi_edge
        .ok_or(Error::Mismatch("semantic successor lost its authored edge"))
}

fn implementation_successor(
    successor: &machine_code::FunctionFragmentSuccessorProvenance,
    provenance: &selected_instructions::SelectedInstructionProvenance,
) -> bool {
    if !successor.fuel.is_empty() || !successor.bindings.is_empty() {
        return false;
    }
    match successor.role {
        SelectedSuccessorRole::CaseDispatchContinuation => successor.psi_edge.is_some(),
        SelectedSuccessorRole::OperationContinuation { operation } => {
            successor.psi_edge.is_none()
                && provenance.operations.contains(&operation)
                && provenance.edges.is_empty()
                && provenance.fuel.is_empty()
        }
        _ => false,
    }
}

fn build_ordinal_index(
    source: &AbstractFunction,
) -> std::collections::BTreeMap<SemanticCodeSite, Option<usize>> {
    let mut index = std::collections::BTreeMap::new();
    let mut record = |site: SemanticCodeSite, ordinal: usize| {
        index
            .entry(site)
            .and_modify(|existing| *existing = None)
            .or_insert(Some(ordinal));
    };
    for (ordinal, operation) in source.operations.iter().enumerate() {
        if let Some(psi_operation) = operation.psi_operation() {
            record(SemanticCodeSite::Operation(psi_operation), ordinal);
        } else {
            match operation {
                abstract_operations::return_or_return_unit_pattern!(psi_edge, ..)
                | AbstractOperation::InvokeNoNormal { psi_edge, .. }
                | AbstractOperation::ExternalCompletion { psi_edge, .. }
                | AbstractOperation::Crash { psi_edge, .. }
                | AbstractOperation::ReturnStructural { psi_edge, .. }
                | AbstractOperation::Jump { psi_edge, .. } => {
                    record(SemanticCodeSite::Edge(*psi_edge), ordinal);
                }
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    record(SemanticCodeSite::Edge(when_true.psi_edge), ordinal);
                    if when_false.psi_edge != when_true.psi_edge {
                        record(SemanticCodeSite::Edge(when_false.psi_edge), ordinal);
                    }
                }
                AbstractOperation::StructuralCase { cases, .. } => {
                    let mut seen = Vec::new();
                    for case in cases {
                        if !seen.contains(&case.psi_edge) {
                            seen.push(case.psi_edge);
                            record(SemanticCodeSite::Edge(case.psi_edge), ordinal);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    index
}

fn lookup_ordinal(
    index: &std::collections::BTreeMap<SemanticCodeSite, Option<usize>>,
    site: SemanticCodeSite,
) -> Result<usize, Error> {
    index.get(&site).copied().flatten().ok_or(Error::Mismatch(
        "fragment semantic site has no unique authored ordinal",
    ))
}

pub(super) fn ordinal(source: &AbstractFunction, site: SemanticCodeSite) -> Result<usize, Error> {
    let mut matches = source
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            // Control transfers carry edges rather than an authored operation.
            let matches = if let Some(psi_operation) = operation.psi_operation() {
                site == SemanticCodeSite::Operation(psi_operation)
            } else {
                match operation {
                    abstract_operations::return_or_return_unit_pattern!(psi_edge, ..)
                    | AbstractOperation::InvokeNoNormal { psi_edge, .. }
                    | AbstractOperation::ExternalCompletion { psi_edge, .. }
                    | AbstractOperation::Crash { psi_edge, .. }
                    | AbstractOperation::ReturnStructural { psi_edge, .. }
                    | AbstractOperation::Jump { psi_edge, .. } => {
                        site == SemanticCodeSite::Edge(*psi_edge)
                    }
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        site == SemanticCodeSite::Edge(when_true.psi_edge)
                            || site == SemanticCodeSite::Edge(when_false.psi_edge)
                    }
                    AbstractOperation::StructuralCase { cases, .. } => cases
                        .iter()
                        .any(|case| site == SemanticCodeSite::Edge(case.psi_edge)),
                    _ => false,
                }
            };
            matches.then_some(index)
        });
    match (matches.next(), matches.next()) {
        (Some(index), None) => Ok(index),
        _ => Err(Error::Mismatch(
            "fragment semantic site has no unique authored ordinal",
        )),
    }
}

pub(super) fn produce(
    fragment: &FunctionFragment,
    source: &AbstractFunction,
) -> Result<Vec<SemanticCodeAttribution>, Error> {
    let ordinals = build_ordinal_index(source);
    let mut rows = Vec::new();
    for instruction in fragment.instructions() {
        let offset = host(instruction.offset)?;
        let length = instruction.bytes.len();
        let mut push = |site, code_offset, byte_count| -> Result<(), Error> {
            rows.push(SemanticCodeAttribution {
                site,
                operation_ordinal: lookup_ordinal(&ordinals, site)?,
                code_offset,
                byte_count,
            });
            Ok(())
        };
        for operation in &instruction.provenance.operations {
            push(SemanticCodeSite::Operation(*operation), offset, length)?;
        }
        match &instruction.control {
            Control::OperationTrap { operation } => {
                if instruction.provenance.operations.as_slice() != [*operation]
                    || !instruction.provenance.edges.is_empty()
                    || !instruction.provenance.fuel.is_empty()
                {
                    return Err(Error::Mismatch(
                        "operation trap changed its owning occurrence",
                    ));
                }
            }
            Control::Crash { psi_edge, .. } => {
                push(SemanticCodeSite::Edge(*psi_edge), offset, length)?
            }
            Control::HostedExitProcess { completion_edge } => {
                push(SemanticCodeSite::Edge(*completion_edge), offset, length)?;
            }
            Control::Return { psi_return_edge } => {
                push(SemanticCodeSite::Edge(*psi_return_edge), offset, length)?
            }
            Control::Jump { successor } if successor.role == SelectedSuccessorRole::Semantic => {
                push(
                    SemanticCodeSite::Edge(semantic_edge(successor)?),
                    offset,
                    length,
                )?
            }
            Control::Jump { successor } => {
                if matches!(
                    successor.role,
                    SelectedSuccessorRole::OperationContinuation { .. }
                ) && !implementation_successor(successor, &instruction.provenance)
                {
                    return Err(Error::Mismatch(
                        "operation jump changed its owning occurrence",
                    ));
                }
            }
            Control::ConditionalBranch {
                when_taken,
                when_fallthrough,
                ..
            } => {
                if [when_taken, when_fallthrough].iter().any(|successor| {
                    successor.role != SelectedSuccessorRole::Semantic
                        && !implementation_successor(successor, &instruction.provenance)
                }) {
                    return Err(Error::Mismatch(
                        "conditional successor has nonsemantic attribution role",
                    ));
                }
                if when_taken.role == SelectedSuccessorRole::Semantic {
                    push(
                        SemanticCodeSite::Edge(semantic_edge(when_taken)?),
                        offset,
                        length,
                    )?;
                }
                let Some(branch) = instruction.branch.as_deref() else {
                    return Err(Error::Mismatch("conditional has no decoded branch"));
                };
                let FunctionFragmentBranchEvidence::Conditional(branch) = branch else {
                    return Err(Error::Mismatch("conditional has jump evidence"));
                };
                if when_fallthrough.role == SelectedSuccessorRole::Semantic {
                    push(
                        SemanticCodeSite::Edge(semantic_edge(when_fallthrough)?),
                        host(branch.when_fallthrough_offset)?,
                        0,
                    )?;
                }
            }
            Control::InternalCall { owner, .. } => {
                if let Some(edge) = owner.edge() {
                    push(SemanticCodeSite::Edge(edge), offset, length)?;
                }
            }
            Control::NormalizedForeignCall {
                invocation_site, ..
            } => {
                if let semantic_vocabulary::InvocationSite::ExternalCompletion(edge) =
                    invocation_site
                {
                    push(SemanticCodeSite::Edge(*edge), offset, length)?;
                }
            }
            Control::None => {
                for edge in &instruction.provenance.edges {
                    push(SemanticCodeSite::Edge(*edge), offset, length)?;
                }
            }
        }
    }
    rows.sort_by_key(|row| (row.operation_ordinal, row.code_offset, row.byte_count));
    rows.dedup();
    let mut joined: Vec<SemanticCodeAttribution> = Vec::new();
    for row in rows {
        if let Some(previous) = joined
            .last_mut()
            .filter(|previous| previous.site == row.site)
        {
            let end = previous
                .code_offset
                .checked_add(previous.byte_count)
                .ok_or(Error::Overflow)?;
            if row.code_offset > end {
                joined.push(row);
                continue;
            }
            if row.code_offset < end {
                return Err(Error::Mismatch("semantic physical intervals overlap"));
            }
            previous.byte_count = previous
                .byte_count
                .checked_add(row.byte_count)
                .ok_or(Error::Overflow)?;
        } else {
            joined.push(row);
        }
    }
    Ok(joined)
}

pub(super) fn validate(
    fragment: &FunctionFragment,
    source: &AbstractFunction,
    rows: &[SemanticCodeAttribution],
) -> Result<(), Error> {
    let ordinals = build_ordinal_index(source);
    // Replay each maximal contiguous site interval directly from physical
    // membership and extent. Edge-owned argument transport can be separated
    // by allocation copies, just like operation-owned transport. Neither site
    // kind implies one contiguous byte interval; overlaps and omissions still
    // reject. No producer merge algorithm is invoked.
    for (index, row) in rows.iter().enumerate() {
        if lookup_ordinal(&ordinals, row.site)? != row.operation_ordinal
            || rows[..index].iter().any(|previous| {
                previous.site == row.site
                    && previous
                        .code_offset
                        .checked_add(previous.byte_count)
                        .is_none_or(|end| end >= row.code_offset)
            })
            || !interval_is_exact(fragment, row)?
        {
            return Err(Error::Mismatch(
                "object semantic interval differs from its physical spans",
            ));
        }
    }
    if rows.windows(2).any(|pair| {
        (pair[0].operation_ordinal, pair[0].code_offset)
            >= (pair[1].operation_ordinal, pair[1].code_offset)
    }) {
        return Err(Error::Mismatch(
            "object semantic attribution order is not canonical",
        ));
    }
    for instruction in fragment.instructions() {
        let present = |site, offset, length| {
            rows.iter().any(|row| {
                row.site == site
                    && row.code_offset <= offset
                    && offset.checked_add(length).is_some_and(|end| {
                        row.code_offset
                            .checked_add(row.byte_count)
                            .is_some_and(|row_end| end <= row_end)
                    })
            })
        };
        for operation in &instruction.provenance.operations {
            if !present(
                SemanticCodeSite::Operation(*operation),
                host(instruction.offset)?,
                instruction.bytes.len(),
            ) {
                return Err(Error::Mismatch("object omits an attributed operation"));
            }
        }
        let edges: Vec<_> = match &instruction.control {
            Control::OperationTrap { operation } => {
                if instruction.provenance.operations.as_slice() != [*operation]
                    || !instruction.provenance.edges.is_empty()
                    || !instruction.provenance.fuel.is_empty()
                {
                    return Err(Error::Mismatch(
                        "operation trap changed its owning occurrence",
                    ));
                }
                Vec::new()
            }
            Control::Crash { psi_edge, .. } => vec![*psi_edge],
            Control::HostedExitProcess { completion_edge } => vec![*completion_edge],
            Control::Return { psi_return_edge } => vec![*psi_return_edge],
            Control::Jump { successor } => {
                if successor.role == SelectedSuccessorRole::Semantic {
                    vec![semantic_edge(successor)?]
                } else {
                    if matches!(
                        successor.role,
                        SelectedSuccessorRole::OperationContinuation { .. }
                    ) && !implementation_successor(successor, &instruction.provenance)
                    {
                        return Err(Error::Mismatch(
                            "operation jump changed its owning occurrence",
                        ));
                    }
                    Vec::new()
                }
            }
            Control::ConditionalBranch {
                when_taken,
                when_fallthrough,
                ..
            } => {
                if [when_taken, when_fallthrough].iter().any(|successor| {
                    successor.role != SelectedSuccessorRole::Semantic
                        && !implementation_successor(successor, &instruction.provenance)
                }) {
                    return Err(Error::Mismatch(
                        "conditional successor has nonsemantic attribution role",
                    ));
                }
                [when_taken, when_fallthrough]
                    .into_iter()
                    .filter(|successor| successor.role == SelectedSuccessorRole::Semantic)
                    .map(semantic_edge)
                    .collect::<Result<Vec<_>, _>>()?
            }
            Control::InternalCall { owner, .. } => owner.edge().into_iter().collect(),
            Control::NormalizedForeignCall {
                invocation_site, ..
            } => match invocation_site {
                semantic_vocabulary::InvocationSite::InvokeNoNormal(_) => {
                    return Err(Error::Mismatch(
                        "foreign invocation names a module invocation edge",
                    ));
                }
                semantic_vocabulary::InvocationSite::Operation(_) => Vec::new(),
                semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                    vec![*edge]
                }
            },
            Control::None => instruction.provenance.edges.clone(),
        };
        let instruction_offset = host(instruction.offset)?;
        let instruction_end = instruction_offset
            .checked_add(instruction.bytes.len())
            .ok_or(Error::Overflow)?;
        for edge in edges {
            // A fallthrough marker can coalesce with adjacent transport for
            // the same edge. Its decoded position, not the branch's bytes,
            // must remain covered by that exact semantic interval.
            if let Control::ConditionalBranch {
                when_fallthrough, ..
            } = &instruction.control
                && when_fallthrough.role == SelectedSuccessorRole::Semantic
                && when_fallthrough.psi_edge == Some(edge)
                && let Some(FunctionFragmentBranchEvidence::Conditional(branch)) =
                    instruction.branch.as_deref()
                && present(
                    SemanticCodeSite::Edge(edge),
                    host(branch.when_fallthrough_offset)?,
                    0,
                )
            {
                continue;
            }
            if !rows.iter().any(|row| {
                row.site == SemanticCodeSite::Edge(edge)
                    && (supports(instruction, row.site, row.code_offset, row.byte_count)
                        || (supports(
                            instruction,
                            row.site,
                            instruction_offset,
                            instruction.bytes.len(),
                        ) && row.code_offset <= instruction_offset
                            && row
                                .code_offset
                                .checked_add(row.byte_count)
                                .is_some_and(|end| instruction_end <= end)))
            }) {
                return Err(Error::Mismatch("object omits an attributed control edge"));
            }
        }
    }
    Ok(())
}

fn supports(
    instruction: &machine_code::FunctionFragmentInstructionSpan,
    site: SemanticCodeSite,
    offset: usize,
    length: usize,
) -> bool {
    let ordinary =
        u64::try_from(offset).ok() == Some(instruction.offset) && length == instruction.bytes.len();
    match site {
        SemanticCodeSite::Operation(operation) => {
            ordinary && instruction.provenance.operations.contains(&operation)
        }
        SemanticCodeSite::Edge(edge) => match &instruction.control {
            Control::OperationTrap { .. } => false,
            Control::Crash { psi_edge, .. } => ordinary && edge == *psi_edge,
            Control::HostedExitProcess { completion_edge } => edge == *completion_edge && ordinary,
            Control::Return { psi_return_edge } => ordinary && edge == *psi_return_edge,
            Control::Jump { successor } => {
                successor.role == SelectedSuccessorRole::Semantic
                    && ordinary
                    && Some(edge) == successor.psi_edge
            }
            Control::ConditionalBranch {
                when_taken,
                when_fallthrough,
                ..
            } => {
                (when_taken.role == SelectedSuccessorRole::Semantic
                    && ordinary
                    && Some(edge) == when_taken.psi_edge)
                    || (when_fallthrough.role == SelectedSuccessorRole::Semantic
                        && Some(edge) == when_fallthrough.psi_edge
                        && length == 0
                        && matches!(
                            instruction.branch.as_deref(),
                            Some(FunctionFragmentBranchEvidence::Conditional(branch)) if u64::try_from(offset).ok() == Some(branch.when_fallthrough_offset)
                        ))
            }
            Control::None => ordinary && instruction.provenance.edges.contains(&edge),
            Control::InternalCall { owner, .. } => ordinary && owner.edge() == Some(edge),
            Control::NormalizedForeignCall {
                invocation_site, ..
            } => {
                ordinary
                    && *invocation_site
                        == semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
            }
        },
    }
}
fn interval_is_exact(
    fragment: &FunctionFragment,
    row: &SemanticCodeAttribution,
) -> Result<bool, Error> {
    // Conditional fallthrough owns a zero-width position after the branch,
    // rather than the branch instruction's bytes.
    if row.byte_count == 0
        && fragment
            .instructions()
            .any(|instruction| supports(instruction, row.site, row.code_offset, 0))
    {
        return Ok(true);
    }
    let end = row
        .code_offset
        .checked_add(row.byte_count)
        .ok_or(Error::Overflow)?;
    let mut minimum = usize::MAX;
    let mut maximum = 0usize;
    let mut covered = 0usize;
    let mut found = false;
    for instruction in fragment.instructions() {
        let start = host(instruction.offset)?;
        let instruction_end = start
            .checked_add(instruction.bytes.len())
            .ok_or(Error::Overflow)?;
        if supports(instruction, row.site, start, instruction.bytes.len()) {
            // Other intervals for this site are checked separately. Compiler
            // spill accesses between them have no authored operation provenance.
            if (instruction_end <= row.code_offset || start >= end)
                && (!instruction.bytes.is_empty() || start != row.code_offset || end != start)
            {
                continue;
            }
            found = true;
            minimum = minimum.min(start);
            maximum = maximum.max(instruction_end);
            covered = covered
                .checked_add(instruction.bytes.len())
                .ok_or(Error::Overflow)?;
        } else if start < end && instruction_end > row.code_offset {
            return Ok(false);
        } else if instruction.bytes.is_empty() && start > row.code_offset && start < end {
            // Even zero-width foreign control/provenance is not absorbed.
            return Ok(false);
        }
    }
    Ok(found && minimum == row.code_offset && maximum == end && covered == row.byte_count)
}
