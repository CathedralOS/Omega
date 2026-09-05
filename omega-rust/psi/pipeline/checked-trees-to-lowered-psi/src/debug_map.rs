//! Terminal debug-map presentation.

use super::*;

pub(super) fn build_debug_map(
    plan: &CheckedTerminalMachineDebugPlan,
    module: &TerminalModule,
) -> Result<TerminalDebugMap, LoweringError> {
    let terminal_machine = module
        .machines
        .first()
        .expect("the selected entry machine is first in its terminal call closure");
    let source_states = &plan.states;
    let has_source_file = |span: source::SourceSpan| {
        plan.source_files
            .iter()
            .any(|file| file.source_id == span.source_id)
    };
    let mut subjects = Vec::<(DebugSubject, source::SourceSpan)>::new();
    let mut push = |subject, span| {
        if let Some(span) = span {
            subjects.push((subject, span));
        }
    };

    push(
        DebugSubject::Machine(terminal_machine.id),
        plan.machine_span,
    );
    let contract_span = plan.contract_span;
    push(
        DebugSubject::Contract(terminal_machine.contract.id),
        contract_span,
    );
    for clause in &terminal_machine.contract.ensures {
        push(DebugSubject::Obligation(clause.obligation), contract_span);
    }

    for (index, block) in terminal_machine.blocks.iter().enumerate() {
        let source_state = source_states
            .get(index)
            .or_else(|| source_states.last())
            .expect("an accepted source machine has at least one state");
        push(DebugSubject::Block(block.id), source_state.state_span);
        for (edge_index, edge) in block.terminator.edges().enumerate() {
            let transition_span = source_state
                .transition_spans
                .get(edge_index)
                .or_else(|| {
                    (source_state.transition_spans.len() == 1)
                        .then(|| &source_state.transition_spans[0])
                })
                .copied()
                .filter(|span| *span != source::SourceSpan::default())
                .filter(|span| has_source_file(*span));
            push(
                DebugSubject::Edge(edge),
                transition_span.or(source_state.state_span),
            );
        }
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let source_span = source_state
                .operation_spans
                .get(operation_index)
                .copied()
                .filter(|span| *span != source::SourceSpan::default())
                .filter(|span| has_source_file(*span));
            if let Some(source_span) = source_span {
                push(DebugSubject::Operation(operation.id), Some(source_span));
                push(
                    DebugSubject::Value(operation.result.expect_scalar().id),
                    Some(source_span),
                );
            } else {
                push(
                    DebugSubject::Operation(operation.id),
                    source_state.state_span,
                );
                push(
                    DebugSubject::Value(operation.result.expect_scalar().id),
                    source_state.state_span,
                );
            }
        }
        for (parameter_index, parameter) in block.parameters.iter().enumerate() {
            if let Some(source_span) = source_state
                .parameter_spans
                .get(parameter_index)
                .copied()
                .flatten()
            {
                push(DebugSubject::Value(parameter.id), Some(source_span));
            }
        }
    }

    if let Some(entry_state) = source_states.first() {
        for (parameter_index, parameter) in terminal_machine.parameters.iter().enumerate() {
            if let Some(source_span) = entry_state
                .parameter_spans
                .get(parameter_index)
                .copied()
                .flatten()
            {
                push(DebugSubject::Value(parameter.id), Some(source_span));
            }
        }
    }
    push(
        DebugSubject::Value(
            terminal_machine
                .result
                .scalar()
                .expect("the checked scalar producer emits a scalar result")
                .id,
        ),
        plan.machine_span,
    );

    subjects.sort_by_key(|(subject, _)| *subject);
    subjects.dedup_by_key(|(subject, _)| *subject);
    let mut source_ids = subjects
        .iter()
        .map(|(_, span)| span.source_id.0)
        .collect::<Vec<_>>();
    source_ids.sort_unstable();
    source_ids.dedup();

    let mut files = Vec::with_capacity(source_ids.len());
    for (index, source_id) in source_ids.iter().copied().enumerate() {
        let source_file = plan
            .source_files
            .iter()
            .find(|file| file.source_id == source::SourceId(source_id))
            .ok_or(LoweringError::MissingDebugSourceFile(source_id))?;
        let id = DebugFileId::new(
            u32::try_from(index)
                .map_err(|_| LoweringError::DebugSourceFileCountOverflow)?
                .checked_add(1)
                .ok_or(LoweringError::DebugSourceFileCountOverflow)?,
        )
        .expect("one-based debug file identity is nonzero");
        files.push(DebugSourceFile {
            id,
            origin: match source_file.origin {
                source::SourceOrigin::User => DebugSourceOrigin::User,
                source::SourceOrigin::Toolchain => DebugSourceOrigin::Toolchain,
            },
            byte_len: u64::try_from(source_file.source.len())
                .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
            digest: source_digest(source_file.source.as_bytes()),
            path: source_file.path.to_string_lossy().into_owned(),
        });
    }

    let sites = subjects
        .into_iter()
        .map(|(subject, span)| {
            let file_index = source_ids
                .binary_search(&span.source_id.0)
                .expect("source identity was collected above");
            let file = DebugFileId::new(
                u32::try_from(file_index)
                    .expect("validated debug file count fits u32")
                    .checked_add(1)
                    .expect("one-based debug file identity fits u32"),
            )
            .expect("one-based debug file identity is nonzero");
            Ok(DebugSite {
                subject,
                span: DebugSourceSpan {
                    file,
                    start: u64::try_from(span.span.start)
                        .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
                    end: u64::try_from(span.span.end)
                        .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let debug_map = TerminalDebugMap {
        semantic: terminal_psi_identity(module).map_err(LoweringError::DebugSemanticCodec)?,
        files,
        sites,
    };
    validate_debug_map(module, &debug_map).map_err(LoweringError::InvalidDebugMap)?;
    Ok(debug_map)
}
