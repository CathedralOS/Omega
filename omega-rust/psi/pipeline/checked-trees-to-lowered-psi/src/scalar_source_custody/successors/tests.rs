use super::*;
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};

fn fixture(copyable: bool, ranked: bool) -> CheckedTrees {
    let properties = if copyable { "[copy]" } else { "" };
    let rank = if ranked {
        "terminates by remaining -> Nat::Descending in 0..6;"
    } else {
        ""
    };
    let source = format!(
        "data Limits {properties} {{ limit: u64; divisor: u64 [3..=5]; }}
         machine walk(remaining: u64 [0..=5], left: Limits, marker: u64, right: Limits)
         {rank} -> u64 {{
             transition remaining > 0 {{
                 true -> walk(remaining - 1, right, marker, left)
                 false -> marker
             }}
         }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("checked mixed successor: {diagnostics:#?}"))
}

fn successor(checked: &CheckedTrees) -> (SymbolHandle, CheckedScalarSuccessor) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .expect("walk");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .expect("mixed scalar graph");
    let state = &graph.states[0];
    let CheckedScalarStateTerminator::Conditional {
        when_true: CheckedScalarBranchDestination::Jump(successor),
        ..
    } = &state.terminator
    else {
        panic!("authored selected successor");
    };
    (state.state, successor.clone())
}

#[test]
fn mixed_roster_preserves_authored_permutation_for_affine_copyable_and_ranked_sources() {
    for copyable in [false, true] {
        for ranked in [false, true] {
            let checked = fixture(copyable, ranked);
            let (state, successor) = successor(&checked);
            assert!(validate(&checked, state, &successor).is_ok());
            let plans = &checked.facts.flow.terminal_scalar_graphs;
            let structural = plans
                .structural_transfers
                .span(successor.structural_transfers)
                .unwrap();
            assert_eq!(structural.len(), 2);
            assert_eq!(
                structural[0].source,
                CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 }
            );
            assert_eq!(
                structural[1].source,
                CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
            );
            let scalar = plans
                .scalar_arguments
                .span(successor.scalar_arguments)
                .unwrap();
            assert_eq!(
                scalar
                    .iter()
                    .map(|row| row.argument_ordinal)
                    .collect::<Vec<_>>(),
                [0, 2]
            );
        }
    }
}

#[test]
fn structural_rows_reject_missing_duplicate_reordered_and_same_typed_foreign_sources() {
    let original = fixture(false, false);
    let (state, original_successor) = successor(&original);
    assert!(validate(&original, state, &original_successor).is_ok());
    for mutation in 0..7 {
        let mut checked = original.clone();
        let mut successor = original_successor.clone();
        let plans = &mut checked.facts.flow.terminal_scalar_graphs;
        let mut rows = plans
            .structural_transfers
            .span(successor.structural_transfers)
            .unwrap()
            .to_vec();
        match mutation {
            0 => {
                rows.pop();
            }
            1 => rows.push(rows[0]),
            2 => rows.swap(0, 1),
            3 => rows[0].source = rows[1].source,
            4 => rows[0].target_parameter_index = 1,
            5 => {
                rows[0].source = CheckedStructuralControlTransferSourcePlan::Parameter { index: 3 }
            }
            _ => {
                rows[0].source = CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                    parameter_index: 1,
                    expression: ExpressionHandle::invalid(),
                }
            }
        }
        successor.structural_transfers = plans.structural_transfers.insert_many(rows);
        assert!(
            validate(&checked, state, &successor).is_err(),
            "structural mutation {mutation}"
        );
    }
}

