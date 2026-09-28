use super::StatementNode;
use crate::checks::termination::progress::origins::tests::Fixture;

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
        crate::semantic::calls::find_state(&fixture.program, fixture.state.state_symbol).unwrap();
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

/// A `&` binding declared from an indexed carrier leaf resolves the element
/// the literal index selects: the carrier's own construction supplies each
/// element's referent.
#[test]
fn shared_reference_binding_from_an_indexed_carrier_leaf_derives_the_element_store() {
    for (index, expected) in [
        ("0", ("context", &[("Context", "scheduler")][..])),
        (
            "1",
            (
                "holder",
                &[("Holder", "view"), ("Context", "scheduler")][..],
            ),
        ),
    ] {
        let fixture = Fixture::with_machines(
            &format!(
                "let boxes: [RefBox; 2] = [RefBox {{ view: &context }}, RefBox {{ view: &holder.view }}]; let borrowed: &Context = boxes[{index}].view;"
            ),
            "borrowed.scheduler",
            &[],
            "data RefBox { view: &Context; }",
        );
        assert_eq!(
            fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
            Some(fixture.subject(expected.0, expected.1))
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
        "let boxes: [RefBox; 2] = [RefBox { view: &context }, RefBox { view: &holder.view }]; let at: usize = slot_index(true); let borrowed: &Context = boxes[at].view;",
        "let boxed: RefBox = RefBox { view: &holder.view }; let borrowed: &Context = choose(&holder, context, true);",
        "let boxed: RefBox = RefBox { view: &holder.view }; let spare: RefBox = RefBox { view: &context }; let borrowed: &Context = pick(&boxed, &spare, true);",
    ] {
        let fixture = Fixture::with_machines(
            statements,
            "borrowed.scheduler",
            &[],
            "data RefBox { view: &Context; } machine choose(former: &Holder, latter: &Context, flag: bool) -> &Context { transition flag { true -> &former.view false -> latter } } machine pick(a: &RefBox, b: &RefBox, flag: bool) -> &Context { transition flag { true -> a.view false -> b.view } } machine slot_index(flag: bool) -> usize { transition flag { true -> 0 false -> 1 } }",
        );
        assert_eq!(
            fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
            None
        );
    }
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

/// A `&` binding whose operand is a member selection out of a record literal
/// carries no storage of its own: the referent is the operand bound to that
/// exact field, so `borrowed.scheduler` derives the borrowed target's leaf.
#[test]
fn shared_reference_bound_from_constructed_record_leaf() {
    let fixture = Fixture::with_machines(
        "let borrowed: &Context = RefBox { view: &context }.view;",
        "borrowed.scheduler",
        &[],
        "data RefBox { view: &Context; }",
    );
    assert_eq!(
        fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
        Some(fixture.subject("context", &[("Context", "scheduler")]))
    );
}

/// A member selection applied after the index never resolved its field
/// symbol (the member lands on an indexed temporary, not a named place), so
/// the operand is not exact provenance and stays unproven.
#[test]
fn shared_reference_bound_through_indexed_literal_member_stays_unproven() {
    let fixture = Fixture::with_machines(
        "let borrowed: &Context = [RefBox { view: &context }, RefBox { view: &holder.view }][1].view;",
        "borrowed.scheduler",
        &[],
        "data RefBox { view: &Context; }",
    );
    assert_eq!(
        fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
        None
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
