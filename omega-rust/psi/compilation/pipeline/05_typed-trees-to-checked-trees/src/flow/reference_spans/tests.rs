use super::project_constraint_refs_to_active_contexts;
use arena::PagedArenaSet;
use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
use facts::FactPlan;

#[test]
fn projection_retains_distinct_context_pages_at_the_same_slot() {
    let mut semantic = FactPlan::default();
    let global = semantic.contexts.append(Default::default());
    semantic.contexts.begin_page(17);
    let local = semantic.contexts.append(Default::default());
    semantic.contexts.begin_page(29);
    let inactive = semantic.contexts.append(Default::default());
    assert_eq!(global.arena_index(), local.arena_index());
    assert_eq!(global.arena_index(), inactive.arena_index());
    assert_ne!(global.generation(), local.generation());
    for context in [global, local, inactive] {
        assert!(semantic.contexts.iter().any(|(known, _)| known == context));
    }

    let reference = |context| FlowConstraintRef {
        kind: FlowConstraintKind::SemanticContext { context },
    };
    for active in [[global, local], [local, global]] {
        let mut semantic_refs = PagedArenaSet::default();
        let active =
            semantic_refs.insert_many(active.map(|context| FlowSemanticContextRef { context }));
        let mut constraints = PagedArenaSet::default();
        let source = constraints.insert_many([
            reference(global),
            reference(inactive),
            FlowConstraintRef::default(),
            reference(local),
        ]);
        let retained = project_constraint_refs_to_active_contexts(
            &mut constraints,
            source,
            active,
            &semantic_refs,
        );
        assert_eq!(
            constraints.span_or_empty(retained),
            [
                reference(global),
                FlowConstraintRef::default(),
                reference(local),
            ],
        );
    }
}
