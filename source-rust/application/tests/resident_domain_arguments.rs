//! Resident transport keeps its full type indices, including the selected domains.
#[path = "support/program.rs"]
mod program;

const DOMAINS: &str = r#"use {omega}/language/core/extent.omg as extent;
use {omega}/language/core/layout.omg as layout;
use {omega}/language/core/access/access.omg as access;
use {omega}/language/core/reflect/reflect.omg as reflect;
use {omega}/language/core/run.omg as run;

data Word { value: u64; }
domain Word::Placed access WordAccess;
domain Word::Other access WordAccess;
data WordPlacement = Word in Word::Placed;
WordAccess satisfies |layout|::AccessPolicy {
    machine plan(schema: |reflect|::TypeSchema) -> |layout|::AccessPlan reaches none crashes none terminates {
        |access|::new_plan(@schema, |layout|::Issuance::Unrestricted)
    }
}
"#;

const RESIDENT_RETURN: &str = r#"machine carry(input: @|extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<WordPlacement, Word>)
    -> @|extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<Word in Word::Placed, Word> { input }"#;

fn inspect(declaration: &str) -> (std::process::Output, String) {
    let source = format!("{DOMAINS}\n{declaration}\nmachine main() {{}}");
    program::invoke(&source, &["psi", "inspect", "--machine", "carry"])
}

#[test]
fn an_owned_package_retains_the_resident_theory_of_its_own_hidden_type() {
    let direct = "data Receipt<some S> [linear] {
        storage: |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<Placement, S>;
    }";
    let nested = "data Owner<T> [linear] {
        storage: |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<Placement, T>;
    }
    data Receipt<some S> [linear] { returned: Owner<S>; }";
    for declaration in [direct, nested] {
        let (output, report) = inspect(&format!(
            "data Placement {{}}\n{declaration}\nmachine carry(value: Receipt) -> Receipt {{ move value }}"
        ));
        assert!(output.status.success(), "{declaration}\n{report}");
        assert!(report.contains("psi selected_machine=carry"), "{report}");
    }
    let (output, report) = inspect(&format!(
        "data Placement {{}}\n{direct}\n
        machine carry(value: Receipt) -> |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<Placement, u8> {{
            let {{ storage }} = move value;
            move storage
        }}"
    ));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("the returned value exposes a witness outside its package opening"),
        "{report}"
    );
}

fn change_result(source: &str) -> String {
    source.replace(
        "Resident<Word in Word::Placed, Word> { input }",
        "Resident<Word in Word::Other, Word> { input }",
    )
}

#[test]
fn a_resident_forward_preserves_its_exact_domain_argument() {
    let (output, report) = inspect(RESIDENT_RETURN);
    assert!(output.status.success(), "{report}");
    assert!(report.contains("psi selected_machine=carry"), "{report}");

    let (output, report) = inspect(&change_result(RESIDENT_RETURN));
    assert!(!output.status.success(), "{report}");
    let (carried, expected) = report
        .lines()
        .find_map(|line| line.split_once("the borrow's referent type declares"))
        .expect(&report);
    assert!(
        carried.contains("Word::Placed") && expected.contains("Word::Other"),
        "{report}"
    );
}

#[test]
fn an_owned_resident_return_names_both_mismatched_domain_arguments() {
    let owned = RESIDENT_RETURN.replace("@|extent|::Extent", "|extent|::Extent");
    let (output, report) = inspect(&owned);
    assert!(output.status.success(), "{report}");

    let (output, report) = inspect(&change_result(&owned));
    assert!(!output.status.success(), "{report}");
    let (carried, expected) = report
        .lines()
        .find_map(|line| line.split_once("the machine's result declares"))
        .expect(&report);
    assert!(carried.contains("Word::Placed"), "{report}");
    assert!(expected.contains("Word::Other"), "{report}");
}

#[test]
fn an_owned_run_keeps_its_domain_index_when_transferred() {
    let carry = |index| {
        format!(
            "machine carry(input: |run|::Run<WordPlacement, Word>) -> |run|::Run<{index}, Word> {{ input }}"
        )
    };
    let (output, report) = inspect(&carry("Word in Word::Placed"));
    assert!(output.status.success(), "{report}");
    assert!(report.contains("psi selected_machine=carry"), "{report}");

    let (output, report) = inspect(&carry("Word in Word::Other"));
    assert!(!output.status.success(), "{report}");
    let (expected, carried) = report
        .lines()
        .find_map(|line| line.split_once("but this value carries"))
        .expect(&report);
    assert!(
        expected.contains("Run") && expected.contains("Word::Other"),
        "{report}"
    );
    assert!(carried.contains("Word::Placed"), "{report}");
}

