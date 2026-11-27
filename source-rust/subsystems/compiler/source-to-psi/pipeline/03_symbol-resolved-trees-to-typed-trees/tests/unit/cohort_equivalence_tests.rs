use std::sync::Arc;

use crate::expressions::operand_owners::CohortOperandOwners;
use crate::lowerer::TypingCohort;
use crate::resolution::{
    DeferredResolvedFrontier, ExtensionRequest, ResolutionRequest, resolve, resolve_deferred,
    resolve_deferred_extension,
};
use crate::source_inputs::SourceMap;
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_state::TypedTrees;
use crate::source_state::expression::ExpressionNode;
use crate::source_state::references::collect_program_operand_owners;
use crate::source_state::statement::StatementNode;
use crate::source_state::types::TypeReferenceNode;
use crate::{
    SeededTypingBase, lower_symbol_resolved_trees, lower_symbol_resolved_trees_to_seeded_base,
};
use symbols::SymbolHandle;
use syntax_trees::SyntaxTrees;

fn parse_single_source(path: &str, text: &str) -> (Arc<SourceMap>, SyntaxTrees) {
    let mut sources = SourceMap::default();
    let id = sources.add(path.into(), text.to_owned()).source_id;
    let tokens = crate::frontend_fixture::Lexer::new(text)
        .tokenize()
        .unwrap();
    let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
    (Arc::new(sources), syntax)
}

