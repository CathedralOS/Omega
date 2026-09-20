use super::{HandleSpan, StatementNode, TypeReferenceNode};
use crate::checks::termination::progress::origins::tests::Fixture;

#[test]
fn a_reference_observes_the_current_value_not_its_declaration_snapshot() {
    for (statements, expected) in [
        ("let borrowed: &Context = &context;", "context"),
        (
            "let borrowed: &Context = &context; context.scheduler = replacement.scheduler;",
            "replacement",
        ),
    ] {
        let fixture = Fixture::new(statements, "borrowed.scheduler");
        assert_eq!(
            fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
            Some(fixture.subject(expected, &[("Context", "scheduler")]))
        );
    }
}

#[test]
fn a_reference_to_a_field_retains_its_full_storage_projection() {
    let fixture = Fixture::new(
        "let borrowed: &SchedulerHandle = &context.scheduler;
         context.scheduler = replacement.scheduler;",
        "borrowed",
    );
    assert_eq!(
        fixture.query(fixture.subject("borrowed", &[])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn constrained_reference_aliases_use_the_same_live_origin_query() {
    let mut fixture = Fixture::new("let borrowed: &Context = &context;", "borrowed.scheduler");
    let subject = fixture.subject("borrowed", &[("Context", "scheduler")]);
    let expected = fixture.subject("context", &[("Context", "scheduler")]);
    assert_eq!(fixture.query(subject.clone()), Some(expected.clone()));
    let state =
        crate::semantic_calls::find_state(&fixture.program, fixture.state.state_symbol).unwrap();
    let statements = state.statement_nodes;
    let StatementNode::LocalData(local) =
        &fixture.program.statement_table.statements(statements)[0]
    else {
        panic!("alias declaration")
    };
    let base_type = local.type_reference;
    let constrained = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type,
            constraints: HandleSpan::empty(),
        });
    let StatementNode::LocalData(local) =
        &mut fixture.program.statement_table.statements_mut(statements)[0]
    else {
        unreachable!()
    };
    local.type_reference = constrained;
    assert_eq!(fixture.query(subject), Some(expected));
}

#[test]
fn a_foreign_same_spelling_root_cannot_supply_a_reference_origin() {
    let mut fixture = Fixture::new("let borrowed: &Context = &context;", "borrowed.scheduler");
    let subject = fixture.subject("borrowed", &[("Context", "scheduler")]);
    assert_eq!(
        fixture.query(subject.clone()),
        Some(fixture.subject("context", &[("Context", "scheduler")]))
    );
    let foreign = fixture
        .program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe_scheduler")
        .unwrap();
    let foreign = fixture
        .program
        .state_parameters(&fixture.program.machine_states(foreign)[0])[0]
        .symbol;
    let state =
        crate::semantic_calls::find_state(&fixture.program, fixture.state.state_symbol).unwrap();
    let StatementNode::LocalData(local) = &fixture
        .program
        .statement_table
        .statements(state.statement_nodes)[0]
    else {
        panic!("alias declaration")
    };
    let typed_trees::expression::ExpressionNode::Borrow(borrow) = fixture
        .program
        .expression_table
        .expression(local.initial_value)
    else {
        panic!("explicit borrow")
    };
    let target = borrow.target;
    let typed_trees::expression::ExpressionNode::Name(name) =
        fixture.program.expression_table.expression_mut(target)
    else {
        panic!("retained source name")
    };
    // Keep the authored `context` spelling; only its nominal root is foreign.
    name.head_symbol = foreign;
    name.symbol = foreign;
    assert_eq!(fixture.query(subject), None);
}

#[test]
fn record_construction_field_arrives_from_its_initializer() {
    let fixture = Fixture::new(
        "let built: Context = Context { scheduler: replacement.scheduler };",
        "built.scheduler",
    );
    assert_eq!(
        fixture.query(fixture.subject("built", &[("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn owned_load_from_constructed_record_arrives_from_its_initializer() {
    let fixture = Fixture::new(
        "let built: Context = Context { scheduler: replacement.scheduler }; let saved: SchedulerHandle = built.scheduler;",
        "saved",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn nested_record_construction_projects_through_the_constructed_field() {
    let fixture = Fixture::new(
        "let built: Holder = Holder { view: replacement };",
        "built.view.scheduler",
    );
    assert_eq!(
        fixture.query(fixture.subject("built", &[("Holder", "view"), ("Context", "scheduler")])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn record_construction_without_the_selected_field_has_no_origin() {
    let fixture = Fixture::new(
        "let built: Holder = Holder { view: replacement };",
        "built.view.scheduler",
    );
    assert_eq!(
        fixture.query(fixture.subject("built", &[("Context", "scheduler")])),
        None
    );
}

/// A shared `&` leaf stored inside an owned carrier names the referent the
/// slot's latest store supplied, not the carrier's own declaration. The
/// demanded storage keeps every projection past the leaf.
#[test]
fn shared_reference_leaf_loads_through_its_stored_referent() {
    for (statements, argument, subject, expected) in [
        // The leaf's initializer referent.
        (
            "let boxed: RefBox = RefBox { view: &context };",
            "boxed.view.scheduler",
            ("boxed", &[("RefBox", "view"), ("Context", "scheduler")][..]),
            ("context", &[("Context", "scheduler")][..]),
        ),
        // The same leaf reached through a later load of the carrier field.
        (
            "let boxed: RefBox = RefBox { view: &context }; let saved: SchedulerHandle = boxed.view.scheduler;",
            "saved",
            ("saved", &[][..]),
            ("context", &[("Context", "scheduler")][..]),
        ),
        // A reference-typed binding to the carrier resolves first, then the
        // leaf inside the referent.
        (
            "let boxed: RefBox = RefBox { view: &context }; let borrowed: &RefBox = &boxed;",
            "borrowed.view.scheduler",
            (
                "borrowed",
                &[("RefBox", "view"), ("Context", "scheduler")][..],
            ),
            ("context", &[("Context", "scheduler")][..]),
        ),
    ] {
        let fixture =
            Fixture::with_machines(statements, argument, &[], "data RefBox { view: &Context; }");
        assert_eq!(
            fixture.query(fixture.subject(subject.0, subject.1)),
            Some(fixture.subject(expected.0, expected.1))
        );
    }
}

/// A leaf load spelled through an exclusive `&mut` binding of the carrier
/// still names the leaf slot's storage exactly: the binding's declaration is
/// the referent's evidence, so `r.view.scheduler` reconstructs the referent
/// the carrier's latest store supplied. The exclusive binding's own type only
/// gates how that leaf is named — it does not mint the referent.
#[test]
fn shared_reference_leaf_load_through_an_exclusive_binding_uses_exact_provenance() {
    for (statements, argument, subject, expected) in [
        // Captured into an owned local's initializer.
        (
            "let mut boxed: RefBox = RefBox { view: &context }; let r: &mut RefBox = &mut boxed; let saved: SchedulerHandle = r.view.scheduler;",
            "saved",
            ("saved", &[][..]),
            ("context", &[("Context", "scheduler")][..]),
        ),
        // Captured into a store on an exclusive input's field.
        (
            "let mut boxed: RefBox = RefBox { view: &context }; let r: &mut RefBox = &mut boxed; dual.spare = r.view.scheduler;",
            "dual.spare",
            ("dual", &[("Dual", "spare")][..]),
            ("context", &[("Context", "scheduler")][..]),
        ),
    ] {
        let fixture =
            Fixture::with_machines(statements, argument, &[], "data RefBox { view: &Context; }");
        assert_eq!(
            fixture.query(fixture.subject(subject.0, subject.1)),
            Some(fixture.subject(expected.0, expected.1))
        );
    }
}

/// An exclusive-binding leaf read stays unproven whenever the frontier cannot
/// name its writes exactly: a store spelled through the `&mut` alias replaces
/// the leaf but cannot be matched to the slot. The operand's literal root is
/// replayed through the binding's own provenance — the store still leaves the
/// leaf's replacement unproven.
#[test]
fn shared_reference_leaf_load_through_an_exclusive_binding_after_an_alias_store_stays_unproven() {
    // `r.view` is rebound through the alias; the replacement's provenance
    // is reachable only through storage the write frame cannot name.
    let fixture = Fixture::with_machines(
        "let mut boxed: RefBox = RefBox { view: &context }; let r: &mut RefBox = &mut boxed; r.view = &holder.view; let saved: SchedulerHandle = r.view.scheduler;",
        "saved",
        &[],
        "data RefBox { view: &Context; }",
    );
    assert_eq!(fixture.query(fixture.subject("saved", &[])), None);
}

/// A leaf demanded directly as the call's operand through an exclusive
/// binding names the referent from the binding's provenance — `r.view.scheduler`
/// spells `boxed.view.scheduler`, which resolves to the leaf's stored
/// referent.
#[test]
fn shared_reference_leaf_operand_through_an_exclusive_binding_uses_exact_provenance() {
    let fixture = Fixture::with_machines(
        "let mut boxed: RefBox = RefBox { view: &context }; let r: &mut RefBox = &mut boxed;",
        "r.view.scheduler",
        &[],
        "data RefBox { view: &Context; }",
    );
    let subject = fixture.subject("r", &[("RefBox", "view"), ("Context", "scheduler")]);
    let expected = fixture.subject("context", &[("Context", "scheduler")]);
    assert_eq!(fixture.query(subject), Some(expected));
}

/// The referent is resolved at the store nearest the demand, so a write to
/// the referent between the slot store and the demand is answered through the
/// ordinary write scan — the latest value source, not the store snapshot.
#[test]
fn shared_reference_leaf_observes_the_current_value_not_its_store_snapshot() {
    let fixture = Fixture::with_machines(
        "let boxed: RefBox = RefBox { view: &context }; context.scheduler = replacement.scheduler; let saved: SchedulerHandle = boxed.view.scheduler;",
        "saved",
        &[],
        "data RefBox { view: &Context; }",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

/// A `&` binding bound to a helper's `&` result names the referent the
/// callee's returned leaf maps to: the leaf is instantiated onto the actual
/// argument's slot, then that slot's stored row resolves its referent.
#[test]
fn shared_reference_local_loads_a_helper_returned_leaf() {
    for (helper, expected) in [
        // The callee returns the leaf slot stored inside its borrowed
        // carrier parameter.
        (
            "machine unbox(b: &RefBox) -> &Context { b.view }",
            ("holder", &[("Holder", "view")][..]),
        ),
        // The leaf reaches the result through a callee-local binding.
        (
            "machine unbox(b: &RefBox) -> &Context { let kept: &Context = b.view; kept }",
            ("holder", &[("Holder", "view")][..]),
        ),
        // The leaf is bound through a nested helper call inside the
        // initializer.
        (
            "machine unbox(b: &RefBox) -> &Context { b.view } machine wrap(b: &RefBox) -> &RefBox { b }",
            ("holder", &[("Holder", "view")][..]),
        ),
    ] {
        let initializer = if helper.contains("wrap") {
            "unbox(wrap(&boxed))"
        } else {
            "unbox(&boxed)"
        };
        let fixture = Fixture::with_machines(
            &format!(
                "let boxed: RefBox = RefBox {{ view: &holder.view }}; let borrowed: &Context = {initializer};"
            ),
            "borrowed.scheduler",
            &[],
            &format!("data RefBox {{ view: &Context; }} {helper}"),
        );
        let mut segments = expected.1.to_vec();
        segments.push(("Context", "scheduler"));
        assert_eq!(
            fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
            Some(fixture.subject(expected.0, &segments))
        );
    }
}

/// The binding stays unproven when no exact referent can be named: an
/// unproven call route into the leaf store, a rebind reached through an
/// unresolved `&mut` alias, or an indexed carrier load whose segment is
/// deliberately coarse.
#[test]
fn shared_reference_local_from_an_unproven_leaf_store_stays_unproven() {
    for statements in [
        "let mut boxed: RefBox = RefBox { view: &context }; boxed.view = choose(&holder, context, true); let borrowed: &Context = boxed.view;",
        "let mut boxed: RefBox = RefBox { view: &context }; let mb: &mut RefBox = &mut boxed; mb.view = &holder.view; let borrowed: &Context = boxed.view;",
        "let boxes: [RefBox; 2] = [RefBox { view: &context }, RefBox { view: &holder.view }]; let borrowed: &Context = boxes[0].view;",
        "let boxed: RefBox = RefBox { view: &holder.view }; let borrowed: &Context = choose(&holder, context, true);",
        "let boxed: RefBox = RefBox { view: &holder.view }; let spare: RefBox = RefBox { view: &context }; let borrowed: &Context = pick(&boxed, &spare, true);",
    ] {
        let fixture = Fixture::with_machines(
            statements,
            "borrowed.scheduler",
            &[],
            "data RefBox { view: &Context; } machine choose(former: &Holder, latter: &Context, flag: bool) -> &Context { transition flag { true -> &former.view false -> latter } } machine pick(a: &RefBox, b: &RefBox, flag: bool) -> &Context { transition flag { true -> a.view false -> b.view } }",
        );
        assert_eq!(
            fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
            None
        );
    }
}

/// Rebinding the leaf slot itself makes the newest stored referent the
/// origin.
#[test]
fn shared_reference_leaf_rebind_tracks_the_latest_slot_store() {
    let fixture = Fixture::with_machines(
        "let mut boxed: RefBox = RefBox { view: &context }; boxed.view = &holder.view; let saved: SchedulerHandle = boxed.view.scheduler;",
        "saved",
        &[],
        "data RefBox { view: &Context; }",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("holder", &[("Holder", "view"), ("Context", "scheduler")]))
    );
}

/// A slot rebind spelled through a `&mut` alias of the carrier cannot be
/// matched to the slot: the exact reference query has no origin for an
/// exclusive binding, so the leaf stays unproven rather than guessing that
/// the alias still names the carrier.
#[test]
fn shared_reference_leaf_rebind_through_an_unresolved_mutable_alias_stays_unproven() {
    let fixture = Fixture::with_machines(
        "let mut boxed: RefBox = RefBox { view: &context }; let mb: &mut RefBox = &mut boxed; mb.view = &holder.view; let saved: SchedulerHandle = boxed.view.scheduler;",
        "saved",
        &[],
        "data RefBox { view: &Context; }",
    );
    assert_eq!(fixture.query(fixture.subject("saved", &[])), None);
}

/// A store into an enclosing slot replaces the whole carrier: the leaf's
/// contents come from the replacement value's own leaf projection.
#[test]
fn shared_reference_leaf_carrier_replacement_tracks_the_new_carrier() {
    for (statements, expected) in [
        // Replaced with a literal carrying a fresh borrow.
        (
            "let mut boxed: RefBox = RefBox { view: &context }; boxed = RefBox { view: &holder.view }; let saved: SchedulerHandle = boxed.view.scheduler;",
            (
                "holder",
                &[("Holder", "view"), ("Context", "scheduler")][..],
            ),
        ),
        // Replaced wholesale by another carrier: the scan relocates to that
        // carrier's leaf and resolves its own latest store.
        (
            "let first: RefBox = RefBox { view: &context }; let mut boxed: RefBox = RefBox { view: &holder.view }; boxed = first; let saved: SchedulerHandle = boxed.view.scheduler;",
            ("context", &[("Context", "scheduler")][..]),
        ),
        // The carrier's own declaration moves through a binding; the leaf's
        // referent stays the one the moved carrier's store supplied.
        (
            "let first: RefBox = RefBox { view: &context }; let boxed: RefBox = first; let saved: SchedulerHandle = boxed.view.scheduler;",
            ("context", &[("Context", "scheduler")][..]),
        ),
    ] {
        let fixture =
            Fixture::with_machines(statements, "saved", &[], "data RefBox { view: &Context; }");
        assert_eq!(
            fixture.query(fixture.subject("saved", &[])),
            Some(fixture.subject(expected.0, expected.1))
        );
    }
}

/// A leaf rebind whose right-hand side is a checked helper's `&` result
/// resolves through that callee's returned expression — the same producer as
/// an ordinary call-result store.
#[test]
fn shared_reference_leaf_rebind_from_a_helper_result() {
    let fixture = Fixture::with_machines(
        "let mut boxed: RefBox = RefBox { view: &context }; boxed.view = lend(holder); let saved: SchedulerHandle = boxed.view.scheduler;",
        "saved",
        &[],
        "data RefBox { view: &Context; } machine lend(holder: &Holder) -> &Context { &holder.view }",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("holder", &[("Holder", "view"), ("Context", "scheduler")]))
    );
}

/// A leaf supplied by a helper whose result picks between inputs through a
/// transition cannot name one referent; the premise stays unproven rather
/// than borrowing either same-shaped candidate.
#[test]
fn shared_reference_leaf_from_an_unproven_route_stays_unproven() {
    let fixture = Fixture::with_machines(
        "let mut boxed: RefBox = RefBox { view: &context }; boxed.view = choose(&holder, context, true); let saved: SchedulerHandle = boxed.view.scheduler;",
        "saved",
        &[],
        "data RefBox { view: &Context; } machine choose(former: &Holder, latter: &Context, flag: bool) -> &Context { transition flag { true -> &former.view false -> latter } }",
    );
    assert_eq!(fixture.query(fixture.subject("saved", &[])), None);
}

/// A call that may write the carrier between the store and the demand leaves
/// the leaf's contents unproven — an opaque or overlapping frame never mints
/// a referent.
#[test]
fn shared_reference_leaf_unknown_call_write_stays_unproven() {
    let fixture = Fixture::with_machines(
        "let mut boxed: RefBox = RefBox { view: &context }; clobber(&mut boxed, &holder.view); let saved: SchedulerHandle = boxed.view.scheduler;",
        "saved",
        &[],
        "data RefBox { view: &Context; } machine clobber(boxed: &mut RefBox, fresh: &Context) { boxed.view = fresh; }",
    );
    assert_eq!(fixture.query(fixture.subject("saved", &[])), None);
}

/// A `&mut` leaf is write-capable: the prefix's stored origins rebase it to
/// its exact referent, and a write through the leaf is then an ordinary store
/// into that referent.
#[test]
fn exclusive_reference_leaf_loads_through_stored_origins() {
    let fixture = Fixture::with_machines(
        "let boxed: MutBox = MutBox { view: context }; let saved: SchedulerHandle = boxed.view.scheduler;",
        "saved",
        &[],
        "data MutBox { view: &mut Context; }",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("context", &[("Context", "scheduler")]))
    );
}

#[test]
fn exclusive_reference_leaf_write_through_arrives_at_the_write_source() {
    let fixture = Fixture::with_machines(
        "let mut boxed: MutBox = MutBox { view: context }; boxed.view.scheduler = replacement.scheduler; let saved: SchedulerHandle = boxed.view.scheduler;",
        "saved",
        &[],
        "data MutBox { view: &mut Context; }",
    );
    assert_eq!(
        fixture.query(fixture.subject("saved", &[])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

/// A `&` result from a checked helper binds the referent its returned
/// expression proves — here the parameter's own field.
#[test]
fn reference_result_helper_binding_derives_the_parameter_leaf() {
    let fixture = Fixture::with_machines(
        "let borrowed: &Context = lend(holder);",
        "borrowed.scheduler",
        &[],
        "machine lend(holder: &Holder) -> &Context { &holder.view }",
    );
    assert_eq!(
        fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
        Some(fixture.subject("holder", &[("Holder", "view"), ("Context", "scheduler")]))
    );
}

/// A `&mut` result whose callee writes nothing through the demanded path
/// still carries the exact input projection: `borrowed` names `context`'s
/// referent storage.
#[test]
fn mutable_reference_result_arrives_from_its_readonly_input() {
    let fixture = Fixture::with_machines(
        "let borrowed: &mut Context = lend_mut(context);",
        "borrowed.scheduler",
        &[],
        "machine lend_mut(context: &mut Context) -> &mut Context { context }",
    );
    assert_eq!(
        fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
        Some(fixture.subject("context", &[("Context", "scheduler")]))
    );
}
