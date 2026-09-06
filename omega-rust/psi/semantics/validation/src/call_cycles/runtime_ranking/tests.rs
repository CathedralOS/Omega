use super::*;
use source::SourceMap;
use source_files_to_tokens::Lexer;

mod meaning;

fn typed(body: &str) -> TypedTrees {
    typed_with_operator(body, "")
}

fn typed_with_operator(body: &str, operator: &str) -> TypedTrees {
    let source = format!(
        "data Progress {{ outer: u64; inner: u64; }}
         measure Progress::Steps lexicographic {{ outer, inner }}
         data Main {{}}
         machine Main::scan_a(&mut self, progress: Progress) -> u64
         terminates by progress -> Progress::Steps;
         {{ transition progress.inner > 0 {{ true -> self.scan_b(progress) false -> 0 }} }}
         machine Main::scan_b(&mut self, remaining: Progress) -> u64
         terminates by remaining -> Progress::Steps;
         {{ {body} }}
         {operator}"
    );
    typed_source(&source)
}

fn typed_source(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize ranking");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("joint_ranking.omg".into(), source.to_owned())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse ranking");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .expect("resolve ranking");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type ranking")
}

const DECREASE: &str = "transition remaining.inner > 0 {
    true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
    false -> 0
}";

fn admitted(program: &TypedTrees) -> Vec<Vec<usize>> {
    let classification = typed_trees::proof_only::classify(program);
    let mut adjacency = vec![Vec::new(); program.machines().len()];
    extend_runtime_adjacency(program, &classification, &mut adjacency);
    admitted_components(program, &classification, &adjacency)
}

#[test]
fn exact_joint_projection_admits_forwarding_then_decrease() {
    let program = typed(DECREASE);
    for machine in program.machines() {
        assert!(RankProjection::resolve(&program, machine).is_some());
    }
    assert_eq!(admitted(&program).len(), 1);
}

#[test]
fn unchanged_occurrence_cannot_be_hidden_by_strict_pair_occurrence() {
    let program = typed(
        "transition remaining.inner > 0 {
        true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
        false -> self.scan_a(remaining)
    }",
    );
    assert!(admitted(&program).is_empty());
}

#[test]
fn equality_graph_checks_every_cycle_not_only_one_dfs_path() {
    assert!(equality_edges_are_acyclic(&[vec![1], vec![2], vec![]]));
    assert!(!equality_edges_are_acyclic(&[vec![1], vec![2], vec![1]]));
    assert!(!equality_edges_are_acyclic(&[vec![0]]));
    assert!(!equality_edges_are_acyclic(&[vec![1, 2], vec![2], vec![1]]));
}

#[test]
fn decrement_requires_the_current_component_guard() {
    for body in [
        "transition { _ -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 }) }",
        "transition remaining.outer > 0 {
            true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
            false -> 0
        }",
    ] {
        assert!(admitted(&typed(body)).is_empty());
    }
}

#[test]
fn mismatched_actual_member_cannot_use_its_old_spelling() {
    let mut program = typed(DECREASE);
    let members = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Member(_)).then_some(handle)
        })
        .collect::<Vec<_>>();
    assert!(!members.is_empty());
    let wrong_field = program.machines()[0].symbol;
    for handle in members {
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(handle) else {
            unreachable!()
        };
        member.member_symbol = wrong_field;
    }
    assert!(admitted(&program).is_empty());
}

#[test]
fn unresolved_actual_receiver_cannot_use_its_old_spelling() {
    let mut program = typed(DECREASE);
    let receivers = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Member(member) => Some(member.receiver),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!receivers.is_empty());
    for receiver in receivers {
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver) else {
            unreachable!()
        };
        path.symbol = SymbolHandle::invalid();
        path.head_symbol = SymbolHandle::invalid();
    }
    assert!(admitted(&program).is_empty());
}

#[test]
fn duplicate_measure_occurrence_does_not_choose_the_first() {
    let mut program = typed(DECREASE);
    let duplicate = program.measures()[0].clone();
    program.push_measure(duplicate);
    assert!(admitted(&program).is_empty());
}

#[test]
fn authored_arithmetic_does_not_supply_primitive_descent() {
    for operator in [
        "operator - u64::unchanged(left: u64, right: u64) -> u64;",
        "operator > u64::not_a_bound(left: u64, right: u64) -> bool;",
    ] {
        assert!(admitted(&typed_with_operator(DECREASE, operator)).is_empty());
    }
    assert_eq!(
        admitted(&typed_with_operator(
            DECREASE,
            "operator - f64::unrelated(left: f64, right: f64) -> f64;"
        ))
        .len(),
        1
    );
}

