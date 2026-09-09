use super::*;

#[test]
fn indexed_flow_lists_match_independent_scans_as_machine_count_grows() {
    for machine_count in [8, 32, 128] {
        let mut semantic = FactPlan::default();
        semantic.append_fact_context(facts::Fact::default());
        for index in 1..=machine_count {
            let machine_symbol = SymbolHandle::from_arena_index(index);
            semantic.append_context(
                ProgramPoint::Machine { machine_symbol },
                HandleSpan::empty(),
            );
            for state_index in 0..4 {
                let state_symbol =
                    SymbolHandle::from_arena_index(machine_count + index * 4 + state_index);
                semantic.append_context(
                    ProgramPoint::State {
                        machine_symbol,
                        state_symbol,
                    },
                    HandleSpan::empty(),
                );
                for statement_index in 0..16 {
                    semantic.append_context(
                        ProgramPoint::Statement {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                        },
                        HandleSpan::empty(),
                    );
                }
            }
        }

        let mut selected_contexts = 0;
        let mut reference_inspections = 0;
        for index in 1..=machine_count {
            let machine_symbol = SymbolHandle::from_arena_index(index);
            for state_index in 0..4 {
                let state_symbol =
                    SymbolHandle::from_arena_index(machine_count + index * 4 + state_index);
                let state_point = ProgramPoint::State {
                    machine_symbol,
                    state_symbol,
                };
                // State-input inference appends facts after initial preparation.
                semantic.append_fact_context(facts::Fact {
                    point: state_point,
                    ..Default::default()
                });
                let points = [
                    ProgramPoint::Global,
                    ProgramPoint::Machine { machine_symbol },
                    state_point,
                ];
                let mut contexts = arena::Arena::default();
                let mut constraints = arena::Arena::default();
                let mut context_span = HandleSpan::empty();
                let mut constraint_span = HandleSpan::empty();
                append_flow_contexts_for_points(
                    &semantic,
                    &mut contexts,
                    &mut context_span,
                    &mut constraints,
                    &mut constraint_span,
                    &points,
                );

                let mut reference_contexts = arena::Arena::default();
                let mut reference_constraints = arena::Arena::default();
                let mut reference_context_span = HandleSpan::empty();
                let mut reference_constraint_span = HandleSpan::empty();
                for point in points {
                    for (context, value) in semantic.contexts.iter() {
                        reference_inspections += 1;
                        if value.point == point {
                            reference_contexts.append_to_span(
                                &mut reference_context_span,
                                FlowSemanticContextRef { context },
                            );
                        }
                    }
                }
                for point in points {
                    for (context, value) in semantic.contexts.iter() {
                        reference_inspections += 1;
                        if value.point == point {
                            append_constraint_ref(
                                &mut reference_constraints,
                                &mut reference_constraint_span,
                                FlowConstraintKind::SemanticContext { context },
                            );
                        }
                    }
                }
                assert_eq!(contexts, reference_contexts);
                assert_eq!(constraints, reference_constraints);
                assert_eq!(context_span, reference_context_span);
                assert_eq!(constraint_span, reference_constraint_span);
                selected_contexts += contexts.len();
            }
        }
        assert_eq!(selected_contexts, (machine_count * 4 * 4) as usize);
        assert!(reference_inspections > selected_contexts * machine_count as usize);
    }
}