#[test]
fn scalar_rows_reject_dense_authored_slot_confusion_and_foreign_primitive_reads() {
    let original = fixture(false, false);
    let (state, original_successor) = successor(&original);
    assert!(validate(&original, state, &original_successor).is_ok());
    for mutation in 0..7 {
        let mut checked = original.clone();
        let mut successor = original_successor.clone();
        let plans = &mut checked.facts.flow.terminal_scalar_graphs;
        let mut rows = plans
            .scalar_arguments
            .span(successor.scalar_arguments)
            .unwrap()
            .to_vec();
        match mutation {
            0 => {
                rows.pop();
            }
            1 => rows.push(rows[0]),
            2 => rows.swap(0, 1),
            3 => rows[1].argument_ordinal = 1,
            4 => rows[1].target_scalar_parameter_index = 2,
            5 => rows[1].primitive_type = crate::PrimitiveType::Bool,
            _ => rows[1].source = CheckedStructuralScalarArgumentSourcePlan::Parameter { index: 0 },
        }
        successor.scalar_arguments = plans.scalar_arguments.insert_many(rows);
        assert!(
            validate(&checked, state, &successor).is_err(),
            "scalar mutation {mutation}"
        );
    }
}

#[test]
fn affine_transfer_requires_exact_statement_root_and_claim_free_permission() {
    let original = fixture(false, false);
    let (state, successor) = successor(&original);
    let (machine, authored) = authored_state(&original, state).unwrap();
    let parameter = original.state_parameters(authored)[3].symbol;
    let machine = machine.symbol;
    let statement = successor.statement_ordinal;
    let permission_source =
        transition_permission_source(&original, machine, authored, &successor).unwrap();
    assert_eq!(
        permission_source,
        PermissionEventSource::Call {
            statement_index: statement as usize,
            call_ordinal: 0,
            target_symbol: machine,
        }
    );
    let handle = original
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.machine_symbol == machine
                && event.state_symbol == state
                && event.root == facts::PlaceRoot::Symbol(parameter)
                && event.source == permission_source
                && event.access == PermissionAccess::Owned)
                .then_some(handle)
        })
        .expect("authored affine transfer");
    assert!(
        validate_affine_permission(&original, machine, state, permission_source, parameter).is_ok()
    );
    for mutation in 0..13 {
        let mut checked = original.clone();
        let ownership = &mut checked.facts.flow.ownership;
        let mut event = ownership.permissions.get(handle).clone();
        match mutation {
            0 => event.machine_symbol = SymbolHandle::invalid(),
            1 => event.state_symbol = SymbolHandle::invalid(),
            2 => event.root = facts::PlaceRoot::Symbol(SymbolHandle::invalid()),
            3 => {
                event.source = PermissionEventSource::Statement {
                    statement_index: statement as usize + 1,
                }
            }
            4 => {
                event.source = PermissionEventSource::Call {
                    statement_index: statement as usize,
                    call_ordinal: 0,
                    target_symbol: state,
                }
            }
            5 => event.kind = PermissionEventKind::AffineDrop,
            6 => event.access = PermissionAccess::Shared,
            7 => event.multiplicity = Multiplicity::Unrestricted,
            8 => event.obligation_live = true,
            9 => {
                event.claim_identity = PermissionClaimIdentity::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::StateEntry,
                    ordinal: 0,
                }
            }
            10 => {
                event.provenance = PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::StateEntry,
                }
            }
            11 => {
                event.segments = ownership
                    .segments
                    .insert_many([facts::PlaceSegment::FixedIndex { index: 0 }])
            }
            _ => {
                ownership.permissions.append(event.clone());
            }
        }
        *ownership.permissions.get_mut(handle) = event;
        assert!(
            validate_affine_permission(&checked, machine, state, permission_source, parameter)
                .is_err(),
            "permission mutation {mutation}"
        );
    }
}

#[test]
fn successor_keeps_source_target_and_transition_coordinates() {
    let checked = fixture(false, false);
    let (state, original) = successor(&checked);
    assert!(validate(&checked, state, &original).is_ok());
    for mutation in 0..4 {
        let mut successor = original.clone();
        match mutation {
            0 => successor.statement_ordinal = u32::MAX,
            1 => successor.is_continuation = !successor.is_continuation,
            2 => successor.target = SymbolHandle::invalid(),
            _ => successor.argument_count -= 1,
        }
        assert!(
            validate(&checked, state, &successor).is_err(),
            "coordinate mutation {mutation}"
        );
    }
}
