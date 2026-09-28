use super::machine_and_entry_state;
use crate::tests::front_end::checked_program;

#[test]
fn projected_transition_cleanup_fences_shapes_outside_the_first_checked_cohort() {
    let sources = [
        (
            "free machine",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            machine route(pair: Pair) {
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "extra affine root",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            data Root {}
            machine Root::route(pair: Pair, extra: Token) {
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "non-Unit states",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            data Root {}
            machine Root::route(pair: Pair) -> i32 {
                transition { _ -> next(pair.left) }
                state next(token: Token) -> i32 { 0 }
            }
            "#,
        ),
        (
            "nominal sibling cleanup",
            r#"
            data Token { value: i32; }
            data Resource {}
            machine Resource::drop(&mut self) {}
            data Pair { left: Token; right: Resource; }
            data Root {}
            machine Root::route(pair: Pair) {
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
    ];
    for (case, source) in sources {
        let checked = checked_program(source);
        let (machine, entry) = machine_and_entry_state(&checked, "route");
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_control_cleanups
                .for_projected_edge(machine, entry, 0)
                .is_none(),
            "{case} must remain outside the bounded projected cleanup cohort"
        );
    }
}

#[test]
fn projected_transition_cleanup_admits_wider_exact_paths() {
    let cases: [(&str, &str, &[&[&str]], &str); 4] = [
        (
            "three-field root",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; middle: Token; right: Token; }
            data Root {}
            machine Root::route(pair: Pair) {
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
            &[&["right"], &["middle"]],
            "named(name(Token))",
        ),
        (
            "nested projection",
            r#"
            data Token { value: i32; }
            data Inner { token: Token; spare: Token; }
            data Pair { left: Inner; right: Token; }
            data Root {}
            machine Root::route(pair: Pair) {
                transition { _ -> next(pair.left.token) }
                state next(token: Token) {}
            }
            "#,
            &[&["right"], &["left", "spare"]],
            "named(name(Token))",
        ),
        (
            "array element",
            r#"
            data Token { value: i32; }
            data Root {}
            machine Root::route(items: [Token; 3]) {
                transition { _ -> next(items[2]) }
                state next(token: Token) {}
            }
            "#,
            &[&["1"], &["0"]],
            "named(name(Token))",
        ),
        (
            "array field element",
            r#"
            data Token { value: i32; }
            data Row { items: [Token; 3]; extra: Token; }
            data Root {}
            machine Root::route(row: Row) {
                transition { _ -> next(row.items[1]) }
                state next(token: Token) {}
            }
            "#,
            &[&["extra"], &["items", "2"], &["items", "0"]],
            "named(name(Token))",
        ),
    ];
    for (case, source, expected_residuals, expected_type) in cases {
        let checked = checked_program(source);
        let (machine, entry) = machine_and_entry_state(&checked, "route");
        let edge = checked
            .facts
            .flow
            .terminal_structural_control_cleanups
            .for_projected_edge(machine, entry, 0)
            .unwrap_or_else(|| panic!("{case} should retain checked projected cleanup"));
        assert_eq!(
            edge.transfer.source_parameter_position, 0,
            "{case}: transfer must name the single source parameter"
        );
        assert_eq!(
            edge.transfer.target_parameter_position, 0,
            "{case}: transfer must name the single target parameter"
        );
        assert_eq!(
            edge.transfer.type_identity, expected_type,
            "{case}: moved leaf keeps the target parameter's exact identity"
        );
        let residual_paths = edge
            .residual_affine_discards
            .iter()
            .map(|residual| {
                assert_eq!(
                    residual.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 0
                    },
                    "{case}: residuals keep the source-parameter root"
                );
                assert_eq!(
                    residual.type_identity, expected_type,
                    "{case}: residual keeps its exact leaf identity"
                );
                residual
                    .path
                    .iter()
                    .map(|segment| match segment {
                        checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                            identity.clone()
                        }
                        checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                            index.to_string()
                        }
                        checked_trees::CheckedUnitStructuralPathSegment::Referent => {
                            panic!("{case}: projected residuals never name a referent")
                        }
                        checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange {
                            ..
                        } => {
                            panic!("{case}: projected residuals never name a byte window")
                        }
                        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex {
                            ..
                        } => {
                            panic!("{case}: projected residuals never name a runtime index")
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            residual_paths.len(),
            expected_residuals.len(),
            "{case}: residual count covers exactly the untouched complement"
        );
        for (path, expected_path) in residual_paths.iter().zip(expected_residuals.iter()) {
            assert_eq!(
                path.len(),
                expected_path.len(),
                "{case}: residual path {path:?} length drifted"
            );
            for (segment, expected) in path.iter().zip(expected_path.iter()) {
                assert!(
                    segment == *expected || segment.ends_with(expected),
                    "{case}: residual segment {segment} drifted from {expected}"
                );
            }
        }
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_control_cleanups
                .for_edge(machine, entry, 0)
                .is_none(),
            "{case}: whole-root consumers must fail closed on a projected edge"
        );
        assert_eq!(
            crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
                &checked.typed,
                &checked.facts,
            ),
            checked.facts.flow.terminal_structural_control_cleanups,
            "{case}: projected cleanup reconstruction is deterministic"
        );
    }
}

#[test]
fn projected_transition_cleanup_retains_construction_local_residuals() {
    let checked = checked_program(
        r#"
        data Token { value: i32; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::route(pair: Pair) {
            let spare: Pair = Pair { left: Token { value: 1 }, right: Token { value: 2 } };
            Sink::take(spare.left);
            transition { _ -> next(pair.left) }
            state next(token: Token) {}
        }
        "#,
    );
    let (machine, entry) = machine_and_entry_state(&checked, "route");
    let declaration = checked
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .unwrap();
    let state = &checked.machine_states(declaration)[0];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("the construction-local temporary opens the entry state")
    };
    let edge = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_projected_edge(machine, entry, 2)
        .expect("the transition follows two prefix statements");
    assert_eq!(edge.statement_ordinal, 2);
    assert_eq!(edge.transfer.source_parameter_position, 0);
    assert_eq!(edge.transfer.target_parameter_position, 0);
    assert_eq!(edge.transfer.path.len(), 1);
    assert!(matches!(
        &edge.transfer.path[0],
        checked_trees::CheckedUnitStructuralPathSegment::Field(identity)
            if identity.ends_with("left")
    ));
    let [local_residual, parameter_residual] = edge.residual_affine_discards.as_slice() else {
        panic!("the temporary remainder dies before the parameter remainder")
    };
    assert_eq!(
        local_residual.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: local.symbol
        }
    );
    assert_eq!(local_residual.path.len(), 1);
    assert!(matches!(
        &local_residual.path[0],
        checked_trees::CheckedUnitStructuralPathSegment::Field(identity)
            if identity.ends_with("right")
    ));
    assert_eq!(local_residual.type_identity, edge.transfer.type_identity);
    assert_eq!(
        parameter_residual.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    );
    assert_eq!(parameter_residual.path, local_residual.path);
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_control_cleanups
            .for_edge(machine, entry, 2)
            .is_none(),
        "whole-root consumers must fail closed on a projected edge"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_control_cleanups
            .for_state(machine, entry)
            .is_none(),
        "a state carrying partial local custody publishes no whole-state slice"
    );
    assert_eq!(
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            &checked.typed,
            &checked.facts,
        ),
        checked.facts.flow.terminal_structural_control_cleanups,
        "projected cleanup reconstruction is deterministic"
    );
    let mut missing_exit = checked.facts.clone();
    missing_exit.flow.ownership.permissions = Default::default();
    assert!(
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            &checked.typed,
            &missing_exit,
        )
        .for_projected_edge(machine, entry, 2)
        .is_none(),
        "missing authoritative affine-drop evidence must reject"
    );
}

