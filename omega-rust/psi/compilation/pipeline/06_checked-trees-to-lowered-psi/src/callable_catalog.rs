//! Prepare callable type identities and publish their source-free contracts.

use crate::shared::lowering_error::LoweringError;
use checked_trees::{CheckedScalarType, CheckedTrees};
use semantic_vocabulary::{CallableContractId, ScalarType};
use typed_trees::callable_values::RuntimeCallableContractHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PreparedCallableTypes {
    sources: Vec<Option<(RuntimeCallableContractHandle, CallableContractId)>>,
}

impl PreparedCallableTypes {
    pub(crate) fn from_checked(checked: &CheckedTrees) -> Result<Self, LoweringError> {
        let mut prepared = Self::default();
        for (source, _) in checked.runtime_callable_contracts.iter() {
            let contract = checked
                .facts
                .callable_contracts
                .contract_for_source(source)
                .map_err(LoweringError::Unsupported)?;
            let ordinal = source.arena_index() as usize;
            if prepared.sources.len() <= ordinal {
                prepared.sources.resize(ordinal + 1, None);
            }
            if prepared.sources[ordinal].is_some() {
                return Err(LoweringError::Unsupported(
                    "callable type has duplicate source ownership",
                ));
            }
            prepared.sources[ordinal] = Some((source, contract.id));
        }
        Ok(prepared)
    }

    pub(crate) fn contract(
        &self,
        source: RuntimeCallableContractHandle,
    ) -> Result<CallableContractId, LoweringError> {
        let Some(Some((retained, id))) = self.sources.get(source.arena_index() as usize) else {
            return Err(LoweringError::Unsupported(
                "callable type has no prepared contract association",
            ));
        };
        if *retained != source {
            return Err(LoweringError::Unsupported(
                "callable type refers to a stale source generation",
            ));
        }
        Ok(*id)
    }

    pub(crate) fn scalar_type(
        &self,
        scalar: CheckedScalarType,
    ) -> Result<ScalarType, LoweringError> {
        match scalar {
            CheckedScalarType::Primitive(primitive) => {
                crate::emission::scalar_types::terminal_scalar_type(primitive)
            }
            CheckedScalarType::Callable(source) => self.contract(source).map(ScalarType::Callable),
        }
    }
}

pub(crate) struct CallableCatalogContext<'scope> {
    pub checked: &'scope CheckedTrees,
    pub scalar_applications:
        &'scope crate::unit::attached_unit::scalar_applications::SourceScalarApplications,
    pub qualifications:
        &'scope crate::expression_preparation::qualifications::PreparedScalarQualifications,
    pub type_ids: &'scope [(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
    pub structural_types: &'scope [terminal_psi::StructuralTypeDeclaration],
    pub domain_ids: &'scope [crate::shared::terminal_identities::StructuralDomainBinding],
    pub service_ids: &'scope [(
        language_semantics::ServiceReachId,
        semantic_vocabulary::ServiceId,
    )],
    pub machine_ids: &'scope [(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
    pub sources: &'scope [symbols::SymbolHandle],
}

pub(crate) struct CallableNamespaceCounters<'scope> {
    pub value: &'scope mut u64,
    pub place: &'scope mut u64,
    pub obligation: &'scope mut u64,
}

