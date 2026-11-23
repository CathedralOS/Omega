#[path = "support/predicate_read.rs"]
mod expected;
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{LoweredPackage, LoweringError, PredicateOperator, PredicateStep, TypeShape};

fn lower(source: &str) -> Result<LoweredPackage, LoweringError> {
    sources::lower(&[("structural.omg", source)])
}

/// Receipt for the L5 readiness report, printed after the checks above it pass.
/// `tools/spec-corpus/lowering_evidence.py` registers each label with its test
/// and credits it only when that test passes.
fn observed(label: &str) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_L5_ASSERT\t{label}");
    }
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
    observed("contract-path/fixed-index-siblings");
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
    observed("contract-path/matrix-segments");
}

#[test]
fn runtime_selector_reads_the_formal_array_under_the_selected_contract() {
    use PredicateStep::{Apply, Formal, Integer};
    let source = "machine f(a:[u8;4], i:u64, b:u8)->u8 requires a[i] < 5 {b}";
    let package = lower(source).unwrap();
    let [f] = package.machines.as_slice() else { panic!("expected f") };
    assert_eq!(f.declaration.path, "f");
    assert!(matches!(package.types.get(f.parameter_types[0]), Some(omega_psi::TypeShape::FixedArray { length: 4, .. })));
    let [clause] = f.contract.requires.as_slice() else { panic!("no synthetic bounds premise") };
    spec_corpus::observe_handoff!(
        psi "psi-output/predicate-array-read",
        assert_eq!(clause.predicate, [Formal(0), Formal(1), expected::read("structural.omg", source, "a[i]"),
            Integer(5.into()), Apply(PredicateOperator::LessThan)]),
    );

    let literal = source.replace("a[i]", "a[1]");
    let frozen = format!("const SECOND:u64=1; {}", source.replace("a[i]", "a[SECOND]"));
    for fixed in [literal, frozen] {
        let package = lower(&fixed).unwrap();
        assert_eq!(package.machines[0].contract.requires[0].predicate, [Formal(0), PredicateStep::FixedIndex(1),
            Integer(5.into()), Apply(PredicateOperator::LessThan)]);
    }
}

#[test]
fn matrix_runtime_selectors_bind_each_read_to_its_own_array_and_dimension() {
    use PredicateStep::{Apply, Formal, Integer};
    for (access, first, row, col) in [("a[i][j]", "a[i]", 1, 2), ("a[j][i]", "a[j]", 2, 1)] {
        let source = format!("machine f(a:[[u8;3];2], i:u64, j:u64) requires {access} < 5 {{}}");
        let package = lower(&source).unwrap();
        let machine = &package.machines[0];
        let Some(omega_psi::TypeShape::FixedArray { length: 2, element }) = package.types.get(machine.parameter_types[0]) else {
            panic!("expected outer extent two")
        };
        assert!(matches!(package.types.get(*element), Some(omega_psi::TypeShape::FixedArray { length: 3, .. })));
        assert_eq!(machine.contract.requires[0].predicate, [Formal(0), Formal(row),
            expected::read("structural.omg", &source, first), Formal(col), expected::read("structural.omg", &source, access),
            Integer(5.into()), Apply(PredicateOperator::LessThan)]);
    }
}

/// Reading `a[4]` of a four-element formal makes the requirement invalid, but
/// the lowered-form checker owns that verdict through the read's bounds
/// precondition. Lowering keeps index 4 exactly, beside the formal's declared
/// extent, instead of refusing, clamping or dropping it. The in-bounds twin
/// `a[3]` differs only in that index, so a consumer can judge the bound from
/// the Psi product alone, at the clause the checker would refuse.
#[test]
fn out_of_bounds_selector_reaches_the_checker_without_redirecting() {
    use PredicateStep::{Apply, FixedIndex, Formal, Integer};
    let lowered = |index: u64| {
        let source = format!("machine f(a:[u8;4], b:u8)->u8 requires a[{index}] < 5 {{b}}");
        let package = lower(&source).unwrap();
        let machine = &package.machines[0];
        let extent = match package.types.get(machine.parameter_types[0]) {
            Some(TypeShape::FixedArray { length, .. }) => *length,
            other => panic!("expected the fixed-array formal, found {other:?}"),
        };
        let [clause] = machine.contract.requires.as_slice() else {
            panic!("expected one requirement")
        };
        let site = clause
            .site
            .span
            .slice(source.as_bytes())
            .map(String::from_utf8_lossy)
            .map(String::from);
        (extent, clause.predicate.clone(), site)
    };
    let expected = |index: u64| {
        let path = vec![
            Formal(0),
            FixedIndex(index),
            Integer(5.into()),
            Apply(PredicateOperator::LessThan),
        ];
        (4, path, Some(format!("requires a[{index}] < 5")))
    };
    assert_eq!(lowered(3), expected(3));
    spec_corpus::observe_handoff!(psi "psi-output/out-of-bounds-contract-index", assert_eq!(lowered(4), expected(4)));
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
    observed("default-domain/indexed-record-field");
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
    observed("default-domain/equal-named-case-payloads");
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
    observed("default-domain/same-typed-payload-siblings");
}