#[test]
fn a_closed_run_requires_the_placement_of_its_own_elements() {
    for (placement, value, refusal) in [
        ("Word", "Word", "must name a domain-qualified type"),
        (
            "WordPlacement",
            "u32",
            "does not qualify the supplied value type `u32`",
        ),
    ] {
        let declaration = format!(
            "machine carry(input: |run|::Run<{placement}, {value}>) -> |run|::Run<{placement}, {value}> {{ input }}"
        );
        let (output, report) = inspect(&declaration);
        assert!(!output.status.success(), "{report}");
        assert!(report.contains(refusal), "{report}");
    }
}

#[test]
fn a_generic_forward_keeps_the_run_placement_selected_by_its_caller() {
    let (output, report) = inspect(
        r#"
machine forward<P,T>(input: |run|::Run<P,T>) -> |run|::Run<P,T> { input }
machine carry(input: |run|::Run<WordPlacement,Word>) -> |run|::Run<WordPlacement,Word> {
    forward::<WordPlacement,Word>(input)
}"#,
    );
    assert!(output.status.success(), "{report}");
}

#[test]
fn another_data_named_run_can_take_unrelated_type_arguments() {
    let (output, report) = inspect(
        r#"
data Run<P,T> { count: u64; }
machine carry(input: Run<u32,bool>) -> Run<u32,bool> { input }
"#,
    );
    assert!(output.status.success(), "{report}");
}

#[test]
fn a_closed_placement_in_a_nested_run_checks_the_supplied_element() {
    let source = r#"
data Envelope<P, T> [linear] { run: |run|::Run<P, T>; }
machine carry(input: Envelope<WordPlacement,Word>) -> Envelope<WordPlacement,Word> { input }
"#;
    let (output, report) = inspect(source);
    assert!(output.status.success(), "{report}");
    let (output, report) = inspect(&source.replace("WordPlacement,Word", "WordPlacement,u32"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("does not qualify the supplied value type `u32`"),
        "{report}"
    );
}

const EMPTY_RUN: &str = r#"machine carry(storage: |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Vacant)
    -> |run|::Run<WordPlacement, Word> in |run|::Run::Live<WordPlacement, Word>
requires storage.length == 0
ensures result.count == 0, result.storage.base == storage.base, result.storage.length == storage.length {
    |run|::empty::<WordPlacement, Word>(storage)
}"#;

#[test]
fn an_empty_run_returns_its_original_vacant_backing() {
    let release = r#"
data Cells = |run|::Run<WordPlacement, Word> in |run|::Run::Live<WordPlacement, Word>;
data Vacant = |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Vacant;
machine carry(input: Cells) -> Vacant
requires input.count == 0, input.storage.length == 0
ensures result.base == input.storage.base, result.length == input.storage.length {
    |run|::Run::release_empty::<WordPlacement, Word>(input)
}"#;
    let (output, report) = inspect(release);
    assert!(output.status.success(), "{report}");
    assert!(report.contains("psi selected_machine=carry"), "{report}");
    for (premise, nonzero) in [
        ("input.count == 0", "input.count == 1"),
        ("input.storage.length == 0", "input.storage.length == 1"),
    ] {
        let (output, report) = inspect(&release.replace(premise, nonzero));
        assert!(!output.status.success(), "{report}");
        assert!(report.contains("cannot prove requires contract"), "{report}");
        assert!(report.contains(premise), "{report}");
    }
}

#[test]
fn an_empty_run_consumes_the_same_authorized_empty_partition() {
    let (output, report) = inspect(EMPTY_RUN);
    assert!(output.status.success(), "{report}");
    for (removed, missing) in [
        ("|extent|::Extent::Granted & ", "Extent::Granted"),
        (" & |extent|::Extent::Vacant", "Extent::Vacant"),
    ] {
        let (output, report) = inspect(&EMPTY_RUN.replace(removed, ""));
        assert!(!output.status.success(), "{report}");
        let (carried, expected) = report
            .split_once("and the parameter it is passed for declares")
            .unwrap_or_else(|| panic!("{report}"));
        assert!(!carried.contains(missing), "{report}");
        assert!(expected.contains(missing), "{report}");
    }
    let (output, report) = inspect(&EMPTY_RUN.replace(
        "requires storage.length == 0",
        "requires storage.length == 1",
    ));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires contract") && report.contains("storage.length == 0"),
        "{report}"
    );
}

#[test]
fn an_empty_live_prefix_preserves_the_original_vacant_capacity() {
    let (output, report) = inspect(
        r#"
data Vacant = |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Vacant;
data Cells = |run|::Run<WordPlacement, Word> in |run|::Run::Live<WordPlacement, Word>;
data Buffer [linear] { live: Cells; spare: Vacant; }
machine carry(storage: Vacant) -> Buffer
ensures result.live.count == 0, result.live.storage.length == 0,
    result.live.storage.base == storage.base, result.spare.base == storage.base,
    result.spare.length == storage.length {
    let parts: |extent|::VacantExtentParts = |extent|::split_vacant_extent(storage, 0);
    Buffer { live: |run|::empty::<WordPlacement, Word>(parts.taken), spare: parts.rest }
}
"#,
    );
    assert!(output.status.success(), "{report}");
    assert!(report.contains("psi selected_machine=carry"), "{report}");
}