fn resolve_fresh(text: &str) -> SymbolResolvedTrees {
    let (sources, syntax) = parse_single_source("cohort_equiv/main.omg", text);
    resolve(ResolutionRequest {
        syntax,
        sources: Some(sources),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .unwrap()
}

fn begin_deferred(text: &str) -> (SeededTypingBase, DeferredResolvedFrontier, Arc<SourceMap>) {
    let (sources, syntax) = parse_single_source("cohort_equiv/main.omg", text);
    let frontier = resolve_deferred(ResolutionRequest {
        syntax,
        sources: Some(Arc::clone(&sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .unwrap();
    let typing = lower_symbol_resolved_trees_to_seeded_base(frontier.shared_trees()).unwrap();
    (typing, frontier, sources)
}

fn activate_deferred(
    mut typing: SeededTypingBase,
    frontier: DeferredResolvedFrontier,
    sources: Arc<SourceMap>,
    bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
) -> (SeededTypingBase, DeferredResolvedFrontier) {
    let frontier = frontier
        .with_prepared_allocations(typing.prepare_allocations().unwrap())
        .unwrap();
    let (frontier, activation) = frontier.activate_sources(sources, bindings).unwrap();
    let typing = typing.activate_body_completion(activation).unwrap();
    (typing, frontier)
}

fn complete_one_machine(
    mut typing: SeededTypingBase,
    frontier: DeferredResolvedFrontier,
    selected: SymbolHandle,
) -> (SeededTypingBase, DeferredResolvedFrontier) {
    let frontier = frontier
        .with_prepared_allocations(typing.prepare_allocations().unwrap())
        .unwrap();
    let completed = frontier.complete_machine_bodies(&[selected]).unwrap();
    let (rebased, frontier) = completed
        .rebase_authored_selections_for_typed_completion(
            typing.typed().authored_declaration_selections(),
        )
        .unwrap();
    let typing = typing.complete_bodies(rebased).unwrap();
    (typing, frontier)
}

fn machine_by_name(program: &TypedTrees, name: &str) -> SymbolHandle {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing machine {name}"))
        .symbol
}

fn assert_owners_match_full(program: &TypedTrees) {
    let expected = collect_program_operand_owners(program);
    let cohort = TypingCohort::default();
    let cohort_owners = CohortOperandOwners::collect(program, &cohort);
    for (handle, node) in program.expression_table.iter_expressions() {
        assert_eq!(
            cohort_owners.expression_owners(handle).collect::<Vec<_>>(),
            expected.expression_owners(handle).collect::<Vec<_>>(),
            "cohort expression owners drifted for {node:?}"
        );
    }
    for (handle, node) in program.type_reference_table.type_reference_rows_from(0) {
        assert_eq!(
            cohort_owners.type_owners(handle).collect::<Vec<_>>(),
            expected.type_owners(handle).collect::<Vec<_>>(),
            "cohort type owners drifted for {node:?}"
        );
    }
}

fn owner_names_for_expression(program: &TypedTrees, owners: &[SymbolHandle]) -> Vec<String> {
    let mut names: Vec<_> = owners
        .iter()
        .map(|symbol| program.symbols.name(*symbol).to_owned())
        .collect();
    names.sort();
    names
}

fn assert_semantic_equivalence_by_machine(left: &TypedTrees, right: &TypedTrees) {
    assert_eq!(left.machines().len(), right.machines().len());
    let left_owners = collect_program_operand_owners(left);
    let right_owners = collect_program_operand_owners(right);

    for left_machine in left.machines() {
        let name = left_machine.name.as_str();
        let right_symbol = machine_by_name(right, name);
        let right_machine = right.machine_by_symbol(right_symbol).unwrap();
        assert_eq!(left_machine.body_status, right_machine.body_status);

        // Compare deferred refusals per machine name.
        let left_refusals: Vec<_> = left
            .deferred_refusals_of(left_machine.symbol)
            .map(|row| row.request)
            .collect();
        let right_refusals: Vec<_> = right
            .deferred_refusals_of(right_symbol)
            .map(|row| row.request)
            .collect();
        assert_eq!(
            left_refusals, right_refusals,
            "deferred refusals drifted for {name}"
        );

        let left_states = left.machine_states(left_machine);
        let right_states = right.machine_states(right_machine);
        assert_eq!(left_states.len(), right_states.len());

        for (l_state, r_state) in left_states.iter().zip(right_states.iter()) {
            let l_stmts = left.statement_table.statements(l_state.statement_nodes);
            let r_stmts = right.statement_table.statements(r_state.statement_nodes);
            assert_eq!(l_stmts.len(), r_stmts.len());
            for (l_stmt, r_stmt) in l_stmts.iter().zip(r_stmts.iter()) {
                match (l_stmt, r_stmt) {
                    (StatementNode::Call(l_call), StatementNode::Call(r_call)) => {
                        assert_eq!(
                            l_call.runtime_call.is_some(),
                            r_call.runtime_call.is_some(),
                            "runtime_call presence drifted in {name}"
                        );
                        assert_eq!(
                            l_call.static_requirement_dispatch.is_some(),
                            r_call.static_requirement_dispatch.is_some(),
                            "static_requirement_dispatch drifted in {name}"
                        );
                        assert_eq!(
                            l_call.authored_machine_application.is_valid(),
                            r_call.authored_machine_application.is_valid(),
                            "authored_machine_application drifted in {name}"
                        );
                        assert_eq!(
                            l_call.machine_arguments.len(),
                            r_call.machine_arguments.len()
                        );
                    }
                    (StatementNode::LocalData(l_local), StatementNode::LocalData(r_local)) => {
                        if let (Some(l_init), Some(r_init)) = (
                            l_local.ordinary_value_initializer(),
                            r_local.ordinary_value_initializer(),
                        ) {
                            let l_expr_owners =
                                left_owners.expression_owners(l_init).collect::<Vec<_>>();
                            let r_expr_owners =
                                right_owners.expression_owners(r_init).collect::<Vec<_>>();
                            assert_eq!(
                                owner_names_for_expression(left, &l_expr_owners),
                                owner_names_for_expression(right, &r_expr_owners),
                                "local initializer expression owners drifted in {name}"
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let collect_machine_callables = |program: &TypedTrees| -> Vec<String> {
        let mut names = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for stmt in program.statement_table.statements(state.statement_nodes) {
                    if let Some(call) = stmt.as_call()
                        && call.authored_machine_application.is_valid()
                    {
                        for callable in
                            program.applied_machine_callables(call.authored_machine_application)
                        {
                            names.push(callable.selected.name.as_str().to_owned());
                        }
                    }
                }
            }
        }
        names.sort();
        names
    };
    assert_eq!(
        collect_machine_callables(left),
        collect_machine_callables(right),
        "applied machine callables drifted"
    );
}

const RICH_EQUIVALENCE_PROGRAM: &str = r#"
data Box<T> { value: T; }
trait Summarizable<T> {
    machine score(item: @T) -> u64;
}
data Entry { points: u64; }
EntryScore satisfies Summarizable<Entry> {
    machine score(item: @Entry) -> u64 { item.points }
}
data Unary = machine(value: u64) -> u64;
machine helper(value: u64) -> u64 { value + 1 }
machine generic_add<const K: u64>(value: u64) -> u64 { value + K }
machine summarize_via<T, S satisfies Summarizable<T>>(item: @T) -> u64 {
    S::score(item)
}
machine apply_twice(target: Unary, value: u64) -> u64 {
    target(target(value))
}
machine driver(input: u64) -> u64
    requires input < 100
{
    let boxed: Box<u64> = Box { value: helper(input) };
    let added: u64 = generic_add::<4u64>(boxed.value);
    let entry: Entry = Entry { points: added };
    let scored: u64 = summarize_via::<Entry, EntryScore>(entry);
    let repeated: u64 = apply_twice(helper, scored);
    transition repeated == added {
        true -> repeated,
        _ -> scored
    }
}
machine deferred_extent() {
    let bytes: [u8; helper(4)];
}
"#;

#[test]
fn incremental_body_completion_matches_fresh_typing() {
    let fresh_resolved = resolve_fresh(RICH_EQUIVALENCE_PROGRAM);
    let fresh_typed = lower_symbol_resolved_trees(&fresh_resolved).unwrap();

    let (typing, frontier, sources) = begin_deferred(RICH_EQUIVALENCE_PROGRAM);
    let (mut typing, mut frontier) = activate_deferred(typing, frontier, sources, Vec::new());

    let pending_in_order: Vec<_> = frontier.pending_machines().collect();
    assert!(pending_in_order.len() >= 5);
    for selected in pending_in_order {
        (typing, frontier) = complete_one_machine(typing, frontier, selected);
    }
    assert_eq!(frontier.pending_machines().count(), 0);

    let final_resolved = typing.resolved().clone();
    let fresh_same_resolved = lower_symbol_resolved_trees(&final_resolved).unwrap();
    let incremental_typed = typing.into_typed();
    assert_owners_match_full(&incremental_typed);
    assert_owners_match_full(&fresh_typed);
    assert_semantic_equivalence_by_machine(&incremental_typed, &fresh_typed);
    assert_semantic_equivalence_by_machine(&incremental_typed, &fresh_same_resolved);
    assert_eq!(
        incremental_typed.roots, fresh_same_resolved.roots,
        "roots differ"
    );
    assert_eq!(
        incremental_typed.deferred_refusals, fresh_same_resolved.deferred_refusals,
        "deferred_refusals differ"
    );
    assert_eq!(
        incremental_typed.target_machine_families, fresh_same_resolved.target_machine_families,
        "target_machine_families differ"
    );
    assert_eq!(
        incremental_typed.applied_domain_applications,
        fresh_same_resolved.applied_domain_applications,
        "applied_domain_applications differ"
    );
}

#[test]
fn out_of_order_body_completion_matches_fresh_typing_semantics() {
    let fresh_resolved = resolve_fresh(RICH_EQUIVALENCE_PROGRAM);
    let fresh_typed = lower_symbol_resolved_trees(&fresh_resolved).unwrap();

    let (typing, frontier, sources) = begin_deferred(RICH_EQUIVALENCE_PROGRAM);
    let (mut typing, mut frontier) = activate_deferred(typing, frontier, sources, Vec::new());

    let mut pending_reversed: Vec<_> = frontier.pending_machines().collect();
    pending_reversed.reverse();
    for selected in pending_reversed {
        (typing, frontier) = complete_one_machine(typing, frontier, selected);
    }
    assert_eq!(frontier.pending_machines().count(), 0);

    let incremental_typed = typing.into_typed();
    assert_owners_match_full(&incremental_typed);
    assert_semantic_equivalence_by_machine(&incremental_typed, &fresh_typed);

    // Check CohortOperandOwners collected specifically for one completed machine.
    let driver = machine_by_name(&incremental_typed, "driver");
    let mut cohort = TypingCohort::continuing(&fresh_resolved, &incremental_typed);
    let completed = [driver];
    cohort = cohort.completing(&completed);
    let scoped = CohortOperandOwners::collect(&incremental_typed, &cohort);
    let full = collect_program_operand_owners(&incremental_typed);
    for (handle, node) in incremental_typed.expression_table.iter_expressions() {
        let is_driver_expr = full.expression_owners(handle).any(|owner| owner == driver);
        if is_driver_expr {
            assert!(
                scoped
                    .expression_owners(handle)
                    .any(|owner| owner == driver),
                "scoped collector missed driver expression {node:?}"
            );
        }
    }
}

// Each body completion closes a cohort holding one machine. `apply` makes a
// runtime call through its `target` parameter, and every other body makes
// direct calls to machines typed in earlier cohorts.
const DIRECT_CALL_PROGRAM: &str = r#"
data Unary = machine(value: u64) -> u64;
machine increment(value: u64) -> u64 { value + 1 }
machine twice(value: u64) -> u64 { increment(increment(value)) }
machine apply(target: Unary, value: u64) -> u64 { target(value) }
machine driver(input: u64) -> u64 {
    let stepped: u64 = twice(input);
    let applied: u64 = apply(increment, stepped);
    applied
}
"#;

/// Every call expression's target and runtime binding, grouped by the
/// machine that owns it, in a form two separately typed programs can compare.
fn call_bindings_by_machine(program: &TypedTrees) -> Vec<(String, String, bool, bool)> {
    let owners = collect_program_operand_owners(program);
    let mut rows = Vec::new();
    for (handle, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let target = if call.target_symbol.is_valid() {
            program.symbols.name(call.target_symbol).to_owned()
        } else {
            String::new()
        };
        let runtime = call.runtime_call.is_some();
        let bound = call.runtime_call.is_some_and(|binding| {
            program
                .runtime_callable_contracts
                .is_valid(binding.contract)
        });
        for owner in owners.expression_owners(handle) {
            rows.push((
                program.symbols.name(owner).to_owned(),
                target.clone(),
                runtime,
                bound,
            ));
        }
    }
    rows.sort();
    rows
}

#[test]
fn cohort_scoped_closing_matches_fresh_typing_for_direct_calls() {
    let fresh_typed = lower_symbol_resolved_trees(&resolve_fresh(DIRECT_CALL_PROGRAM)).unwrap();

    let (typing, frontier, sources) = begin_deferred(DIRECT_CALL_PROGRAM);
    let (mut typing, mut frontier) = activate_deferred(typing, frontier, sources, Vec::new());
    let apply = machine_by_name(typing.typed(), "apply");
    let pending: Vec<_> = frontier.pending_machines().collect();
    assert!(pending.contains(&apply));
    for selected in pending {
        let predecessor = typing.resolved().clone();
        let retained = typing.typed().clone();
        (typing, frontier) = complete_one_machine(typing, frontier, selected);
        if selected != apply {
            continue;
        }
        let program = typing.typed();
        let base = TypingCohort::continuing(&predecessor, &retained);
        assert!(
            program
                .expression_table
                .iter_expressions_from(base.typed_expressions + 1)
                .any(|(_, node)| matches!(
                    node,
                    ExpressionNode::Call(call) if call.runtime_call.is_some()
                )),
            "completing `apply` adds a runtime call"
        );
        // The cohort that completed `apply` reaches its runtime call, so its
        // binding walk stays inside the cohort.
        let completed = [apply];
        let owning = TypingCohort::continuing(&predecessor, &retained).completing(&completed);
        assert!(
            crate::expressions::runtime_callables::cohort_walk_reaches_new_sites(program, &owning)
                .unwrap()
        );
        // A cohort that does not own `apply` misses that call, so binding
        // would walk the whole program instead of leaving it unbound.
        assert!(
            !crate::expressions::runtime_callables::cohort_walk_reaches_new_sites(program, &base)
                .unwrap()
        );
    }
    assert_eq!(frontier.pending_machines().count(), 0);

    let fresh_same_resolved = lower_symbol_resolved_trees(typing.resolved()).unwrap();
    let incremental_typed = typing.into_typed();
    assert_semantic_equivalence_by_machine(&incremental_typed, &fresh_typed);
    assert_semantic_equivalence_by_machine(&incremental_typed, &fresh_same_resolved);
    let bindings = call_bindings_by_machine(&incremental_typed);
    assert!(
        bindings
            .iter()
            .any(|(owner, _, runtime, bound)| owner == "apply" && *runtime && *bound),
        "the runtime call in `apply` is bound: {bindings:?}"
    );
    assert_eq!(bindings, call_bindings_by_machine(&fresh_typed));
    assert_eq!(bindings, call_bindings_by_machine(&fresh_same_resolved));
    assert_eq!(
        incremental_typed.deferred_refusals,
        fresh_same_resolved.deferred_refusals
    );
}

#[test]
fn seeded_extension_matches_fresh_typing() {
    let base_text = r#"
public data Box<T> { value: T; }
public trait Summarizable<T> {
    machine score(item: @T) -> u64;
}
public machine base_helper(value: u64) -> u64 { value + 2 }
public machine capacity() -> u64 { 4 }
"#;
    let extension_text = r#"
data Pair<A, B> { first: A; second: B; }
trait LocalMeasure<T> {
    machine measure(item: @T) -> u64;
}
data Item { weight: u64; }
ItemScore satisfies LocalMeasure<Item> {
    machine measure(item: @Item) -> u64 { item.weight }
}
machine generic_step<const N: u64>(value: u64) -> u64 { value + N }
machine use_base(value: u64) -> u64
    requires value < 50
{
    let box_val: |base|::Box<u64> = |base|::Box { value: |base|::base_helper(value) };
    let pair: Pair<u64, u64> = Pair { first: box_val.value, second: generic_step::<3u64>(value) };
    let item: Item = Item { weight: pair.first + pair.second };
    let measured: u64 = ItemScore::measure(item);
    transition pair.first == measured {
        true -> pair.first,
        _ -> measured
    }
}
machine ext_array() {
    let bytes: [u8; |base|::capacity()];
}
"#;
    let pkg = semantic_vocabulary::PackageKeyIdentity::from_digest([31; 32]).unwrap();
    let root = std::path::PathBuf::from("cohort_equiv");
    let mut base_sources = SourceMap::default();
    let base_id = base_sources
        .add_with_metadata(
            root.join("base.omg"),
            base_text.to_owned(),
            root.clone(),
            Some(pkg),
            source_admission::SourceOrigin::User,
        )
        .source_id;
    let base_tokens = crate::frontend_fixture::Lexer::new(base_text)
        .tokenize()
        .unwrap();
    let base_syntax =
        crate::frontend_fixture::parse_syntax_trees_with_id(base_id, &base_tokens).unwrap();

    let mut ext_sources = base_sources.clone();
    let ext_id = ext_sources
        .add_with_metadata(
            root.join("ext.omg"),
            extension_text.to_owned(),
            root,
            Some(pkg),
            source_admission::SourceOrigin::User,
        )
        .source_id;
    let ext_tokens = crate::frontend_fixture::Lexer::new(extension_text)
        .tokenize()
        .unwrap();
    let ext_syntax =
        crate::frontend_fixture::parse_syntax_trees_with_id(ext_id, &ext_tokens).unwrap();
    let bindings = vec![
        crate::source_symbols::SourceScopedTopLevelBinding::file_import(ext_id, base_id, "base"),
    ];

    let mut combined_syntax = base_syntax.clone();
    combined_syntax.extend_from(&ext_syntax);
    let fresh_resolved = resolve(ResolutionRequest {
        syntax: combined_syntax,
        sources: Some(Arc::new(ext_sources.clone())),
        top_level_bindings: bindings.clone(),
        retained_base: None,
    })
    .unwrap();
    let fresh_typed = lower_symbol_resolved_trees(&fresh_resolved).unwrap();

    // Seed incremental pipeline from base_text (completed cleanly).
    let base_sources = Arc::new(base_sources);
    let ext_sources = Arc::new(ext_sources);
    let base_resolved = resolve(ResolutionRequest {
        syntax: base_syntax,
        sources: Some(base_sources),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .unwrap();
    let mut base_typing = lower_symbol_resolved_trees_to_seeded_base(base_resolved).unwrap();
    let base_for_ext = base_typing.resolved_base_for_extension().unwrap();
    let ext = resolve_deferred_extension(ExtensionRequest {
        base: Arc::clone(&base_for_ext),
        syntax: ext_syntax,
        sources: Arc::clone(&ext_sources),
        top_level_bindings: bindings.clone(),
    })
    .unwrap();
    let (rebased, mut frontier) = ext
        .rebase_for_typed_continuation(base_typing.typed().authored_declaration_selections())
        .unwrap();
    let base_snapshot = base_typing.typed().clone();
    let mut extended = base_typing.append_resolved(rebased).unwrap();

    // Verify CohortOperandOwners::collect for the extended declarations against full program walk.
    let ext_cohort = TypingCohort::continuing(&base_for_ext, &base_snapshot);
    let scoped = CohortOperandOwners::collect(extended.typed(), &ext_cohort);
    let full = collect_program_operand_owners(extended.typed());
    let ext_pair = machine_by_name(extended.typed(), "use_base");
    for (handle, node) in extended.typed().expression_table.iter_expressions() {
        if full
            .expression_owners(handle)
            .any(|owner| owner == ext_pair)
        {
            assert!(
                scoped
                    .expression_owners(handle)
                    .any(|owner| owner == ext_pair),
                "cohort owner collector missed extended declaration expression {node:?}"
            );
        }
    }

    // Activate and complete the extended bodies incrementally.
    (extended, frontier) = activate_deferred(extended, frontier, ext_sources, bindings);
    let pending: Vec<_> = frontier.pending_machines().collect();
    assert!(!pending.is_empty());
    for selected in pending {
        (extended, frontier) = complete_one_machine(extended, frontier, selected);
    }

    let final_resolved = extended.resolved().clone();
    let fresh_same_resolved = lower_symbol_resolved_trees(&final_resolved).unwrap();
    let incremental_typed = extended.into_typed();
    assert_owners_match_full(&incremental_typed);
    assert_semantic_equivalence_by_machine(&incremental_typed, &fresh_typed);
    assert_semantic_equivalence_by_machine(&incremental_typed, &fresh_same_resolved);
    assert_eq!(incremental_typed.roots, fresh_same_resolved.roots);
    assert_eq!(
        incremental_typed.deferred_refusals,
        fresh_same_resolved.deferred_refusals
    );
    assert_eq!(
        incremental_typed.target_machine_families,
        fresh_same_resolved.target_machine_families
    );
    assert!(
        incremental_typed
            .deferred_refusals
            .iter()
            .all(|row| !matches!(
                row.request,
                crate::source_state::deferred_refusals::DeferredRequest::UndeclaredCallName
                    | crate::source_state::deferred_refusals::DeferredRequest::UndeclaredTypeName
            )),
        "unexpected undeclared-name refusal in seeded extension: {:?}",
        incremental_typed.deferred_refusals
    );
    // Verify call bindings match on every expression in both programs.
    let call_targets = |program: &TypedTrees| -> Vec<String> {
        let mut names: Vec<_> = program
            .expression_table
            .iter_expressions()
            .filter_map(|(_, node)| match node {
                ExpressionNode::Call(call) => {
                    let target = call.target_symbol;
                    target
                        .is_valid()
                        .then(|| program.symbols.name(target).to_owned())
                }
                _ => None,
            })
            .collect();
        names.sort();
        names
    };
    assert_eq!(call_targets(&incremental_typed), call_targets(&fresh_typed));
    let type_names = |program: &TypedTrees| -> Vec<String> {
        let mut names: Vec<_> = program
            .type_reference_table
            .type_reference_rows_from(0)
            .filter_map(|(_, node)| match node {
                TypeReferenceNode::Named { symbol, .. } => symbol
                    .is_valid()
                    .then(|| program.symbols.name(*symbol).to_owned()),
                _ => None,
            })
            .collect();
        names.sort();
        names
    };
    assert_eq!(type_names(&incremental_typed), type_names(&fresh_typed));
}

#[test]
fn activate_sources_handles_matching_and_narrowed_source_maps() {
    let keep_text = "machine keep(value: u64) -> u64 { value + 10 }\n";
    let drop_text = "machine dropped() -> u64 { 99 }\n";

    let mut both_sources = SourceMap::default();
    let keep_id = both_sources
        .add("activate/keep.omg".into(), keep_text.to_owned())
        .source_id;
    let _drop_id = both_sources
        .add("activate/drop.omg".into(), drop_text.to_owned())
        .source_id;
    let both_sources = Arc::new(both_sources);

    let keep_tokens = crate::frontend_fixture::Lexer::new(keep_text)
        .tokenize()
        .unwrap();
    let keep_syntax =
        crate::frontend_fixture::parse_syntax_trees_with_id(keep_id, &keep_tokens).unwrap();

    // Case 1: equal SourceMap (`sources == current_sources` across distinct Arcs) uses the fast path.
    let frontier = resolve_deferred(ResolutionRequest {
        syntax: keep_syntax,
        sources: Some(Arc::clone(&both_sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .unwrap();
    let mut typing = lower_symbol_resolved_trees_to_seeded_base(frontier.shared_trees()).unwrap();
    let cloned_sources = Arc::new((*both_sources).clone());
    assert!(!Arc::ptr_eq(&both_sources, &cloned_sources));
    assert_eq!(both_sources, cloned_sources);
    let _ = typing.prepare_allocations().unwrap();
    let activated_equal = typing
        .clone()
        .activate_sources(Arc::clone(&both_sources), cloned_sources, Vec::new())
        .unwrap();
    assert_eq!(
        activated_equal.typed().symbols,
        activated_equal.resolved().symbols
    );

    // Case 2: distinct current_sources (`sources != current_sources`) runs two-step activation
    // while retaining the predecessor's source coordinates.
    let mut narrowed_sources = (*both_sources).clone();
    narrowed_sources.retain_selected_sources(|file| file.source_id == keep_id);
    let narrowed_sources = Arc::new(narrowed_sources);
    assert_ne!(both_sources, narrowed_sources);

    let activated_distinct = typing
        .activate_sources(
            Arc::clone(&both_sources),
            Arc::clone(&narrowed_sources),
            Vec::new(),
        )
        .unwrap();
    assert_eq!(
        activated_distinct.typed().symbols,
        activated_distinct.resolved().symbols
    );
    assert_eq!(
        activated_distinct.typed().symbols.source_map().as_deref(),
        Some(narrowed_sources.as_ref())
    );
}
