use super::expressions::ensured_call_result_bounds;
use super::expressions::expression_integer_value;
use diagnostics::Diagnostic;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableIndexedExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;

mod validation;
pub(in crate::checks) use validation::ranges_seam_owns;

use self::validation::check_indexed_access;
pub(super) use self::validation::is_builtin_scalar_index;
use super::expression_transfer::{ExpressionTransfer, ExpressionTransferSink};
use super::facts::RangeFacts;

/// Every bounds verdict each scalar `[]` occurrence received. An occurrence is
/// decided only when all of its checks proved the index: a range window, an
/// unsupported collection or a rejection decides nothing.
#[derive(Default)]
pub(super) struct IndexVerdicts {
    verdicts: Vec<(ExpressionHandle, bool, Option<u64>)>,
    /// Comparisons proved against a retained constant recipe's extent, for
    /// the target closure to discharge against its closed value.
    obligations: Vec<checked_trees::ConstantRecipeObligation>,
}

impl IndexVerdicts {
    pub(super) fn extend(&mut self, other: Self) {
        self.verdicts.extend(other.verdicts);
        self.obligations.extend(other.obligations);
    }

    /// The occurrences every check proved, ordered by handle position, beside
    /// the recipe obligations those proofs depend on.
    pub(super) fn into_parts(
        self,
    ) -> (
        Vec<ExpressionHandle>,
        Vec<(ExpressionHandle, u64)>,
        Vec<checked_trees::ConstantRecipeObligation>,
    ) {
        let mut verdicts = self.verdicts;
        verdicts.sort_by_key(|(expression, _, _)| {
            checked_trees::operators::handle_position(*expression)
        });
        let mut proven: Vec<ExpressionHandle> = Vec::new();
        let mut coordinates = Vec::new();
        let mut index = 0;
        while index < verdicts.len() {
            let expression = verdicts[index].0;
            let mut all = true;
            let mut coordinate = verdicts[index].2;
            while index < verdicts.len() && verdicts[index].0 == expression {
                all &= verdicts[index].1;
                if coordinate != verdicts[index].2 {
                    coordinate = None;
                }
                index += 1;
            }
            if all {
                proven.push(expression);
                if let Some(coordinate) = coordinate {
                    coordinates.push((expression, coordinate));
                }
            }
        }
        (proven, coordinates, self.obligations)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_expression<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program Machine,
    state: &State,
    call_frames: Option<&::validation::CallFrameResolver<'program>>,
    facts: &mut RangeFacts<'_>,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
    verdicts: &mut IndexVerdicts,
) {
    let transfer = ExpressionTransfer {
        program,
        machine,
        state,
        call_frames,
    };
    let mut sink = IndexChecks {
        transfer: &transfer,
        diagnostics,
        verdicts,
    };
    transfer.transfer(facts, expression, &mut sink);
}

struct IndexChecks<'context, 'program, 'frames> {
    transfer: &'context ExpressionTransfer<'program, 'frames>,
    diagnostics: &'context mut Vec<Diagnostic>,
    verdicts: &'context mut IndexVerdicts,
}

impl ExpressionTransferSink for IndexChecks<'_, '_, '_> {
    /// `align_of(view)` over a borrowed slice observes the first element,
    /// which exists only for a nonempty view. Its proof-only `view[0]` owes
    /// the same length fact as an ordinary first-element index, but it is not
    /// an access: no verdict or lowering obligation is retained for it.
    fn call_entry(&mut self, facts: &RangeFacts<'_>, call: &TableCallExpression) {
        let Some(request) = &call.layout_query else {
            return;
        };
        let first = request.nonempty_view;
        if !first.is_valid() {
            return;
        }
        let program = self.transfer.program;
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(first) else {
            return;
        };
        let mut failures = Vec::new();
        let verdict = check_indexed_access(
            program,
            self.transfer.machine,
            self.transfer.state,
            facts,
            first,
            indexed,
            &mut failures,
            &mut Vec::new(),
        );
        if verdict == self::validation::BoundsCheckResult::ProvenScalar && failures.is_empty() {
            return;
        }
        let view = program.expression_table.display_name(indexed.collection);
        self.diagnostics.push(
            Diagnostic::error(format!(
                "`align_of({view})` observes the slice's first element and needs the nonempty fact `{view}.len >= 1`, which is not proved here in {}::{}",
                self.transfer.machine.name, self.transfer.state.name
            ))
            .with_source_span(program.expression_table.source_span(first)),
        );
    }

    fn indexed_access(
        &mut self,
        facts: &RangeFacts<'_>,
        expression: ExpressionHandle,
        indexed: &TableIndexedExpression,
    ) {
        // Producers have already run and invalidated their writes. The
        // selected operator's bounds contract is discharged at this access.
        let verdict = check_indexed_access(
            self.transfer.program,
            self.transfer.machine,
            self.transfer.state,
            facts,
            expression,
            indexed,
            self.diagnostics,
            &mut self.verdicts.obligations,
        );
        // Retain the evaluated selector's singleton observation from the same
        // all-arrival range walk. This does not replace evaluation or its bound:
        // lowering uses it only to separate storage for overlap decisions.
        let coordinate = self
            .transfer
            .program
            .expression_interval(indexed.index)
            .and_then(|interval| {
                let low = interval.low?;
                (interval.high == Some(low)).then_some(low)
            })
            .or_else(|| expression_integer_value(self.transfer.program, facts, indexed.index))
            .or_else(|| {
                let (low, high) = ensured_call_result_bounds(
                    self.transfer.program,
                    self.transfer.machine.symbol,
                    indexed.index,
                )?;
                let low = low?;
                (high == Some(low)).then_some(low)
            })
            .and_then(|value| u64::try_from(value).ok());
        self.verdicts.verdicts.push((
            expression,
            verdict == self::validation::BoundsCheckResult::ProvenScalar,
            coordinate,
        ));
    }
}
