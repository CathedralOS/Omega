use super::{
    BigInt, BinaryOperator, Engine, ExpressionNode, Polynomial, PrimitiveType, RankingRangeMeasure,
    RankingRangePremises, RankingRangeState, TypeReferenceNode, TypedTrees,
    exact_integer_parameter, lengths, meanings, prove_ranking_range_transition, validate_mapping,
};
#[test]
fn constructor_operands_retain_their_nominal_type_for_selected_meaning() {
    let program =
        typed("data Card { rank: u64; } machine make(card: Card) -> Card { Card { rank: 1 } }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (expression, owner) = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            if let ExpressionNode::StructLiteral(literal) = node {
                Some((handle, literal.type_symbol))
            } else {
                None
            }
        })
        .expect("constructor operand");
    let expected = program
        .type_reference_table
        .find_named_type_reference(owner)
        .expect("exact Card carrier");
    assert_eq!(
        meanings::builtin(&program, machine, state, expression, 0),
        Some(Some(expected))
    );
}

#[test]
fn state_aliases_cannot_reuse_root_or_sibling_formal_symbols() {
    let mut program = typed(
        "machine walk(left: u32, right: u32) -> u32 { transition { _ -> next(right, left) } state next(first: u32, second: u32) { first } }",
    );
    let machine = program.machines()[0].clone();
    let root = program.machine_states(&machine)[0].clone();
    let state = program.machine_states(&machine)[1].clone();
    let root_parameters = program.state_parameters(&root);
    let mapping = [root_parameters[1].symbol, root_parameters[0].symbol];
    assert!(validate_mapping(&program, &machine, &state, &mapping).is_some());
    let original = program.state_parameters(&state)[0].symbol;
    for forged in [mapping[1], program.state_parameters(&state)[1].symbol] {
        program.state_parameters.span_mut_or_empty(state.parameters)[0].symbol = forged;
        assert!(validate_mapping(&program, &machine, &state, &mapping).is_none());
    }
    program.state_parameters.span_mut_or_empty(state.parameters)[0].symbol = original;
    assert!(validate_mapping(&program, &machine, &state, &mapping).is_some());
}

#[test]
fn missing_auxiliary_source_alias_cannot_skip_destination_copy_equality() {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};

    let program = typed(
        r#"
        machine walk(remaining: u32 [0..=5], step: u32)
        requires step > 0;
        terminates by remaining in 0..=5;
        -> u32 {
            transition { _ -> drop(remaining) }
            state drop(pending: u32 [0..=5]) {
                transition { _ -> next(pending, 0, 1) }
            }
            state next(left: u32, first: u32, second: u32) { left }
        }
    "#,
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    let root = &states[0];
    let source = &states[1];
    let destination = &states[2];
    let parameters = program.state_parameters(root);
    let custody = program
        .ranking_expression_custody_for(machine.symbol)
        .unwrap();
    let subject = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Name(name) if name.symbol == parameters[0].symbol)
                .then_some(handle)
        })
        .unwrap();
    let StatementNode::Transition(transition) =
        &program.statement_table.statements(source.statement_nodes)[0]
    else {
        panic!("one arrival");
    };
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        panic!("named arrival");
    };
    assert!(
        prove_ranking_range_transition(
            &program,
            machine,
            custody.rank_range.unwrap(),
            RankingRangeMeasure::Single(subject),
            RankingRangePremises::EntryInvariant,
            RankingRangeState {
                state: source,
                entry_parameters: &[parameters[0].symbol]
            },
            RankingRangeState {
                state: destination,
                entry_parameters: &[
                    parameters[0].symbol,
                    parameters[1].symbol,
                    parameters[1].symbol
                ]
            },
            &[],
            &[],
            program.statement_table.expression_handles(*arguments),
        )
        .is_none()
    );
}

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn slice_length_projection_does_not_numeric_bind_its_descriptor() {
    let program = typed("machine length(values: &[u8]) -> u64 { values.len }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (expression, receiver) = program
        .expression_table
        .iter_expressions()
        .find_map(|(expression, _)| {
            crate::value_custody::places::collection_length_receiver(
                &program,
                machine,
                Some(state),
                expression,
            )
            .map(|receiver| (expression, receiver))
        })
        .unwrap();
    let bindings = lengths::bindings(&program, machine, state, None);
    let mut engine = Engine::strict_with_symbol_bindings(&program, machine, &[]);
    assert!(engine.normalize(expression).is_none());
    lengths::install(
        &program,
        machine,
        state,
        state,
        &bindings,
        &mut engine,
        &[expression],
    )
    .unwrap();
    let length = engine.normalize(expression).unwrap();
    assert_eq!(length, Polynomial::atom(bindings[0].1.clone()));
    assert!(engine.normalize(receiver).is_none());
    assert!(engine.bind_strict_projection(expression, length));
    assert!(!engine.bind_strict_projection(expression, Polynomial::default()));
    assert!(!engine.bind_strict_projection(receiver, Polynomial::default()));
}

#[test]
fn slice_projection_checks_subslice_geometry_before_using_its_length() {
    let program = typed("machine tail(values: &[u8]) -> &[u8] { values[1..] }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = program
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, ExpressionNode::Indexed(_)).then_some(expression)
        })
        .unwrap();
    let bindings = lengths::bindings(&program, machine, state, None);
    let length = Polynomial::atom(bindings[0].1.clone());
    let mut engine = Engine::strict_with_symbol_bindings(&program, machine, &[]);
    assert!(
        lengths::actual(&program, machine, state, expression, &bindings, &mut engine).is_none()
    );
    assert!(engine.install_hypotheses(vec![(
        BinaryOperator::GreaterOrEqual,
        length.clone(),
        Polynomial::constant(BigInt::from_i64(1))
    )]));
    assert_eq!(
        lengths::actual(&program, machine, state, expression, &bindings, &mut engine),
        Some(length.sub(&Polynomial::constant(BigInt::from_i64(1))))
    );
}

