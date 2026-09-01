use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.swap(0, 1)),
        StraightLineBooleanNotImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.pop();
        }),
        StraightLineBooleanNotImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineBooleanNotImmediateTranslationError::TargetProvenance
    );
}

#[test]
fn target_immediate_corruption_fails_closed() {
    for mutate in [
        mutate_edge as fn(&mut TargetOperation),
        mutate_source_value,
        mutate_value,
        substitute_operation,
    ] {
        assert_eq!(
            candidate_error(|target| mutate(&mut target.functions[0].operation)),
            StraightLineBooleanNotImmediateTranslationError::TargetOperation
        );
    }
}

fn mutate_edge(operation: &mut TargetOperation) {
    let TargetOperation::ReturnBooleanImmediate { psi_edge, .. } = operation else {
        unreachable!()
    };
    *psi_edge = EdgeId::new(68_200).unwrap();
}

fn mutate_source_value(operation: &mut TargetOperation) {
    let TargetOperation::ReturnBooleanImmediate { source_value, .. } = operation else {
        unreachable!()
    };
    *source_value = ValueId::new(68_201).unwrap();
}

fn mutate_value(operation: &mut TargetOperation) {
    let TargetOperation::ReturnBooleanImmediate { value, .. } = operation else {
        unreachable!()
    };
    *value = true;
}

fn substitute_operation(operation: &mut TargetOperation) {
    *operation = TargetOperation::ReturnBooleanNotParameter {
        psi_edge: EdgeId::new(68_007).unwrap(),
        source_value: ValueId::new(68_006).unwrap(),
        parameter_index: 0,
        location: omega_target_operations::ScalarParameterLocation::IncomingStack {
            byte_offset: 0,
        },
    };
}
