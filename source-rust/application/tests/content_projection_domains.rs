//! Core Content admits the two resource algebras declared in content.omg.
#[path = "support/program.rs"]
mod program;

const ARBITRARY_ALGEBRA: &str = r#"use {omega}/language/core/content.omg as algebra;
data Region { length: u64; }
domain Region::Owned;
machine Owned::measure(subject: @Region) -> u64
satisfies |algebra|::Content<Region, u64>::project { subject.length }
machine retain(input: Region in Owned) -> Region in Owned { input }
machine main() {}"#;

#[test]
fn a_declaring_package_cannot_choose_an_arbitrary_content_algebra() {
    let (output, report) = program::invoke(ARBITRARY_ALGEBRA, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("core Content requires IntervalSet<Space> or CountedQuantity<Unit>"),
        "{report}"
    );
}

#[test]
fn a_user_content_trait_keeps_its_declared_algebra() {
    let source = ARBITRARY_ALGEBRA
        .replace(
            "use {omega}/language/core/content.omg as algebra;",
            "trait Content<Owner, A> { machine project(subject: @Owner) -> A; }",
        )
        .replace("|algebra|::Content", "Content");
    program::assert_silent_return(&source);
}

const FIELD_PROJECTION: &str = r#"use {omega}/language/core/content.omg as algebra;
use {omega}/language/core/nat.omg as nat;
data Bytes {}
data Region { length: u64; }
domain Region::Owned;
machine Owned::measure(subject: @Region) -> |algebra|::CountedQuantity<Bytes>
satisfies |algebra|::Content<Region, |algebra|::CountedQuantity<Bytes>>::project {
    |algebra|::CountedQuantity { magnitude: embed(subject.length) as |nat|::Nat }
}
machine retain(input: Region in Owned) -> Region in Owned { input }
machine main() {}"#;