#[test]
fn integer_rank_bindings_require_the_canonical_type_symbol() {
    let tokens = source_files_to_tokens::Lexer::new(
        "machine value(input: u64, signed: i64) -> u64 { input }",
    )
    .tokenize()
    .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let mut program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let parameters = program.state_parameters(state);
    let unsigned = parameters[0].type_reference;
    let signed = parameters[1].type_reference;
    assert_eq!(
        exact_integer_parameter(&program, unsigned),
        Some(PrimitiveType::U64)
    );
    let TypeReferenceNode::Named { name, .. } = program
        .type_reference_table
        .type_reference(unsigned)
        .clone()
    else {
        panic!("primitive unsigned type");
    };
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(signed).clone()
    else {
        panic!("primitive signed type");
    };
    for symbol in [Default::default(), symbol] {
        let forged = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: name.clone(),
            });
        assert_eq!(exact_integer_parameter(&program, forged), None);
    }
}

mod scalar_views {
    //! Declared scalar views beyond identity produce their rank from the body.
    use super::super::{
        RankingRangeEdgeProof, RankingRangeMeasure, RankingRangePremises, declared_scalar_view,
        prove_ranking_range_edge, prove_ranking_range_entry,
    };
    use super::typed;
    use typed_trees::TypedTrees;
    use typed_trees::machine::Machine;
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    const DOUBLED: &str = r#"
        data Countdown {}
        measure Countdown::Doubled(value: u8) -> u8 { value * 2 }
        machine walk(remaining: u8 [0..=100])
        terminates by remaining -> Countdown::Doubled in 0..=200;
        -> u8 {
            transition remaining > 0 {
                true -> walk(remaining - 1)
                false -> remaining
            }
        }
    "#;

    fn view(program: &TypedTrees) -> Option<super::super::DeclaredScalarView> {
        let machine = &program.machines()[0];
        let root = &program.machine_states(machine)[0];
        let custody = program
            .ranking_expression_custody_for(machine.symbol)
            .expect("custody");
        declared_scalar_view(
            program,
            root,
            custody.subjects[0],
            &machine
                .termination_plan
                .implementation_witness
                .as_ref()
                .expect("witness")
                .view_path,
        )
    }

    fn computed(program: &TypedTrees) -> RankingRangeMeasure {
        let machine = &program.machines()[0];
        let custody = program
            .ranking_expression_custody_for(machine.symbol)
            .expect("custody");
        let selected = view(program).expect("computation view");
        let computation = selected.computation.expect("computed body");
        RankingRangeMeasure::Computed {
            subject: custody.subjects[0],
            parameter: computation.parameter,
            body: computation.body,
            carrier: selected.carrier,
        }
    }

