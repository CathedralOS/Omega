//! Retain public requirement contracts before releasing their checked forms.

use super::Program;
use crate::lowering::body::Signature;
use crate::lowering::publication::{RequirementReview, RequirementReviewMetadata};
use crate::lowering::static_contracts::SourceServiceCeiling;
use crate::source_state::signature::StateSignature;
use crate::source_state::trait_definition::{AppliedTraitScope, TraitDefinition};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::contracts::CrashCeiling;
use terminal_psi::{BoundaryLifetimeDependency, TerminalCallableTermination, TerminalModule};

impl Program<'_> {
    pub(crate) fn review_requirements(
        &self,
        context: MachineId,
        module: &TerminalModule,
    ) -> Vec<RequirementReview> {
        // Read closed schemas from the primary form and generic declarations
        // from their bodyless forms. Generic body instances can repeat headers
        // and must not contribute a second account of the declaration.
        let mut signatures: Vec<(SymbolHandle, &Signature, Option<MachineId>)> = if self.rigid {
            Vec::new()
        } else {
            self.static_contracts
                .schemas
                .iter()
                .flat_map(|schema| &schema.requirements)
                .filter_map(|row| {
                    row.signature
                        .as_ref()
                        .map(|signature| (row.declaration, signature, None))
                })
                .collect()
        };
        if !self.rigid || module.entry.is_none() {
            signatures.extend(self.declared_requirement_headers().filter_map(
                |(declaration, machine)| {
                    self.signatures
                        .get(machine)
                        .map(|signature| (declaration, signature, Some(machine)))
                },
            ));
        }
        signatures
            .into_iter()
            .filter_map(|(declaration, signature, machine)| {
                let (owner, source) = self.typed.trait_machine_signature(declaration)?;
                if !owner.is_public || owner.is_boundary {
                    return None;
                }
                let metadata = self.read_public_requirement_contract(
                    context,
                    module,
                    signature,
                    (owner, source),
                    machine,
                );
                Some(RequirementReview {
                    declaration,
                    metadata,
                })
            })
            .collect()
    }

    fn read_public_requirement_contract(
        &self,
        context: MachineId,
        module: &TerminalModule,
        signature: &Signature,
        definition: (&TraitDefinition, &StateSignature),
        machine: Option<MachineId>,
    ) -> Result<RequirementReviewMetadata, String> {
        let (owner, source) = definition;
        if !owner.type_parameters.is_empty()
            || !owner.lifetime_parameters.is_empty()
            || !source.type_parameters.is_empty()
            || !source.conformance_bounds.is_empty()
            || !source.native_callback_parameters.is_empty()
            || source.is_default
        {
            return Err(
                "public requirement review needs its complete static/default/callback contract"
                    .to_owned(),
            );
        }
        let application = AppliedTraitScope::abstract_scope(owner);
        let exact_sources = self
            .exact_toolchain_sources()
            .map_err(|_| "public requirement review lost exact toolchain source custody")?;
        let (parameter_types, result, elided) = self
            .typed
            .exact_requirement_definition_types(&application, source, exact_sources)
            .ok_or("public requirement has no complete exact signature identity")?;
        let lifetime_parameter_count = source
            .lifetime_parameters
            .len()
            .checked_add(elided)
            .ok_or("public requirement lifetime namespace exceeds usize")?;
        let envelope = self
            .static_envelope(context, source, false)
            .map_err(|refusal| format!("public requirement operational envelope: {refusal:?}"))?;
        let service_reach = match envelope.service_reach {
            SourceServiceCeiling::Open => None,
            SourceServiceCeiling::Closed(ref services) if services.is_empty() => Some(Vec::new()),
            SourceServiceCeiling::Closed(_) => {
                return Err(
                    "public requirement review needs its nonempty service identity roster"
                        .to_owned(),
                );
            }
        };
        if !envelope.invocations.is_empty() {
            return Err(
                "public requirement review needs its synchronous invocation roster".to_owned(),
            );
        }
        let crash_is_open = match &signature.crashes {
            CrashCeiling::Inferred => true,
            CrashCeiling::DeclaredNone => false,
            CrashCeiling::Authored(routes) if routes.is_empty() => false,
            _ => return Err("public requirement review needs its complete crash routes".to_owned()),
        };
        let terminates = match envelope.termination {
            TerminalCallableTermination::NotGuaranteed => false,
            TerminalCallableTermination::Terminates { ref premises } if premises.is_empty() => true,
            _ => {
                return Err(
                    "public requirement review needs its complete progress premises".to_owned(),
                );
            }
        };
        let published_groups = if let Some(machine) = machine {
            let header = module
                .machine(machine)
                .ok_or("public requirement lost its published header")?;
            if !header.blocks.is_empty() || self.symbols.get(machine) != Some(&source.symbol) {
                return Err(
                    "public requirement review does not own a bodyless declaration header"
                        .to_owned(),
                );
            }
            header.contract.result_lifetimes.as_ref()
        } else {
            None
        };
        // Some headers publish reference groups during lowering.
        // Byte views can have declared lifetimes even when that
        // publication has no ordinary reference to record.
        let groups = if let Some(groups) = published_groups {
            groups.clone()
        } else {
            self.declared_result_lifetimes(
                machine.unwrap_or(context),
                signature,
                &module.structural_types,
            )?
            .ok_or("public requirement result has an unresolved lifetime frontier")?
        };
        let mut returned_parameters = Vec::new();
        for group in groups {
            if group.results.iter().any(|path| !path.is_empty()) {
                return Err(
                    "public requirement review needs projected result lifetime paths".to_owned(),
                );
            }
            if group.inputs.is_empty() && !group.results.is_empty() {
                return Err(
                    "public requirement review needs its caller-chosen result lifetime contract"
                        .to_owned(),
                );
            }
            for input in group.inputs {
                let position = match input {
                    BoundaryLifetimeDependency::Referent { argument_index, .. } => argument_index,
                    BoundaryLifetimeDependency::Stored {
                        argument_index,
                        path,
                        ..
                    } if path.is_empty() => argument_index,
                    _ => {
                        return Err(
                            "public requirement review needs projected input lifetime paths"
                                .to_owned(),
                        );
                    }
                };
                returned_parameters.push(position);
            }
        }
        returned_parameters.sort_unstable();
        returned_parameters.dedup();
        let mut parameter_qualifications = Vec::new();
        let mut structural_index = 0u32;
        for (position, value) in signature.parameters.iter().enumerate() {
            if !signature.is_place(position) {
                continue;
            }
            for domain in value.qualification().domains.members() {
                let definition = self
                    .typed
                    .domain_definition(domain.declaration)
                    .ok_or("public requirement qualification lost its exact domain declaration")?;
                if !definition.type_parameters.is_empty() || !definition.index_arguments.is_empty()
                {
                    return Err(
                        "public requirement review needs its complete domain application arguments"
                            .to_owned(),
                    );
                }
                parameter_qualifications.push((structural_index, domain.declaration));
            }
            structural_index = structural_index
                .checked_add(1)
                .ok_or("public requirement structural namespace exceeds u32")?;
        }
        Ok(RequirementReviewMetadata {
            parameter_types,
            result_type: source.return_type.is_valid().then_some(result),
            lifetime_parameter_count,
            service_reach,
            crash_is_open,
            returned_parameters,
            parameter_qualifications,
            suspends: envelope.suspends,
            blocks: envelope.blocks,
            terminates,
        })
    }
}
