use super::*;
use target_operations::TerminalPsiProvenance;

#[test]
fn target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.swap(0, 1)),
        StraightLineIntegerWidenImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.pop();
        }),
        StraightLineIntegerWidenImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineIntegerWidenImmediateTranslationError::TargetProvenance
    );
}

#[test]
fn target_immediate_corruption_fails_closed() {
    for mutate in [
        mutate_edge as fn(&mut TargetOperation),
        mutate_source_value,
        mutate_type,
        mutate_value,
        substitute_operation,
    ] {
        assert_eq!(
            candidate_error(|target| mutate(&mut target.functions[0].operation)),
            StraightLineIntegerWidenImmediateTranslationError::TargetOperation
        );
    }
}

fn mutate_edge(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerImmediate { psi_edge, .. } = operation else {
        unreachable!()
    };
    *psi_edge = EdgeId::new(64_200).unwrap();
}

fn mutate_source_value(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerImmediate { source_value, .. } = operation else {
        unreachable!()
    };
    *source_value = ValueId::new(64_201).unwrap();
}

fn mutate_type(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerImmediate { scalar_type, .. } = operation else {
        unreachable!()
    };
    *scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
}

fn mutate_value(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerImmediate { value, .. } = operation else {
        unreachable!()
    };
    *value = IntegerValue::Signed(-1);
}

fn substitute_operation(operation: &mut TargetOperation) {
    *operation = TargetOperation::ReturnIntegerParameter {
        psi_edge: EdgeId::new(64_007).unwrap(),
        source_value: ValueId::new(64_006).unwrap(),
        scalar_type: target_type(),
        parameter_index: 0,
        location: target_operations::ScalarParameterLocation::IncomingStack { byte_offset: 0 },
    };
}
