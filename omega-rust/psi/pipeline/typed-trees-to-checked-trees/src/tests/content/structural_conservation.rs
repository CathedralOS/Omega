//! Routed owned result fields require conserved authority, not annotations.

use super::checked;
use crate::tests::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
};

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
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize structural conservation fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse structural conservation fixture");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve structural conservation fixture");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type structural conservation fixture");
    let result = crate::lower_typed_trees(typed);
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
fn routed_structural_result_rejects_annotation_without_conservation() {
    assert_structural_rejection(&partition_program("Region in Granted", ""));
}

#[test]
fn routed_structural_result_rejects_unaccounted_output_field() {
    assert_structural_rejection(&partition_program(
        "Region in Granted",
        "ensures Granted::content(old(&whole)) == Granted::content(&result.taken)",
    ));
}

#[test]
fn routed_structural_result_accepts_exact_split_conservation() {
    let checked = checked(&partition_program("Region in Granted", SPLIT_LAW));
    assert_eq!(
        checked
            .facts
            .qualifications
            .content
            .conservation_plans
            .len(),
        1
    );
}

#[test]
fn routed_structural_result_rejects_borrowed_partition_source() {
    let diagnostics = rejection_diagnostics(&partition_program("&Region in Granted", SPLIT_LAW));
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("structural content result")
                || (diagnostic.message.contains("borrowed parameter")
                    && diagnostic.message.contains("consumed owned input"))
        }),
        "a conservation annotation cannot turn a loan into owned custody: {diagnostics:#?}"
    );
}

#[test]
fn routed_structural_result_rejects_unqualified_owned_partition_source() {
    let diagnostics = rejection_diagnostics(&partition_program("Region", SPLIT_LAW));
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("projection")
                && diagnostic.message.contains("requires exact qualification")
        }),
        "the projection itself requires qualified input: {diagnostics:#?}"
    );
}

#[test]
fn routed_structural_result_rejects_duplicate_output_in_conservation() {
    assert_structural_rejection(&partition_program(
        "Region in Granted",
        "ensures Granted::content(old(&whole)) == separate(Granted::content(&result.taken), Granted::content(&result.taken))",
    ));
}

#[test]
fn routed_structural_result_rejects_another_domains_equal_projection() {
    let source = partition_program("Region in Granted", SPLIT_LAW)
        .replace(
            "data Parts {",
            r#"
            domain Region::OtherGranted established by OtherProvider::grant;
            machine OtherGranted::content(region: &Region) -> CountedQuantity<ByteUnit>
            satisfies Content<CountedQuantity<ByteUnit>>::project
            { CountedQuantity { magnitude: region.length } }
            boundary trait OtherProvider {
                machine grant(root: Region) -> Region
                ensures result in Region::OtherGranted;
            }
            data Parts {
            "#,
        )
        .replace("rest: Region in Granted", "rest: Region in OtherGranted")
        .replace(
            "Granted::content(&result.rest)",
            "OtherGranted::content(&result.rest)",
        );
    assert_structural_rejection(&source);
}

#[test]
fn routed_structural_result_accepts_requires_qualified_partition_source() {
    checked(&partition_program(
        "Region",
        &format!("requires whole in Granted {SPLIT_LAW}"),
    ));
}

#[test]
fn routed_structural_result_accepts_nested_record_partition_paths() {
    let source = partition_program("Region in Granted", SPLIT_LAW)
        .replace(
            "data Parts {",
            "data Wrapper { value: Parts; } data Parts {",
        )
        .replace("-> Parts", "-> Wrapper")
        .replace("&result.taken", "&result.value.taken")
        .replace("&result.rest", "&result.value.rest");
    checked(&source);
}

#[test]
fn routed_structural_result_rejects_unaccounted_array_elements() {
    let source = partition_program("Region in Granted", "")
        .replace(
            "taken: Region in Granted;",
            "taken: [Region in Granted; 3];",
        )
        .replace("rest: Region in Granted;", "");
    assert_structural_rejection(&source);
}

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
    checked(&source);
    assert_structural_rejection(&source.replace("&result.taken[1]", "&result.taken[0]"));
}

