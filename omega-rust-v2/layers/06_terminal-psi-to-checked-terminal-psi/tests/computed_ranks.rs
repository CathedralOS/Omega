fn check_computed_ranks(unit: &omega_psi::LoweredPackage) -> Result<(), omega_checker::ComputedRankError> {
    let core = omega_toolchain::core_interface();
    let descending = omega_lowering::portable_declaration(core.id(r"Nat\Descending")).unwrap();
    let distance = omega_lowering::portable_declaration(core.id(r"Nat\BoundedDistance")).unwrap();
    omega_checker::check_computed_ranks(unit, &descending, &distance)
}
use omega_psi::{BlockTerminator, RankComparison, RankExpression, RankOperand, SuccessorEdge};

fn lower(program: &str) -> omega_psi::LoweredPackage {
    let tokens = omega_lexer::lex(program.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(program.as_bytes(), &tokens).unwrap();
    let source = omega_resolver::ParsedSource {
        logical_path: "rank.omg",
        text: program,
        tokens: &tokens,
        tree: &tree,
    };
    let resolved = omega_toolchain::resolve_source_package(&[omega_resolver::StageSource { package: "app", source }]).unwrap();
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    omega_lowering::lower_package(&prepared).unwrap()
}

#[test]
fn exact_rank_formation_uses_entry_facts_and_points_at_the_failing_operation() {
    let defined = "const K:u64=3; machine f(n:u64) requires n <= 100 terminates by K + n {}";
    check_computed_ranks(&lower(defined)).unwrap();
    let unbounded = defined.replace("requires n <= 100", "");
    let err = check_computed_ranks(&lower(&unbounded)).unwrap_err();
    assert!(
        err.message.contains("Exact Add")
            && err.message.contains("u64")
            && err
                .message
                .contains("right operand is at most 18446744073709551612"),
        "{err:?}"
    );
    assert_eq!(err.site.span.slice(unbounded.as_bytes()), Some(b"K + n".as_slice()));
    let nested = unbounded.replace("K + n", "(K + n) - K");
    let err = check_computed_ranks(&lower(&nested)).unwrap_err();
    assert_eq!(err.site.span.slice(nested.as_bytes()), Some(b"K + n".as_slice()));
    let underflow = defined
        .replace("n <= 100", "n == 3")
        .replace("K + n", "K - n - 1");
    assert!(
        check_computed_ranks(&lower(&underflow))
            .unwrap_err()
            .message
            .contains("Exact Subtract")
    );
}

#[test]
fn computes_descent_from_the_real_call_and_ignores_a_forged_label() {
    let src = "const K:u64=3; machine f(n:u64)->u64 requires n <= 100 terminates by K + n { transition n == 0 { true -> 0, false -> f(n - 1), } }";
    let mut good = lower(src);
    check_computed_ranks(&good).unwrap();
    let edge = &mut good.machines[0].contract.ranking.as_mut().unwrap().edges[0];
    edge.successor = Some(RankExpression::Operand(RankOperand::Constant(0.into())));
    assert!(
        check_computed_ranks(&good)
            .unwrap_err()
            .message
            .contains("actual arguments")
    );
    let mut repeated = lower(&src.replace("f(n - 1)", "f(n)"));
    assert!(
        check_computed_ranks(&repeated)
            .unwrap_err()
            .message
            .contains("cycle")
    );
    repeated.machines[0].contract.ranking.as_mut().unwrap().edges[0].comparison = RankComparison::Strict;
    assert!(
        check_computed_ranks(&repeated)
            .unwrap_err()
            .message
            .contains("strict decrease")
    );
    let mut missing = lower(src);
    missing.machines[0]
        .contract
        .ranking
        .as_mut()
        .unwrap()
        .edges
        .clear();
    assert!(
        check_computed_ranks(&missing)
            .unwrap_err()
            .message
            .contains("every internal call")
    );
}

#[test]
fn a_wrapped_distance_and_a_saturated_distance_have_distinct_descent() {
    let src = "const K:u64=3; machine f(n:u64)->u64 terminates by K -% n { transition n == 4 { true -> f(n + 1), false -> 0, } }";
    check_computed_ranks(&lower(src)).unwrap();
    assert!(
        check_computed_ranks(&lower(&src.replace("-%", "-|")))
            .unwrap_err()
            .message
            .contains("cycle")
    );
    let increases = src.replace("== 4", "== 3");
    assert!(
        check_computed_ranks(&lower(&increases))
            .unwrap_err()
            .message
            .contains("non-increasing")
    );
}

#[test]
fn every_cycle_in_a_group_must_have_a_strict_edge() {
    let src = "machine f(n:u64)->u64 requires n <= 100 terminates by 3 + n {transition {_ -> g(n),}}
             machine g(n:u64)->u64 requires n <= 100 terminates by 3 + n {transition n == 0 {true -> 0,false -> f(n - 1),}}";
    check_computed_ranks(&lower(src)).unwrap();
    assert!(
        check_computed_ranks(&lower(&src.replace("f(n - 1)", "f(n)")))
            .unwrap_err()
            .message
            .contains("cycle")
    );
}

#[test]
fn an_intraprocedural_backedge_cannot_hide_in_a_machine_with_no_calls() {
    let mut unit = lower("machine f(n:u64) terminates by n +% 1 {}");
    let block = &mut unit.machines[0].body.blocks[0];
    let BlockTerminator::Return { id, .. } = block.terminator else {
        panic!("entry should return")
    };
    block.terminator = BlockTerminator::Jump(SuccessorEdge {
        selected_case: None,
        requires_adjacency: false,
        id,
        target: block.id,
        arguments: vec![],
        scalar_qualification_changes: vec![],
        domain_completions: vec![],
        structural_arguments: vec![],
        cleanup: vec![],
    });
    assert!(check_computed_ranks(&unit).is_err());
}

#[test]
fn a_structural_argument_cannot_satisfy_a_scalar_entry_bound() {
    let mut unit = lower("machine f(n:u64,k:u64)->u64 requires k == 1 terminates by n +% 0 {transition n == 0 {true -> 0,false -> f(n-k,k),}}");
    check_computed_ranks(&unit).unwrap();
    let actuals = unit.machines[0]
        .body
        .blocks
        .iter_mut()
        .find_map(|b| match &mut b.terminator {
            BlockTerminator::TerminalCall { arguments, .. } => Some(arguments),
            _ => None,
        })
        .unwrap();
    actuals[1] = omega_psi::CallArgument::CopyWhole(omega_psi::PlaceId(1.try_into().unwrap()));
    assert!(
        check_computed_ranks(&unit)
            .unwrap_err()
            .message
            .contains("scalar formal")
    );
}

#[test]
fn the_checker_checks_the_selected_view_and_rejects_orphaned_arithmetic_occurrences() {
    let mut unit = lower("machine f(n:u64) terminates by n +% 3 {}");
    let view = &mut unit.machines[0].contract.ranking.as_mut().unwrap().view;
    std::sync::Arc::make_mut(&mut view.name).path.push_str("Other");
    assert!(
        check_computed_ranks(&unit)
            .unwrap_err()
            .message
            .contains("admitted natural view")
    );
    let mut unit = lower("machine f(n:u64) terminates by n +% 3 {}");
    unit.machines[0].contract.ranking.as_mut().unwrap().rank = RankExpression::Operand(RankOperand::Input(0));
    assert!(
        check_computed_ranks(&unit)
            .unwrap_err()
            .message
            .contains("occurrence")
    );
}
