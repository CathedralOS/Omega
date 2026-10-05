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
pub(super) struct IndexVerdicts(Vec<(ExpressionHandle, bool)>);

impl IndexVerdicts {
    /// The occurrences every check proved, ordered by handle position.
    pub(super) fn into_proven(self) -> Vec<ExpressionHandle> {
        let mut verdicts = self.0;
        verdicts
            .sort_by_key(|(expression, _)| checked_trees::operators::handle_position(*expression));
        let mut proven: Vec<ExpressionHandle> = Vec::new();
        let mut index = 0;
        while index < verdicts.len() {
            let expression = verdicts[index].0;
            let mut all = true;
            while index < verdicts.len() && verdicts[index].0 == expression {
                all &= verdicts[index].1;
                index += 1;
            }
            if all {
                proven.push(expression);
            }
        }
        proven
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_expression<'program>(
    program: &'program typed_trees::TypedTrees,
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
        );
        self.verdicts.0.push((
            expression,
            verdict == self::validation::BoundsCheckResult::ProvenScalar,
        ));
    }
}
