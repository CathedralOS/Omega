use crate::CheckingRequest;
use crate::tests::front_end::{checked_program, typed_program, typed_program_result};
use crate::tests::lower_typed_trees;

#[test]
fn generic_template_identity_is_positional_across_parameter_renames() {
    fn identity(
        machine_parameter: &str,
        value: &str,
        item: &str,
    ) -> (u64, typed_trees::typed_trees::MachineTemplateCommitment) {
        let source = format!(
            r#"
                boundary machine admitted<machine {machine_parameter}>({value}: i32)
                where machine {machine_parameter}({item}: i32)
                    requires {item} > 0;
                requires {value} > 0;
                ensures true;
            "#
        );
        let typed = typed_program(&source);
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("accepted template should exist");
        (
            crate::monomorphization::generic_machine_template_report_fingerprint(
                &typed,
                admitted.symbol,
            )
            .expect("generic template should have a report identity"),
            crate::monomorphization::generic_machine_template_commitment(&typed, admitted.symbol)
                .expect("generic template should have a strong identity"),
        )
    }

    assert_eq!(
        identity("F", "value", "item"),
        identity("Operation", "input", "candidate")
    );
}

#[test]
fn generic_template_identity_normalizes_crash_route_buckets() {
    fn report_fingerprint(crash_clauses: &str) -> u64 {
        let source = format!(
            r#"
                boundary machine admitted<T>(first: bool, second: bool)
                {crash_clauses}
                ensures true;
            "#
        );
        let typed = typed_program(&source);
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_report_fingerprint(
            &typed,
            admitted.symbol,
        )
        .expect("generic template should have an identity")
    }

    let grouped = r#"
        crashes Trap
            first
            second
    "#;
    let split = r#"
        crashes Trap
            second
        crashes Trap
            first
    "#;
    let duplicated = r#"
        crashes Trap
            first
            second
            first
    "#;
    let unconditional = r#"
        crashes Trap
    "#;
    let explicit_true = r#"
        crashes Trap
            true
    "#;
    let unconditional_with_guard = r#"
        crashes Trap
            first
        crashes Trap
    "#;

    assert_eq!(report_fingerprint(grouped), report_fingerprint(split));
    assert_eq!(report_fingerprint(grouped), report_fingerprint(duplicated));
    assert_eq!(
        report_fingerprint(unconditional),
        report_fingerprint(explicit_true)
    );
    assert_eq!(
        report_fingerprint(unconditional),
        report_fingerprint(unconditional_with_guard)
    );
    assert_ne!(
        report_fingerprint(grouped),
        report_fingerprint(unconditional)
    );

    fn slot_report_fingerprint(crash_clauses: &str) -> u64 {
        let source = format!(
            r#"
                boundary machine admitted<T, machine Operation>()
                where machine Operation(first: bool, second: bool)
                    {crash_clauses};
                ensures true;
            "#
        );
        let typed = typed_program(&source);
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_report_fingerprint(
            &typed,
            admitted.symbol,
        )
        .expect("generic template should have an identity")
    }

    assert_eq!(
        slot_report_fingerprint(grouped),
        slot_report_fingerprint(split)
    );
    assert_eq!(
        slot_report_fingerprint(grouped),
        slot_report_fingerprint(duplicated)
    );
    assert_eq!(
        slot_report_fingerprint(unconditional),
        slot_report_fingerprint(explicit_true)
    );
    assert_eq!(
        slot_report_fingerprint(unconditional),
        slot_report_fingerprint(unconditional_with_guard)
    );
}

#[test]
fn contract_only_static_selections_do_not_consume_generic_machine_schema() {
    let source = r#"
        pub data Index { case Zero; }
        data Stream<machine S>
        where machine S(index: Index) -> Index;
        { case Empty; case More(tail: Stream<S>); }
        data Main {}

        boundary machine source(index: Index) -> Index;

        machine equivalent<machine A, machine B>(a: Stream<A>, b: Stream<B>) -> bool
        where machine A(index: Index) -> Index;
        where machine B(index: Index) -> Index;
        {
            transition { _ -> (true) }
        }

        machine reflexive<machine A>(a: Stream<A>)
        where machine A(index: Index) -> Index;
        ensures equivalent<A, A>(a, a)
        {
        }

        machine concrete_premise(a: Stream<source>)
        requires equivalent<source, source>(a, a)
        {
        }

        machine Main::run(&mut self) {}
    "#;

    let checked = checked_program(source);

    let equivalent = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "equivalent")
        .expect("generic relation schema");
    assert_eq!(checked.machine_type_parameters(equivalent).len(), 2);
    assert!(
        checked
            .machine_specializations
            .iter()
            .all(|specialization| specialization.template != equivalent.symbol)
    );
}

#[test]
fn generic_member_borrows_use_the_receivers_exact_type_arguments() {
    for (member, accepted) in [("first", true), ("second", false)] {
        let source = format!(
            r#"
            data Pair<Left, Right> {{ first: Left; second: Right; }}
            machine inspect<Left, Right, machine Visit>(pair: Pair<Left, Right>)
            where machine Visit(value: &Left);
            {{ Visit(pair.{member}); }}
        "#
        );
        let typed = typed_program_result(&source).expect("generic member source types");
        let result = lower_typed_trees(typed, &CheckingRequest::settled());
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
    }
}
