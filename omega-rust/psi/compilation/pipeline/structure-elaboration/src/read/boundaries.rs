//! Resolve bodyless requirements by their declaration, retaining the service
//! which owns the call and its independently declared completion behavior.

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use typed_trees::static_applications::StaticMachineApplicationHandle;

use super::{Program, services};
use crate::{Signature, boundaries::BoundarySource};

impl Program<'_> {
    pub(super) fn names_boundary_carrier(
        &self,
        target: SymbolHandle,
        qualifier: SymbolHandle,
    ) -> bool {
        target.is_valid()
            && qualifier.is_valid()
            && (self
                .boundaries
                .rows
                .iter()
                .any(|(_, source)| source.source == target && source.owner == qualifier)
                || self
                    .typed
                    .trait_machine_signature(target)
                    .is_some_and(|(owner, _)| owner.is_boundary && owner.symbol == qualifier))
    }

    pub(super) fn boundary_application_is_empty(
        &self,
        context: MachineId,
        target: SymbolHandle,
        application: StaticMachineApplicationHandle,
    ) -> Checked<bool> {
        let Some((owner, _)) = self.typed.trait_machine_signature(target) else {
            return Ok(false);
        };
        if !owner.is_boundary {
            return Ok(false);
        }
        if !application.is_valid() {
            return Ok(true);
        }
        let actual = self
            .typed
            .expression_table
            .static_machine_application(application);
        if !actual.arguments.is_empty()
            || !actual.selected_machine_arguments.is_empty()
            || !actual.input_selections.is_empty()
            || !actual.bindings.is_empty()
            || !actual.lifetime_bindings.is_empty()
            || actual.conformance_selection.is_some()
            || actual.callable_contract_source.is_valid()
            || (actual.registration_operation.is_valid()
                && actual.registration_operation != target
                && actual.registration_operation != owner.symbol)
        {
            return Err(services::unavailable(
                context,
                "boundary call has an unspent or mismatched static application",
            ));
        }
        Ok(true)
    }

    pub(super) fn boundary_call(
        &mut self,
        context: MachineId,
        target: SymbolHandle,
    ) -> Checked<Option<(BoundaryMachineId, Signature)>> {
        let typed = self.typed;
        let Some((owner, declared)) = typed.trait_machine_signature(target) else {
            return Ok(None);
        };
        if !owner.is_boundary {
            return Ok(None);
        }
        if let Some((id, source)) = self
            .boundaries
            .rows
            .iter()
            .find(|(_, source)| source.source == target)
        {
            return Ok(Some((id, source.signature.clone())));
        }
        if !owner.lifetime_parameters.is_empty() || !owner.type_parameters.is_empty() {
            return Err(services::unavailable(
                context,
                "boundary call needs its complete declaring trait application",
            ));
        }
        let envelope = self.signature_envelope(context, declared)?;
        let service = self
            .typed
            .service_reaches
            .id_for_symbol(owner.symbol)
            .ok_or_else(|| {
                services::unavailable(
                    context,
                    "boundary requirement has no owning service declaration",
                )
            })?;
        let owning_service = services::service(self.typed, context, service)?;
        let declaration = crate::declare::boundary_source(
            self.typed,
            target,
            owner.symbol,
            declared,
            self.typed
                .trait_requirement_callable_identity_text(owner, declared),
        )
        .map_err(|reason| services::unavailable(context, reason))?;
        let completion =
            typed_trees::boundary_completions::boundary_requirement_completion(self.typed, target);
        let (signature, _) = self.boundary_signature(context, declared)?;
        let mut envelope = envelope;
        // Invoking a trait requirement reaches its owner independently of the
        // provider eventually selected for it.
        envelope
            .service_reach
            .extend(services::service_closure(self.typed, context, &[service])?);
        envelope.service_reach.sort();
        envelope.service_reach.dedup();
        let id = self.boundaries.rows.push(BoundarySource {
            source: target,
            owner: owner.symbol,
            signature: signature.clone(),
            envelope,
            completion,
            owning_service: Some(owning_service),
            declaration,
        });
        Ok(Some((id, signature)))
    }
}
