//! The lowered-form checker reads a type that carries a declared domain,
//! `T in D`, as the carrier `T` and the set of domains on it
//! (`wiki/spec/language/domains.md`, "Literals and expected types": "the
//! supplied expression's static type must carry exactly the domains the
//! destination declares"). A value keeps its domains through a parameter, a
//! call and a result; a cast entry keeps or removes one by name; nothing
//! else attaches or drops one.
//!
//! One program pins the accepted forms from source. Each twin changes one
//! machine and is not accepted.

use lowered_discharge::RouteVerdict;

// Compilation shares process-wide counters; run one at a time.
static COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

const DOMAINS: &str = "\
use omega::language::core::AccessPolicy;
use omega::language::core::AccessPlan;
use omega::language::core::Schema;

domain i32::Km;
machine carry(distance: i32 in Km) -> i32 in Km {
    distance
}

data Reading {
    value: i32;
}
domain Reading::Checked;
data Sample {
    reading: Reading in Checked;
    count: i32;
}

data Seal {
    mark: i32;
}
SealAccess satisfies AccessPolicy {
    machine plan(schema: Schema) -> AccessPlan {
        let plan: AccessPlan;
        plan
    }
}
domain Seal::Issued
    access SealAccess;
data Sealed {
    seal: Seal in Issued;
    count: i32;
}

data Ticket {
    number: i32;
}
domain Ticket::Stamped established by stamp;
machine stamp(ticket: Ticket) -> Ticket in Stamped {
    ticket as +Stamped
}
machine main() { }
";

/// A qualified value passes to a parameter and a result of its exact type,
/// `=Km` keeps the domain and `-Km` removes it.
const ACCEPTED: &str = "\
machine relay(distance: i32 in Km) -> i32 in Km {
    carry(distance)
}
machine kept(distance: i32 in Km) -> i32 in Km {
    distance as =Km
}
machine removed(distance: i32 in Km) -> i32 {
    distance as -Km
}
machine compared(left: i32 in Km, right: i32 in Km) -> bool {
    (left as -Km) > (right as -Km)
}
machine added(distance: i32) -> i32 in Km {
    distance as +Km
}
machine landed() -> i32 in Km {
    5 as +Km
}
";

/// The two judged parts of an accepted program: its machines, and that
/// `SealAccess::plan` refines the `plan` its trait requires.
const ACCEPTED_PARTS: [&str; 2] = ["accepted", "accepted"];

