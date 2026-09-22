use crate::execution::terminal_unit::types::ShapeCollector;

use crate::execution::terminal_unit::control::build_checked_machine;
use crate::tests::front_end::checked_program;
use language_semantics::SemanticDomainTable;

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
            machine Record::replace(&mut self, value: u8) {{ self.value = identity(value) as u8 in {policy}; }}
            machine Record::walk(&mut self, value: u8) {{
                self.value = identity(value) as u8 in {policy};
                transition {{ _ -> done(value) }}
                state done(&mut self, value: u8) {{ self.value = identity(value) as u8 in {policy}; }}
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
                        &[],
                        &[],
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
                        &[],
                        &[],
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
