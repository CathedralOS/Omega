#[path = "support/source.rs"]
mod sources;

use omega_lowering::{LoweredPackage, LoweringError, PredicateOperator, PredicateStep};

fn lower(source: &str) -> Result<LoweredPackage, LoweringError> {
    sources::lower(&[("structural.omg", source)])
}

#[test]
fn sibling_fixed_indices_of_one_formal_keep_distinct_complete_paths() {
    use PredicateStep::{Apply, FixedIndex, Formal};
    let package = lower("machine f(a:[u8;4], b:u8)->u8 requires a[0] < a[3] {b}").unwrap();
    assert_eq!(
        package.machines[0].contract.requires[0].predicate,
        [
            Formal(0),
            FixedIndex(0),
            Formal(0),
            FixedIndex(3),
            Apply(PredicateOperator::LessThan)
        ]
    );
}

#[test]
fn matrix_indices_retain_every_ordered_segment() {
    use PredicateStep::{Apply, FixedIndex, Formal};
    let package = lower("machine g(m:[[u8;3];2], b:u8)->u8 requires m[1][2] == m[0][2] {b}").unwrap();
    assert_eq!(
        package.machines[0].contract.requires[0].predicate,
        [
            Formal(0),
            FixedIndex(1),
            FixedIndex(2),
            Formal(0),
            FixedIndex(0),
            FixedIndex(2),
            Apply(PredicateOperator::Equal)
        ]
    );
}

#[test]
fn runtime_selector_forms_no_fixed_index_path() {
    let error = lower("machine f(a:[u8;4], i:u64, b:u8)->u8 requires a[i] < 5 {b}").unwrap_err();
    assert_eq!(error.message, "contract index requires a fixed selector");
}

#[test]
fn out_of_bounds_selector_rejects_instead_of_redirecting() {
    let error = lower("machine f(a:[u8;4], b:u8)->u8 requires a[4] < 5 {b}").unwrap_err();
    assert_eq!(error.message, "contract index is outside its fixed array");
}

#[test]
fn default_domain_indexes_a_record_field_after_selecting_it() {
    use PredicateStep::{Apply, Field, FixedIndex, SelfValue};
    let package = lower("data Window requires self.cells[0] <= self.cells[2] { cells:[u8;3]; }").unwrap();
    let window = &package.data[0];
    let cells = window.fields[0].declaration.clone();
    assert_eq!(
        window.requires[0].predicate,
        [
            SelfValue,
            Field(cells.clone()),
            FixedIndex(0),
            SelfValue,
            Field(cells),
            FixedIndex(2),
            Apply(PredicateOperator::LessOrEqual)
        ]
    );
}

#[test]
fn equal_named_cases_in_distinct_sums_select_the_exact_owned_case_and_field() {
    use PredicateStep::{Apply, CasePayload, IsCase, SelfValue};
    let package = lower(
        "data Alpha { case Same(w:u8, v:u8); case Other; } \
         data Beta requires (self in Beta::Same) && self.v <= self.w { case Empty; case Same(v:u8, w:u8); }",
    )
    .unwrap();
    let mut constrained = package.data.iter().filter(|data| !data.requires.is_empty());
    let (Some(beta), None) = (constrained.next(), constrained.next()) else {
        panic!("expected only Beta to carry an invariant")
    };
    let [_, same] = beta.cases.as_slice() else {
        panic!("expected Beta's declared cases")
    };
    let [v, w] = same.fields.as_slice() else {
        panic!("expected Beta::Same payload fields")
    };
    // Alpha's equal-spelled case and fields are distinct identities and are
    // never selected for Beta's payload.
    let foreign: Vec<_> = package
        .data
        .iter()
        .filter(|data| data.id != beta.id)
        .flat_map(|data| &data.cases)
        .collect();
    assert!(!foreign.is_empty(), "Alpha must be lowered");
    for case in &foreign {
        assert_ne!(case.declaration, same.declaration);
        assert!(
            case.fields
                .iter()
                .all(|field| field.declaration != v.declaration && field.declaration != w.declaration)
        );
    }
    let payload = |field| CasePayload {
        case: same.declaration.clone(),
        field,
    };
    assert_eq!(
        beta.requires[0].predicate,
        [
            SelfValue,
            IsCase(same.declaration.clone()),
            SelfValue,
            payload(v.declaration.clone()),
            SelfValue,
            payload(w.declaration.clone()),
            Apply(PredicateOperator::LessOrEqual),
            Apply(PredicateOperator::And)
        ]
    );
}

#[test]
fn same_typed_payload_siblings_retain_their_case_and_field_without_implied_membership() {
    use PredicateStep::{Apply, CasePayload, IsCase, SelfValue};
    let package = lower("data Pair requires (self in Pair::Both) && self.left == self.right { case Neither; case Both(left:u8, right:u8); }").unwrap();
    let pair = &package.data[0];
    let both = &pair.cases[1];
    let [left, right] = both.fields.as_slice() else {
        panic!("expected both payload fields")
    };
    let payload = |field| CasePayload {
        case: both.declaration.clone(),
        field,
    };
    assert_ne!(left.declaration, right.declaration);
    assert_eq!(
        pair.requires[0].predicate,
        [
            SelfValue,
            IsCase(both.declaration.clone()),
            SelfValue,
            payload(left.declaration.clone()),
            SelfValue,
            payload(right.declaration.clone()),
            Apply(PredicateOperator::Equal),
            Apply(PredicateOperator::And)
        ]
    );
}
