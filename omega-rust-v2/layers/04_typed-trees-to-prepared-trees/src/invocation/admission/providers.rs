use super::*;
use omega_typer::{BoundarySlot, ConformanceRow, RequirementSlot, Site, SlotDirection, TypedTrees};
use std::sync::Arc;

use crate::{TargetIdentity, TargetObservation};

/// The exact checked target selection used to run a requirement during preparation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SelectedRealization {
    pub requirement: RequirementSlot,
    pub arguments: Vec<ValueType>,
    pub implementation: DeclarationId,
    pub conformance: Option<DeclarationId>,
    pub binding: Site,
    pub checked_contract: Arc<[u8]>,
    pub selected_plan: Arc<[u8]>,
}

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

impl ConstantEvaluation<'_, '_> {
    pub(super) fn checked_provider(
        &self,
        call: &TypedCall,
        application: Option<&TypedCall>,
        slot: &BoundarySlot,
    ) -> Result<SelectedProvider<'_>, EvaluationError> {
        if slot.direction != SlotDirection::ProgramToProvider {
            return Err("the selected runtime boundary slot cannot execute during preparation".into());
        }
        let proof = self
            .providers
            .checked
            .iter()
            .find(|proof| {
                std::ptr::eq(proof.trees, self.trees)
                    && proof.call == call.node
                    && proof.binding.requirement == slot.requirement
                    && proof.realization.arguments == slot.arguments
                    && proof.application.as_ref().map(|selected| &selected.requirement) == application
                    && self
                        .target
                        .is_some_and(|target| target.identity() == &proof.target)
            })
            .ok_or("the selected runtime boundary slot cannot execute during preparation")?;
        let observation = TargetObservation::SelectedRealization {
            target: proof.target.clone(),
            realization: proof.realization.clone(),
        };
        if !self
            .target
            .is_some_and(|target| target.matches_observation(&observation))
        {
            return Err("the checked provider does not match the selected target plan".into());
        }
        if !proof.observations.iter().all(|observation| {
            self.target
                .is_some_and(|target| target.matches_observation(observation))
        }) {
            return Err("the checked provider proof depends on a different target selection".into());
        }
        for dependency in &proof.observations {
            self.record_observation(dependency.clone());
        }
        let implementation = &proof.binding.implementation;
        let application = if let Some(selected) = &proof.application {
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
                return Err("the checked provider does not preserve the selected positional application".into());
            }
            Some(realized)
        } else {
            None
        };
        Ok(SelectedProvider {
            implementation,
            application,
            observation,
        })
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
    let operands = operands.0.borrow();
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
    Ok(Rc::new(CallBindings(RefCell::new(bindings), application)))
}

#[cfg(test)]
mod tests;
