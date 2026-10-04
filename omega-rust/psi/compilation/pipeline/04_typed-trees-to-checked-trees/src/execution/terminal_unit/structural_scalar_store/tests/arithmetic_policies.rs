use crate::execution::terminal_unit::types::ShapeCollector;

use crate::execution::terminal_unit::control::build_checked_machine;
use crate::tests::front_end::checked_program;
use language_semantics::SemanticDomainTable;

#[test]
fn owned_local_stores_use_statement_sequence_field_policy() {
    for policy in ["Wrapping", "Saturating", "Trapping"] {
        let source = format!(
            "data Record {{ value: u8 in {policy}; }}
             machine run(value: u8) {{
                 let mut record: Record;
                 record.value = value as u8 in {policy};
             }}"
        );
        let checked = checked_program(&source);
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "run")
            .unwrap();
        let mut shapes = ShapeCollector::new(program);
        let plan = build_checked_machine(
            program,
            &checked.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
            },
            &mut shapes,
            machine,
            None,
        );
        assert!(plan.is_some(), "owned local with {policy} field");
    }
}

#[test]
fn builtin_store_policies_do_not_become_nominal_body_qualifications() {
    for (policy, identity) in [
        ("Wrapping", SemanticDomainTable::WRAPPING),
        ("Saturating", SemanticDomainTable::SATURATING),
        ("Trapping", SemanticDomainTable::TRAPPING),
    ] {
        let source = format!(
            r#"
            domain u8::Marker requires true;
            machine identity(value: u8) -> u8 {{ value }}
            data Record {{ value: u8 in {policy}; }}
            machine Record::replace(record: &mut Record, value: u8) {{ record.value = identity(value) as u8 in {policy}; }}
            machine Record::walk(record: &mut Record, value: u8) {{
                record.value = identity(value) as u8 in {policy};
                transition {{ _ -> done(record, value) }}
                state done(record: &mut Record, value: u8) {{ record.value = identity(value) as u8 in {policy}; }}
            }}
        "#
        );
        let checked = checked_program(&source);
        let program = &checked.typed;
        let nominal = program.domain_definitions()[0].semantic_id;
        assert_ne!(nominal, identity);
        for (name, graph) in [("Record::replace", false), ("Record::walk", true)] {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
                .unwrap();
            assert!(
                checked
                    .facts
                    .qualifications
                    .for_machine(machine.symbol)
                    .unwrap()
                    .body_committed
                    .contains(&identity)
            );
            let mut shapes = ShapeCollector::new(program);
            if graph {
                assert!(
                    super::super::super::state_graph::build(
                        program,
                        &checked.facts,
                        crate::execution::terminal_unit::ScalarCalleePlans {
                            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                            structural_returns: &checked
                                .facts
                                .flow
                                .terminal_structural_scalar_returns
                        },
                        &mut shapes,
                        machine,
                        None,
                    )
                    .unwrap()
                    .body_qualifications
                    .is_empty()
                );
            } else {
                assert!(
                    build_checked_machine(
                        program,
                        &checked.facts,
                        crate::execution::terminal_unit::ScalarCalleePlans {
                            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                            structural_returns: &checked
                                .facts
                                .flow
                                .terminal_structural_scalar_returns
                        },
                        &mut shapes,
                        machine,
                        None,
                    )
                    .unwrap()
                    .body_qualifications
                    .is_empty()
                );
            }
            let mut facts = checked.facts.clone();
            facts
                .qualifications
                .machines
                .iter_mut()
                .find(|fact| fact.machine == machine.symbol)
                .unwrap()
                .body_committed
                .push(nominal);
            if graph {
                assert!(
                    super::super::super::state_graph::build(
                        program,
                        &facts,
                        crate::execution::terminal_unit::ScalarCalleePlans {
                            boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                            structural_returns: &facts.flow.terminal_structural_scalar_returns
                        },
                        &mut shapes,
                        machine,
                        None,
                    )
                    .is_none()
                );
            } else {
                assert_eq!(
                    build_checked_machine(
                        program,
                        &facts,
                        crate::execution::terminal_unit::ScalarCalleePlans {
                            boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                            structural_returns: &facts.flow.terminal_structural_scalar_returns
                        },
                        &mut shapes,
                        machine,
                        None,
                    )
                    .unwrap()
                    .body_qualifications,
                    vec![nominal]
                );
            }
        }
    }
}

