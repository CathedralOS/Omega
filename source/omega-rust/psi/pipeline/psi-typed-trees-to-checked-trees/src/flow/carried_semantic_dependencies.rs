use super::{CanonicalPlace, canonical_place_type_reference, project_type_reference_from_segments};
use psi_checked_trees::{
    CheckFacts, CheckedSemanticDependencies, CheckedSemanticDependency,
    CheckedSemanticDependencyExposure as Exposure, CheckedSemanticDependencyKind as Kind,
};
use psi_language_semantics::{PermissionEventKind, PermissionEventSource};
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::{TypedTrees, types::TypeReferenceHandle};

/// Rederive the complete canonical semantic-dependency table from compiler
/// authority rather than trusting a previously retained table.
///
/// `program` must be the final typed program paired with `facts`. The retained
/// `facts.flow.semantic_dependencies` rows are deliberately not consulted.
pub(crate) fn derive_checked_semantic_dependencies(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedSemanticDependencies {
    let mut rows = Vec::new();

    // Machine-head types are semantic dependencies even when no ownership
    // event materializes them (notably a returned nominal constructed at the
    // exit). Their declaration visibility determines whether the edge enters
    // public compatibility identity or only private artifact identity.
    for machine in program.machines() {
        let Some(entry) = program.machine_states(machine).first() else {
            continue;
        };
        let exposure = machine_signature_exposure(program, machine.symbol);
        for parameter in program.state_parameters(entry) {
            append_type_dependencies(
                program,
                &mut rows,
                machine.symbol,
                parameter.type_reference,
                exposure,
                false,
            );
        }
        append_type_dependencies(
            program,
            &mut rows,
            machine.symbol,
            entry.return_type,
            exposure,
            false,
        );
    }

    // A direct call result can be consumed by another call without ever
    // becoming a named place. Checked call facts still retain the exact caller
    // and target, so join the target's return type here rather than depending
    // on ownership-place materialization.
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            let Some(return_type) = call_return_type_reference(program, call.target_symbol) else {
                continue;
            };
            append_type_dependencies(
                program,
                &mut rows,
                state.machine_symbol,
                return_type,
                Exposure::PrivateImplementation,
                false,
            );
        }
    }

    for (_, event) in facts.flow.ownership.permissions.iter() {
        let statement_index = event_statement_index(program, event.state_symbol, event.source);
        let segments = facts.flow.ownership.segments.span_or_empty(event.segments);
        let Some(type_reference) = event_type_reference(
            program,
            event.state_symbol,
            statement_index,
            event.root,
            segments,
        ) else {
            continue;
        };
        let exposure = event_exposure(
            program,
            event.machine_symbol,
            event.state_symbol,
            event.root,
        );
        append_type_dependencies(
            program,
            &mut rows,
            event.machine_symbol,
            type_reference,
            exposure,
            event.kind == PermissionEventKind::AffineDrop,
        );
    }

    for plan in &facts.flow.terminal_nominal_affine_unit_cleanups.machines {
        let exposure = machine_signature_exposure(program, plan.machine.machine);
        for cleanup in &plan.cleanups {
            push_cleanup_machine(
                &mut rows,
                plan.machine.machine,
                cleanup.cleanup_machine,
                exposure,
            );
        }
    }
    for plan in &facts.flow.terminal_structural_scalar_returns.machines {
        let exposure = machine_signature_exposure(program, plan.machine);
        for cleanup in &plan.cleanup_actions {
            let psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                cleanup,
            ) = cleanup
            else {
                continue;
            };
            push_cleanup_machine(&mut rows, plan.machine, cleanup.cleanup_machine, exposure);
        }
    }

    rows.sort_by_key(|row| {
        (
            row.consumer_machine.arena_index(),
            row.dependency.arena_index(),
            row.kind,
            row.exposure,
        )
    });
    CheckedSemanticDependencies { rows }
}

fn append_type_dependencies(
    program: &TypedTrees,
    rows: &mut Vec<CheckedSemanticDependency>,
    consumer_machine: SymbolHandle,
    type_reference: TypeReferenceHandle,
    exposure: Exposure,
    automatic_cleanup: bool,
) {
    let mut dependencies = Vec::new();
    collect_nominal_symbols(program, type_reference, &mut dependencies);
    for dependency in dependencies {
        for kind in [Kind::NominalIdentity, Kind::Layout, Kind::OwnershipBehavior] {
            push_promoting(
                rows,
                CheckedSemanticDependency {
                    consumer_machine,
                    dependency,
                    exposure,
                    kind,
                },
            );
        }
        if !automatic_cleanup {
            continue;
        }
        push_promoting(
            rows,
            CheckedSemanticDependency {
                consumer_machine,
                dependency,
                exposure,
                kind: Kind::AutomaticCleanup,
            },
        );
        if let Some(cleanup_machine) =
            crate::checks::nominal_drop_machine_symbol(program, dependency)
        {
            push_cleanup_machine(rows, consumer_machine, cleanup_machine, exposure);
        }
    }
}

fn event_statement_index(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    source: PermissionEventSource,
) -> usize {
    match source {
        PermissionEventSource::StateEntry => 0,
        PermissionEventSource::Statement { statement_index }
        | PermissionEventSource::Call {
            statement_index, ..
        } => statement_index,
        PermissionEventSource::StateExit => program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .find(|state| state.symbol == state_symbol)
            .map_or(0, |state| {
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .len()
            }),
    }
}

