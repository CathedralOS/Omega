use super::*;
use omega_typed::{BoundarySlot, ConformanceRow, SlotDirection, TypedTrees};

use crate::{TargetIdentity, TargetObservation};

pub type SelectedRealization = omega_target_contract::SelectedRealization<ValueType, DeclarationId>;

#[derive(Default)]
pub(crate) struct ProviderAdmission<'trees, 'text> {
    pub(crate) checked: Vec<CheckedProvider<'trees, 'text>>,
}

/// A checked Build plan proves the exact outbound application, its local
/// realization, checked refinement and proof/build trust. Retained observation
/// data alone cannot authorize any selection.
pub struct CheckedProvider<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    call: TreeNodeId,
    binding: &'trees ConformanceRow,
    application: Option<CheckedProviderApplication>,
    target: TargetIdentity,
    realization: SelectedRealization,
    observations: Vec<TargetObservation>,
}

/// Both applications are normalized by the checked provider plan. Identical
/// positional generic arguments may still have different declaration owners.
struct CheckedProviderApplication {
    requirement: TypedCall,
    implementation: TypedCall,
}

pub(super) struct SelectedProvider<'a> {
    pub(super) implementation: &'a DeclarationId,
    pub(super) application: Option<&'a TypedCall>,
    pub(super) observation: TargetObservation,
}

const UNSELECTED_SLOT: PreparationReason = PreparationReason::UnselectedBoundarySlot;

impl ConstantEvaluation<'_, '_> {
    /// Every checked proof for the exact typed call, slot, application and
    /// target identity is judged independently. Only a fully applicable proof
    /// can authorize execution, so rejected or stale evidence never shadows a
    /// later applicable proof and never contributes observations. Applicable
    /// proofs must agree on the executed realization; otherwise the slot fails
    /// closed instead of depending on evidence order.
    pub(super) fn checked_provider(
        &self,
        call: &TypedCall,
        application: Option<&TypedCall>,
        slot: &BoundarySlot,
    ) -> Result<SelectedProvider<'_>, EvaluationError> {
        if slot.direction != SlotDirection::ProgramToProvider {
            return Err(UNSELECTED_SLOT.into());
        }
        let target = self.target.ok_or(UNSELECTED_SLOT)?;
        let mut refusal = None;
        let mut selected: Option<(&CheckedProvider<'_, '_>, Option<&TypedCall>)> = None;
        for proof in self.providers.checked.iter().filter(|proof| {
            std::ptr::eq(proof.trees, self.trees)
                && proof.call == call.node
                && proof.binding.requirement == slot.requirement
                && proof.realization.arguments == slot.arguments
                && proof.application.as_ref().map(|selected| &selected.requirement) == application
                && target.identity() == &proof.target
        }) {
            match self.applicable_provider(proof, call, target) {
                Err(reason) => {
                    if refusal.is_none() {
                        refusal = Some(reason);
                    }
                }
                Ok(realized) => match selected {
                    None => selected = Some((proof, realized)),
                    Some((chosen, chosen_application)) => {
                        // Both realizations already matched the target's unique plan for
                        // this slot. The proof-local binding and application must agree too.
                        if chosen.binding != proof.binding || chosen_application != realized {
                            return Err(PreparationReason::CompetingProviderRealizations.into());
                        }
                    }
                },
            }
        }
        let Some((proof, application)) = selected else {
            return Err(refusal.unwrap_or(UNSELECTED_SLOT).into());
        };
        for dependency in &proof.observations {
            self.record_observation(dependency.clone());
        }
        Ok(SelectedProvider {
            implementation: &proof.binding.implementation,
            application,
            observation: TargetObservation::SelectedRealization {
                target: proof.target.clone(),
                realization: proof.realization.clone(),
            },
        })
    }

    /// Judge one candidate without recording anything. The certificate must
    /// realize its own conformance row, match the selected plan, depend only on
    /// observations of the selected target and preserve the positional application.
    fn applicable_provider<'proof>(
        &self,
        proof: &'proof CheckedProvider<'_, '_>,
        call: &TypedCall,
        target: &crate::TargetSemantics,
    ) -> Result<Option<&'proof TypedCall>, PreparationReason> {
        let binding = proof.binding;
        let realization = &proof.realization;
        if !self
            .trees
            .conformance_rows
            .iter()
            .any(|row| std::ptr::eq(row, binding))
            || realization.requirement != binding.requirement
            || realization.implementation != binding.implementation
            || realization.conformance != binding.conformance
            || realization.binding != binding.site
        {
            return Err(PreparationReason::ProviderConformanceMismatch);
        }
        let observation = TargetObservation::SelectedRealization {
            target: proof.target.clone(),
            realization: realization.clone(),
        };
        if !target.matches_observation(&observation) {
            return Err(PreparationReason::ProviderTargetPlanMismatch);
        }
        if !proof
            .observations
            .iter()
            .all(|observation| target.matches_observation(observation))
        {
            return Err(PreparationReason::ProviderProofTargetChanged);
        }
        let implementation = &binding.implementation;
        if let Some(selected) = &proof.application {
            let required = &selected.requirement;
            let realized = &selected.implementation;
            if &realized.machine != implementation
                || realized.node != call.node
                || realized.type_arguments != required.type_arguments
                || realized
                    .enclosing_arguments
                    .iter()
                    .any(|(_, arguments)| !arguments.is_empty())
                || realized.evidence.is_some()
                || !realized
                    .operands
                    .iter()
                    .map(|operand| (&operand.node, &operand.subject, &operand.callable))
                    .eq(required
                        .operands
                        .iter()
                        .map(|operand| (&operand.node, &operand.subject, &operand.callable)))
            {
                return Err(PreparationReason::ProviderApplicationMismatch);
            }
            Ok(Some(realized))
        } else {
            Ok(None)
        }
    }
}