pub(crate) fn lower_contract_headers(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    context: &CallableCatalogContext<'_>,
    counters: &mut CallableNamespaceCounters<'_>,
) -> Result<Vec<terminal_psi::TerminalCallableContract>, LoweringError> {
    use crate::scalar_lowering::contracts::{ContractStructuralResult, ContractViewNamespace};
    use crate::shared::terminal_identities::{allocate_dense, contract_id, place_id, value_id};
    use terminal_psi::{BoundaryMachineResult, TerminalMachineResult, ValueDeclaration};

    let checked = context.checked;
    let contract_base = context
        .machine_ids
        .iter()
        .map(|(_, machine)| machine.get())
        .max()
        .unwrap_or(0);
    let mut contracts = Vec::new();
    for row in checked.facts.callable_contracts.contracts() {
        let source = &checked.runtime_callable_contract(row.source).signature;
        let source_parameters = checked.state_signature_parameters(source);
        let plan = row.signature().map_err(LoweringError::Unsupported)?;
        let scalar_parameters = plan
            .scalar_parameters
            .iter()
            .map(|parameter| {
                let source = source_parameters
                    .get(parameter.source_position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "callable scalar formal lost its source position",
                    ))?;
                let qualified = context
                    .qualifications
                    .signature_value_type(checked, source.type_reference)?;
                if qualified.scalar_type
                    != crate::emission::scalar_types::terminal_checked_scalar_type(
                        checked,
                        parameter.scalar_type,
                    )?
                {
                    return Err(LoweringError::Unsupported(
                        "callable scalar formal changes its checked carrier",
                    ));
                }
                Ok(ValueDeclaration {
                    id: value_id(allocate_dense(counters.value)?),
                    scalar_type: qualified.scalar_type,
                    qualifications: qualified.qualifications,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let structural_parameters = crate::unit::attached_unit::lower_unit_parameters(
            identities,
            &plan.structural_parameters,
            context.type_ids,
            context.structural_types,
            context.domain_ids,
            counters.place,
        )?;
        let erased_scalar = plan
            .erased_scalar_parameters
            .iter()
            .map(|parameter| {
                Ok(ValueDeclaration {
                    id: value_id(allocate_dense(counters.value)?),
                    scalar_type: crate::emission::scalar_types::terminal_checked_scalar_type(
                        checked,
                        parameter.scalar_type,
                    )?,
                    qualifications: Default::default(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let erased_proof = plan
            .erased_proof_parameters
            .iter()
            .map(|parameter| {
                Ok(terminal_psi::ErasedProofFormal {
                    source_position: parameter.source_position,
                    type_identity: identities.intern(
                        &checked.type_identity_text(parameter.type_identity).ok_or(
                            LoweringError::Unsupported(
                                "callable erased formal lost its semantic type",
                            ),
                        )?,
                    ),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let scalar_result = if matches!(
            plan.result,
            checked_trees::CheckedBoundaryMachineResultPlan::Scalar(_)
        ) {
            Some(
                context
                    .qualifications
                    .signature_value_type(checked, source.return_type)?,
            )
        } else {
            None
        };
        let result = crate::unit::attached_unit::lower_public_result(
            identities,
            scalar_result,
            &plan.result,
            context.type_ids,
            context.structural_types,
            context.domain_ids,
        )?;
        let proof_result = match &result {
            BoundaryMachineResult::Unit => TerminalMachineResult::Unit,
            BoundaryMachineResult::Scalar(qualified) => {
                TerminalMachineResult::Scalar(ValueDeclaration {
                    id: value_id(allocate_dense(counters.value)?),
                    scalar_type: qualified.scalar_type,
                    qualifications: qualified.qualifications,
                })
            }
            BoundaryMachineResult::Structural(result) => {
                TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
                    place: place_id(allocate_dense(counters.place)?),
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: result.projected_qualifications.clone(),
                    reference_sources:
                        crate::unit::attached_unit::parameters::lower_reference_result_sources(
                            identities,
                            checked,
                            &plan.result_reference_sources,
                            result.structural_type,
                            context.structural_types,
                            &structural_parameters,
                        )?,
                })
            }
        };
        let predicate_parameters = plan
            .structural_parameters
            .iter()
            .zip(&structural_parameters)
            .map(|(source, parameter)| {
                (
                    source.position,
                    terminal_psi::StructuralParameterDeclaration {
                        position: source.position,
                        ..parameter.clone()
                    },
                )
            })
            .collect::<Vec<_>>();
        let views = ContractViewNamespace {
            scalar_applications: context.scalar_applications,
            constant_values: &checked.typed.constant_recipe_closure.values,
            parameters: &predicate_parameters,
            result: match &proof_result {
                TerminalMachineResult::Structural(result) => ContractStructuralResult::Declared {
                    position: u32::try_from(source_parameters.len()).map_err(|_| {
                        LoweringError::Unsupported("callable result position exceeds u32")
                    })?,
                    declaration: result,
                },
                _ => ContractStructuralResult::Absent,
            },
            structural_types: context.structural_types,
        };
        let proof = crate::callable_contracts::lower_runtime_callable_contract(
            identities,
            checked,
            row.source,
            contract_id(contract_base.checked_add(row.id.get()).ok_or(
                LoweringError::Unsupported("callable proof contract identity exceeds range"),
            )?),
            crate::callable_contracts::RuntimeContractNamespace {
                scalar: &scalar_parameters,
                erased_scalar: &erased_scalar,
                erased_proof: &erased_proof,
                scalar_result: proof_result.scalar(),
                views,
            },
            counters.obligation,
        )?;
        let entry_claim_paths = plan.entry_claim_paths.iter().map(|claim| {
            let formal = structural_parameters.get(claim.argument_index as usize)
                .ok_or(LoweringError::Unsupported("callable entry claim has no structural formal"))?;
            Ok(terminal_psi::TerminalDynamicClaimPath { argument_index: claim.argument_index,
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities, &claim.path, formal.structural_type, context.structural_types,
                )? })
        }).collect::<Result<Vec<_>, LoweringError>>()?;
        let result_claim_paths = match &result {
            BoundaryMachineResult::Structural(result) => plan.result_claim_paths.iter().map(|path|
                crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities, path, result.structural_type, context.structural_types,
                )).collect::<Result<Vec<_>, _>>()?,
            _ if plan.result_claim_paths.is_empty() => Vec::new(),
            _ => return Err(LoweringError::Unsupported("callable scalar result carries structural claims")),
        };
        let result_reference_regions =
            crate::unit::dynamic_dispatch::lower_result_reference_regions(
                identities,
                checked,
                &plan.result_reference_sources,
                &result,
                &structural_parameters
                    .iter()
                    .map(|parameter| (parameter.structural_type, parameter.access))
                    .collect::<Vec<_>>(),
                context.type_ids,
                context.structural_types,
            )?;
        contracts.push(terminal_psi::TerminalCallableContract {
            id: row.id,
            identity: identities.intern(&checked.type_identity_text(row.identity).ok_or(
                LoweringError::Unsupported("callable contract lost its normalized identity"),
            )?),
            scalar_parameters,
            structural_parameters,
            parameter_order: crate::unit::attached_unit::parameters::lower_runtime_parameter_order(
                &plan.scalar_parameters,
                &plan.structural_parameters,
            )?,
            result,
            proof_result,
            entry_claim_paths,
            result_claim_paths,
            result_reference_regions,
            proof,
            evidence_contract: Vec::new(),
            envelope: lower_envelope(context, source)?,
        });
    }
    Ok(contracts)
}

fn lower_envelope(
    context: &CallableCatalogContext<'_>,
    signature: &typed_trees::signature::StateSignature,
) -> Result<terminal_psi::TerminalCallableEnvelope, LoweringError> {
    let checked = context.checked;
    let reach = signature.service_reach_row;
    if reach.0 == 0
        || reach.0 as usize > checked.service_reach_rows.len()
        || signature.service_reach_is_installation_bound
        || checked.service_reach_rows.abstract_bound(reach).is_some()
    {
        return Err(LoweringError::Unsupported(
            "runtime callable service ceiling is still open or installation dependent",
        ));
    }
    let service_reach = crate::shared::terminal_identities::lower_service_reach_sorted(
        checked.service_reach_rows.services(reach),
        context.service_ids,
    )?;
    lower_operational_envelope(
        checked,
        context.service_ids,
        checked.state_signature_parameters(signature),
        checked.state_signature_invokes(signature),
        service_reach,
        signature.suspends,
        signature.blocks,
        &signature.termination_guarantee,
    )
}

fn lower_operational_envelope(
    checked: &CheckedTrees,
    service_ids: &[(
        language_semantics::ServiceReachId,
        semantic_vocabulary::ServiceId,
    )],
    parameters: &[typed_trees::signature::StateParameter],
    authored_invocations: &[typed_trees::signature::AuthoredInvocation],
    service_reach: Vec<semantic_vocabulary::ServiceId>,
    suspends: bool,
    blocks: bool,
    progress: &language_semantics::TerminationGuarantee,
) -> Result<terminal_psi::TerminalCallableEnvelope, LoweringError> {
    use crate::shared::terminal_identities::lookup_service_id;
    use terminal_psi::{TerminalCallableInvocation, TerminalCallableTermination};
    let mut invocations = Vec::new();
    for invocation in authored_invocations {
        invocations.push(match invocation.target {
            typed_trees::signature::AuthoredInvocationTarget::Unresolved => {
                return Err(LoweringError::Unsupported(
                    "runtime callable invocation ceiling is unresolved",
                ));
            }
            typed_trees::signature::AuthoredInvocationTarget::Parameter { ordinal, symbol } => {
                let parameter = parameters
                    .get(ordinal as usize)
                    .filter(|parameter| {
                        parameter.symbol == symbol && !parameter.relevance.is_erased()
                    })
                    .ok_or(LoweringError::Unsupported(
                        "runtime callable invocation ceiling lost its exact live formal",
                    ))?;
                let runtime_position = parameters
                    .iter()
                    .take(ordinal as usize)
                    .filter(|parameter| !parameter.relevance.is_erased())
                    .count();
                if !matches!(
                    checked_trees::CheckedScalarType::from_type_reference(
                        &checked.typed,
                        parameter.type_reference
                    ),
                    Some(CheckedScalarType::Callable(_))
                ) {
                    return Err(LoweringError::Unsupported(
                        "runtime invocation ceiling requires a callable scalar formal",
                    ));
                }
                TerminalCallableInvocation::Parameter(u32::try_from(runtime_position).map_err(
                    |_| {
                        LoweringError::Unsupported(
                            "runtime callable invocation position exceeds u32",
                        )
                    },
                )?)
            }
            typed_trees::signature::AuthoredInvocationTarget::Service(symbol) => {
                let source = checked.service_reaches.id_for_symbol(symbol).ok_or(
                    LoweringError::Unsupported(
                        "runtime callable invocation ceiling lost its service declaration",
                    ),
                )?;
                TerminalCallableInvocation::Service(lookup_service_id(service_ids, source)?)
            }
        });
    }
    invocations.sort();
    invocations.dedup();
    let termination = match progress {
        language_semantics::TerminationGuarantee::NoGuarantee => {
            TerminalCallableTermination::NotGuaranteed
        }
        language_semantics::TerminationGuarantee::Terminates { premises }
            if premises.is_empty() =>
        {
            TerminalCallableTermination::Terminates {
                premises: Vec::new(),
            }
        }
        language_semantics::TerminationGuarantee::Terminates { .. } => {
            return Err(LoweringError::Unsupported(
                "runtime callable progress premises need their checked source-free subject and profile plan",
            ));
        }
    };
    Ok(terminal_psi::TerminalCallableEnvelope {
        service_reach,
        invocations,
        suspends,
        blocks,
        termination,
    })
}

pub(crate) fn lower_named_envelope(
    checked: &CheckedTrees,
    service_ids: &[(
        language_semantics::ServiceReachId,
        semantic_vocabulary::ServiceId,
    )],
    source: symbols::SymbolHandle,
) -> Result<terminal_psi::TerminalCallableEnvelope, LoweringError> {
    let machine = checked.machine(source).ok_or_else(|| {
        LoweringError::MachineNotFound(checked.symbols.display_path(source, "::"))
    })?;
    let state = checked
        .machine_states(machine)
        .first()
        .ok_or(LoweringError::Unsupported(
            "selected callable has no named entry state",
        ))?;
    let facts = &checked.facts.service_reaches;
    let reach = facts.for_machine(source).ok_or(LoweringError::Unsupported(
        "selected callable lost its checked service contract",
    ))?;
    let contract = facts
        .plan_for_machine(source)
        .ok_or(LoweringError::Unsupported(
            "selected callable has no checked service ceiling",
        ))?;
    let service_reach =
        crate::unit::attached_unit::parameters::lower_installation_machine_service_ceiling(
            checked,
            source,
            contract,
            language_semantics::ServiceReachSummary {
                direct: reach.inferred_direct,
                transitive: reach.inferred_transitive,
            },
            service_ids,
        )?;
    let operational =
        checked
            .facts
            .operational
            .machine_by_symbol(source)
            .ok_or(LoweringError::Unsupported(
                "selected callable lost its checked operational contract",
            ))?;
    lower_operational_envelope(
        checked,
        service_ids,
        checked.state_parameters(state),
        checked.machine_invokes(machine),
        service_reach,
        operational.published_may_suspend,
        operational.published_may_block,
        machine
            .termination_plan
            .interface
            .published()
            .unwrap_or(&language_semantics::TerminationGuarantee::NoGuarantee),
    )
}

/// One prepared namespace serves construction, storage and invocation alike.
#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedCallables {
    pub(crate) types: PreparedCallableTypes,
    pub(crate) catalog: terminal_psi::TerminalCallableCatalog,
    selections: Vec<
        Option<(
            checked_trees::CheckedScalarComputationHandle,
            semantic_vocabulary::MachineDescriptorId,
        )>,
    >,
}

impl PreparedCallables {
    pub(crate) fn prepare(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        context: &CallableCatalogContext<'_>,
        counters: &mut CallableNamespaceCounters<'_>,
    ) -> Result<Self, LoweringError> {
        use semantic_vocabulary::MachineDescriptorId;
        use typed_trees::type_identity::IndexIdentityArgument;
        let checked = context.checked;
        let mut prepared = Self {
            types: PreparedCallableTypes::from_checked(checked)?,
            catalog: terminal_psi::TerminalCallableCatalog {
                contracts: lower_contract_headers(identities, context, counters)?,
                descriptors: Vec::new(),
                machine_envelopes: Vec::new(),
            },
            selections: Vec::new(),
        };
        for source in context.sources {
            for handle in crate::expression_preparation::computation_graph::machine_computations(
                checked, *source,
            )? {
                let node = checked.facts.values.scalar_computations.nodes.get(handle);
                let checked_trees::CheckedScalarComputationKind::MachineValue {
                    value,
                    machine,
                    state,
                    ..
                } = &node.kind
                else {
                    continue;
                };
                let ordinal = handle.arena_index() as usize;
                if prepared.selections.len() <= ordinal {
                    prepared.selections.resize(ordinal + 1, None);
                }
                if let Some((previous, _)) = prepared.selections[ordinal] {
                    if previous != handle {
                        return Err(LoweringError::Unsupported(
                            "machine selection has a stale computation generation",
                        ));
                    }
                    continue;
                }
                let CheckedScalarType::Callable(contract_source) = node.scalar_type else {
                    return Err(LoweringError::Unsupported(
                        "machine selection lost its checked callable carrier",
                    ));
                };
                let contract = prepared.types.contract(contract_source)?;
                let selected = crate::shared::terminal_identities::lookup_machine_id(
                    context.machine_ids,
                    *machine,
                )?;
                let entry = checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .callable_entry_for_machine(*machine)
                    .ok_or(LoweringError::Unsupported(
                        "machine selection has no admitted named entry",
                    ))?;
                if entry.state != *state {
                    return Err(LoweringError::Unsupported(
                        "machine selection changes its checked named state",
                    ));
                }
                let application =
                    checked
                        .runtime_machine_value_identity(value, &[])
                        .map_err(|reason| {
                            LoweringError::InvalidDeclarationIdentity(reason.to_string())
                        })?;
                let origin = terminal_psi::TerminalMachineDescriptorOrigin {
                    application: identities.intern(&application),
                    conformance: if value.conformance.is_valid() {
                        Some(
                            identities.intern(
                                &checked
                                    .runtime_callable_index_identity(
                                        contract_source,
                                        IndexIdentityArgument::Subject(value.conformance),
                                        &[],
                                    )
                                    .map_err(|reason| {
                                        LoweringError::InvalidDeclarationIdentity(
                                            reason.to_string(),
                                        )
                                    })?,
                            ),
                        )
                    } else {
                        None
                    },
                    requirement: if value.requirement.is_valid() {
                        Some(
                            identities.intern(
                                &checked
                                    .runtime_callable_index_identity(
                                        contract_source,
                                        IndexIdentityArgument::Subject(value.requirement),
                                        &[],
                                    )
                                    .map_err(|reason| {
                                        LoweringError::InvalidDeclarationIdentity(
                                            reason.to_string(),
                                        )
                                    })?,
                            ),
                        )
                    } else {
                        None
                    },
                };
                let machine_ordinal = usize::try_from(selected.get() - 1).map_err(|_| {
                    LoweringError::Unsupported("selected machine ordinal exceeds usize")
                })?;
                if prepared.catalog.machine_envelopes.len() <= machine_ordinal {
                    prepared
                        .catalog
                        .machine_envelopes
                        .resize(machine_ordinal + 1, None);
                }
                if prepared.catalog.machine_envelopes[machine_ordinal].is_none() {
                    prepared.catalog.machine_envelopes[machine_ordinal] =
                        Some(terminal_psi::TerminalMachineEnvelope {
                            machine: selected,
                            envelope: lower_named_envelope(checked, context.service_ids, *machine)?,
                        });
                }
                let descriptor =
                    if let Some(row) = prepared.catalog.descriptors.iter().find(|row| {
                        row.contract == contract && row.machine == selected && row.origin == origin
                    }) {
                        row.id
                    } else {
                        let id = MachineDescriptorId::new(
                            u64::try_from(prepared.catalog.descriptors.len())
                                .ok()
                                .and_then(|count| count.checked_add(1))
                                .ok_or(LoweringError::Unsupported(
                                    "machine descriptor namespace exceeds range",
                                ))?,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "machine descriptor identity is zero",
                        ))?;
                        prepared.catalog.descriptors.push(
                            terminal_psi::TerminalMachineDescriptor {
                                id,
                                contract,
                                machine: selected,
                                origin,
                                refinement_obligations: Vec::new(),
                                refinement_evidence: Vec::new(),
                            },
                        );
                        id
                    };
                prepared.selections[ordinal] = Some((handle, descriptor));
            }
        }
        Ok(prepared)
    }

    pub(crate) fn descriptor(
        &self,
        source: checked_trees::CheckedScalarComputationHandle,
    ) -> Result<&terminal_psi::TerminalMachineDescriptor, LoweringError> {
        let Some(Some((retained, descriptor))) = self.selections.get(source.arena_index() as usize)
        else {
            return Err(LoweringError::Unsupported(
                "machine selection has no prepared descriptor",
            ));
        };
        if *retained != source {
            return Err(LoweringError::Unsupported(
                "machine selection refers to a stale computation generation",
            ));
        }
        self.catalog
            .descriptor(*descriptor)
            .ok_or(LoweringError::Unsupported(
                "machine selection lost its retained descriptor",
            ))
    }
}

pub(crate) fn finalize_descriptor_evidence(
    module: &mut terminal_psi::TerminalModule,
    next_obligation: &mut u64,
) -> Result<(), LoweringError> {
    terminal_semantics::callable_envelopes::validate_callable_operational_refinements(module)
        .map_err(LoweringError::Unsupported)?;
    for ordinal in 0..module.callables.descriptors.len() {
        let descriptor = &module.callables.descriptors[ordinal];
        let count =
            terminal_semantics::calls::callable_refinement::callable_contract_refinement_goals(
                module,
                descriptor.contract,
                descriptor.machine,
            )
            .map_err(LoweringError::Unsupported)?
            .len();
        let mut obligations = Vec::with_capacity(count);
        for _ in 0..count {
            let raw = crate::shared::terminal_identities::allocate_dense(next_obligation)?;
            obligations.push(semantic_vocabulary::ObligationId::new(raw).ok_or(
                LoweringError::Unsupported("callable refinement obligation identity is zero"),
            )?);
        }
        module.callables.descriptors[ordinal].refinement_obligations = obligations;
        let descriptor = module.callables.descriptors[ordinal].id;
        let evidence =
            crate::callable_contracts::derive_descriptor_refinement_evidence(module, descriptor)?;
        module.callables.descriptors[ordinal].refinement_evidence = evidence;
    }
    Ok(())
}
