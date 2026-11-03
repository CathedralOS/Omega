//! A checked implementation retains its own result promises under the selected
//! capture telescope. The public requirement is not evidence that the private
//! body establishes a stronger result. This view copies that body's contract,
//! without copying its executable or granting boundary issuance authority.

use super::{AppliedCallableOwner, AppliedCallableSignature};
use crate::{TypedTrees, signature, trait_definition::RetainedTraitScope};
use symbols::SymbolHandle;

impl TypedTrees {
    pub(super) fn prepare_realization_contracts(
        &mut self,
        selected: &crate::typed_trees::ClosedConformanceApplication,
        lexical_owner: SymbolHandle,
        scope: RetainedTraitScope,
    ) -> Result<(), &'static str> {
        let conformance = self
            .conformance_by_symbol(selected.declaration)
            .ok_or("realization contract lost its selected implementation")?;
        if conformance.type_parameters.is_empty() && conformance.lifetime_parameters.is_empty() {
            return Ok(());
        }
        let application = self
            .portable_conformance_application_identity_in_owner(selected, lexical_owner)
            .ok_or("realization contract lost its complete capture identity")?;
        let owner = AppliedCallableOwner::Realization {
            declaration: selected.declaration,
            application,
            lexical_owner,
        };
        let rows = conformance
            .rows
            .iter()
            .filter(|row| row.source.owns_realization())
            .map(|row| (row.realization_machine, row.realization_state))
            .collect::<Vec<_>>();
        let environment = scope
            .scope(self)
            .ok_or("realization contract lost its supplying scope")?
            .environment();
        for (machine_symbol, entry) in rows {
            if self
                .applied_callable_signatures
                .iter()
                .any(|(_, prepared)| prepared.owner == owner && prepared.signature.symbol == entry)
            {
                continue;
            }
            let machine = self
                .machine_by_symbol(machine_symbol)
                .ok_or("realization contract lost its owned machine")?;
            let state = self
                .state_in_machine(machine, entry)
                .ok_or("realization contract lost its exact entry")?;
            let declared = signature::StateSignature {
                symbol: entry,
                name: state.name.clone(),
                lifetime_parameters: machine.lifetime_parameters.clone(),
                type_parameters: machine.type_parameters,
                parameters: state.parameters,
                return_type: state.return_type,
                contracts: machine.contracts,
                invokes: machine.invokes,
                service_reach_row: machine.service_reach_row,
                service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
                suspends: machine.suspends,
                blocks: machine.blocks,
                termination_guarantee: match &machine.termination_plan.interface {
                    language_semantics::TerminationInterface::Published(guarantee) => {
                        guarantee.clone()
                    }
                    _ => language_semantics::TerminationGuarantee::NoGuarantee,
                },
                ..Default::default()
            };
            let mut substitution = self.materialize_trait_application(&environment)?;
            let signature = substitution.signature(self, &declared)?;
            self.applied_callable_signatures
                .insert(AppliedCallableSignature {
                    owner: owner.clone(),
                    signature,
                });
        }
        Ok(())
    }

    /// Only the exact selected implementation supplies this view. Closed
    /// executable instances already have substituted signatures of their own.
    pub fn realization_call_signature(
        &self,
        lexical_owner: SymbolHandle,
        call: &crate::expression::TableCallExpression,
    ) -> Option<&signature::StateSignature> {
        let dispatch = call.static_requirement_dispatch.as_ref()?;
        if call.target_symbol != dispatch.realization_state {
            return None;
        }
        let selections = self.selected_conformance_applications(
            lexical_owner,
            dispatch.selection_application,
            dispatch.evidence_parameter,
            dispatch.requirement,
            dispatch.application_report_fingerprint,
            dispatch.application_commitment,
        );
        let [selection] = selections.as_slice() else {
            return None;
        };
        let application =
            self.portable_conformance_application_identity_in_owner(selection, lexical_owner)?;
        let owner = AppliedCallableOwner::Realization {
            declaration: selection.declaration,
            application,
            lexical_owner,
        };
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| {
                (prepared.owner == owner && prepared.signature.symbol == call.target_symbol)
                    .then_some(&prepared.signature)
            })
    }
}