#[test]
fn a_content_projection_retains_the_selected_unit_and_its_subject_field() {
    let (output, report) =
        program::invoke(FIELD_PROJECTION, &["psi", "inspect", "--machine", "retain"]);
    assert!(output.status.success(), "{report}");
    let domain = report
        .lines()
        .find(|line| line.starts_with("domain ") && line.contains("Owned"))
        .expect(&report);
    assert!(
        domain.contains("kind: CountedQuantity") && domain.contains("Bytes"),
        "{domain}"
    );
    assert!(
        domain.contains(r#"CountedQuantity(RuntimeScalarEmbedding(["length"]))"#),
        "{domain}"
    );
}

#[test]
fn an_arbitrary_helper_call_remains_outside_the_content_projection_fragment() {
    let source = FIELD_PROJECTION
        .replace(
            "machine Owned::measure",
            "machine read(subject: @Region) -> u64 { subject.length }\nmachine Owned::measure",
        )
        .replace("embed(subject.length)", "embed(read(subject))");
    let (output, report) = program::invoke(&source, &["psi", "inspect", "--machine", "retain"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("closed proof-natural fragment"), "{report}");
}

#[test]
fn interval_projection_orders_proof_naturals_before_qualifying_the_resource() {
    let source = FIELD_PROJECTION
        .replace("data Region { length: u64; }", "data Region { base: address; length: u64; }")
        .replace("CountedQuantity", "IntervalSet")
        .replace("magnitude: embed(subject.length) as |nat|::Nat", "start: embed(subject.base) as |nat|::Nat, end: (embed(subject.base) + embed(subject.length)) as |nat|::Nat");
    let (output, report) = program::invoke(&source, &["psi", "inspect", "--machine", "retain"]);
    assert!(output.status.success(), "{report}");
    let domain = report
        .lines()
        .find(|line| line.starts_with("domain ") && line.contains("Owned"))
        .expect(&report);
    assert!(domain.contains(r#"IntervalSet([(RuntimeScalarEmbedding(["base"]), Add(RuntimeScalarEmbedding(["base"]), RuntimeScalarEmbedding(["length"])))])"#), "{domain}");

    let reversed = source.replace("start: embed(subject.base) as |nat|::Nat, end: (embed(subject.base) + embed(subject.length))",
        "start: (embed(subject.base) + embed(subject.length)) as |nat|::Nat, end: embed(subject.base)");
    let (output, report) = program::invoke(&reversed, &["psi", "inspect", "--machine", "retain"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("content projection") && report.contains("not proved"),
        "{report}"
    );
}

#[test]
fn promising_content_membership_requires_a_qualified_returned_value() {
    let promise = FIELD_PROJECTION.replace(
        "-> Region in Owned { input }",
        "-> Region ensures result in Owned { input as -Owned }",
    );
    program::assert_silent_return(&promise);
    let forged = promise.replace("input as -Owned", "Region { length: 0u64 }");
    let (output, report) = program::invoke(&forged, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("normal completion does not establish its promised runtime membership"),
        "{report}"
    );
}

#[test]
fn installed_core_joins_two_owned_vacant_extents_only_under_its_contract() {
    let source = r#"use {omega}/language/core/extent.omg as ranges;
machine join(
    left: |ranges|::Extent in |ranges|::Extent::Granted & |ranges|::Extent::Vacant,
    right: |ranges|::Extent in |ranges|::Extent::Granted & |ranges|::Extent::Vacant
) -> |ranges|::Extent in |ranges|::Extent::Granted & |ranges|::Extent::Vacant
requires
    embed(left.length) + embed(right.length) <= 18446744073709551615,
    left.length == 0 || right.length == 0 || embed(right.base) == embed(left.base) + embed(left.length)
{
    |ranges|::merge_vacant_extents(left, right)
}
machine main() {}"#;
    let inspect = &["psi", "inspect", "--machine", "join"];
    let (output, report) = program::invoke(source, inspect);
    assert!(output.status.success(), "{report}");

    let gap = source.replace("embed(left.base) + embed(left.length)", "embed(left.base)");
    let (output, report) = program::invoke(&gap, inspect);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires") && report.contains("merge_vacant_extents"),
        "{report}"
    );
}

fn with_transfer_contract() -> String {
    let transfer = r#"use {omega}/language/core/foundation.omg as proof;
public boundary trait Transfer {
        machine pass(input: Region in Owned) -> Region
        reaches none
        ensures result in Owned,
            |proof|::Identity::<|algebra|::CountedQuantity<Bytes>>(
                Owned::measure(old(@input)), Owned::measure(@result))
        crashes none;
    }
    "#;
    FIELD_PROJECTION.replace(
        "machine main() {}",
        &format!("{transfer}machine main() {{}}"),
    )
}

#[test]
fn an_admitted_content_guarantee_reopens_with_its_actual_input_and_result() {
    use semantic_vocabulary::{ContentPlaceVersion, ContentTerm, Proposition};
    use terminal_psi::TerminalMachineResult;
    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "source-content-guarantee",
        false,
    )
    .unwrap();
    let artifact = scratch.path().join("transfer.psi");
    let (output, report) = program::invoke(
        &with_transfer_contract(),
        &[
            "psi",
            "inspect",
            "--machine",
            "retain",
            "--output",
            artifact.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{report}");
    let bytes = std::fs::read(&artifact).unwrap();
    let saved = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let module = saved.semantic_module();
    let projection = module
        .types
        .structural_domains
        .iter()
        .find(|domain| {
            module
                .identities
                .text(domain.identity)
                .is_some_and(|text| text.contains("Owned"))
        })
        .unwrap()
        .content_projection
        .as_ref()
        .unwrap();
    let (contract, equation) = module
        .execution
        .declaration_contracts
        .iter()
        .find_map(|contract| {
            contract
                .ensures
                .iter()
                .find_map(|clause| match &clause.proposition {
                    Proposition::ContentConservation(equation) => Some((contract, equation)),
                    _ => None,
                })
        })
        .expect("the public bodyless contract retains its actual equation");
    assert_eq!(equation.algebra(), &projection.algebra);
    let TerminalMachineResult::Structural(returned) = &contract.result else {
        panic!("structural result")
    };
    let [input] = contract.structural_parameters.as_slice() else {
        panic!("one structural input")
    };
    let mut observed = [equation.left(), equation.right()].map(|term| {
        let ContentTerm::Projection {
            projection: owner,
            subject,
        } = term
        else {
            panic!("one projection per side")
        };
        assert_eq!(*owner, projection.identity);
        assert!(subject.segments.is_empty());
        (subject.version, subject.root)
    });
    observed.sort();
    assert_eq!(
        observed,
        [
            (ContentPlaceVersion::Entry, input.place),
            (ContentPlaceVersion::Current, returned.place)
        ]
    );
}

#[test]
fn a_content_guarantee_cannot_borrow_an_unqualified_input_or_an_entry_result() {
    let declaration = with_transfer_contract();
    let unqualified = declaration.replace("pass(input: Region in Owned)", "pass(input: Region)");
    let (output, report) = program::invoke(&unqualified, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("exact qualification on its observed place"),
        "{report}"
    );
    let invalid_old =
        declaration.replace("Owned::measure(@result)", "Owned::measure(old(@result))");
    let (output, report) = program::invoke(&invalid_old, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("has no entry content"), "{report}");
}

#[test]
fn a_boundary_call_composes_the_declared_interval_law_at_its_normal_result() {
    use semantic_vocabulary::{ContentPlaceVersion, ContentTerm};
    use terminal_psi::{BoundaryContentGuarantee, OperationKind};
    use terminal_semantics::content_transformations::boundary_calls;

    let source = r#"use {omega}/language/core/extent.omg as extent;
        machine forward(input: |extent|::Extent in |extent|::Extent::Granted) -> |extent|::Extent
            crashes Trap crashes Abort { |extent|::VacantContentTransfer::transfer(input) }
        machine main() {}"#;
    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "boundary-content-call",
        false,
    )
    .unwrap();
    let artifact = scratch.path().join("forward.psi");
    let (out, report) = program::invoke(
        source,
        &[
            "psi",
            "inspect",
            "--machine",
            "forward",
            "--output",
            artifact.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "{report}");
    let saved =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&std::fs::read(artifact).unwrap())
            .unwrap();
    let module = saved.semantic_module();
    let ordinal = module
        .machines
        .iter()
        .position(|machine| !machine.content_partition_compositions.is_empty())
        .unwrap();
    let machine = &module.machines[ordinal];
    let [row] = machine.content_partition_compositions.as_slice() else {
        panic!("one authored normal equation")
    };
    let operation = machine.operation(row.producer_operation).unwrap();
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    } = &operation.kind
    else {
        panic!("ordinary boundary call")
    };
    let declaration = module.boundary_machine(*boundary).unwrap();
    let [BoundaryContentGuarantee::Conservation(law)] = declaration.content_guarantees.as_slice()
    else {
        panic!("declared interval law")
    };
    let [receipt] = completion_receipts.as_slice() else {
        panic!("one consumed input")
    };
    let [input] = structural_arguments.as_slice() else {
        panic!("one owned argument")
    };
    let result = operation.result.as_structural().unwrap();
    assert_eq!(row.source_report_fingerprint, law.report_fingerprint);
    assert_eq!(row.source, law.conservation);
    assert_eq!(row.input_claims, [receipt.claim]);
    assert_eq!(result.claims.len(), 1);
    let [
        ContentTerm::Projection {
            projection: left,
            subject: a,
        },
        ContentTerm::Projection {
            projection: right,
            subject: b,
        },
    ] = [row.derived.left(), row.derived.right()]
    else {
        panic!("two projections")
    };
    assert_eq!(left, right);
    assert_eq!(
        (a.version, b.version),
        (ContentPlaceVersion::Current, ContentPlaceVersion::Current)
    );
    let mut roots = [input.place, result.place];
    roots.sort_unstable();
    assert_eq!([a.root, b.root], roots);
    assert!(boundary_calls::verify_machine(module, machine).is_ok());

    let mut forged = module.clone();
    forged.machines[ordinal].content_partition_compositions[0].input_claims =
        vec![result.claims[0].claim];
    assert!(boundary_calls::verify_machine(&forged, &forged.machines[ordinal]).is_err());
    forged.machines[ordinal].content_partition_compositions[0] = row.clone();
    forged.machines[ordinal].content_partition_compositions[0].substitutions[0]
        .target
        .root = result.place;
    assert!(boundary_calls::verify_machine(&forged, &forged.machines[ordinal]).is_err());
    forged.machines[ordinal]
        .content_partition_compositions
        .clear();
    assert!(boundary_calls::verify_machine(&forged, &forged.machines[ordinal]).is_err());
}

/// A machine that forwards an `Extent` through a boundary call restates the
/// boundary's interval law as its own: `extent.omg` declares
/// `Identity(Granted::content(old(@storage)), Granted::content(@result))` on
/// `VacantContentTransfer::transfer`, and the forwarder promises the same
/// equation over its own input and result. The twin calls a boundary that
/// keeps `Granted` but declares no content law; the forwarder's promise then
/// has nothing to stand on and is refused.
#[test]
fn a_forwarder_restates_the_interval_law_its_boundary_call_declares() {
    const FORWARDER: &str = r#"use {omega}/language/core/extent.omg as extent;
use {omega}/language/core/foundation.omg as proof;
use {omega}/language/core/content.omg;
use {omega}/language/core/nat.omg;
public boundary trait Move {
    machine pass(storage: |extent|::Extent in |extent|::Extent::Granted) -> |extent|::Extent
    reaches none
    ensures result in |extent|::Extent::Granted,
        |proof|::Identity::<|content|::IntervalSet<|nat|::Nat>>(|extent|::Granted::content(old(@storage)), |extent|::Granted::content(@result))
    crashes Trap crashes Abort;
}
machine forward(input: |extent|::Extent in |extent|::Extent::Granted) -> |extent|::Extent
ensures
    result in |extent|::Extent::Granted,
    |proof|::Identity::<|content|::IntervalSet<|nat|::Nat>>(|extent|::Granted::content(old(@input)), |extent|::Granted::content(@result))
crashes Trap crashes Abort
{ CALL(input) }
machine main() {}"#;
    let core = FORWARDER.replace("CALL", "|extent|::VacantContentTransfer::transfer");
    let (output, report) = program::invoke(&core, &["--check"]);
    assert!(output.status.success(), "{report}");
    let declared = FORWARDER.replace("CALL", "Move::pass");
    let (output, report) = program::invoke(&declared, &["--check"]);
    assert!(output.status.success(), "{report}");

    let lawless = declared.replace(
        ",\n        |proof|::Identity::<|content|::IntervalSet<|nat|::Nat>>(|extent|::Granted::content(old(@storage)), |extent|::Granted::content(@result))",
        "",
    );
    assert_ne!(lawless, declared);
    let (output, report) = program::invoke(&lawless, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("declares no unchanged projection for its result"),
        "{report}"
    );
}
