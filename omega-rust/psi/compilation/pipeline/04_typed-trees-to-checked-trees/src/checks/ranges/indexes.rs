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

pub(super) fn check_expression<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program Machine,
    state: &State,
    call_frames: Option<&::validation::CallFrameResolver<'program>>,
    facts: &mut RangeFacts<'_>,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
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
    };
    transfer.transfer(facts, expression, &mut sink);
}

struct IndexChecks<'context, 'program, 'frames> {
    transfer: &'context ExpressionTransfer<'program, 'frames>,
    diagnostics: &'context mut Vec<Diagnostic>,
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
        check_indexed_access(
            self.transfer.program,
            self.transfer.machine,
            self.transfer.state,
            facts,
            expression,
            indexed,
            self.diagnostics,
        );
    }
}