fn event_type_reference(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    root: psi_facts::PlaceRoot,
    segments: &[psi_facts::PlaceSegment],
) -> Option<TypeReferenceHandle> {
    match root {
        psi_facts::PlaceRoot::Symbol(_) => canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            &CanonicalPlace {
                root,
                segments: segments.to_vec(),
            },
        ),
        psi_facts::PlaceRoot::Expression(expression) => {
            let base = match program.expression_table.expression(expression) {
                psi_typed_trees::expression::ExpressionNode::Call(call) => {
                    call_return_type_reference(program, call.target_symbol)
                }
                _ => super::expression_type_reference_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    expression,
                ),
            }?;
            project_type_reference_from_segments(program, base, segments)
        }
        psi_facts::PlaceRoot::TypeReference(type_reference) => {
            project_type_reference_from_segments(program, type_reference, segments)
        }
        psi_facts::PlaceRoot::Unknown => None,
    }
}

fn call_return_type_reference(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    for machine in program.machines() {
        if machine.symbol == target {
            return program
                .machine_states(machine)
                .first()
                .map(|state| state.return_type);
        }
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target)
        {
            return Some(state.return_type);
        }
    }
    None
}

fn collect_nominal_symbols(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &mut Vec<SymbolHandle>,
) {
    if !type_reference.is_valid() {
        return;
    }
    use psi_typed_trees::types::TypeReferenceNode;
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => collect_nominal_symbols(program, *referee, symbols),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_nominal_symbols(program, *element_type, symbols)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            push_nominal_symbol(program, *base_symbol, symbols);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_nominal_symbols(program, *argument, symbols);
            }
        }
        TypeReferenceNode::DynamicTrait {
            symbol,
            conformance,
            ..
        } => {
            push_nominal_symbol(program, *symbol, symbols);
            if let Some(conformance) = conformance {
                push_nominal_symbol(program, *conformance, symbols);
            }
        }
        TypeReferenceNode::Named { symbol, .. } => push_nominal_symbol(program, *symbol, symbols),
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
    }
}

fn push_nominal_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
    symbols: &mut Vec<SymbolHandle>,
) {
    if !symbol.is_valid()
        || matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::BuiltinType
                | SymbolKind::TypeParameter
                | SymbolKind::ConformanceParameter
                | SymbolKind::MachineParameter
                | SymbolKind::PropositionParameter
                | SymbolKind::PropositionMachineParameter
        )
        || symbols.contains(&symbol)
    {
        return;
    }
    symbols.push(symbol);
}

fn event_exposure(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    root: psi_facts::PlaceRoot,
) -> Exposure {
    let psi_facts::PlaceRoot::Symbol(root_symbol) = root else {
        return Exposure::PrivateImplementation;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol && machine.is_public)
    else {
        return Exposure::PrivateImplementation;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        return Exposure::PrivateImplementation;
    };
    if entry.symbol == state_symbol
        && (root_symbol == machine.symbol
            || program
                .state_parameters(entry)
                .iter()
                .any(|parameter| parameter.symbol == root_symbol))
    {
        Exposure::PublicInterface
    } else {
        Exposure::PrivateImplementation
    }
}

fn machine_signature_exposure(program: &TypedTrees, machine_symbol: SymbolHandle) -> Exposure {
    if program
        .machines()
        .iter()
        .any(|machine| machine.symbol == machine_symbol && machine.is_public)
    {
        Exposure::PublicInterface
    } else {
        Exposure::PrivateImplementation
    }
}

fn push_cleanup_machine(
    rows: &mut Vec<CheckedSemanticDependency>,
    consumer_machine: SymbolHandle,
    cleanup_machine: SymbolHandle,
    exposure: Exposure,
) {
    push_promoting(
        rows,
        CheckedSemanticDependency {
            consumer_machine,
            dependency: cleanup_machine,
            exposure,
            kind: Kind::AutomaticCleanupMachine,
        },
    );
}

fn push_promoting(rows: &mut Vec<CheckedSemanticDependency>, candidate: CheckedSemanticDependency) {
    if let Some(existing) = rows.iter_mut().find(|existing| {
        existing.consumer_machine == candidate.consumer_machine
            && existing.dependency == candidate.dependency
            && existing.kind == candidate.kind
    }) {
        if candidate.exposure == Exposure::PublicInterface {
            existing.exposure = Exposure::PublicInterface;
        }
    } else {
        rows.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use crate::derive_checked_semantic_dependencies;
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn derivation_ignores_retained_rows_and_reproduces_lowering_output() {
        let tokens = Lexer::new(
            r#"
            pub data Token { value: u64; }
            pub machine make() -> Token { Token { value: 7u64 } }
            "#,
        )
        .tokenize()
        .expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let mut checked = crate::lower_typed_trees(typed).expect("check");
        let expected = checked.facts.flow.semantic_dependencies.clone();
        assert!(!expected.rows.is_empty(), "fixture must carry dependencies");

        checked.facts.flow.semantic_dependencies.rows.clear();
        let rederived = derive_checked_semantic_dependencies(&checked.typed, &checked.facts);

        assert_eq!(rederived, expected);
    }
}