    fn entry(program: &TypedTrees) -> bool {
        let machine: &Machine = &program.machines()[0];
        let root = &program.machine_states(machine)[0];
        let range = program
            .ranking_expression_custody_for(machine.symbol)
            .and_then(|custody| custody.rank_range)
            .expect("range");
        prove_ranking_range_entry(program, machine, root, range, computed(program))
    }

    fn self_edge(program: &TypedTrees) -> Option<RankingRangeEdgeProof> {
        let machine = &program.machines()[0];
        let root = &program.machine_states(machine)[0];
        let range = program
            .ranking_expression_custody_for(machine.symbol)
            .and_then(|custody| custody.rank_range)
            .expect("range");
        let StatementNode::Transition(transition) =
            &program.statement_table.statements(root.statement_nodes)[0]
        else {
            panic!("one transition");
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            panic!("guarded transition");
        };
        let TransitionTargetNode::Named { arguments, .. } =
            program.statement_table.transition_target(transition.target)
        else {
            panic!("named self edge");
        };
        prove_ranking_range_edge(
            program,
            machine,
            root,
            range,
            computed(program),
            RankingRangePremises::RankInvariant,
            &[(guard, true)],
            &[],
            program.statement_table.expression_handles(*arguments),
        )
    }

    #[test]
    fn computation_bodies_classify_only_when_strictly_increasing_and_builtin() {
        let program = typed(DOUBLED);
        let selected = view(&program).expect("doubled view");
        assert!(selected.computation.is_some());
        for body in [
            "value + 1",
            "value * value + 3",
            "(value + 1) * 2",
            "1 + value",
        ] {
            let program = typed(&DOUBLED.replace("value * 2", body));
            assert!(
                view(&program).is_some_and(|view| view.computation.is_some()),
                "{body}"
            );
        }
        // Identity stays identity; no computation is attached.
        let identity = typed(&DOUBLED.replace("{ value * 2 }", "{ value }"));
        assert!(view(&identity).is_some_and(|view| view.computation.is_none()));
        // Constant, non-monotone, subtracting, or widening bodies do not apply.
        for body in ["value * 0 + 1", "value - 1", "value * 0"] {
            let program = typed(&DOUBLED.replace("value * 2", body));
            assert!(view(&program).is_none(), "{body}");
        }
        let widened = typed(&DOUBLED.replace("(value: u8) -> u8", "(value: u16) -> u16"));
        assert!(view(&widened).is_none());
        // An authored operator owning the spelling removes builtin meaning.
        let authored = typed(&format!(
            "operator * u8::mul(left: u8, right: u8) -> u8; {DOUBLED}"
        ));
        assert!(view(&authored).is_none());
    }

    #[test]
    fn computed_ranks_prove_membership_descent_and_carrier_formation() {
        let program = typed(DOUBLED);
        assert!(entry(&program));
        let proof = self_edge(&program).expect("self edge");
        assert!(proof.membership_and_pinning && proof.strictly_decreases);
        // The range constrains the produced rank, not the subject: the
        // subject's own interval is too narrow for `value * 2` at entry. A
        // cyclic edge only preserves membership under the rank invariant it
        // assumes, so entry is where the narrow range rejects.
        let narrow = typed(&DOUBLED.replace("in 0..=200", "in 0..=100"));
        assert!(!entry(&narrow));
        assert!(self_edge(&narrow).is_some_and(|proof| proof.strictly_decreases));
        // Membership inside the authored range is not formation inside the
        // carrier: `[0..=200] * 2` fits `0..=400` but not `u8`.
        let overflowing = typed(
            &DOUBLED
                .replace("[0..=100]", "[0..=200]")
                .replace("in 0..=200", "in 0..=400"),
        );
        assert!(!entry(&overflowing));
        // An unconstrained subject proves neither membership nor formation.
        let unbounded = typed(&DOUBLED.replace("remaining: u8 [0..=100]", "remaining: u8"));
        assert!(!entry(&unbounded));
        // A stalled self edge keeps its membership but proves no descent.
        let stalled = typed(&DOUBLED.replace("walk(remaining - 1)", "walk(remaining)"));
        let proof = self_edge(&stalled).expect("stalled edge");
        assert!(proof.membership_and_pinning && !proof.strictly_decreases);
    }
}

