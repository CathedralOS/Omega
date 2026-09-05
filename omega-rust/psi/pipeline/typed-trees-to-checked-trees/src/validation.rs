use diagnostics::Diagnostic;
use flow_effects::OperationalPlan;
use proof::obligations::ProofPlan;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

pub(crate) struct ValidatedTypedProgram<'program> {
    pub(crate) proof_plan: ProofPlan<'program>,
    pub(crate) operational: OperationalPlan,
    pub(crate) validation_facts: validation::ProgramValidationFacts,
}

pub(crate) fn validate_typed_program<'program>(
    program: &'program TypedTrees,
    opaque_property_receipts: &[validation::OpaqueDataPropertyReceipt],
    allow_pending_opaque_copy: bool,
) -> Result<ValidatedTypedProgram<'program>, Vec<Diagnostic>> {
    validate_atomic_result_custody(program)?;

    let validation_facts = if allow_pending_opaque_copy {
        validation::validate_preliminary_program_after_generic_contract_entailment_with_facts(
            program,
        )?
    } else {
        validation::validate_program_after_generic_contract_entailment_with_facts_and_opaque_property_receipts(
            program,
            opaque_property_receipts,
        )?
    };

    let proof_plan = proof::obligations::build_proof_plan(program);
    proof::checker::check_proof_plan(&proof_plan)?;

    let operational = flow_effects::infer_operational_may(program);
    validation::validate_behavior_plan(program, &operational)?;
    crate::call_acknowledgements::validate_call_acknowledgements(program, &operational)?;

    Ok(ValidatedTypedProgram {
        proof_plan,
        operational,
        validation_facts,
    })
}

fn validate_atomic_result_custody(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let ExpressionNode::Atomic(atomic) = expression else {
                return None;
            };
            (!atomic.result_custody.is_valid_for(atomic.ordering)
                || (atomic.result_custody.requires_result_destination()
                    && !atomic.result.is_valid()))
            .then(|| {
                Diagnostic::error(
                    "atomic expression result custody does not match its operation axis",
                )
            })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use language_core::atomic::{
        AtomicCompareExchangeOnceResultCustody as OnceCustody,
        AtomicCompareExchangeOutcomeIdentity as Outcome,
        AtomicExpressionResultCustody as ResultCustody,
        AtomicObservingCompareExchangeOperation as Operation,
        AtomicObservingCompareExchangeResultShape as Shape, AtomicOrderingPlan, MemoryOrdering,
    };
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableAtomicExpression};

    use super::validate_atomic_result_custody;

    fn program_with_atomic(
        ordering: AtomicOrderingPlan,
        result_custody: ResultCustody,
        has_result: bool,
    ) -> TypedTrees {
        let mut program = TypedTrees::default();
        let value = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        program
            .expression_table
            .insert(ExpressionNode::Atomic(TableAtomicExpression {
                value,
                result: if has_result {
                    value
                } else {
                    ExpressionHandle::invalid()
                },
                ordering,
                result_custody,
            }));
        program
    }

    #[test]
    fn checked_boundary_accepts_exact_single_attempt_result_custody() {
        let program = program_with_atomic(
            AtomicOrderingPlan::CompareExchangeOnce {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            },
            ResultCustody::ObservingCompareExchangeOnce(OnceCustody::CANONICAL),
            true,
        );

        assert!(validate_atomic_result_custody(&program).is_ok());
    }

    #[test]
    fn checked_boundary_rejects_incomplete_or_substituted_single_attempt_custody() {
        let once_ordering = AtomicOrderingPlan::CompareExchangeOnce {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        };
        let decisive_ordering = AtomicOrderingPlan::CompareExchange {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        };
        for (ordering, custody, has_result) in [
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody::CANONICAL),
                false,
            ),
            (
                decisive_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody::CANONICAL),
                true,
            ),
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody {
                    operation: Operation::Decisive,
                    ..OnceCustody::CANONICAL
                }),
                true,
            ),
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody {
                    result_shape: Shape::ExchangedOrMismatchedObserved,
                    ..OnceCustody::CANONICAL
                }),
                true,
            ),
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody {
                    outcome_identity: Outcome::AtomicCompareExchangeOutcome,
                    ..OnceCustody::CANONICAL
                }),
                true,
            ),
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody {
                    outcome_identity: Outcome::AtomicTryExchangeOnceOutcome,
                    ..OnceCustody::CANONICAL
                }),
                true,
            ),
            (once_ordering, ResultCustody::Scalar, true),
        ] {
            let program = program_with_atomic(ordering, custody, has_result);
            assert!(validate_atomic_result_custody(&program).is_err());
        }
    }

    #[test]
    fn checked_boundary_keeps_decisive_scalar_custody_unchanged() {
        let program = program_with_atomic(
            AtomicOrderingPlan::CompareExchange {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            },
            ResultCustody::Scalar,
            true,
        );

        assert!(validate_atomic_result_custody(&program).is_ok());
    }
}