#[test]
fn an_empty_run_cannot_use_another_elements_placement() {
    let (output, report) = inspect(&EMPTY_RUN.replace("WordPlacement, Word", "WordPlacement, u32"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("does not qualify the supplied value type `u32`"),
        "{report}"
    );
}

#[test]
fn a_same_named_constructor_cannot_issue_live_for_matching_fields() {
    let source = r#"
machine empty(storage: |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Vacant)
    -> |run|::Run<WordPlacement, Word> in |run|::Run::Live<WordPlacement, Word>
requires storage.length == 0 {
    let r: |run|::Run<WordPlacement, Word> = |run|::Run {
        storage: storage as = |extent|::Extent::Granted - |extent|::Extent::Vacant, count: 0
    };
    r as +Live<WordPlacement, Word>
}
"#;
    let source = format!("{DOMAINS}\n{source}\nmachine main() {{}}");
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("not one of the domain's establishment routes")
            && report.contains("in machine `empty`"),
        "{report}"
    );
}

#[test]
fn an_unqualified_placement_is_invalid_even_when_the_element_is_still_open() {
    let source = format!(
        "{DOMAINS}\nmachine carry<T>(r: |run|::Run<Word,T>) -> |run|::Run<Word,T> {{ r }}\nmachine main() {{}}"
    );
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("placement index `Word` must name a domain-qualified type"),
        "{report}"
    );
}

#[test]
fn a_count_write_cannot_populate_an_empty_live_run() {
    let source = EMPTY_RUN.replace(
        "|run|::empty::<WordPlacement, Word>(storage)\n}",
        "let mutable r: |run|::Run<WordPlacement,Word> in |run|::Run::Live<WordPlacement,Word> = |run|::empty::<WordPlacement,Word>(storage);\n    r.count = 1;\n    r\n}",
    );
    let (output, report) = inspect(&source);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("part of a value that carries `Run::Live`")
            && report.contains("not one of the domain's establishment routes"),
        "{report}"
    );
}

#[test]
fn a_resident_view_checks_its_lent_extent_before_asking_for_custody() {
    let declaration = |placement, schema| {
        format!(
            "machine carry(input: @mutable |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<{placement}, {schema}>) -> @mutable Word in Word::Placed {{\n    |access|::view_resident_mut::<WordPlacement, Word, Word in Word::Placed>(input)\n}}"
        )
    };
    let (output, report) = inspect(&declaration("Word in Word::Placed", "Word"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report
            .contains("resident view needs evaluated layout, content, profile and policy custody"),
        "{report}"
    );

    for (placement, schema, carried_type) in [
        ("Word in Word::Other", "Word", "Word::Other"),
        ("Word in Word::Placed", "u64", "u64"),
    ] {
        let (output, report) = inspect(&declaration(placement, schema));
        assert!(!output.status.success(), "{report}");
        let (carried, expected) = report
            .lines()
            .find_map(|line| line.split_once("the parameter it is passed for declares"))
            .expect(&report);
        assert!(
            carried.contains("Resident") && carried.contains(carried_type),
            "{report}"
        );
        assert!(
            expected.contains("Resident")
                && (expected.contains("Word::Placed") || expected.contains("WordPlacement"))
                && expected.contains(", Word)"),
            "{report}"
        );
        assert!(report.contains("main.omg:18:"), "{report}");
    }
}

#[test]
fn a_resident_view_preserves_the_placement_even_with_a_shared_policy() {
    let (output, report) = inspect(
        r#"machine carry(input: @mutable |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<WordPlacement, Word>) -> @mutable Word in Word::Other {
    |access|::view_resident_mut::<WordPlacement, Word, Word in Word::Other>(input)
}"#,
    );
    assert!(!output.status.success(), "{report}");
    let (placement, view) = report
        .lines()
        .find_map(|line| line.split_once("but the requested view carries"))
        .expect(&report);
    assert!(placement.contains("Word::Placed"), "{report}");
    assert!(view.contains("Word::Other"), "{report}");
}

#[test]
fn a_resident_view_requires_a_qualified_index_of_its_value_type() {
    for (index, schema, expected) in [
        ("Word", "Word", "must name a domain-qualified type"),
        (
            "WordPlacement",
            "u16",
            "does not qualify the supplied value type `u16`",
        ),
    ] {
        let body = format!(
            "machine carry(input: @mutable |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<{index}, {schema}>) -> @mutable Word in Word::Placed {{\n    |access|::view_resident_mut::<{index}, {schema}, Word in Word::Placed>(input)\n}}"
        );
        let (output, report) = inspect(&body);
        assert!(!output.status.success(), "{report}");
        assert!(report.contains(expected), "{report}");
    }
}