mod field_views {
    //! A declared field view's coordinate follows the measure body's exact
    //! projection chain and reads a borrowed subject's referent.
    use super::super::{
        RankingRangeEdgeProof, RankingRangeMeasure, RankingRangePremises, prove_ranking_range_edge,
        prove_ranking_range_entry,
    };
    use super::typed;
    use typed_trees::TypedTrees;
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    const NESTED: &str = r#"
        data Inner { remaining: u64 [0..=5]; }
        data Bounds { limit: u64 [0..=5]; }
        data Countdown { inner: Inner; bounds: Bounds; }
        measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.inner.remaining }
        machine walk(countdown: Countdown)
        requires countdown.inner.remaining <= countdown.bounds.limit;
        terminates by countdown -> Countdown::Remaining in 0..=countdown.bounds.limit;
        -> u64 {
            transition countdown.inner.remaining > 0 {
                true -> walk(Countdown { inner: Inner { remaining: countdown.inner.remaining - 1 }, bounds: countdown.bounds })
                false -> countdown.inner.remaining
            }
        }
    "#;

    const BORROWED: &str = r#"
        data Card { power: u64; }
        measure Card::PowerOrder(card: Card) -> u64 { card.power }
        machine walk(card: &Card, ceiling: u64 [0..=5])
        requires card.power <= ceiling;
        terminates by card -> Card::PowerOrder in 0..=ceiling;
        -> u64 {
            transition card.power > 0 {
                true -> walk(&Card { power: card.power - 1 }, ceiling)
                false -> card.power
            }
        }
    "#;

    const FLOW_BOUND: &str = r#"
        data Countdown { remaining: u64 [0..=5]; limit: u64; }
        measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.remaining }
        machine walk(countdown: Countdown)
        requires countdown.remaining <= countdown.limit && countdown.limit <= 5;
        terminates by countdown -> Countdown::Remaining in 0..(countdown.limit + 1);
        -> u64 {
            transition countdown.remaining > 0 {
                true -> walk(Countdown { remaining: countdown.remaining - 1, limit: countdown.limit })
                false -> countdown.remaining
            }
        }
    "#;

    fn field(program: &TypedTrees) -> RankingRangeMeasure {
        let machine = &program.machines()[0];
        let custody = program
            .ranking_expression_custody_for(machine.symbol)
            .expect("custody");
        RankingRangeMeasure::Field {
            subject: custody.subjects[0],
            measure: program.measures()[0].symbol,
        }
    }

    fn entry(program: &TypedTrees) -> bool {
        let machine = &program.machines()[0];
        let root = &program.machine_states(machine)[0];
        let range = program
            .ranking_expression_custody_for(machine.symbol)
            .and_then(|custody| custody.rank_range)
            .expect("range");
        prove_ranking_range_entry(program, machine, root, range, field(program))
    }

    fn self_edge(program: &TypedTrees) -> Option<RankingRangeEdgeProof> {
        let machine = &program.machines()[0];
        let root = &program.machine_states(machine)[0];
        let range = program
            .ranking_expression_custody_for(machine.symbol)
            .and_then(|custody| custody.rank_range)
            .expect("range");
        let StatementNode::Transition(transition) =
            &program.statement_table.statements(root.statement_nodes)[0]
        else {
            panic!("one transition");
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            panic!("guarded transition");
        };
        let TransitionTargetNode::Named { arguments, .. } =
            program.statement_table.transition_target(transition.target)
        else {
            panic!("named self edge");
        };
        prove_ranking_range_edge(
            program,
            machine,
            root,
            range,
            field(program),
            RankingRangePremises::EntryInvariant,
            &[(guard, true)],
            &[],
            program.statement_table.expression_handles(*arguments),
        )
    }

    #[test]
    fn nested_projection_ranks_and_pins_through_the_exact_chain() {
        let program = typed(NESTED);
        assert!(entry(&program));
        let proof = self_edge(&program).expect("self edge");
        assert!(proof.membership_and_pinning && proof.strictly_decreases);
        // Rebuilding the sibling record with a literal limit still contains
        // every next rank, but it is not the pinned endpoint.
        let written =
            typed(&NESTED.replace("bounds: countdown.bounds", "bounds: Bounds { limit: 5 }"));
        assert!(entry(&written));
        assert!(self_edge(&written).is_none());
        // Forwarding the ranked sub-record keeps membership without descent.
        let stalled = typed(&NESTED.replace(
            "inner: Inner { remaining: countdown.inner.remaining - 1 }",
            "inner: countdown.inner",
        ));
        let proof = self_edge(&stalled).expect("stalled edge");
        assert!(proof.membership_and_pinning && !proof.strictly_decreases);
        // Entry evidence must name the exact path.
        let unrelated = typed(&NESTED.replace(
            "requires countdown.inner.remaining <= countdown.bounds.limit;",
            "requires countdown.bounds.limit <= countdown.bounds.limit;",
        ));
        assert!(!entry(&unrelated));
        // A same-spelled field of another declaration is a foreign owner.
        let foreign = typed(&format!(
            "data Twin {{ remaining: u64 [0..=5]; }} {}",
            NESTED.replace("inner: Inner { remaining", "inner: Twin { remaining")
        ));
        assert!(self_edge(&foreign).is_none());
    }

    #[test]
    fn computed_endpoint_lands_under_a_flow_bound_not_in_the_declaration() {
        // `limit` is an unbounded u64 field, so `limit + 1` is not statically
        // formed; the requires bound lands it inside the carrier under the
        // edge's own hypotheses.
        let program = typed(FLOW_BOUND);
        assert!(entry(&program));
        let proof = self_edge(&program).expect("self edge");
        assert!(proof.membership_and_pinning && proof.strictly_decreases);
        // Without the flow bound the same endpoint can overflow its carrier.
        let unbounded = typed(&FLOW_BOUND.replace(" && countdown.limit <= 5", ""));
        assert!(!entry(&unbounded));
    }

    #[test]
    fn computed_endpoint_operations_land_under_the_same_hypotheses() {
        // The normalized polynomial cannot excuse an intermediate that
        // already escaped its carrier: `limit + u64::MAX` overflows before
        // the trailing `- u64::MAX + 1` cancels it away.
        let overflowing = typed(&FLOW_BOUND.replace(
            "countdown.limit + 1",
            "(countdown.limit + 18446744073709551615u64) - 18446744073709551615u64 + 1",
        ));
        assert!(!entry(&overflowing));
        // An intermediate underflow is just as invisible to the final
        // polynomial.
        let underflowing =
            typed(&FLOW_BOUND.replace("countdown.limit + 1", "countdown.limit - 6 + 8"));
        assert!(!entry(&underflowing));
        // A `u8` literal selects no shared carrier with the `u64` field.
        let mixed = typed(&FLOW_BOUND.replace("countdown.limit + 1", "countdown.limit + 1u8"));
        assert!(!entry(&mixed));
        // `255u8 + 1u8` has no builtin carrier of its own, but its operation
        // still computes in `u8` and cannot form `256`.
        let anonymous_carrier = typed(&FLOW_BOUND.replace("countdown.limit + 1", "255u8 + 1u8"));
        assert!(!entry(&anonymous_carrier));
        // An endpoint whose every operation lands under the hypotheses still
        // proves.
        let landing =
            typed(&FLOW_BOUND.replace("countdown.limit + 1", "(countdown.limit + 2) - 1"));
        assert!(entry(&landing));
    }

    #[test]
    fn borrowed_subject_reads_its_referent_and_arrives_as_a_borrow() {
        let program = typed(BORROWED);
        assert!(entry(&program));
        let proof = self_edge(&program).expect("self edge");
        assert!(proof.membership_and_pinning && proof.strictly_decreases);
        let replaced = typed(&BORROWED.replace("}, ceiling)", "}, 5)"));
        assert!(self_edge(&replaced).is_none());
        // The formal itself forwarded keeps the referent, so no descent.
        let forwarded = typed(&BORROWED.replace("&Card { power: card.power - 1 }", "card"));
        let proof = self_edge(&forwarded).expect("forwarded edge");
        assert!(proof.membership_and_pinning && !proof.strictly_decreases);
        // An owned literal is not an arrival of the borrowed formal.
        let owned = typed(&BORROWED.replace("walk(&Card {", "walk(Card {"));
        assert!(self_edge(&owned).is_none());
        // Without the entry fact nothing bounds the unconstrained field.
        let unbounded = typed(&BORROWED.replace("requires card.power <= ceiling;", ""));
        assert!(!entry(&unbounded));
    }
}

