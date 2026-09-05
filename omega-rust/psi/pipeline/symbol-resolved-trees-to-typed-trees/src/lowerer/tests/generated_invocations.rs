use super::*;
use typed_trees::signature::AuthoredInvocationTarget;

const BASE: &str = "boundary trait Console { machine write(value: i32) reaches Console; }";

#[test]
fn seeded_invocations_retain_service_and_parameter_identity_and_source_occurrences() {
    let extension_source = "pub machine direct() reaches Console invokes Console; {}\n\
        pub machine parameter(first: Console, second: Console) reaches Console invokes second; {}";
    let (base, extension) = seeded_plain_data_inputs(BASE, extension_source);
    let retained = base.typed().clone();
    let expected_reaches = extension.trees().authored_service_reach_rows.clone();
    let typed = lower_seeded_extension(extension, base)
        .unwrap_or_else(|(_, error)| panic!("reuse retained Console reach: {error:?}"));
    assert!(retained_typed_base_is_exact_prefix(&retained, &typed));
    assert_eq!(typed.service_reaches, retained.service_reaches);
    assert_eq!(typed.service_reach_rows, retained.service_reach_rows);
    assert_eq!(
        typed.authored_service_reach_rows.len(),
        expected_reaches.len()
    );
    for actual in &typed.authored_service_reach_rows {
        let expected = expected_reaches
            .iter()
            .find(|row| row.owner == actual.owner)
            .unwrap();
        assert_eq!(actual.owner, expected.owner);
        assert_eq!(actual.keyword_source_spans, expected.keyword_source_spans);
        assert_eq!(actual.installation_bound, expected.installation_bound);
        assert_eq!(actual.targets.len(), expected.targets.len());
        for (actual, expected) in actual.targets.iter().zip(&expected.targets) {
            assert_eq!(actual.service, expected.service);
            assert_eq!(actual.source_span, expected.source_span);
        }
    }
    let console = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Console")
        .unwrap();
    for name in ["direct", "parameter"] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let [invocation] = typed.machine_invokes(machine) else {
            panic!("one exact invocation");
        };
        let expected_name = if name == "direct" {
            assert_eq!(
                invocation.target,
                AuthoredInvocationTarget::Service(console.symbol)
            );
            "Console"
        } else {
            let parameters = typed.state_parameters(&typed.machine_states(machine)[0]);
            assert_eq!(
                invocation.target,
                AuthoredInvocationTarget::Parameter {
                    ordinal: 1,
                    symbol: parameters[1].symbol
                }
            );
            "second"
        };
        let span = invocation.source_span;
        assert_eq!(
            &extension_source[span.span.start..span.span.end],
            expected_name
        );
        assert_eq!(
            span.source_id,
            typed
                .authored_service_reach_rows_for(machine.symbol)
                .next()
                .unwrap()
                .keyword_source_spans[0]
                .source_id
        );
    }

    let mut changed = typed;
    changed.authored_service_reach_rows[0].targets.clear();
    assert!(!retained_typed_base_is_exact_prefix(&retained, &changed));
}

#[test]
fn seeded_invocations_without_reaches_reuse_normalized_service_rows() {
    let (base, extension) =
        seeded_plain_data_inputs(BASE, "pub machine generated() invokes Console; {}");
    let retained = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("invokes contributes existing Console reach");
    assert!(retained_typed_base_is_exact_prefix(&retained, &typed));
    assert_eq!(
        typed.authored_service_reach_rows,
        retained.authored_service_reach_rows
    );
    let machine = typed.machines().last().unwrap();
    let console = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Console")
        .unwrap();
    assert_eq!(
        typed.service_reach_rows.services(machine.service_reach_row),
        &[typed.service_reaches.id_for_symbol(console.symbol).unwrap()]
    );
    assert!(
        matches!(typed.machine_invokes(machine), [invocation] if invocation.target == AuthoredInvocationTarget::Service(console.symbol))
    );
}

#[test]
fn seeded_invocations_append_new_normalized_service_sets_without_rebinding_base_rows() {
    let (base, extension) = seeded_plain_data_inputs(
        "boundary trait Console { machine write() reaches Console; } boundary trait FilesystemHost { machine write() reaches FilesystemHost; }",
        "pub machine generated() reaches Console + FilesystemHost invokes Console; {}\n\
         pub machine reordered() reaches FilesystemHost + Console invokes FilesystemHost; {}",
    );
    let retained = base.typed().clone();
    let typed = lower_seeded_extension(extension, base).expect("append a normalized service set");
    assert!(retained_typed_base_is_exact_prefix(&retained, &typed));
    assert_eq!(typed.service_reaches, retained.service_reaches);
    assert_ne!(typed.service_reach_rows, retained.service_reach_rows);
    let console = typed.service_reaches.id_for_name("Console").unwrap();
    let filesystem = typed.service_reaches.id_for_name("FilesystemHost").unwrap();
    let generated = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "generated")
        .unwrap();
    let reordered = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "reordered")
        .unwrap();
    assert_eq!(generated.service_reach_row, reordered.service_reach_row);
    assert_eq!(
        typed
            .service_reach_rows
            .services(generated.service_reach_row),
        &[console, filesystem]
    );
    assert!(
        retained
            .service_reach_rows
            .services(generated.service_reach_row)
            .is_empty()
    );

    let mut changed = typed.clone();
    changed.service_reach_rows = language_semantics::ServiceReachRowTable::default();
    changed.service_reach_rows.intern(vec![filesystem]);
    changed.service_reach_rows.intern(vec![console]);
    changed.service_reach_rows.intern(vec![console, filesystem]);
    assert!(!retained_typed_base_is_exact_prefix(&retained, &changed));
    changed.service_reach_rows = language_semantics::ServiceReachRowTable::default();
    assert!(!retained_typed_base_is_exact_prefix(&retained, &changed));
}

#[test]
fn seeded_invocations_reject_incompatible_typed_service_rows_transactionally() {
    let (mut base, extension) = seeded_plain_data_inputs(
        "boundary trait Console { machine write() reaches Console; } boundary trait FilesystemHost { machine write() reaches FilesystemHost; }",
        "pub machine generated() reaches Console + FilesystemHost invokes Console; {}",
    );
    let filesystem = base
        .typed()
        .service_reaches
        .id_for_name("FilesystemHost")
        .unwrap();
    let console = base.typed().service_reaches.id_for_name("Console").unwrap();
    base.typed_mut().service_reach_rows = language_semantics::ServiceReachRowTable::default();
    base.typed_mut().service_reach_rows.intern(vec![filesystem]);
    base.typed_mut().service_reach_rows.intern(vec![console]);
    let retained = base.typed().clone();
    let (returned, error) = lower_seeded_extension(extension, base)
        .expect_err("resolved row IDs cannot overwrite changed typed meanings");
    assert_eq!(
        error,
        SeededContinuationError::ResolvedSemanticTablesChanged
    );
    assert_eq!(returned.into_typed(), retained);
}
