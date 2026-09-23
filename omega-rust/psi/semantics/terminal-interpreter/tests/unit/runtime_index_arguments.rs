//! Runtime-index structural argument bounds replay the caller's published
//! contract facts, not only the scalar qualification catalog's retained
//! integer entry-range rows.
//!
//! A `RuntimeIndex` segment carries no authority of its own: the selector
//! names one direct scalar parameter of the calling machine and the spelled
//! `minimum`/`maximum` must equal what the caller's published evidence
//! proves. Machines whose contract facts publish only as `requires`
//! propositions — the Unit-effect lane emits no scalar qualification rows —
//! fold the same conjuncts the checked admission read into the same closed
//! interval. These tests pin both channels and the exact-equality rule the
//! verifier applies to the segment's spelled bounds.

use super::call_modules::structural_scalar_field_call_module;
use super::{machine_id, operation_id, structural_type_id, value_id, verify_module};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};
use terminal_psi::{
    OperationKind, ScalarIntegerRange, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalModule, ValueDeclaration,
};
use terminal_verifier::{ModuleError, ProofBundle, VerificationError};

/// A two-element fixed array hangs off the owner record's `item` field; the
/// shared-borrow call argument selects one element at runtime through the
/// caller's own scalar parameter. `requires` is the machine's published
/// contract proposition roster — authored clauses merge into it — and
/// `minimum`/`maximum` are the segment's spelled bounds under test.
fn runtime_index_argument_module(
    integer: IntegerType,
    requires: Vec<Proposition>,
    minimum: IntegerValue,
    maximum: IntegerValue,
) -> TerminalModule {
    let mut module = structural_scalar_field_call_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::Structural(structural_type_id(97));
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(97),
        identity: "test::Cells".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(96),
            length: 2,
        },
    });
    let caller = &mut module.machines[0];
    // The scalar store wrote into `item`; with `item` a cell array the call
    // is the caller's only remaining operation.
    caller.blocks[0].operations.drain(..2);
    caller.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(40),
        scalar_type: ScalarType::Integer(integer),
    }];
    caller.contract.requires = requires;
    let operation = caller.blocks[0]
        .operations
        .iter_mut()
        .find(|operation| operation.id == operation_id(3))
        .expect("the structural scalar call stays");
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    structural_arguments[0]
        .path
        .push(StructuralPathSegment::RuntimeIndex {
            selector: 0,
            minimum,
            maximum,
        });
    module
}

fn selector(integer: IntegerType) -> ScalarTerm {
    ScalarTerm::value(value_id(40), ScalarType::Integer(integer))
}

fn endpoint(integer: IntegerType, value: IntegerValue) -> ScalarTerm {
    ScalarTerm::integer(integer, value).expect("the endpoint lands in its carrier")
}

fn lte(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    Proposition::LessOrEqual(left, right)
}

fn invalid_path(module: &TerminalModule) -> ModuleError {
    match verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    ) {
        Err(VerificationError::Module(error)) => error,
        other => panic!("a runtime-index segment without proven bounds rejects: {other:?}"),
    }
}

#[test]
fn requires_conjuncts_bound_a_runtime_indexed_shared_argument() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let module = runtime_index_argument_module(
        integer,
        vec![lte(
            selector(integer),
            endpoint(integer, IntegerValue::Unsigned(1)),
        )],
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(1),
    );
    assert!(
        module.scalar_qualifications.integer_entry_ranges.is_empty(),
        "the contract-fact channel is the only bound evidence this caller publishes"
    );
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a requires-bound selector verifies");
}

#[test]
fn merged_conjunctions_fold_to_the_same_closed_interval() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    // Two authored clauses publish as one merged proposition; the fold walks
    // the conjunction exactly like the scalar-qualification replay does.
    let module = runtime_index_argument_module(
        integer,
        vec![Proposition::Conjunction(vec![
            lte(
                endpoint(integer, IntegerValue::Unsigned(0)),
                selector(integer),
            ),
            lte(
                selector(integer),
                endpoint(integer, IntegerValue::Unsigned(1)),
            ),
        ])],
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(1),
    );
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("conjoined requires propositions bound the selector");
}

#[test]
fn selectors_without_contract_evidence_reject() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let module = runtime_index_argument_module(
        integer,
        Vec::new(),
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(1),
    );
    assert!(matches!(
        invalid_path(&module),
        ModuleError::InvalidStructuralArgumentPath { .. }
    ));
}

