//! Readable primitive inputs initialize distinct locals and immutable snapshots.

use semantic_vocabulary::{IeeeFloatValue, IntegerSign, IntegerValue};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, StructuralAccess};

pub(super) fn source(scalar: &str) -> String {
    format!(
        "machine replace(destination: &mut {scalar}, value: {scalar}) {{
            destination = value;
        }}
        machine observe(output: &mut {scalar}, input: &{scalar},
            initial_output: &mut {scalar}, replacement: {scalar}) {{
            let mut scratch: {scalar} = input;
            let before: {scalar} = scratch;
            replace(&mut scratch, replacement);
            output = scratch;
            initial_output = before;
        }}"
    )
}

#[test]
fn every_primitive_reference_initializes_a_local_with_a_distinct_snapshot() {
    for width in [8, 16, 32, 64] {
        for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
            let (name, initial, replacement) = match sign {
                IntegerSign::Unsigned => (
                    format!("u{width}"),
                    IntegerValue::Unsigned(1),
                    IntegerValue::Unsigned((1u128 << width) - 1),
                ),
                IntegerSign::Signed => (
                    format!("i{width}"),
                    IntegerValue::Signed(1),
                    IntegerValue::Signed(-(1i128 << (width - 1))),
                ),
            };
            execute(
                &name,
                super::integer(sign, width, initial),
                super::integer(sign, width, replacement),
            );
        }
    }
    execute(
        "bool",
        TerminalScalarValue::Boolean(false),
        TerminalScalarValue::Boolean(true),
    );
    for (name, initial, replacement) in [
        (
            "f32",
            IeeeFloatValue::Binary32(0x8000_0000),
            IeeeFloatValue::Binary32(0x7fc0_0042),
        ),
        (
            "f64",
            IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
            IeeeFloatValue::Binary64(0x7ff8_0000_0000_0042),
        ),
    ] {
        execute(
            name,
            TerminalScalarValue::IeeeFloat(initial),
            TerminalScalarValue::IeeeFloat(replacement),
        );
    }
}

fn execute(scalar: &str, initial: TerminalScalarValue, replacement: TerminalScalarValue) {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        execute_with_access(scalar, initial, replacement, access);
    }
}

fn execute_with_access(
    scalar: &str,
    initial: TerminalScalarValue,
    replacement: TerminalScalarValue,
    access: StructuralAccess,
) {
    let source = source(scalar);
    let source = if access == StructuralAccess::MutableBorrow {
        source.replace(
            &format!("input: &{scalar}"),
            &format!("input: &mut {scalar}"),
        )
    } else {
        source
    };
    let artifact = super::artifact(&source);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(caller.parameters.len(), 1);
    assert_eq!(caller.structural_parameters.len(), 3);
    assert_eq!(
        caller
            .structural_parameters
            .iter()
            .map(|parameter| parameter.access)
            .collect::<Vec<_>>(),
        [
            StructuralAccess::MutableBorrow,
            access,
            StructuralAccess::MutableBorrow
        ]
    );
    let operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let [
        input_read,
        establish,
        snapshot,
        call,
        current_read,
        output_store,
        snapshot_store,
    ] = operations.as_slice()
    else {
        panic!("ordered read/establish/snapshot/call/read/stores: {operations:?}");
    };
    assert!(
        matches!(input_read.kind, OperationKind::PrimitiveScalarRead { source }
        if source == caller.structural_parameters[1].place)
    );
    assert!(
        matches!(establish.kind, OperationKind::EstablishPrimitiveLocal { value }
        if value == input_read.result.scalar().unwrap().id)
    );
    let local = establish.result.structural().unwrap().place;
    assert!(
        caller
            .structural_parameters
            .iter()
            .all(|parameter| parameter.place != local)
    );
    for read in [snapshot, current_read] {
        assert!(
            matches!(read.kind, OperationKind::PrimitiveScalarRead { source } if source == local)
        );
    }
    assert_ne!(snapshot.result, current_read.result);
    let OperationKind::CallUnit {
        structural_arguments,
        arguments,
        ..
    } = &call.kind
    else {
        panic!("ordinary Unit call");
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].place, local);
    assert_eq!(
        structural_arguments[0].access,
        StructuralAccess::MutableBorrow
    );
    assert!(structural_arguments[0].path.is_empty());
    assert_eq!(arguments, &[caller.parameters[0].id]);
    for (store, parameter, read) in [
        (output_store, &caller.structural_parameters[0], current_read),
        (snapshot_store, &caller.structural_parameters[2], snapshot),
    ] {
        assert!(
            matches!(store.kind, OperationKind::WriteOnlyPrimitiveStore { destination, value }
            if destination == parameter.place && value == read.result.scalar().unwrap().id)
        );
    }

    let parameters = caller
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| TerminalStructuralValue {
            opaque_identity: 71 + position as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    // Each output starts with the opposite value, so both committed stores are observable.
    let starting_values = [initial, initial, replacement];
    let primitive_values = starting_values
        .iter()
        .enumerate()
        .map(|(position, value)| TerminalStructuralPrimitiveValue {
            argument_index: position as u32,
            value: *value,
        })
        .collect::<Vec<_>>();
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[replacement],
            &parameters,
            &primitive_values,
        )
        .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut complete = false;
    for _ in 0..64 {
        let status = execution.resume(&mut meter).unwrap();
        let values = execution.structural_primitive_values();
        assert_eq!(
            values[1].value, initial,
            "input referent must remain unchanged"
        );
        observations.push([values[0].value, values[2].value]);
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected status {other:?}"),
        }
    }
    assert!(complete);
    observations.dedup();
    assert_eq!(
        observations,
        [
            [initial, replacement],
            [replacement, replacement],
            [replacement, initial],
        ]
    );
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let usage = meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
            .unwrap();
        assert_eq!(
            usage.executions(),
            1,
            "operation {:?} must not replay",
            operation.id
        );
        assert_eq!(usage.units(), 1);
    }
}
