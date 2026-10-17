//! A completed call's metadata uses the shared normal-return value owner.
//!
//! Capture the exact operation after its earlier operands and before later
//! siblings. Selected public equations, including the common target-family
//! equations, describe this occurrence. No contract syntax or private body is
//! reinterpreted here, and future facts cannot describe an earlier descriptor.

use crate::proof_contracts::contract_entailment as entailment;
use entailment::citations::CitationFacts;
use entailment::structural_judgment::{StructuralJudge, StructuralTerm};
use entailment::structural_prefix::{StructuralPrefixSite, StructuralPrefixVisitor};
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::{machine::Machine, state::State, types::TypeReferenceHandle};

type LengthSite = (SymbolHandle, SymbolHandle, ExpressionHandle);
type ActiveLengthSite = (
    SymbolHandle,
    SymbolHandle,
    ExpressionHandle,
    std::thread::ThreadId,
);

pub(super) fn normal_return_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<u64> {
    // Bare mutable source inputs have no invocation guard. This optional proof
    // query must decline instead of recursively assuming its own result.
    let memos = crate::frozen_program::frozen_program_memos(program)?;
    let site: LengthSite = (machine.symbol, state.symbol, expression);
    if let Some(length) = memos.completed_collection_lengths.lock().ok()?.get(&site) {
        return Some(*length);
    }
    let active = (site.0, site.1, site.2, std::thread::current().id());
    if memos
        .active_collection_lengths
        .lock()
        .ok()?
        .insert(active, ())
        .is_some()
    {
        return None;
    }
    let _active = ActiveLengthQuery {
        memos,
        site: active,
    };
    let statement_index =
        entailment::completed_values::expression_statement_index(program, state, expression)?;
    let reference = crate::expression_result_type_reference(program, machine, state, expression)?;
    crate::slice_view_shell(program, reference, &[])?;
    let mut visitor = NormalReturnLength {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: entailment::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            ),
        },
        expression,
        reference,
        length: None,
        recognized: true,
    };
    entailment::structural_prefix::observe_runtime_state_prefix(
        program,
        machine,
        state,
        &mut visitor,
    )?;
    let length = visitor.recognized.then_some(visitor.length?)?;
    // Only successful observations are cached. A declined recursive query is
    // unavailable evidence, never a completed negative result for another path.
    memos
        .completed_collection_lengths
        .lock()
        .ok()?
        .insert(site, length);
    Some(length)
}

struct ActiveLengthQuery<'query> {
    memos: &'query crate::frozen_program::CheckingMemos,
    site: ActiveLengthSite,
}

impl Drop for ActiveLengthQuery<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.memos.active_collection_lengths.lock() {
            active.remove(&self.site);
        }
    }
}

struct NormalReturnLength {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    length: Option<u64>,
    recognized: bool,
}

impl StructuralPrefixVisitor for NormalReturnLength {
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn visit<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'program>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    ) {
        let mut judge = entailment::structural_case_arms::structural_arm_judge(
            program, judge, hypotheses, equations,
        );
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        let Some(state) = program.state_in_machine(machine, self.site.state) else {
            self.recognized = false;
            return;
        };
        let Some(outcomes) = judge.normal_operation_results(
            self.site.statement_index,
            self.expression,
            environment,
            (machine, state, self.reference),
        ) else {
            self.recognized = false;
            return;
        };
        for outcome in outcomes {
            let length = entailment::structural_terms::observed_collection_measure(
                program,
                self.reference,
                CollectionMeasure::Length,
                || Some(outcome.value),
            )
            .and_then(|measure| {
                let StructuralTerm::Integer(value) = outcome.judge.resolve(measure) else {
                    return None;
                };
                value.to_u64()
            });
            match (self.length, length) {
                (None, Some(length)) => self.length = Some(length),
                (Some(previous), Some(length)) if previous == length => {}
                _ => self.recognized = false,
            }
        }
    }
}
