use omega_psi::{
    BlockTerminator, CallArgument, CallRequirement, CallSite, CallTarget, ContractClause, LoweredMachine, LoweredPackage, OperationKind, StaticArgument,
};

/// The two premise questions a checker asks of one closed callback invocation,
/// read from Psi alone.
#[derive(Debug)]
pub struct CallbackPremises<'a> {
    pub call: CallSite,
    pub binder: usize,
    /// The machine this instance selected for the binder, which the call executes.
    pub selected: &'a LoweredMachine,
    /// The call's actuals, substituted for the bound's formals.
    pub arguments: &'a [CallArgument],
    /// The binder clauses this call owes under `arguments`, in clause order.
    pub owed: Vec<&'a ContractClause>,
    /// The refinement: the bound's premises must imply `provided` at the same formals.
    pub bound: &'a [ContractClause],
    pub provided: &'a [ContractClause],
}

/// Every listed callback invocation of `instance`, in listing order. Returns
/// `None` if a listing is malformed: a binder that is not a callable selection,
/// a missing or repeated call site, a callee other than the selected machine,
/// an arity or formal type that disagrees with the selection, or requirements
/// that skip, reorder or exceed the bound's clauses.
pub fn callback_premises<'a>(
    package: &'a LoweredPackage,
    instance: &'a LoweredMachine,
) -> Option<Vec<CallbackPremises<'a>>> {
    let body = &instance.body;
    let mut found = Vec::with_capacity(body.callback_invocations.len());
    for (index, invocation) in body.callback_invocations.iter().enumerate() {
        if body.callback_invocations[..index]
            .iter()
            .any(|earlier| earlier.call == invocation.call)
        {
            return None;
        }
        let StaticArgument::Callable(selection) = instance.arguments.get(invocation.binder)? else {
            return None;
        };
        let (callee, arguments, requirements) = call_at(instance, invocation.call)?;
        if callee != CallTarget::Machine(selection.machine) || arguments.len() != selection.parameters.len() {
            return None;
        }
        let expected = (0..selection.requires.len()).map(|clause| CallRequirement { site: invocation.call, clause });
        if !requirements.iter().copied().eq(expected) {
            return None;
        }
        let selected = package.machines.iter().find(|m| m.id == selection.machine)?;
        if selected.parameter_types != selection.parameters || selected.result_type != selection.result {
            return None;
        }
        found.push(CallbackPremises {
            call: invocation.call,
            binder: invocation.binder,
            selected,
            arguments,
            owed: selection.requires.iter().collect(),
            bound: &selection.requires,
            provided: &selected.contract.requires,
        });
    }
    Some(found)
}

fn call_at(
    machine: &LoweredMachine,
    site: CallSite,
) -> Option<(CallTarget, &[CallArgument], &[CallRequirement])> {
    match site {
        CallSite::Operation(id) => machine
            .body
            .operations
            .iter()
            .find(|op| op.id == id)
            .and_then(|op| match &op.kind {
                OperationKind::Call {
                    callee,
                    arguments,
                    requirements,
                } => Some((*callee, arguments.as_slice(), requirements.as_slice())),
                _ => None,
            }),
        CallSite::Edge(id) => machine
            .body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                BlockTerminator::TerminalCall {
                    id: edge,
                    callee,
                    arguments,
                    requirements,
                    ..
                } if *edge == id => Some((*callee, arguments.as_slice(), requirements.as_slice())),
                _ => None,
            }),
        // A cleanup action invokes the cleaned value's selected cleanup machine with no
        // requirement list, never a binder, so a listing that names one is malformed.
        CallSite::Cleanup(_) => None,
    }
}