#[test]
fn declined_record_ranking_cannot_fall_through_to_scalar_syntax() {
    let program = typed_source(
        "data Progress { outer: u64; inner: u64; }
        measure Progress::Steps lexicographic { outer, inner }
        data Main {}
        machine Main::scan_a(&mut self, progress: u64) -> u64
        terminates by progress -> Progress::Steps;
        { transition progress == 0 { true -> 0 false -> self.scan_b(progress - 1) } }
        machine Main::scan_b(&mut self, progress: u64) -> u64
        terminates by progress -> Progress::Steps;
        { transition progress == 0 { true -> 0 false -> self.scan_a(progress - 1) } }",
    );
    let mut diagnostics = Vec::new();
    let symbols = crate::symbols::TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty());
    super::super::validate_machine_call_cycles(&program, &symbols, &mut diagnostics);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("machine call cycle")),
        "declared projection must not borrow scalar syntactic admission: {diagnostics:#?}"
    );
}

#[test]
fn canonical_unsigned_scalar_rank_uses_joint_component_admission() {
    let program = typed_source(
        "data Main {}
        machine Main::scan_a(&mut self, remaining: u64) -> u64
        terminates by remaining;
        { transition remaining == 0 { true -> 0 false -> self.scan_b(remaining - 1) } }
        machine Main::scan_b(&mut self, remaining: u64) -> u64
        terminates by remaining;
        { transition remaining == 0 { true -> 0 false -> self.scan_a(remaining - 1) } }",
    );
    for machine in program.machines() {
        assert!(
            RankProjection::resolve(&program, machine).is_some(),
            "{:#?}",
            machine.termination_plan
        );
    }
    let mut diagnostics = Vec::new();
    let symbols = crate::symbols::TopLevelSymbols::build(&program, &mut diagnostics);
    super::super::validate_machine_call_cycles(&program, &symbols, &mut diagnostics);
    assert!(
        diagnostics.is_empty(),
        "legacy scalar fixture changed: {diagnostics:#?}"
    );
}

#[test]
fn scalar_projection_does_not_prove_retained_ranges_or_view_arguments() {
    let mut program = typed_source(
        "machine scalar(remaining: u64) -> u64
        terminates by remaining;
        { transition { _ -> 0 } }",
    );
    assert!(RankProjection::resolve(&program, &program.machines()[0]).is_some());
    program.machines_mut()[0]
        .termination_plan
        .implementation_witness
        .as_mut()
        .unwrap()
        .rank_range = Some(language_semantics::RankRange {
        floor: "0".into(),
        ceiling: "5".into(),
        ceiling_inclusive: true,
    });
    assert!(RankProjection::resolve(&program, &program.machines()[0]).is_none());
    program.machines_mut()[0]
        .termination_plan
        .implementation_witness
        .as_mut()
        .unwrap()
        .rank_range = None;
    let machine = program.machines()[0].symbol;
    let custody_index = program
        .ranking_expression_custody
        .iter()
        .position(|custody| custody.machine == machine)
        .unwrap();
    let subject = program.ranking_expression_custody[custody_index].subjects[0];
    program.ranking_expression_custody[custody_index].rank_range = Some(subject);
    assert!(RankProjection::resolve(&program, &program.machines()[0]).is_none());
    program.ranking_expression_custody[custody_index].rank_range = None;
    program.ranking_expression_custody[custody_index]
        .view_arguments
        .push(subject);
    assert!(RankProjection::resolve(&program, &program.machines()[0]).is_none());
}

#[test]
fn unknown_effect_before_the_guard_prevents_entry_rank_replay() {
    let mut program = typed(DECREASE);
    let machine = &program.machines()[1];
    let state = &program.machine_states(machine)[0];
    let mut statements = program
        .statement_table
        .statements(state.statement_nodes)
        .to_vec();
    statements.insert(0, StatementNode::Call(Default::default()));
    // Keep the authored state and rank identities while inserting an unknown
    // call into its exact statement prefix.
    let mut changed = machine.clone();
    let mut changed_state = state.clone();
    changed_state.statement_nodes = Default::default();
    for statement in statements {
        program
            .statement_table
            .push_statement(&mut changed_state.statement_nodes, statement);
    }
    changed.states = Default::default();
    program.push_machine_state(&mut changed, changed_state);
    *program.machines_mut().get_mut(1).expect("second machine") = changed;
    assert!(admitted(&program).is_empty());
}
