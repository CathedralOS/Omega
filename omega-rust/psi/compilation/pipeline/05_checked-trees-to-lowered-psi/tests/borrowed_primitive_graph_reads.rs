//! Scalar-graph bodies read borrowed primitive formals through the loan.
//!
//! A `&u8` formal of a scalar-returning machine is readable storage: each
//! referent read is a `PrimitiveScalarRead` of the formal's own place, not a
//! scalar slot the caller filled with a snapshot. A scalar formal before the
//! borrowed one keeps its own entry slot; the two namespaces stay apart.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};
use terminal_psi::{OperationKind, StructuralAccess};

const SOURCE: &str = r#"
machine is_seventy(leaf: &u8) -> bool {
    transition leaf == 70 { true -> (true) _ -> (false) }
}
machine above(limit: u8, leaf: &u8) -> bool {
    transition leaf > limit { true -> (true) _ -> (false) }
}
"#;

fn byte(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

/// Produce `root`, check that its only borrowed formal is read in place, and
/// run it once per `(scalar arguments, referent, expected)` case.
fn run_cases(root: &str, cases: &[(&[TerminalScalarValue], u128, bool)]) {
    let checked = crate::front_end::checked_program(SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name(root),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap_or_else(|error| panic!("{root} reads its borrowed formal: {error:?}"))
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let [formal] = entry.structural_parameters.as_slice() else {
        panic!("{root} keeps exactly one structural formal");
    };
    assert_eq!(formal.access, StructuralAccess::SharedBorrow);
    let reads = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::PrimitiveScalarRead { source, path } => Some((*source, path.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !reads.is_empty()
            && reads
                .iter()
                .all(|(source, path)| *source == formal.place && path.is_empty()),
        "{root} reads the formal's own place: {reads:?}"
    );
    for (arguments, referent, expected) in cases {
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            arguments,
            TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue {
                    opaque_identity: 41,
                    structural_type: formal.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                primitive_values: &[TerminalStructuralPrimitiveValue {
                    argument_index: 0,
                    value: byte(*referent),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            execution
                .resume(
                    &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                    &mut AcceptTerminalEffects
                )
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Boolean(*expected)
            )),
            "{root} over referent {referent}"
        );
    }
}

#[test]
fn scalar_graph_reads_its_borrowed_primitive_formal_in_place() {
    run_cases("is_seventy", &[(&[], 70, true), (&[], 69, false)]);
}

#[test]
fn scalar_formal_before_a_borrowed_formal_keeps_its_own_slot() {
    run_cases(
        "above",
        &[
            (&[byte(60)], 61, true),
            (&[byte(60)], 60, false),
            (&[byte(5)], 4, false),
        ],
    );
}