mod remainder_endpoints {
    //! Opaque remainder atoms embed their operand's display: the edge map
    //! transports them only through the operand's own simultaneous
    //! substitution, re-minted under the actual's exact polynomial.
    use super::super::{
        RankingRangeEdgeProof, RankingRangeMeasure, RankingRangePremises, prove_ranking_range_edge,
        prove_ranking_range_entry,
    };
    use super::typed;
    use typed_trees::TypedTrees;
    use typed_trees::machine::Machine;
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    const COMPUTED_COPY: &str = r#"
        machine dive(remaining: u64 [0..=5], cap: u64)
        terminates by remaining in 0..(cap % 5 + 6);
        -> u64 {
            transition remaining > 0 {
                true -> dive(remaining - 1, cap - 0)
                false -> remaining
            }
        }
    "#;

    const MOVED_COPY: &str = r#"
        machine dive(remaining: u64 [0..=5], cap: u64, spare: u64)
        terminates by remaining in 0..(cap % 5 + 6);
        -> u64 {
            transition remaining > 0 {
                true -> dive(remaining - 1, spare, cap)
                false -> remaining
            }
        }
    "#;

    fn edge(program: &TypedTrees, premises: RankingRangePremises) -> Option<RankingRangeEdgeProof> {
        let machine: &Machine = &program.machines()[0];
        let root = &program.machine_states(machine)[0];
        let custody = program
            .ranking_expression_custody_for(machine.symbol)
            .expect("custody");
        let range = custody.rank_range.expect("range");
        let StatementNode::Transition(transition) =
            &program.statement_table.statements(root.statement_nodes)[0]
        else {
            panic!("one transition");
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            panic!("guarded transition");
        };
        let TransitionTargetNode::Named { arguments, .. } =
            program.statement_table.transition_target(transition.target)
        else {
            panic!("named self edge");
        };
        prove_ranking_range_edge(
            program,
            machine,
            root,
            range,
            RankingRangeMeasure::Single(custody.subjects[0]),
            premises,
            &[(guard, true)],
            &[],
            program.statement_table.expression_handles(*arguments),
        )
    }

