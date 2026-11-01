//! Calls retain the checking occurrence that admitted their operands.
//! Statement execution and a bound declaration application have different
//! custody; both enter the same scalar call scheduler after that join.

use arena::Handle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedCallOrigin {
    Statement(Handle<super::FlowCallFact>),
    Declaration {
        application: crate::CheckedRankApplicationFormationHandle,
        call: Handle<crate::CheckedDeclarationCall>,
    },
}

impl CheckedCallOrigin {
    /// Read the call only inside its complete owning application. The
    /// declaration recipe alone grants no statement or invocation authority.
    pub fn declaration<'plan>(
        self,
        plan: &'plan crate::CheckedRankArrivalPlan,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
    ) -> Result<
        (
            &'plan crate::CheckedRankApplicationFormation,
            &'plan crate::CheckedDeclarationCall,
        ),
        &'static str,
    > {
        let Self::Declaration { application, call } = self else {
            return Err("call has statement custody, not declaration-application custody");
        };
        if !plan.application_formations.is_valid(application)
            || !plan.application_calls.is_valid(call)
        {
            return Err("declaration call has an invalid application or call handle");
        }
        let application = plan.application_formations.get(application);
        if application.machine != machine
            || application.state != state
            || application.machine_body.is_empty()
            || !application.machine_body.start().is_valid()
            || plan
                .bindings
                .span(application.bindings)
                .is_none_or(|bindings| bindings.is_empty() && application.owned_copy.is_none())
            || plan
                .application_calls
                .span(application.application.calls)
                .is_none()
            || !application
                .application
                .calls
                .handles()
                .any(|candidate| candidate == call)
        {
            return Err("declaration call changed its owning body, state or complete input roster");
        }
        if let Some(input) = application.owned_copy {
            if !input.formal.is_valid()
                || !input.actual_type.is_valid()
                || !plan.coordinates.is_valid(input.actual)
            {
                return Err("declaration call has an invalid whole-value input receipt");
            }
            let coordinate = plan.coordinates.get(input.actual);
            if !coordinate.parameter.is_valid()
                || plan.path_segments.span(coordinate.path).is_none()
                || plan.entry_indexes.span(coordinate.entry_indexes).is_none()
            {
                return Err("declaration call changed its whole-value input coordinate");
            }
        }
        let call = plan.application_calls.get(call);
        if !call.expression.is_valid()
            || !call.authored_target.is_valid()
            || !call.target_machine.is_valid()
            || !call.target_state.is_valid()
            || call.target_contract.is_none()
        {
            return Err("declaration call has no complete selected target contract binding");
        }
        Ok((application, call))
    }

    pub fn is_valid(self) -> bool {
        match self {
            Self::Statement(call) => call.is_valid(),
            Self::Declaration { application, call } => application.is_valid() && call.is_valid(),
        }
    }

    pub fn statement(self) -> Option<Handle<super::FlowCallFact>> {
        match self {
            Self::Statement(call) => Some(call),
            Self::Declaration { .. } => None,
        }
    }
}

impl From<Handle<super::FlowCallFact>> for CheckedCallOrigin {
    fn from(call: Handle<super::FlowCallFact>) -> Self {
        Self::Statement(call)
    }
}