#[test]
fn projected_transition_cleanup_fences_unaccounted_temporary_custody() {
    let sources = [
        (
            "temporary consumed by two calls",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            data Sink {}
            machine Sink::take(token: Token) {}
            data Root {}
            machine Root::route(pair: Pair) {
                let spare: Pair = Pair { left: Token { value: 1 }, right: Token { value: 2 } };
                Sink::take(spare.left);
                Sink::take(spare.right);
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "temporary moved through the edge argument",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            data Sink {}
            machine Sink::take(token: Token) {}
            data Root {}
            machine Root::route(pair: Pair) {
                let spare: Pair = Pair { left: Token { value: 1 }, right: Token { value: 2 } };
                Sink::take(spare.left);
                transition { _ -> next(spare.right) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "parameter consumed before the edge",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            data Sink {}
            machine Sink::take(token: Token) {}
            data Root {}
            machine Root::route(pair: Pair) {
                Sink::take(pair.left);
                transition { _ -> next(pair.right) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "untouched dying temporary",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            data Root {}
            machine Root::route(pair: Pair) {
                let spare: Pair = Pair { left: Token { value: 1 }, right: Token { value: 2 } };
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
        (
            "nominal temporary root",
            r#"
            data Token { value: i32; }
            data Pair { left: Token; right: Token; }
            data Resource {}
            machine Resource::drop(&mut self) {}
            data Root {}
            machine Root::route(pair: Pair) {
                let resource: Resource = Resource {};
                transition { _ -> next(pair.left) }
                state next(token: Token) {}
            }
            "#,
        ),
    ];
    for (case, source) in sources {
        let checked = checked_program(source);
        let (machine, entry) = machine_and_entry_state(&checked, "route");
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_control_cleanups
                .projected_edges
                .iter()
                .all(|edge| edge.machine != machine || edge.state != entry),
            "{case} must remain outside the construction-local cohort"
        );
    }
}

#[test]
fn projected_transition_cleanup_admits_multi_state_machines() {
    // The projected-edge plan is keyed by (machine, state, statement) and
    // revalidates custody per edge, so a machine with more than two states
    // admits the same bounded cohort as long as the forwarding state and its
    // target keep the one-parameter Unit shape.
    let checked = checked_program(
        r#"
        data Token { value: i32; }
        data Pair { left: Token; right: Token; }
        data Root {}
        machine Root::route(pair: Pair) {
            transition { _ -> mid(pair.left) }
            state mid(token: Token) {
                transition { _ -> sink(token) }
            }
            state sink(token: Token) {}
        }
        "#,
    );
    let (machine, entry) = machine_and_entry_state(&checked, "route");
    let edge = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_projected_edge(machine, entry, 0)
        .expect("the multi-state machine keeps its projected entry-edge cleanup");
    assert_eq!(edge.machine, machine);
    assert_eq!(edge.state, entry);
    assert_eq!(edge.transfer.source_parameter_position, 0);
    assert_eq!(edge.transfer.path.len(), 1);
    let [sibling] = edge.residual_affine_discards.as_slice() else {
        panic!("exactly one maximal sibling residual should remain")
    };
    assert_eq!(
        sibling.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    );
    assert!(matches!(
        &sibling.path[0],
        checked_trees::CheckedUnitStructuralPathSegment::Field(identity)
            if identity.ends_with("right")
    ));

    // The downstream consumer replays the recorded evidence exactly, so a
    // drifted row still cannot join.
    let rebuilt =
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            &checked.typed,
            &checked.facts,
        );
    assert_eq!(
        rebuilt,
        checked.facts.flow.terminal_structural_control_cleanups
    );
}