    fn entry(program: &TypedTrees) -> bool {
        let machine: &Machine = &program.machines()[0];
        let root = &program.machine_states(machine)[0];
        let custody = program
            .ranking_expression_custody_for(machine.symbol)
            .expect("custody");
        let range = custody.rank_range.expect("range");
        prove_ranking_range_entry(
            program,
            machine,
            root,
            range,
            RankingRangeMeasure::Single(custody.subjects[0]),
        )
    }

    #[test]
    fn remainder_endpoints_transport_through_value_equal_actuals() {
        // `cap - 0` is not the bare formal the static pin requires, but the
        // relational edge proves the substituted endpoint identical.
        let program = typed(COMPUTED_COPY);
        assert!(entry(&program));
        for premises in [
            RankingRangePremises::RankInvariant,
            RankingRangePremises::EntryInvariant,
        ] {
            let proof = edge(&program, premises).expect("self edge");
            assert!(proof.membership_and_pinning && proof.strictly_decreases);
        }
        // A different carrier changes the endpoint value: the re-minted atom
        // keeps the operand's exact identity rather than forcing equality.
        // Entry still proves -- the rejection is the edge's transport, not
        // the fixture's own range.
        let moved = typed(MOVED_COPY);
        assert!(entry(&moved));
        assert!(edge(&moved, RankingRangePremises::RankInvariant).is_none());
        assert!(edge(&moved, RankingRangePremises::EntryInvariant).is_none());
    }
}

