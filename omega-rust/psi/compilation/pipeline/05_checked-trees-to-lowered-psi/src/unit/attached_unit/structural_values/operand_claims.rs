//! Completed operands supply aggregate claims at their exact selected paths.
//! Records and arrays use one custody lookup: published bindings, results and
//! arrivals precede entry-parameter transfers. No constructor mints a child claim.

use super::super::{LoweringError, StructuralMultiplicity, unsupported};
use super::emission;

impl emission::Emission<'_, '_, '_> {
    /// The claims a whole owned operand holds beneath its argument path,
    /// relative to that path. A produced value publishes them on its
    /// operation result, completed binding or owned-move arrival; an entry
    /// parameter's claims are the identities the checker transfers out of it
    /// at this statement.
    pub(super) fn claims_held_by(
        &self,
        argument: &terminal_psi::StructuralArgument,
    ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
        let rebase = |claims: &[terminal_psi::StructuralResultClaimBinding]| {
            claims
                .iter()
                .filter_map(|claim| {
                    claim
                        .path
                        .strip_prefix(argument.path.as_slice())
                        .map(|suffix| terminal_psi::StructuralResultClaimBinding {
                            claim: claim.claim,
                            path: suffix.to_vec(),
                        })
                })
                .collect::<Vec<_>>()
        };
        if let Some((_, binding)) = self
            .operations
            .structural_values
            .iter()
            .find(|(_, binding)| binding.place() == argument.place)
        {
            return Ok(rebase(binding.claims()));
        }
        if let Some(result) = self
            .operations
            .operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place)
        {
            return Ok(rebase(&result.claims));
        }
        if let Some((_, claims)) = self
            .arrival_claims
            .iter()
            .find(|(place, _)| *place == argument.place)
        {
            return Ok(rebase(claims));
        }
        let Some((position, parameter)) = self
            .evaluation
            .structural_parameters
            .iter()
            .find(|(_, parameter)| parameter.place == argument.place)
        else {
            return unsupported("constructor operand has no published claim custody");
        };
        let (_, authored) = crate::declarations::state_declaration(self.checked, self.state)?;
        let source = self
            .checked
            .state_parameters(authored)
            .get(*position as usize)
            .ok_or(LoweringError::Unsupported(
                "constructor operand parameter has no authored source",
            ))?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
            return Ok(Vec::new());
        }
        let mut claims = Vec::new();
        for (_, event) in self.checked.facts.flow.ownership.permissions.iter() {
            if event.machine_symbol != self.machine
                || event.state_symbol != self.state
                || event.root != facts::PlaceRoot::Symbol(source.symbol)
                || event.kind != language_semantics::PermissionEventKind::Transfer
                || event.access != language_semantics::PermissionAccess::Owned
                || event.multiplicity != language_semantics::Multiplicity::Linear
                || !event.obligation_live
                || event.source
                    != (language_semantics::PermissionEventSource::Statement {
                        statement_index: self.statement as usize,
                    })
            {
                continue;
            }
            let path = validation::structural_claim_path(
                &self.checked.typed,
                source.type_reference,
                self.checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments),
            )
            .map_err(LoweringError::Unsupported)?;
            claims.push(terminal_psi::StructuralResultClaimBinding {
                claim: crate::terminal_identities::lookup_claim_id(
                    self.calls.claims.bindings(),
                    event.claim_identity,
                )?,
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    &path,
                    parameter.structural_type,
                    self.structural_types,
                )?,
            });
        }
        Ok(rebase(&claims))
    }
}