#[test]
fn bounds_on_another_parameter_never_transfer() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    // The contract bounds a second caller parameter, not the selector.
    let mut module = runtime_index_argument_module(
        integer,
        vec![lte(
            ScalarTerm::value(value_id(41), ScalarType::Integer(integer)),
            endpoint(integer, IntegerValue::Unsigned(1)),
        )],
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(1),
    );
    module.machines[0].parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(41),
        scalar_type: ScalarType::Integer(integer),
    });
    assert!(matches!(
        invalid_path(&module),
        ModuleError::InvalidStructuralArgumentPath { .. }
    ));
}

#[test]
fn spelled_bounds_must_equal_the_proven_interval() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    // `i <= 0` proves [0, 0]; a segment spelling [0, 1] overstates it.
    let overstated = runtime_index_argument_module(
        integer,
        vec![lte(
            selector(integer),
            endpoint(integer, IntegerValue::Unsigned(0)),
        )],
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(1),
    );
    assert!(matches!(
        invalid_path(&overstated),
        ModuleError::InvalidStructuralArgumentPath { .. }
    ));
    // `i <= 3` proves [0, 3]; a segment spelling [0, 1] understates it. The
    // equality rule is the same one the retained roster applies.
    let understated = runtime_index_argument_module(
        integer,
        vec![lte(
            selector(integer),
            endpoint(integer, IntegerValue::Unsigned(3)),
        )],
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(1),
    );
    assert!(matches!(
        invalid_path(&understated),
        ModuleError::InvalidStructuralArgumentPath { .. }
    ));
}

#[test]
fn signed_selectors_owe_an_explicit_lower_bound() {
    let integer = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    // A signed carrier supplies no implicit `0 <=` half: `i <= 1` alone
    // proves no nonnegative interval.
    let missing_lower = runtime_index_argument_module(
        integer,
        vec![lte(
            selector(integer),
            endpoint(integer, IntegerValue::Signed(1)),
        )],
        IntegerValue::Signed(0),
        IntegerValue::Signed(1),
    );
    assert!(matches!(
        invalid_path(&missing_lower),
        ModuleError::InvalidStructuralArgumentPath { .. }
    ));
    // With the explicit lower conjunct the same spelled bounds verify.
    let bounded = runtime_index_argument_module(
        integer,
        vec![Proposition::Conjunction(vec![
            lte(
                endpoint(integer, IntegerValue::Signed(0)),
                selector(integer),
            ),
            lte(
                selector(integer),
                endpoint(integer, IntegerValue::Signed(1)),
            ),
        ])],
        IntegerValue::Signed(0),
        IntegerValue::Signed(1),
    );
    verify_module(
        &bounded,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a signed selector with both conjuncts verifies");
}

#[test]
fn retained_range_rows_still_satisfy_the_segment() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    // The roster channel keeps its own evidence rule: `0 <= i` and `i <= 3`
    // publish as requires conjuncts backing the retained row, and an extra
    // authored `i <= 1` narrows the folded interval below what the segment
    // spells. The roster row remains valid evidence on its own.
    let mut module = runtime_index_argument_module(
        integer,
        vec![Proposition::Conjunction(vec![
            lte(
                endpoint(integer, IntegerValue::Unsigned(0)),
                selector(integer),
            ),
            lte(
                selector(integer),
                endpoint(integer, IntegerValue::Unsigned(3)),
            ),
            lte(
                selector(integer),
                endpoint(integer, IntegerValue::Unsigned(1)),
            ),
        ])],
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(3),
    );
    module.scalar_qualifications.integer_entry_ranges = vec![ScalarIntegerRange {
        machine: machine_id(1),
        parameter: value_id(40),
        integer_type: integer,
        minimum: IntegerValue::Unsigned(0),
        maximum: IntegerValue::Unsigned(3),
    }];
    // The maximum the segment spells must still fit the declared extent.
    let StructuralTypeShape::FixedArray { length, .. } = &mut module.structural_types[2].shape
    else {
        unreachable!()
    };
    *length = 4;
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a retained range row remains sufficient evidence");
}

#[test]
fn a_runtime_index_grants_no_access_authority() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let mut module = runtime_index_argument_module(
        integer,
        vec![lte(
            selector(integer),
            endpoint(integer, IntegerValue::Unsigned(1)),
        )],
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(1),
    );
    // The segment proves bounds only; upgrading the loan still requires the
    // caller's own mutable authority, which a shared borrow cannot supply.
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].access = terminal_psi::StructuralAccess::MutableBorrow;
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "a shared source cannot supply a mutable loan through a runtime index"
    );
}
