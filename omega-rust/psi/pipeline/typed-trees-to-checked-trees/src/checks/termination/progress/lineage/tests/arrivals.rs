use super::{ProgressSubject, SymbolHandle};
use crate::checks::termination::progress::lineage::ParameterLineage;
use crate::checks::termination::progress::lineage::StateParameterLineage;
use crate::checks::termination::progress::lineage::places;
use crate::checks::termination::progress::lineage::resolve_subject_lineage;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed).unwrap()
}

fn field(program: &typed_trees::TypedTrees, path: &str) -> SymbolHandle {
    program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field)
                if program.symbols.display_path(field.symbol, "::") == path =>
            {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect("fixture field")
}

fn machine<'program>(
    program: &'program typed_trees::TypedTrees,
    name: &str,
) -> &'program typed_trees::machine::Machine {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("fixture machine")
}

fn parameter(program: &typed_trees::TypedTrees, state_index: usize, name: &str) -> SymbolHandle {
    let walk = machine(program, "walk");
    let state = &program.machine_states(walk)[state_index];
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
        .map(|parameter| parameter.symbol)
        .expect("fixture parameter")
}

/// A deep replacement through a reference field leaf is the demanded path's
/// exact provenance: the finite projected arrival derives the replacement
/// input's premise instead of collapsing at the reference boundary.
#[test]
fn projected_arrival_through_a_reference_leaf_derives_the_replacement_premise() {
    let program = checked(
        r#"
        data Main {}
        machine Main::run(&mut self) {}
        data Scheduler [copy] {}
        data Context { scheduler: Scheduler; }
        data RefBox { view: &mut Context; }
        machine walk(b: &mut RefBox, r: &Context, remaining: u64) -> u64
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> step(b, r, remaining - 1)
                false -> 0
            }
            state step(b: &mut RefBox, r: &Context, remaining: u64) -> u64 {
                b.view.scheduler = r.scheduler;
                transition remaining > 0 {
                    true -> walk(b, r, remaining - 1)
                    false -> 0
                }
            }
        }
    "#,
    );
    let walk = machine(&program, "walk");
    let view = field(&program, "RefBox::view");
    let scheduler = field(&program, "Context::scheduler");
    let entry_b = parameter(&program, 0, "b");
    let entry_r = parameter(&program, 0, "r");
    let step_b = parameter(&program, 1, "b");
    let demand = ProgressSubject {
        root: step_b,
        projections: vec![view, scheduler],
    };
    assert_eq!(
        places::partition(&program, walk, &demand),
        Some(demand.clone())
    );
    let lineage = StateParameterLineage::derive(&program, &program.facts.flow, walk, &demand, None);
    let ParameterLineage::Exact(origins) = resolve_subject_lineage(&lineage.values, demand) else {
        panic!("reference-leaf arrival keeps exact origins")
    };
    assert!(
        origins.contains(&ProgressSubject {
            root: entry_r,
            projections: vec![scheduler],
        }),
        "the replacement input's exact premise: {origins:?}"
    );
    assert!(
        origins.contains(&ProgressSubject {
            root: entry_b,
            projections: vec![view, scheduler],
        }),
        "the unchanged arrival's exact premise: {origins:?}"
    );
}

