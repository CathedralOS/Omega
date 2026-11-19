use super::*;
use crate::TargetObservation;
use omega_typer::{CrashRoute, TypedTrees, TypingObligation};

#[derive(Default)]
pub(crate) struct InvocationProofs<'trees, 'text> {
    pub(crate) facts: Vec<CheckedInvocationFact<'trees, 'text>>,
}

/// A checker judgment applies to this occurrence, selected body, instantiated
/// inputs and one proposition. A postcondition also records the returned value.
pub struct CheckedInvocationFact<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    request: ProofRequest<'trees>,
    application: Option<TypedCall>,
    observations: Vec<TargetObservation>,
    operands: Vec<PreparedValue>,
    result: Option<PreparedValue>,
}

#[derive(Clone, Copy)]
pub(crate) struct InvocationSite<'a> {
    pub(super) call: TreeNodeId,
    pub(super) signature: &'a TypedSignature,
    pub(super) body: TreeNodeId,
}

#[derive(Clone, Copy)]
pub(crate) struct ProofRequest<'a> {
    pub(super) invocation: InvocationSite<'a>,
    pub(super) claim: InvocationClaim<'a>,
}

#[derive(Clone, Copy)]
pub(crate) enum InvocationClaim<'a> {
    Requires(TreeNodeId),
    CrashDisproved { route: &'a CrashRoute, guard: &'a TypingObligation },
    Ensures(&'a TypingObligation),
}

impl InvocationClaim<'_> {
    fn same_claim(
        &self,
        other: Self,
    ) -> bool {
        match (*self, other) {
            (Self::Requires(checked), Self::Requires(requested)) => checked == requested,
            (
                Self::CrashDisproved {
                    route: checked_route,
                    guard: checked_guard,
                },
                Self::CrashDisproved { route, guard },
            ) => std::ptr::eq(checked_route, route) && std::ptr::eq(checked_guard, guard),
            (Self::Ensures(checked), Self::Ensures(requested)) => std::ptr::eq(checked, requested),
            _ => false,
        }
    }

    fn refusal(self) -> &'static str {
        match self {
            Self::Requires(_) => "the selected precondition requires a checked proof for the supplied arguments",
            Self::CrashDisproved { route, .. } => match route.cause {
                CrashCause::Trap => "the selected Trap route requires a checked disproof for the supplied arguments",
                CrashCause::Abort => "the selected Abort route requires a checked disproof for the supplied arguments",
            },
            Self::Ensures(_) => "the selected postcondition requires a checked proof for the arguments and result",
        }
    }
}

impl CheckedInvocationFact<'_, '_> {
    fn covers(
        &self,
        trees: &TypedTrees<'_>,
        request: ProofRequest<'_>,
        application: Option<&TypedCall>,
    ) -> bool {
        let checked = self.request.invocation;
        let requested = request.invocation;
        std::ptr::eq(self.trees, trees)
            && checked.call == requested.call
            && std::ptr::eq(checked.signature, requested.signature)
            && checked.body == requested.body
            && self.request.claim.same_claim(request.claim)
            && match (self.application.as_ref(), application) {
                (None, None) => true,
                (Some(checked), Some(actual)) => {
                    checked.machine == actual.machine
                        && checked.type_arguments == actual.type_arguments
                        && checked.enclosing_arguments == actual.enclosing_arguments
                        && checked.evidence == actual.evidence
                }
                _ => false,
            }
    }
}

impl ConstantEvaluation<'_, '_> {
    pub(in crate::invocation) fn has_invocation_proof(
        &self,
        request: ProofRequest<'_>,
        application: Option<&TypedCall>,
    ) -> bool {
        self.invocation_proofs
            .facts
            .iter()
            .any(|fact| fact.covers(self.trees, request, application))
    }

    pub(in crate::invocation) fn admit_proof(
        &self,
        request: ProofRequest<'_>,
        arguments: &CallBindings,
        stack: &[PreparedValue],
    ) -> Result<(), EvaluationError> {
        let application = arguments.1.as_deref();
        let arguments = arguments.0.borrow();
        // The first bindings are immutable contract inputs. A body can append
        // and mutate its own locals without changing these entry bindings.
        let parameter_count = request.invocation.signature.parameters.len();
        let result = match request.claim {
            InvocationClaim::Requires(_) | InvocationClaim::CrashDisproved { .. } => None,
            InvocationClaim::Ensures(_) => Some(stack.last().ok_or(UNSUPPORTED_BODY)?),
        };
        let matching = self.invocation_proofs.facts.iter().filter(|fact| {
            fact.covers(self.trees, request, application)
                && arguments
                    .iter()
                    .take(parameter_count)
                    .map(|argument| &argument.value)
                    .eq(&fact.operands)
                && fact.result.as_ref() == result
        });
        let mut changed_target = false;
        for fact in matching {
            if !fact.observations.is_empty()
                && !self.target.is_some_and(|target| {
                    fact.observations
                        .iter()
                        .all(|observation| target.matches_observation(observation))
                })
            {
                changed_target = true;
                continue;
            }
            for observation in &fact.observations {
                self.record_observation(observation.clone());
            }
            return Ok(());
        }
        if changed_target {
            return Err("the checked invocation proof depends on a different target selection".into());
        }
        Err(request.claim.refusal().into())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
pub(super) fn probe_literal_guarantee<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    invocation: InvocationSite<'trees>,
    operands: Vec<PreparedValue>,
    result: &BigInt,
) -> Option<InvocationProofs<'trees, 'text>> {
    let signature = invocation.signature;
    let tree = trees.trees().nth(signature.node.file)?;
    let resolved = tree.resolved();
    let file = resolved.file;
    let ensures = file.child(signature.node.node, NodeKind::Ensures)?;
    let [predicate] = *file.nodes[ensures].children.as_slice() else { return None };
    let term = file.ungroup(predicate);
    let [subject, literal] = *file.nodes[term].children.as_slice() else {
        return None;
    };
    if !matches!(resolved.symbol(subject), Some(Symbol::Builtin(name)) if name == "result") || landed_value(&tree, literal, &signature.result).ok()?? != *result
    {
        return None;
    }
    let mut operations = tree.semantics_at(term).filter_map(|semantic| match semantic {
        NodeSemantics::Operation(operation) => Some(operation),
        _ => None,
    });
    let operation = operations.next()?;
    if operations.next().is_some()
        || operation.token != "=="
        || operation.result != ValueType::Bool
        || operation.operands.as_slice() != [signature.result.clone(), signature.result.clone()]
    {
        return None;
    }
    let obligation = trees.obligations.iter().find(|obligation| {
        obligation.node == file.node_id(predicate)
            && matches!(&obligation.kind,
            ObligationKind::ProveGuarantee { machine, predicate } if machine == &signature.machine && predicate == &obligation.site)
    })?;
    Some(InvocationProofs {
        facts: vec![CheckedInvocationFact {
            trees,
            request: ProofRequest {
                invocation,
                claim: InvocationClaim::Ensures(obligation),
            },
            application: None,
            observations: vec![],
            operands,
            result: Some(PreparedValue::Integer(result.clone())),
        }],
    })
}
