use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, TableIndexedExpression};
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
        let coordinate =
            ::validation::expression_interval(facts.expression_intervals, indexed.index)
                .and_then(|interval| {
                    let low = interval.low?;
                    (interval.high == Some(low)).then_some(low)
                })
                .or_else(|| {
                    super::expressions::expression_integer_value(
                        self.transfer.program,
                        facts,
                        indexed.index,
                    )
                })
                .or_else(|| {
                    let (low, high) = super::expressions::ensured_call_result_bounds(
                        self.transfer.program,
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