#[test]
fn whole_borrowed_primitive_stores_share_arithmetic_policy_leaf_normalization() {
    use checked_trees::CheckedIntegerBinaryKind;
    for (policy, expected_kind) in [
        ("Wrapping", CheckedIntegerBinaryKind::WrappingAdd),
        ("Saturating", CheckedIntegerBinaryKind::SaturatingAdd),
        ("Trapping", CheckedIntegerBinaryKind::TrappingAdd),
    ] {
        let checked = checked_program(&format!(
            "machine bump(hits: &mut i32 in {policy}) {{ hits = hits + 1; }}"
        ));
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "bump")
            .unwrap();
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|plan| plan.machine == machine.symbol)
            .unwrap_or_else(|| panic!("missing whole borrowed {policy} store plan"));
        assert!(
            plan.operations.iter().any(|operation| matches!(
                operation,
                checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    destination: checked_trees::CheckedStoreRoot::Parameter { position: 0 },
                    path,
                    value: checked_trees::CheckedCallScalarArgument::Pure(
                        checked_trees::CheckedScalarExpression::IntegerBinary { kind, left, .. }
                    ),
                    ..
                } if path.is_empty()
                    && *kind == expected_kind
                    && matches!(left.as_ref(), checked_trees::CheckedScalarExpression::StorageRead { symbol, .. }
                        if *symbol == program.state_parameters(&program.machine_states(machine)[0])[0].symbol)
            )),
            "{policy}: {plan:#?}",
        );

        // Arithmetic-policy normalization does not grant mutation to a shared
        // checked destination, even when its root and scalar type still match.
        let state = &program.machine_states(machine)[0];
        let mut parameters = plan.structural_parameters.clone();
        parameters[0].access = checked_trees::CheckedStructuralAccess::SharedBorrow;
        let typed_trees::statement::StatementNode::Assignment(assignment) =
            &program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("authored increment")
        };
        assert!(
            super::super::plan_assignment(
                program,
                &checked.facts,
                machine,
                state,
                super::super::destination::StoreRoots::Parameters {
                    structural: &parameters,
                    scalar: &plan.scalar_parameters,
                },
                0,
                assignment,
                super::super::AssignmentSource::Authored,
                &crate::execution::terminal_unit::control::LocalConstructionTrace::default(),
            )
            .is_none(),
            "{policy} must not amplify a shared destination",
        );
    }
}

#[test]
fn whole_borrowed_primitive_stores_do_not_peel_range_or_nominal_domains() {
    for (declaration, constraint) in [
        ("", "[0..=8]"),
        (
            "domain u64::Slot requires self <= 8 && self != 5;",
            "in Slot",
        ),
    ] {
        let checked = checked_program(&format!(
            "{declaration}
             machine replace(value: &mut u64 {constraint}) {{ value = 3; }}"
        ));
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "replace")
            .unwrap();
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter()
                .all(|plan| plan.machine != machine.symbol),
            "{constraint} needs its own write obligations, not arithmetic-policy peeling",
        );
    }
}

#[test]
fn arithmetic_policy_references_keep_read_and_write_authority_distinct() {
    let checked = checked_program(
        "machine copy(source: &i32 in Wrapping, destination: &mut i32 in Wrapping) {
             destination = source + 1;
         }",
    );
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "copy")
        .unwrap();
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .any(|plan| plan.machine == machine.symbol),
        "shared policy reference remains readable"
    );

    let diagnostics = crate::tests::front_end::checked_program_result(
        "machine bump(hits: &write i32 in Wrapping) { hits = hits + 1; }",
    )
    .expect_err("write-only access cannot supply the increment's observation");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("write-only")),
        "{diagnostics:#?}"
    );
}