#[test]
fn routed_structural_result_preserves_unique_identity_wrapper() {
    let source = partition_program("Region in Granted", "").replace("rest: Region in Granted;", "");
    checked(&source);
}

#[test]
fn routed_structural_result_preserves_alternative_single_claims() {
    let source = partition_program("Region in Granted", "")
        .replace(
            "taken: Region in Granted;",
            "case Taken(value: Region in Granted);",
        )
        .replace(
            "rest: Region in Granted;",
            "case Rest(value: Region in Granted);",
        );
    checked(&source);
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
fn routed_partition_fields_forward_without_restatement() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        r#"
        machine forward(partition: &Partition, whole: Region in Granted) -> Parts
        reaches Partition {
            let parts: Parts = partition.split(whole);
            Parts { taken: parts.taken, rest: parts.rest }
        }
    "#
    );
    checked(&source);
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
    checked(&source);
}

#[test]
fn routed_partition_fields_survive_separate_owned_moves() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        r#"
        machine forward(partition: &Partition, whole: Region in Granted) -> Parts
        reaches Partition {
            let parts: Parts = partition.split(whole);
            let taken: Region in Granted = parts.taken;
            let rest: Region in Granted = parts.rest;
            Parts { taken: taken, rest: rest }
        }
    "#
    );
    checked(&source);
}

#[test]
fn routed_owned_result_annotation_cannot_mint_custody() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        r#"
        machine forge(raw: Region) -> Region in Granted { raw }
    "#
    );
    let diagnostics = rejection_diagnostics(&source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requirement for return")),
        "{diagnostics:#?}"
    );
}

#[test]
fn routed_owned_field_requires_live_unmodified_membership() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        r#"
        machine forward(partition: &Partition, whole: Region in Granted) -> Parts
        reaches Partition {
            let mut parts: Parts = partition.split(whole);
            parts.taken.length = 0;
            Parts { taken: parts.taken, rest: parts.rest }
        }
    "#
    );
    let diagnostics = rejection_diagnostics(&source);
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("not proven in domain")
                || diagnostic.message.contains("requirement for return")
        ),
        "{diagnostics:#?}"
    );
}

#[test]
fn routed_local_preserves_requires_membership() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        r#"
        data Wrapped { region: Region in Granted; }
        machine forward(whole: Region) -> Wrapped
        requires whole in Granted {
            let claimed: Region in Granted = whole;
            Wrapped { region: claimed }
        }
    "#
    );
    checked(&source);
}

#[test]
fn routed_mutable_field_must_be_restored_before_return() {
    let source = r#"
        domain u32::Secret established by Provider::issue;
        boundary trait Provider { machine issue() -> u32 ensures result in Secret; }
        data Row { secret: u32 in Secret; }
        machine overwrite(row: &mut Row, raw: u32) { row.secret = raw; }
    "#;
    let diagnostics = rejection_diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requirement for return")),
        "{diagnostics:#?}"
    );
}

#[test]
fn routed_mutable_field_can_restore_existing_qualification() {
    let source = r#"
        domain u32::Secret established by Provider::issue;
        boundary trait Provider { machine issue() -> u32 ensures result in Secret; }
        data Row { secret: u32 in Secret; }
        machine overwrite(row: &mut Row, raw: u32, replacement: u32 in Secret) {
            row.secret = raw;
            row.secret = replacement;
        }
    "#;
    checked(source);
}

#[test]
fn routed_mutable_root_must_be_restored_before_return() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        "machine corrupt(region: &mut Region in Granted) { region.length = 0; }"
    );
    checked(&source.replace("region.length = 0;", ""));
    let diagnostics = rejection_diagnostics(&source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requirement for return")),
        "{diagnostics:#?}"
    );
}

#[test]
fn authorized_content_result_can_be_returned_without_a_local() {
    let source = format!(
        "{}\n{}",
        partition_program("Region in Granted", SPLIT_LAW),
        r#"
        machine receive(provider: &RootProvider, raw: Region) -> Region in Granted
        reaches RootProvider { provider.grant(raw) }
    "#
    );
    checked(&source);
}