/// The verdict of each judged part of `DOMAINS` followed by `machines`,
/// staged as a standalone `main.omg`: one word, and what a refusal names.
fn verdicts(name: &str, machines: &str) -> Vec<String> {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root =
        std::env::temp_dir().join(format!("omega-domain-types-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&root).expect("staging directory");
    let main = root.join("main.omg");
    std::fs::write(&main, format!("{DOMAINS}{machines}")).expect("staged source");
    let frontier = compiler::CompilationSession::default().typed_frontier_without_build(
        compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
        None,
    );
    let _ = std::fs::remove_dir_all(&root);
    let frontier = frontier.unwrap_or_else(|failure| {
        panic!(
            "{name}: Psi 00 through 04 must complete: {:?}",
            failure.into_diagnostics()
        )
    });
    lowered_discharge::judge_compilation(
        &frontier.typed,
        structure_elaboration::Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            calling: frontier.calling.borrow(&frontier.typed).selection(),
            providers: frontier.provider_selections(),
            bindings: &frontier.bindings,
        },
    )
    .iter()
    .map(|verdict| match verdict {
        Ok(RouteVerdict::Accepted(_)) => "accepted".to_owned(),
        Ok(RouteVerdict::Rejected { .. }) => "rejected".to_owned(),
        Ok(RouteVerdict::Incomplete(_)) => "incomplete".to_owned(),
        Ok(RouteVerdict::FallBack(refusal)) => format!("not-supported: {refusal:?}"),
        Err(defect) => format!("defect: {defect:?}"),
    })
    .collect()
}

#[test]
fn a_value_keeps_its_domain_through_parameters_results_and_entries() {
    assert_eq!(verdicts("accepted", ACCEPTED), ACCEPTED_PARTS);
}

/// The one verdict of a twin, which must not be an acceptance and must name
/// `expected`.
fn refused_with(name: &str, machines: &str, expected: &str) {
    let verdicts = verdicts(name, machines);
    assert!(
        verdicts.len() == 1 && verdicts[0].contains(expected) && verdicts[0] != "accepted",
        "{name}: {verdicts:#?}"
    );
}

#[test]
fn a_plain_argument_for_a_qualified_parameter_is_not_accepted() {
    refused_with(
        "plain_argument",
        "machine wrong(distance: i32) -> i32 in Km {\n    carry(distance)\n}\n",
        "typing argument types",
    );
}

#[test]
fn a_plain_value_for_a_qualified_result_is_not_accepted() {
    refused_with(
        "plain_result",
        "machine wrong(distance: i32) -> i32 in Km {\n    distance\n}\n",
        "typing arm value type",
    );
}

#[test]
fn a_qualified_value_for_a_plain_result_is_not_accepted() {
    refused_with(
        "qualified_result",
        "machine wrong(distance: i32 in Km) -> i32 {\n    distance\n}\n",
        "typing arm value type",
    );
}

#[test]
fn a_builtin_comparison_of_qualified_operands_is_not_accepted() {
    refused_with(
        "qualified_operands",
        "machine wrong(left: i32 in Km, right: i32 in Km) -> bool {\n    left > right\n}\n",
        "operator on an operand that is not a plain or policy-qualified scalar",
    );
}

/// Who may add a domain is on its declaration. Anyone may add `Km`, which
/// restricts nothing (`added` and `landed` above), and `stamp` is the one
/// route of `Stamped`: any other machine is outside it.
#[test]
fn adding_a_routed_domain_outside_its_routes_is_refused_by_name() {
    refused_with(
        "outside_route",
        "machine wrong(ticket: Ticket) -> Ticket in Stamped {\n    ticket as +Stamped\n}\n",
        "cast that adds a domain outside its establishment routes",
    );
}

/// A value its route issued travels on, and the domain is not cast off it.
#[test]
fn removing_a_routed_domain_is_refused_by_name() {
    refused_with(
        "routed_removal",
        "machine wrong(ticket: Ticket in Stamped) -> Ticket {\n    ticket as -Stamped\n}\n",
        "removal of a domain with restricted issuance",
    );
}

/// Nothing states a domain's predicates as an obligation on a cast's
/// operand yet, so a program that declares such a domain is not accepted.
#[test]
fn a_domain_with_predicates_is_refused_by_name() {
    refused_with(
        "predicates",
        "domain i32::Positive\n    requires self > 0;\nmachine wrong(value: i32) -> i32 in Positive {\n    value as +Positive\n}\n",
        "domain with predicates",
    );
}

/// A domain on a data value travels as one on a scalar does. A domain with
/// no policy restricts no reader, so a field of the value is read; a
/// governed value is passed on whole, stored in a field of its exact type
/// and moved out again.
const ACCEPTED_DATA: &str = "\
machine relayed(reading: Reading in Checked) -> Reading in Checked {
    reading
}
machine inside(reading: Reading in Checked) -> i32 {
    reading.value
}
machine gathered(reading: Reading in Checked) -> Sample {
    Sample { reading: reading, count: 1 }
}
machine bare(reading: Reading in Checked) -> Reading {
    reading as -Checked
}
machine zeroed() -> i32 {
    let sample: Sample;
    sample.count
}
machine forwarded(seal: Seal in Issued) -> Sealed {
    Sealed { seal: seal, count: 1 }
}
machine counted(sealed: Sealed) -> i32 {
    sealed.count
}
machine unsealed(sealed: Sealed) -> Seal in Issued {
    sealed.seal
}
machine held(seal: &Seal in Issued) { }
machine lent(sealed: &Sealed) {
    held(&sealed.seal)
}
machine checked(reading: Reading) -> Reading in Checked {
    reading as +Checked
}
machine stamped(ticket: Ticket) -> Ticket in Stamped {
    stamp(ticket)
}
";

#[test]
fn a_data_value_keeps_its_domain_and_a_governed_one_is_passed_on_whole() {
    assert_eq!(verdicts("accepted_data", ACCEPTED_DATA), ACCEPTED_PARTS);
}

#[test]
fn a_plain_record_for_a_qualified_field_is_not_accepted() {
    refused_with(
        "plain_field",
        "machine wrong(reading: Reading) -> Sample {\n    Sample { reading: reading, count: 1 }\n}\n",
        "typing record literal field type",
    );
}