/// A store through the same reference leaf whose helper source is provably
/// read-only on the demanded projection is consumable: `pick_mut` never
/// writes `context.scheduler`, so the arrival keeps the exact input premise
/// alongside the unchanged self-arrival.
#[test]
fn read_only_helper_store_through_a_reference_leaf_derives_the_exact_premise() {
    let program = checked(
        r#"
        data Main {}
        machine Main::run(&mut self) {}
        data Scheduler [copy] {}
        data Context { scheduler: Scheduler; }
        data RefBox { view: &mut Context; }
        machine pick_mut(context: &mut Context) -> Scheduler { context.scheduler }
        machine walk(b: &mut RefBox, r: &Context, remaining: u64) -> u64
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> step(b, r, remaining - 1)
                false -> 0
            }
            state step(b: &mut RefBox, r: &Context, remaining: u64) -> u64 {
                b.view.scheduler = pick_mut(r);
                transition remaining > 0 {
                    true -> walk(b, r, remaining - 1)
                    false -> 0
                }
            }
        }
    "#,
    );
    let walk = machine(&program, "walk");
    let view = field(&program, "RefBox::view");
    let scheduler = field(&program, "Context::scheduler");
    let entry_b = parameter(&program, 0, "b");
    let entry_r = parameter(&program, 0, "r");
    let step_b = parameter(&program, 1, "b");
    let demand = ProgressSubject {
        root: step_b,
        projections: vec![view, scheduler],
    };
    assert_eq!(
        places::partition(&program, walk, &demand),
        Some(demand.clone())
    );
    let lineage = StateParameterLineage::derive(&program, &program.facts.flow, walk, &demand, None);
    let ParameterLineage::Exact(origins) = resolve_subject_lineage(&lineage.values, demand) else {
        panic!("reference-leaf helper arrival keeps exact origins")
    };
    assert!(
        origins.contains(&ProgressSubject {
            root: entry_r,
            projections: vec![scheduler],
        }),
        "the replacement input's exact premise: {origins:?}"
    );
    assert!(
        origins.contains(&ProgressSubject {
            root: entry_b,
            projections: vec![view, scheduler],
        }),
        "the unchanged arrival's exact premise: {origins:?}"
    );
}

/// Rebidding the reference field to a call's result supplies no exact referent:
/// the demanded path through it stays unproven.
#[test]
fn reference_alias_without_exact_provenance_retains_no_checked_guarantee() {
    let program = checked(
        r#"
        data Main {}
        machine Main::run(&mut self) {}
        data Scheduler [copy] {}
        data Context { scheduler: Scheduler; }
        data RefBox { view: &mut Context; }
        machine opaque_view(context: &mut Context) -> &mut Context { context }
        machine walk(b: &mut RefBox, r: &mut Context, remaining: u64) -> u64
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> step(b, r, remaining - 1)
                false -> 0
            }
            state step(b: &mut RefBox, r: &mut Context, remaining: u64) -> u64 {
                b.view = opaque_view(r);
                transition remaining > 0 {
                    true -> walk(b, r, remaining - 1)
                    false -> 0
                }
            }
        }
    "#,
    );
    let walk = machine(&program, "walk");
    let view = field(&program, "RefBox::view");
    let scheduler = field(&program, "Context::scheduler");
    let step_b = parameter(&program, 1, "b");
    let demand = ProgressSubject {
        root: step_b,
        projections: vec![view, scheduler],
    };
    let lineage = StateParameterLineage::derive(&program, &program.facts.flow, walk, &demand, None);
    assert_eq!(
        resolve_subject_lineage(&lineage.values, demand),
        ParameterLineage::Ambiguous
    );
}

/// A generic application still names its own declaration, so an exact
/// projection into it is verified; past the unresolved argument leaf the
/// projection's own declared field identity carries the bounded path.
#[test]
fn unresolved_generic_leaf_keeps_exact_field_projections() {
    let program = checked(
        r#"
        data Main {}
        machine Main::run(&mut self) {}
        data Scheduler [copy] {}
        data Context { scheduler: Scheduler; }
        data Box<T> { item: T; }
        machine walk(b: &Box<Context>, node: &Node, remaining: u64) -> u64
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> step(b, node, remaining - 1)
                false -> 0
            }
            state step(b: &Box<Context>, node: &Node, remaining: u64) -> u64 {
                transition remaining > 0 {
                    true -> walk(b, node, remaining - 1)
                    false -> 0
                }
            }
        }
        data Node { next: &Node; }
    "#,
    );
    let walk = machine(&program, "walk");
    let item = field(&program, "Box::item");
    let scheduler = field(&program, "Context::scheduler");
    for (state_index, name) in [(0, "b"), (1, "b")] {
        let b = parameter(&program, state_index, name);
        let demand = ProgressSubject {
            root: b,
            projections: vec![item, scheduler],
        };
        assert_eq!(
            places::partition(&program, walk, &demand),
            Some(demand.clone())
        );
        let lineage =
            StateParameterLineage::derive(&program, &program.facts.flow, walk, &demand, None);
        assert_eq!(
            resolve_subject_lineage(&lineage.values, demand.clone()),
            ParameterLineage::Exact(vec![ProgressSubject {
                root: parameter(&program, 0, "b"),
                projections: vec![item, scheduler],
            }])
        );
    }
}
