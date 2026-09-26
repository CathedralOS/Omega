use crate::tests::front_end::checked_program_result;

fn check(source: &str, rejection: Option<&str>) {
    let _ = checked_fixture(source, rejection);
}

fn checked_fixture(source: &str, rejection: Option<&str>) -> Option<checked_trees::CheckedTrees> {
    match checked_program_result(source) {
        Ok(checked) => {
            assert!(rejection.is_none(), "expected {rejection:?}\n{source}");
            Some(checked)
        }
        Err(diagnostics) => {
            let Some(rejection) = rejection else {
                panic!("check: {diagnostics:#?}\nsource:\n{source}");
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection}: {diagnostics:#?}\nsource:\n{source}"
            );
            if rejection == "creates local borrow" {
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("is still active")),
                    "expected active-loan conflict: {diagnostics:#?}\n{source}"
                );
                assert!(
                    diagnostics
                        .iter()
                        .all(|diagnostic| !diagnostic.message.contains("cannot prove")),
                    "bounds must validate before borrow rejection: {diagnostics:#?}\n{source}"
                );
            }
            None
        }
    }
}

fn window_source(scalar: &str, requirement: &str, body: &str, access: &str) -> String {
    let requirement = if requirement.is_empty() {
        String::new()
    } else {
        format!("requires {requirement};")
    };
    format!(
        "machine window(items: &[i32; 4], mut original: {scalar}) -> u64
         {requirement} {{ {body} let view: &[i32] = items[{access}]; view.len }}"
    )
}

#[test]
fn inclusive_normalization_overflow_never_wraps_into_a_valid_window() {
    for (access, rejection) in [
        ("0..=9223372036854775807", "inclusive end overflow"),
        ("0..9223372036854775807", "end bound"),
        ("0..=4", "end bound"),
    ] {
        check(
            &window_source("u64", "original <= 4", "", access),
            Some(rejection),
        );
    }
    check(&window_source("u64", "original <= 4", "", "0..4"), None);
    check(&window_source("u64", "original <= 4", "", "0..=3"), None);
}

#[test]
fn arithmetic_snapshot_cannot_copy_a_pre_assignment_lower_bound() {
    for before_mutation in [false, true] {
        let capture = "let cut: i64 = original - 1;";
        let mutation = "original = replacement;";
        let body = if before_mutation {
            format!("{capture} {mutation}")
        } else {
            format!("{mutation} {capture}")
        };
        let source = format!(
            "machine window(items: &[i32; 4], mut original: i64 [0..=5], replacement: i64 [0..=4]) -> u64
             requires 0 <= original - 1 && original - 1 <= 4
                 && 0 <= replacement && replacement <= 4;
             {{ {body} let view: &[i32] = items[0..cut]; view.len }}"
        );
        check(&source, (!before_mutation).then_some("cannot prove"));
    }
}