impl ConstantEvaluation<'_, '_> {
    pub(super) fn check_provider_contract(
        &self,
        requirement_index: usize,
        requirement: (&TypedSignature, Option<&TypedCall>),
        implementation: (&TypedSignature, Option<&TypedCall>),
    ) -> Result<(), EvaluationError> {
        let effects = self
            .machine_effects(requirement_index)
            .ok_or(UNSUPPORTED_CONTRACT)?;
        let (requirement, requirement_application) = requirement;
        let (implementation, implementation_application) = implementation;
        let apply = |application: Option<&TypedCall>, carrier: &ValueType| application.map_or_else(|| carrier.clone(), |call| call.instantiate(carrier));
        if !requirement.terminates
            || requirement.crash_ceiling.is_none()
            || !effects.reach.is_empty()
            || effects.pause.suspend
            || effects.pause.block
            || !effects.invokes.is_empty()
            || apply(requirement_application, &requirement.result) != apply(implementation_application, &implementation.result)
            || !requirement
                .parameters
                .iter()
                .map(|(_, carrier)| apply(requirement_application, carrier))
                .eq(implementation
                    .parameters
                    .iter()
                    .map(|(_, carrier)| apply(implementation_application, carrier)))
        {
            return Err(UNSUPPORTED_CONTRACT.into());
        }
        Ok(())
    }
}

pub(super) fn bind_requirement(
    requirement: &TypedSignature,
    tree: &TypedTree<'_>,
    operands: &CallBindings,
    application: Option<Rc<TypedCall>>,
) -> Result<Rc<CallBindings>, EvaluationError> {
    // Requirement parameter names belong to its own declaration. Project the
    // already evaluated values by ordinal, preserving the entry snapshots.
    let file = tree.resolved().file;
    let parameters = file
        .child(requirement.node.node, NodeKind::Params)
        .map_or(&[][..], |list| file.nodes[list].children.as_slice());
    let operands = operands.values.borrow();
    if parameters.len() != operands.len() {
        return Err(UNSUPPORTED_CONTRACT.into());
    }
    let bindings = parameters
        .iter()
        .zip(operands.iter())
        .map(|(&binder, operand)| BindingValue {
            binder,
            carrier: operand.carrier.clone(),
            value: operand.value.clone(),
            writable: false,
        })
        .collect();
    Ok(Rc::new(CallBindings {
        values: RefCell::new(bindings),
        application,
        ..Default::default()
    }))
}

#[cfg(test)]
mod tests;
