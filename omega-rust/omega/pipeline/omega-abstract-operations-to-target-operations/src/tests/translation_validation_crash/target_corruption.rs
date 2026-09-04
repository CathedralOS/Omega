use super::*;

#[test]
fn crash_candidate_and_provenance_corruption_fails_closed() {
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(2_113).unwrap()],
            edges: vec![EdgeId::new(2_004).unwrap()],
        },
        TerminalPsiProvenance {
            operations: Vec::new(),
            edges: vec![EdgeId::new(2_114).unwrap()],
        },
        TerminalPsiProvenance {
            operations: Vec::new(),
            edges: vec![EdgeId::new(2_004).unwrap(), EdgeId::new(2_115).unwrap()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLineScalarCrashTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: EdgeId::new(2_004).unwrap(),
                source_value: ValueId::new(2_003).unwrap(),
                value: false,
            };
        }),
        StraightLineScalarCrashTranslationError::TargetOperation
    );
    for mutate in [
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(2_116).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { cause, .. } = operation else {
                unreachable!()
            };
            *cause = CrashCause::Abort;
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { site_guard, .. } = operation else {
                unreachable!()
            };
            site_guard.pop();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { site_guard, .. } = operation else {
                unreachable!()
            };
            site_guard.swap(0, 1);
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { site_guard, .. } = operation else {
                unreachable!()
            };
            site_guard.push(CrashPredicateTerm::new(Proposition::Truth));
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { site_guard, .. } = operation else {
                unreachable!()
            };
            site_guard[0] = CrashPredicateTerm::new(Proposition::Falsehood);
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash {
                frontier_lower_bound,
                ..
            } = operation
            else {
                unreachable!()
            };
            frontier_lower_bound.pop();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash {
                frontier_lower_bound,
                ..
            } = operation
            else {
                unreachable!()
            };
            frontier_lower_bound.swap(0, 1);
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash {
                frontier_lower_bound,
                ..
            } = operation
            else {
                unreachable!()
            };
            frontier_lower_bound.push(ClaimId::new(2_117).unwrap());
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash {
                frontier_lower_bound,
                ..
            } = operation
            else {
                unreachable!()
            };
            frontier_lower_bound[0] = ClaimId::new(2_118).unwrap();
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| mutate(&mut candidate.functions[0].operation)),
            StraightLineScalarCrashTranslationError::TargetOperation
        );
    }
}
