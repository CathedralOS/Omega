//! Exact carrier fit at the executed conversion, not at declaration entry.
//!
//! The normal-return owner evaluates preceding operands and captures the cast
//! input before observing its result. This retains call guarantees and retires
//! stale storage facts in the same way as ordinary call requirements. Address
//! bounds stay symbolic; a native representation choice supplies no premise.

use super::citations::CitationFacts;
use super::structural_case_arms::structural_arm_judge;
use super::structural_judgment::{StructuralJudge, StructuralTerm};
use super::structural_prefix::{StructuralPrefixSite, StructuralPrefixVisitor};
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub(crate) fn exact_cast_fits_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(cast) = program.expression_table.cast_node(expression) else {
        return false;
    };
    let Some(target) = program.primitive_type_reference(cast.target_type) else {
        return false;
    };
    let Some(source_reference) =
        crate::expression_result_type_reference(program, machine, state, cast.value)
    else {
        return false;
    };
    let Some(statement_index) =
        super::completed_values::expression_statement_index(program, state, expression)
    else {
        return false;
    };
    let mut visitor = CastFit {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            ),
        },
        expression,
        source_reference,
        target,
        seen: false,
        proven: true,
    };
    let recognized = super::structural_prefix::observe_runtime_state_prefix(
        program,
        machine,
        state,
        &mut visitor,
    )
    .is_some();
    recognized && visitor.seen && visitor.proven
}

struct CastFit {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    source_reference: TypeReferenceHandle,
    target: PrimitiveType,
    seen: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for CastFit {
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
        self.seen = true;
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        let Some(state) = program.state_in_machine(machine, self.site.state) else {
            self.proven = false;
            return;
        };
        let Some(entries) = judge.normal_operation_entries(
            self.site.statement_index,
            self.expression,
            environment,
            (machine, state, self.source_reference),
        ) else {
            self.proven = false;
            return;
        };
        self.proven &= !entries.is_empty();
        for entry in entries {
            let [value] = entry.arguments.as_slice() else {
                self.proven = false;
                continue;
            };
            let fits = entry.judge.observed_integer_fits(
                value.clone(),
                self.source_reference,
                self.target,
            );
            self.proven &= fits;
        }
    }
}