mod moved_record_copy {
    //! A required entry packed inside a record carrier diverges its copies the
    //! same way bare scalar copies diverge: the carrier rebuilds through a
    //! literal whose unique leaf is a strict step, so the literal names the
    //! moved copy and the bare-forward sibling demotes to a stale snapshot.
    use super::super::{
        RankingRangeEdgeProof, RankingRangeMeasure, RankingRangePremises, RankingRangeState,
        discover_state_entry_mappings, prove_ranking_range_transition,
        ranking_range_required_symbols,
    };
    use super::typed;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::machine::Machine;
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    const PACKED: &str = r#"
        data Pair { left: u32 [0..=5]; }
        machine walk(remaining: u32 [0..=5])
        terminates by remaining in 0..=5;
        -> u32 {
            transition { _ -> s(Pair { left: remaining }, remaining) }
            state s(pair: Pair, copy: u32 [0..=5]) {
                transition pair.left > 0 {
                    true -> s(Pair { left: pair.left - 1 }, copy)
                    false -> pair.left
                }
            }
        }
    "#;

    fn machine(program: &TypedTrees) -> &Machine {
        &program.machines()[0]
    }

    fn mappings(
        program: &TypedTrees,
        premises: RankingRangePremises,
    ) -> Option<Vec<Vec<SymbolHandle>>> {
        let machine = machine(program);
        let custody = program
            .ranking_expression_custody_for(machine.symbol)
            .expect("custody");
        let range = custody.rank_range.expect("range");
        let measure = RankingRangeMeasure::Single(custody.subjects[0]);
        let required = ranking_range_required_symbols(program, machine, range, measure, premises)
            .expect("required symbols");
        let remaining = program.state_parameters(&program.machine_states(machine)[0])[0].symbol;
        discover_state_entry_mappings(program, machine, remaining, &required)
    }

    /// The `s -> s` guarded edge's proof under the discovered correspondence.
    fn cycle_edge(
        program: &TypedTrees,
        entry_parameters: &[SymbolHandle],
    ) -> Option<RankingRangeEdgeProof> {
        let machine = machine(program);
        let state = &program.machine_states(machine)[1];
        let custody = program
            .ranking_expression_custody_for(machine.symbol)
            .expect("custody");
        let range = custody.rank_range.expect("range");
        let StatementNode::Transition(transition) =
            &program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("one transition");
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            panic!("guarded transition");
        };
        let TransitionTargetNode::Named { arguments, .. } =
            program.statement_table.transition_target(transition.target)
        else {
            panic!("named self edge");
        };
        prove_ranking_range_transition(
            program,
            machine,
            range,
            RankingRangeMeasure::Single(custody.subjects[0]),
            RankingRangePremises::RankInvariant,
            RankingRangeState {
                state,
                entry_parameters,
            },
            RankingRangeState {
                state,
                entry_parameters,
            },
            &[(guard, true)],
            &[],
            program.statement_table.expression_handles(*arguments),
        )
    }

    #[test]
    fn a_rebuilt_literal_names_the_moved_copy_of_a_required_entry() {
        let program = typed(PACKED);
        let remaining =
            program.state_parameters(&program.machine_states(machine(&program))[0])[0].symbol;
        for premises in [
            RankingRangePremises::RankInvariant,
            RankingRangePremises::EntryInvariant,
        ] {
            let mappings = mappings(&program, premises).expect("telescope");
            // `pair` keeps `remaining`'s role at `Pair`'s unique leaf; the
            // bare-forward `copy` demoted to a premise-free stale snapshot.
            assert_eq!(mappings[1], vec![remaining, SymbolHandle::default()]);
        }
        let mappings = mappings(&program, RankingRangePremises::RankInvariant).unwrap();
        let proof = cycle_edge(&program, &mappings[1]).expect("cycle edge");
        assert!(proof.membership_and_pinning && proof.strictly_decreases);
    }

    #[test]
    fn a_rebuilt_literal_without_a_step_keeps_both_claims_contested() {
        // `Pair { left: pair.left }` forwards the ranked leaf unchanged: the
        // copies never diverge, so neither demotes and the cycle edge keeps
        // membership but proves no descent.
        let program = typed(&PACKED.replace("pair.left - 1", "pair.left"));
        let mappings = mappings(&program, RankingRangePremises::RankInvariant).expect("telescope");
        let remaining =
            program.state_parameters(&program.machine_states(machine(&program))[0])[0].symbol;
        assert_eq!(mappings[1], vec![remaining, remaining]);
        // Contested copies give the edge no moved carrier to read: the rank
        // does not decrease through a forwarded literal.
        assert!(cycle_edge(&program, &mappings[1]).is_none());
    }
}
