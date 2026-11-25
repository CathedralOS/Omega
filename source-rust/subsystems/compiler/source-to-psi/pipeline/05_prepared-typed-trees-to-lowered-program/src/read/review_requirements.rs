//! Retain public requirement contracts before releasing their checked forms.

use super::Program;
use crate::body::{Policy, Signature};
use crate::publication::{
    RequirementInvocation, RequirementReview, RequirementReviewMetadata, RequirementService,
};
use crate::reach::SourceInvocation;
use crate::static_contracts::SourceServiceCeiling;
use crate::source_state::signature::StateSignature;
use crate::source_state::trait_definition::{AppliedTraitScope, TraitDefinition};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
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
                if !owner.is_public && !owner.is_boundary {
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
        if !owner.lifetime_parameters.is_empty()
            || !source.conformance_bounds.is_empty()
            || !source.native_callback_parameters.is_empty()
            || source.is_default
        {
            return Err(
                "public requirement review needs its complete parent lifetime, conformance, default, or callback contract"
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
            .static_envelope(context, source, owner.is_boundary)
            .map_err(|refusal| format!("public requirement operational envelope: {refusal:?}"))?;
        let service = |id| {
            let scope = self.typed.service_effect_scope(id, None)?;
            let declaration = scope.declaration;
            let application = if declaration.type_parameters.is_empty()
                && declaration.lifetime_parameters.is_empty()
            {
                None
            } else {
                Some(
                    scope
                        .portable_application_identity(self.typed)
                        .and_then(|id| self.typed.type_identity_text(id))
                        .ok_or("public requirement lost its service application")?,
                )
            };
            Ok::<_, &'static str>(RequirementService {
                declaration: declaration.symbol,
                application,
            })
        };
        let service_reach = match &envelope.service_reach {
            SourceServiceCeiling::Open => None,
            SourceServiceCeiling::Closed(services) => Some(
                services
                    .iter()
                    .map(|id| service(*id))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        let synchronous_invocations = envelope
            .invocations
            .iter()
            .map(|invocation| match invocation {
                SourceInvocation::Parameter(position) => {
                    Ok(RequirementInvocation::Parameter(*position))
                }
                SourceInvocation::Service(id) => service(*id).map(RequirementInvocation::Service),
            })
            .collect::<Result<_, _>>()?;
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
                &module.types.structural_types,
            )?
            .ok_or("public requirement result has an unresolved lifetime frontier")?
        };
        let mut returned_parameters = Vec::new();
        for group in groups {
            // The scoped result type retains the binder on a caller-chosen
            // borrow, including one carried inside a record or sum. Only a
            // group tied to an input needs a returned-borrow origin.
            if group.inputs.is_empty() {
                continue;
            }
            if group.results.iter().any(|path| !path.is_empty()) {
                return Err(
                    "public requirement review needs projected result lifetime paths".to_owned(),
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
        // The generic check form appends a scalar for each rigid value binder.
        // Those scalars belong to the method's static telescope in package review.
        let (formals, rigid_values) = signature
            .parameters
            .split_at_checked(parameter_types.len())
            .ok_or("public requirement is missing lowered source parameters")?;
        let rigid_count = match machine {
            Some(id) => *self
                .value_parameters
                .get(id)
                .ok_or("public requirement lost its checked value parameter count")?,
            None => 0,
        };
        if rigid_values.len() != rigid_count {
            return Err("public requirement source and lowered parameter counts differ".to_owned());
        }
        if !rigid_values.is_empty() {
            use crate::body::{Qualification, ValueType};
            let binders = self
                .requirement_binders(machine.unwrap_or(context), source)
                .map_err(|_| "public requirement lost its declared value parameters")?;
            if binders.len() != rigid_values.len()
                || binders
                    .iter()
                    .zip(rigid_values)
                    .any(|((_, carrier), formal)| {
                        *formal != ValueType::Scalar(*carrier, Qualification::NONE)
                    })
            {
                return Err(
                    "public requirement changed a rigid value parameter's carrier".to_owned(),
                );
            }
        }
        let mut parameter_qualifications = Vec::new();
        let mut parameter_policies = Vec::new();
        let mut structural_parameter_positions = Vec::new();
        for (position, value) in formals.iter().enumerate() {
            let structural = signature.is_place(position);
            let position = u32::try_from(position)
                .map_err(|_| "public requirement parameter position exceeds u32")?;
            if structural {
                structural_parameter_positions.push(position);
            }
            let qualification = value.qualification();
            if let Some(policy) = qualification.policy {
                let carrier = value
                    .scalar()
                    .ok_or("an arithmetic policy needs a scalar carrier")?;
                parameter_policies.push((position, carrier, Policy::domain(Some(policy))));
            }
            for domain in qualification.domains.members() {
                if structural {
                    // Public structural preconditions currently carry nominal declarations.
                    let definition = self.typed.domain_definition(domain.declaration).ok_or(
                        "public requirement qualification lost its exact domain declaration",
                    )?;
                    if !definition.type_parameters.is_empty()
                        || !definition.index_arguments.is_empty()
                    {
                        return Err("public requirement review needs its complete domain application arguments".to_owned());
                    }
                }
                parameter_qualifications.push((position, domain));
            }
        }
        Ok(RequirementReviewMetadata {
            parameter_types,
            structural_parameter_positions,
            result_type: source.return_type.is_valid().then_some(result),
            lifetime_parameter_count,
            service_reach,
            synchronous_invocations,
            published_crash_routes: signature.declared_crash_routes().map(<[_]>::to_vec),
            returned_parameters,
            parameter_qualifications,
            parameter_policies,
            suspends: envelope.suspends,
            blocks: envelope.blocks,
            terminates,
        })
    }
}