/// Who may read a governed value is in its policy's plan, which nothing
/// evaluates, so no machine is shown to be a reader.
#[test]
fn reading_inside_a_governed_value_is_refused_by_name() {
    refused_with(
        "governed_read",
        "machine wrong(seal: Seal in Issued) -> i32 {\n    seal.mark\n}\n",
        "read inside a value governed by an access policy",
    );
}

#[test]
fn writing_inside_a_governed_value_is_refused_by_name() {
    refused_with(
        "governed_write",
        "machine wrong(seal: Seal in Issued) -> Seal in Issued {\n    let mut held: Seal in Issued = seal;\n    held.mark = 2;\n    held\n}\n",
        "inside a value governed by an access policy",
    );
}

#[test]
fn removing_a_governed_domain_is_refused_by_name() {
    refused_with(
        "governed_removal",
        "machine wrong(seal: Seal in Issued) -> Seal {\n    seal as -Issued\n}\n",
        "removal of a domain with restricted issuance",
    );
}

/// A policy names its issuers in its plan, which nothing evaluates, so no
/// machine is shown to be one. The zeroed plan `SealAccess` returns would
/// admit nobody in any case.
#[test]
fn adding_a_governed_domain_is_refused_by_name() {
    refused_with(
        "governed_added",
        "machine wrong(seal: Seal) -> Seal in Issued {\n    seal as +Issued\n}\n",
        "cast that adds a domain whose issuers are in an access plan",
    );
}

/// A record with a governed field is gated: zero is not a value somebody
/// issued, so the record is constructed with that field supplied.
#[test]
fn zero_establishing_a_record_with_a_governed_field_is_refused_by_name() {
    refused_with(
        "governed_zero",
        "machine wrong() -> i32 {\n    let sealed: Sealed;\n    sealed.count\n}\n",
        "zero-established value in a domain that must be established",
    );
}

/// The parser has no `..` yet, so a literal names every field or is not
/// one; when `..` arrives it zero-establishes the fields it omits and meets
/// the same gate as `let sealed: Sealed;`.
#[test]
fn omitting_a_governed_field_from_a_literal_is_not_accepted() {
    refused_with(
        "governed_omitted",
        "machine wrong() -> Sealed {
    Sealed { count: 1 }
}
",
        "typing record literal misses a field",
    );
}

#[test]
fn a_literal_for_a_governed_field_is_not_accepted() {
    refused_with(
        "governed_literal",
        "machine wrong() -> Sealed {\n    Sealed { seal: Seal { mark: 1 }, count: 1 }\n}\n",
        "typing record literal field type",
    );
}

/// A loan of a governed field is a loan of its exact type: a machine that
/// takes the plain carrier would read inside it.
#[test]
fn lending_a_governed_field_as_its_plain_carrier_is_not_accepted() {
    refused_with(
        "governed_loan",
        "machine peek(seal: &Seal) -> i32 {
    seal.mark
}
machine wrong(sealed: &Sealed) -> i32 {
    peek(&sealed.seal)
}
",
        "typing argument types",
    );
}

#[test]
fn writing_through_a_reference_to_a_governed_value_is_refused_by_name() {
    refused_with(
        "governed_reference_write",
        "machine wrong(seal: &mut Seal in Issued) {
    seal.mark = 2;
}
",
        "inside a value governed by an access policy",
    );
}

/// A governed place is not overwritten either: replacing the whole value
/// is a write to it.
#[test]
fn replacing_a_governed_field_is_refused_by_name() {
    refused_with(
        "governed_replace",
        "machine wrong(seal: Seal in Issued) -> Sealed {
    let mut sealed: Sealed = Sealed { seal: seal, count: 1 };
    sealed.count = 2;
    sealed.seal = sealed.seal;
    sealed
}
",
        "replacement of a value governed by an access policy",
    );
}

/// `Seal` is not `[copy]`, so the one way to hold a second governed value
/// would be to take it out of a record that is only lent. Ownership refuses
/// that: the lender still owns the field.
#[test]
fn moving_a_governed_field_out_of_an_exclusive_loan_is_not_accepted() {
    refused_with(
        "governed_moved_out",
        "machine wrong(sealed: &mut Sealed) -> Seal in Issued {\n    sealed.seal\n}\n",
        "owned extraction from borrowed storage",
    );
}

#[test]
fn rebuilding_a_record_from_a_lent_governed_field_is_not_accepted() {
    refused_with(
        "governed_rebuilt",
        "machine wrong(sealed: &Sealed) -> Sealed {\n    Sealed { seal: sealed.seal, count: 1 }\n}\n",
        "owned extraction from borrowed storage",
    );
}
