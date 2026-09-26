//! Routed owned result fields require conserved authority, not annotations.

use crate::tests::front_end::{checked_program, checked_program_result};

fn partition_program(parameter: &str, conservation: &str) -> String {
    format!(
        r#"
        data ByteUnit {{}}
        data CountedQuantity<Unit> {{ magnitude: u64; }}
        trait Content<A> {{ machine project(subject: &Self) -> A; }}
        data Region [linear] {{ length: u64; }}
        domain Region::Granted established by RootProvider::grant;
        machine Granted::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {{ CountedQuantity {{ magnitude: region.length }} }}

        boundary trait RootProvider {{
            machine grant(root: Region) -> Region
            ensures result in Region::Granted;
        }}
        data Parts {{
            taken: Region in Granted;
            rest: Region in Granted;
        }}
        boundary trait Partition {{
            machine split(whole: {parameter}) -> Parts
            {conservation};
        }}
        "#,
    )
}

fn rejection_diagnostics(source: &str) -> Vec<diagnostics::Diagnostic> {
    let result = checked_program_result(source);
    assert!(result.is_err(), "unproved structural authority must reject");
    result.err().unwrap_or_default()
}

fn assert_structural_rejection(source: &str) {
    let diagnostics = rejection_diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural content result")),
        "expected structural content result diagnostic: {diagnostics:#?}"
    );
}

const SPLIT_LAW: &str = r#"
    ensures
        Granted::content(old(&whole))
        == separate(Granted::content(&result.taken), Granted::content(&result.rest))
"#;

#[test]
fn routed_structural_result_accounts_for_each_array_element_once() {
    let source = partition_program("Region in Granted", SPLIT_LAW)
        .replace(
            "taken: Region in Granted;",
            "taken: [Region in Granted; 2];",
        )
        .replace("rest: Region in Granted;", "")
        .replace("&result.taken)", "&result.taken[0])")
        .replace("&result.rest)", "&result.taken[1])");
    checked_program(&source);
    assert_structural_rejection(&source.replace("&result.taken[1]", "&result.taken[0]"));
}

#[test]
fn routed_structural_result_names_unimplemented_outcome_partition_checking() {
    let source = partition_program("Region in Granted", "")
        .replace(
            "taken: Region in Granted;",
            "case Taken(first: Region in Granted, second: Region in Granted);",
        )
        .replace(
            "rest: Region in Granted;",
            "case Rest(value: Region in Granted);",
        );
    let diagnostics = rejection_diagnostics(&source);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("outcome-specific partition conservation")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn routed_structural_result_rejects_mutually_exclusive_input_claims() {
    let source = partition_program("Input", SPLIT_LAW)
        .replace("data Parts {", "data Input { case Left(left: Region in Granted); case Right(right: Region in Granted); } data Parts {")
        .replace("Granted::content(old(&whole))", "separate(Granted::content(old(&whole.left)), Granted::content(old(&whole.right)))");
    assert_structural_rejection(&source);
}

#[test]
fn routed_local_initializer_cannot_mint_custody() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        r#"
        data Wrapped { region: Region in Granted; }
        machine forge(raw: Region) -> Wrapped {
            let claimed: Region in Granted = raw;
            Wrapped { region: claimed }
        }
    "#
    );
    let diagnostics = rejection_diagnostics(&source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("initializer")),
        "{diagnostics:#?}"
    );
}

#[test]
fn routed_partition_merge_preserves_call_ensures() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        r#"
        boundary trait Merge {
            machine merge(parts: Parts) -> Region
            ensures result in Granted,
                separate(Granted::content(old(&parts.taken)), Granted::content(old(&parts.rest)))
                == Granted::content(&result);
        }
        data Wrapped { region: Region in Granted; }
        machine merge_wrapper(merger: &Merge, parts: Parts) -> Wrapped
        reaches Merge {
            let whole: Region in Granted = merger.merge(parts);
            Wrapped { region: whole }
        }
    "#
    );
    checked_program(&source);
}

#[test]
fn routed_mutable_root_must_be_restored_before_return() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        "machine corrupt(region: &mut Region in Granted) { region.length = 0; }"
    );
    checked_program(&source.replace("region.length = 0;", ""));
    let diagnostics = rejection_diagnostics(&source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requirement for return")),
        "{diagnostics:#?}"
    );
}
