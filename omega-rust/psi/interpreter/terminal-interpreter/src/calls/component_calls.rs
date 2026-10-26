//! Checked cross-instance calls. Host adapters never enter this result lane.

use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{BoundaryMachineResult, OperationKind, OperationResult};

use crate::effects::results::TerminalEffectResult;
use crate::effects::{AdmittedBoundarySupply, TerminalEffectRejection};
use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::instance::ComponentBindings;
use crate::results::{TerminalExecutionResult, TerminalExecutionStatus};
use crate::values::TerminalScalarValue;

impl<'artifact> TerminalExecution<'artifact> {
    pub(crate) fn execute_component_call(
        &mut self,
        operation: &terminal_psi::Operation,
        arguments: &[TerminalScalarValue],
        meter: &mut TerminalFuelMeter,
        components: &mut ComponentBindings<'_, 'artifact>,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let reject = |reason| TerminalInterpretError::EffectRejected {
            operation: operation.id,
            rejection: TerminalEffectRejection::new(reason),
        };
        let OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            completion_receipts,
            ..
        } = &operation.kind
        else {
            return Err(reject(
                "component dispatch reached another operation".to_owned(),
            ));
        };
        // Dispatch reaches this lane only under an admitted installation
        // supply, so this roster is the receiving host's — the raw lane
        // carries an empty one and can never select a component provider.
        let binding = components.get_mut(boundary).ok_or_else(|| {
            reject("component invocation has no established producing instance binding".to_owned())
        })?;
        let selected = binding.selected();
        let expected_supply = AdmittedBoundarySupply::ComponentProvider {
            provider: selected.provider(),
            provider_boundary: selected.provider_boundary(),
            candidate: selected.candidate(),
        };
        if selected.receiver() != self.program.identity
            || selected.receiving_boundary() != *boundary
            || self.boundary_supplies.get(boundary) != Some(&expected_supply)
        {
            return Err(reject(
                "component invocation differs from the exact receiving installation".to_owned(),
            ));
        }
        if !binding.pending_at(operation.id) {
            let receiver = self.program.verified_boundary_machine(boundary)?;
            let prepared = self.prepare_component_arguments(
                structural_arguments,
                completion_receipts,
                receiver,
                binding.provider_program(),
                selected.provider_boundary(),
                selected.candidate(),
            )?;
            binding
                .prepare(
                    operation.id,
                    arguments,
                    structural_arguments,
                    &prepared.values,
                    self.account.as_ref(),
                )
                .map_err(&reject)?;
            // Admission above executed no child operation. All fallible source
            // checks finished before preparing it; commit the ordinary transfer
            // before allowing that child to consume the original atoms.
            self.live_affine_frontier = prepared.remaining_affine;
            self.live_claims = prepared.remaining_claims;
            for place in prepared.retired_sources {
                self.structural_values
                    .remove(&place)
                    .expect("owned source was staged without parent mutation");
            }
        }
        match binding
            .invoke(operation.id, arguments, structural_arguments, meter)
            .map_err(&reject)?
        {
            Some(TerminalExecutionStatus::SponsorExhausted(exhaustion)) => {
                return Ok(OperationFlow::Yield(
                    TerminalExecutionStatus::SponsorExhausted(exhaustion),
                ));
            }
            Some(_) => {
                // A component crash or domain exit is not normal return. Its
                // custody stays in the producing instance; no result is forged.
                return Err(reject(
                    "component invocation did not return normally".to_owned(),
                ));
            }
            None => {}
        }
        let completed = binding
            .completed()
            .ok_or_else(|| reject("component invocation lost its owned completion".to_owned()))?;
        let selected = binding.selected();
        if completed.provider() != selected.provider()
            || completed.candidate() != selected.candidate()
            || Some(completed.instance_namespace())
                == self.local_structural_identities.exposure_namespace
        {
            return Err(reject(
                "component completion has another provider, entry or instance namespace".to_owned(),
            ));
        }
        let receiver = self
            .program
            .verified_boundary_machine(boundary)?
            .result
            .clone();
        let provider = binding.provider_program();
        let supplied = &provider
            .program
            .verified_boundary_machine(&selected.provider_boundary())?
            .result;
        let mut result_claims = self.live_claims.clone();
        let returned = match (&receiver, supplied, completed.value()) {
            (
                BoundaryMachineResult::Unit,
                BoundaryMachineResult::Unit,
                TerminalExecutionResult::Unit,
            ) => TerminalEffectResult::Unit,
            (
                BoundaryMachineResult::Scalar(left),
                BoundaryMachineResult::Scalar(right),
                TerminalExecutionResult::Scalar(value),
            ) if left == right && value.scalar_type() == left.scalar_type => {
                TerminalEffectResult::Scalar(*value)
            }
            (
                BoundaryMachineResult::Structural(left),
                BoundaryMachineResult::Structural(right),
                TerminalExecutionResult::Structural(returned),
            ) => {
                let types = self.program.structural_types();
                let receiver_type = types
                    .get(&left.structural_type)
                    .ok_or_else(|| reject("component receiver lost its result type".to_owned()))?;
                let provider_types = provider.program.structural_types();
                let provider_type = provider_types
                    .get(&right.structural_type)
                    .ok_or_else(|| reject("component producer lost its result type".to_owned()))?;
                if returned.value.structural_type != right.structural_type
                    || left.multiplicity != right.multiplicity
                    || !left.qualifications.is_empty()
                    || !right.qualifications.is_empty()
                    || !left.projected_qualifications.is_empty()
                    || !right.projected_qualifications.is_empty()
                    || receiver_type.owns_linear_claim
                        != (left.multiplicity == terminal_psi::StructuralMultiplicity::Linear)
                    || provider_type.owns_linear_claim != receiver_type.owns_linear_claim
                    || (!receiver_type.owns_linear_claim
                        && (!receiver_type.no_code_affine_cleanup
                            || !provider_type.no_code_affine_cleanup))
                    || !self
                        .program
                        .catalogs
                        .identities
                        .text(receiver_type.identity)
                        .zip(
                            provider
                                .program
                                .catalogs
                                .identities
                                .text(provider_type.identity),
                        )
                        .is_some_and(|(left, right)| !left.is_empty() && left == right)
                {
                    return Err(reject(
                        "component result lacks exact closed type or transferable Inert custody"
                            .to_owned(),
                    ));
                }
                // The owned completion authenticated an actual host atom from
                // the producing execution. Rehome only the artifact-local type
                // key; retain the same nonrecycled host token and exact payload.
                let mut value = returned.value.clone();
                value.structural_type = left.structural_type;
                let claims = completed.returned_claims().ok_or_else(|| {
                    reject("component return lost its owned claim bindings".to_owned())
                })?;
                if let OperationResult::Structural(result) = &operation.result {
                    if self.structural_values.contains_key(&result.place)
                        || crate::custody::place_is_claimed(&self.live_claims, result.place)
                        || self
                            .live_affine_frontier
                            .iter()
                            .any(|value| value.place == result.place)
                    {
                        return Err(reject(
                            "component receiving result place still owns custody".to_owned(),
                        ));
                    }
                    if receiver_type.owns_linear_claim {
                        if claims.len() != 1
                            || !claims[0].path.is_empty()
                            || returned.claims != [claims[0].claim]
                            || result.claims.len() != 1
                            || !result.claims[0].path.is_empty()
                        {
                            return Err(reject(
                                "linear component result lacks exactly one whole returned claim"
                                    .to_owned(),
                            ));
                        }
                    } else if !claims.is_empty()
                        || !returned.claims.is_empty()
                        || !result.claims.is_empty()
                    {
                        return Err(reject(
                            "nonlinear component result carries unexpected claims".to_owned(),
                        ));
                    }
                    for claim in &result.claims {
                        crate::custody::bind_result_claim(
                            &mut result_claims,
                            claim,
                            result.place,
                            result.multiplicity,
                        )?;
                    }
                } else {
                    return Err(reject(
                        "component structural result has no receiving home".to_owned(),
                    ));
                }
                self.local_structural_identities
                    .reserve_host_at_cursor(&value, self.local_structural_identities.cursor())?;
                TerminalEffectResult::Structural(value)
            }
            _ => {
                return Err(reject(
                    "component completion disagrees with the checked receiving result".to_owned(),
                ));
            }
        };
        let provider = completed.provider();
        let candidate = completed.candidate();
        let instance_namespace = completed.instance_namespace();
        self.commit_boundary_result(&operation.result, &receiver, returned)?;
        self.live_claims = result_claims;
        self.effects
            .push(crate::effects::TerminalEffect::ComponentCall {
                operation: operation.id,
                boundary: *boundary,
                provider,
                candidate,
                instance_namespace,
                effects: binding.acknowledge(),
            });
        Ok(OperationFlow::Advance)
    }
}
