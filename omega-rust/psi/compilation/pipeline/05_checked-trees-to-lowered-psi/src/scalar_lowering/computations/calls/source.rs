//! Join an invocation to the checking owner that admitted its exact operands.
//! Statement captures and bound declaration applications retain their distinct
//! source custody; scheduling consumes this borrowed view for either origin.

use super::super::{Expansion, LoweringError, Site, unsupported};
use crate::emission::operation_emission::buffer::{SourceCallCoordinate, SourceInvocation};
use checked_trees::expression::ExpressionHandle;

pub(super) struct CallSource<'a> {
    pub(super) expression: ExpressionHandle,
    pub(super) arguments: &'a [ExpressionHandle],
    pub(super) coordinate: SourceInvocation,
}

impl<'a> Expansion<'a> {
    pub(super) fn call_source(
        &self,
        origin: checked_trees::CheckedCallOrigin,
        expression: ExpressionHandle,
        callee: &checked_trees::CheckedScalarCallee,
        ordinal: u32,
        site: &Site<'_>,
    ) -> Result<CallSource<'a>, LoweringError> {
        let control = &self.checked.facts.flow.control;
        match origin {
            checked_trees::CheckedCallOrigin::Statement(handle) => {
                if !control.calls.is_valid(handle) {
                    return unsupported("scalar computation lost its exact checked invocation");
                }
                let call = control.calls.get(handle);
                let state =
                    control
                        .state(self.machine, site.state)
                        .ok_or(LoweringError::Unsupported(
                            "scalar computation invocation state is absent",
                        ))?;
                if !state.calls.handles().any(|candidate| candidate == handle)
                    || control.is_retired(site.state, call)
                    || call.statement_index != site.statement as usize
                    || call.call_ordinal != ordinal as usize
                    || call.target_symbol != callee.called_signature()
                    || call.authored_expression != expression
                {
                    return unsupported("scalar computation invocation coordinate disagrees");
                }
                Ok(CallSource {
                    expression,
                    arguments: control.call_arguments.span(call.authored_arguments).ok_or(
                        LoweringError::Unsupported(
                            "scalar invocation lost its authored argument roster",
                        ),
                    )?,
                    coordinate: SourceCallCoordinate {
                        state: site.state,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                    }
                    .into(),
                })
            }
            checked_trees::CheckedCallOrigin::Declaration { application, call } => {
                let plan = &self.checked.facts.termination.rank_arrivals;
                let (_, retained) = origin
                    .declaration(plan, self.machine, site.state)
                    .map_err(LoweringError::Unsupported)?;
                Ok(CallSource {
                    expression,
                    arguments: plan
                        .application_call_arguments
                        .span(retained.checked_arguments)
                        .ok_or(LoweringError::Unsupported(
                            "declaration invocation lost its checked argument roster",
                        ))?,
                    coordinate: SourceInvocation::Declaration {
                        state: site.state,
                        application,
                        call,
                        observation: self.declaration_observation.ok_or(
                            LoweringError::Unsupported(
                                "declaration invocation has no captured observation scope",
                            ),
                        )?,
                    },
                })
            }
        }
    }
}
