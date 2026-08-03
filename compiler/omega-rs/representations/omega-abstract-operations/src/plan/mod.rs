mod capacity;
mod code;

pub use code::{AbstractOperationCode, AbstractOperationPlan};

impl Default for AbstractOperationPlan {
    fn default() -> Self {
        Self::with_capacity(0, 0, 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AbstractOperationCode, AbstractOperationPlan, AbstractSemanticSummary,
        CheckedNoCodePermissionReason, PermissionRealizationCandidate,
        PermissionRealizationCandidateKind,
    };
    use psi_arena::Arena;

    #[test]
    fn plan_constructor_keeps_code_and_semantic_roots_explicit() {
        let code = AbstractOperationCode {
            functions: Arena::with_capacity(1),
            instructions: Arena::with_capacity(2),
            operands: Arena::with_capacity(3),
            runtime_value_operands: Arena::with_capacity(4),
        };
        let semantics = AbstractSemanticSummary::with_capacity(5, 6, 7, 8, 9);
        let candidates = vec![PermissionRealizationCandidate {
            source_event_index: 1,
            kind: PermissionRealizationCandidateKind::CheckedNoCode {
                reason: CheckedNoCodePermissionReason::ExplicitZeroCodeConsume,
            },
        }];

        let plan =
            AbstractOperationPlan::with_roots(code.clone(), semantics.clone(), candidates.clone());

        assert_eq!(plan.code, code);
        assert_eq!(plan.semantics, semantics);
        assert_eq!(plan.permission_realization_candidates, candidates);
        assert_eq!(
            plan.semantics.boundaries.footprints,
            crate::BoundaryFootprintPlan::default()
        );
    }
}
