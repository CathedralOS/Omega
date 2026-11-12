/*
Checked-fact custody installed on the assembled `TerminalModule`: access views and lifecycles,
closed reach and conformance applications, external provider candidates, operation crash contracts,
payload memberships, restored reborrow call uses, retained borrow custody, and suspension call plans.
*/

use checked_trees::CheckedTrees;
use language_semantics::Multiplicity;
use lowered_psi::LoweredSourceCallOccurrence;
use semantic_vocabulary::{BoundaryMachineId, ContentPlaceVersion, DomainSemanticId, ValueId};
use terminal_psi::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineResult, OperationKind,
    StructuralAccess, StructuralMultiplicity, TerminalModule, ValueDeclaration,
};

use crate::emission::scalar_types::terminal_scalar_type;
use crate::proofs::{content_conservation, evidence_lowering};
use crate::shared::lowering_error::{LoweringError, invalid_suspension, unsupported};
use crate::unit::attached_unit::checked_unit_boundary_identity;

pub(crate) mod access_lifecycles {

    use checked_trees::CheckedTrees;
    use lowered_psi::LoweredPsi;
    use terminal_psi::{
        OperationKind, TerminalAccessLifecycleOperation, TerminalAccessLifecyclePlan,
        TerminalIdentityTableBuilder,
    };
    use typed_trees::typed_trees::{AccessLifecycleOperation, AccessLifecyclePlan};

    use crate::shared::lowering_error::{LoweringError, unsupported};

    pub(crate) fn retain(
        identities: &mut TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        lowered: &mut LoweredPsi,
    ) -> Result<(), LoweringError> {
        let module = &lowered.semantic_module;
        let mut plans = Vec::new();
        for (_, row) in checked.typed.access_lifecycle_plans.iter() {
            let AccessLifecyclePlan::Evaluated {
                callable,
                operation,
                policy,
                schema,
                policy_machine,
                target,
                placement,
                storage,
                profile_carrier,
                ..
            } = row
            else {
                continue;
            };
            let private_declaration =
                typed_trees::typed_trees::AccessExecutionDeclaration::authenticate(
                    &checked.typed,
                    *callable,
                )
                .map_err(|_| {
                    LoweringError::Unsupported(
                        "access lifecycle lost its authenticated declaration",
                    )
                })?;
            let mut boundaries = Vec::new();
            for occurrence in &lowered.source_call_occurrences {
                if occurrence.source_target != *callable
                    && !private_declaration
                        .is_some_and(|declaration| occurrence.source_target == declaration.state())
                {
                    continue;
                }
                let Some(operation) = occurrence.terminal_site.operation() else {
                    continue;
                };
                let Some(operation) = module
                    .operations()
                    .find(|candidate| candidate.id == operation)
                else {
                    return unsupported("access lifecycle lost its emitted call occurrence");
                };
                if let OperationKind::BoundaryCall { boundary, .. } = operation.kind {
                    boundaries.push(boundary);
                }
            }
            boundaries.sort_unstable();
            boundaries.dedup();
            if boundaries.is_empty() {
                continue;
            }
            let schema_data = checked.typed.data_definition_by_symbol(*schema).ok_or(
                LoweringError::Unsupported("access lifecycle lost its exact schema definition"),
            )?;
            let schema_type = super::access_view_plans::schema_type(checked, module, schema_data)
                .ok_or(LoweringError::Unsupported(
                "access lifecycle lost its emitted schema type",
            ))?;
            let mut identity = |symbol| {
                checked
                    .typed
                    .normalized_hermetic_symbol_identity(symbol)
                    .map(|text| identities.intern(&text))
                    .map_err(|_| {
                        LoweringError::Unsupported("access lifecycle lost declaration identity")
                    })
            };
            let policy_identity = identity(*policy)?;
            let policy_plan_machine_identity = identity(*policy_machine)?;
            let schema_identity = identity(*schema)?;
            let storage = storage_contract(identities, checked, module, storage)?;
            let image_fields = image_fields(
                identities,
                checked,
                module,
                schema_data,
                schema_type,
                placement
                    .access()
                    .plan()
                    .entries()
                    .iter()
                    .map(|entry| entry.field()),
            )?;
            let execution = execution_contract(
                identities,
                checked,
                exact_toolchain_sources,
                lowered,
                *callable,
                *profile_carrier,
            )?;
            for boundary in boundaries {
                plans.push(TerminalAccessLifecyclePlan {
                    boundary,
                    target: target
                        .as_ref()
                        .map(|target| terminal_psi::TargetLayoutInput {
                            target: target.profile.clone(),
                            pointer_size: target.pointer_size,
                            pointer_alignment: target.pointer_alignment,
                            case_tag_size: target.case_tag_size,
                            case_tag_alignment: target.case_tag_alignment,
                            little_endian: target.little_endian,
                        }),
                    operation: match operation {
                        AccessLifecycleOperation::Initialize => {
                            TerminalAccessLifecycleOperation::Initialize
                        }
                        AccessLifecycleOperation::MoveOut => {
                            TerminalAccessLifecycleOperation::MoveOut
                        }
                        AccessLifecycleOperation::View => TerminalAccessLifecycleOperation::View,
                        AccessLifecycleOperation::Relocate => {
                            TerminalAccessLifecycleOperation::Relocate
                        }
                    },
                    execution: execution.clone(),
                    schema: schema_type,
                    image_fields: image_fields.clone(),
                    storage: storage.clone(),
                    policy_identity,
                    policy_plan_machine_identity,
                    schema_identity,
                    placement: placement.clone(),
                });
            }
        }
        plans.sort_unstable_by(|left, right| {
            (
                left.boundary,
                left.target.as_ref().map(|target| target.target.as_str()),
            )
                .cmp(&(
                    right.boundary,
                    right.target.as_ref().map(|target| target.target.as_str()),
                ))
        });
        if plans
            .windows(2)
            .any(|pair| pair[0].boundary == pair[1].boundary && pair[0].target == pair[1].target)
        {
            return unsupported("placement boundary has competing lifecycle plans");
        }
        lowered.semantic_module.access_lifecycle_plans = plans;
        Ok(())
    }

    fn image_fields<'placement>(
        identities: &mut TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        module: &terminal_psi::TerminalModule,
        data: &typed_trees::data::DataDefinition,
        schema: semantic_vocabulary::StructuralTypeId,
        field_names: impl Iterator<Item = &'placement str>,
    ) -> Result<Vec<semantic_vocabulary::StructuralFieldId>, LoweringError> {
        let Some(terminal_psi::StructuralTypeShape::Record { fields, .. }) = module
            .structural_type(schema)
            .map(|declaration| &declaration.shape)
        else {
            return unsupported("placement image lost its exact record schema");
        };
        field_names
            .map(|field_name| {
                let source = checked
                    .typed
                    .data_members(data)
                    .iter()
                    .filter_map(|member| member.as_field())
                    .find(|field| field.name.as_str() == field_name && !field.relevance.is_erased())
                    .ok_or(LoweringError::Unsupported(
                        "placement image lost its normalized source field",
                    ))?;
                let identity = identities.intern(
                    &checked_trees::CheckedMemberIdentity::of_member(
                        source.identity,
                        source.name.clone(),
                    )
                    .spelling(),
                );
                fields
                    .iter()
                    .find(|field| field.identity == identity && !field.relevance.is_erased())
                    .map(|field| field.id)
                    .ok_or(LoweringError::Unsupported(
                        "placement image lost its emitted field correspondence",
                    ))
            })
            .collect()
    }

    fn execution_contract(
        identities: &mut TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        lowered: &LoweredPsi,
        callable: symbols::SymbolHandle,
        profile_carrier: Option<typed_trees::types::TypeReferenceHandle>,
    ) -> Result<terminal_psi::TerminalAccessLifecycleExecution, LoweringError> {
        use terminal_psi::TerminalAccessLifecycleExecution;
        let module = &lowered.semantic_module;
        let declaration = typed_trees::typed_trees::AccessExecutionDeclaration::authenticate(
            &checked.typed,
            callable,
        )
        .map_err(|_| LoweringError::Unsupported("placement primitive declaration is malformed"))?;
        let Some(declaration) = declaration else {
            if checked
                .typed
                .closed_requirement_application(callable)
                .is_none()
            {
                return unsupported(
                    "access lifecycle has neither a requirement nor a compiler execution contract",
                );
            }
            return Ok(TerminalAccessLifecycleExecution::SelectedProvider);
        };
        let profile_data = profile_carrier
            .and_then(|reference| checked.typed.closed_data_definition(reference))
            .ok_or(LoweringError::Unsupported(
                "placement primitive lost its checked profile carrier",
            ))?;
        let profile_type = Some(
            super::access_view_plans::schema_type(checked, module, profile_data).ok_or(
                LoweringError::Unsupported("placement primitive profile type was not emitted"),
            )?,
        );
        let mut profile_requirements = Vec::new();
        let profile_positions: &[usize] = match declaration.operation() {
            AccessLifecycleOperation::Initialize => &[2],
            AccessLifecycleOperation::MoveOut | AccessLifecycleOperation::View => &[1],
            AccessLifecycleOperation::Relocate => &[2, 3],
        };
        for role_position in 0..profile_positions.len() {
            profile_requirements.push(profile_requirement(
                identities,
                checked,
                exact_toolchain_sources,
                lowered,
                declaration
                    .profile_role(&checked.typed, role_position)
                    .map_err(|_| {
                        LoweringError::Unsupported("placement primitive lost its exact supply role")
                    })?,
            )?)
        }
        Ok(TerminalAccessLifecycleExecution::CompilerPrimitive {
            profile_type,
            profile_requirements,
        })
    }

    fn profile_requirement(
        identities: &mut TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        lowered: &LoweredPsi,
        role: typed_trees::types::TypeReferenceHandle,
    ) -> Result<terminal_psi::TerminalAccessProfileRequirement, LoweringError> {
        let typed = &checked.typed;
        let role_identity = typed.type_identity(
            typed_trees::type_identity::TypeIdentityRequest::ordinary(role),
        );
        let is_core = |symbol| {
            typed
                .symbols
                .symbol_source_span(symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_some_and(|source| {
                    symbols::core_source_at(source)
                        == Some(language_core::core_vocabulary::CoreSource::Layout)
                })
        };
        let mut selected = None;
        for occurrence in &lowered.source_call_occurrences {
            let Some((declaration, signature)) =
                typed.trait_machine_signature(occurrence.source_target)
            else {
                continue;
            };
            let template = typed
                .closed_requirement_application(occurrence.source_target)
                .map_or(signature.symbol, |application| application.template);
            if !is_core(declaration.symbol)
                || !is_core(template)
                || typed
                    .normalized_hermetic_symbol_identity(declaration.symbol)
                    .ok()
                    .as_deref()
                    != Some("toolchain::ResourceProfileSupply")
                || typed.symbols.name(template).rsplit("::").next() != Some("profile")
            {
                continue;
            }
            let Some(application) = typed.closed_requirement_application(occurrence.source_target)
            else {
                continue;
            };
            let [supply_role] = application.trait_arguments else {
                return unsupported("profile supply lost its closed role argument");
            };
            if typed.type_identity(typed_trees::type_identity::TypeIdentityRequest::ordinary(
                *supply_role,
            )) != role_identity
            {
                continue;
            }
            let Some(operation) = occurrence.terminal_site.operation() else {
                continue;
            };
            let Some(operation) = lowered
                .semantic_module
                .operations()
                .find(|candidate| candidate.id == operation)
            else {
                return unsupported("profile supply lost its emitted call");
            };
            let OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
                continue;
            };
            let owner = crate::machine_lowering::declarations::nominal(
                identities,
                checked,
                exact_toolchain_sources,
                declaration.symbol,
            )
            .map_err(LoweringError::Unsupported)?;
            let callable = typed.trait_requirement_callable_identity(declaration, signature);
            if !callable.is_valid() {
                return unsupported("profile supply lost its exact overload identity");
            }
            let requirement = terminal_psi::TerminalAccessProfileRequirement {
                boundary,
                owner,
                callable: identities.intern(&typed.callable_identity_text(callable)),
            };
            if selected
                .as_ref()
                .is_some_and(|previous| previous != &requirement)
            {
                return unsupported("profile supply has competing requirement coordinates");
            }
            selected = Some(requirement);
        }
        selected.ok_or(LoweringError::Unsupported(
            "placement initialization requires its core profile supply call",
        ))
    }

    fn storage_contract(
        identities: &mut TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        module: &terminal_psi::TerminalModule,
        storage: &typed_trees::typed_trees::AccessLifecycleStorage,
    ) -> Result<terminal_psi::TerminalAccessStorage, LoweringError> {
        use typed_trees::typed_trees::AccessLifecycleOwner;
        let carrier = |reference| {
            let identity = checked.typed.type_identity_request_text(
                typed_trees::type_identity::TypeIdentityRequest::ordinary(reference),
            );
            module
                .structural_types
                .iter()
                .find(|declaration| {
                    module.identities.text(declaration.identity) == Some(identity.as_str())
                })
                .map(|declaration| declaration.id)
                .ok_or(LoweringError::Unsupported(
                    "checked storage carrier lost its emitted correspondence",
                ))
        };
        let field = |identities: &mut TerminalIdentityTableBuilder,
                     carrier,
                     member: &typed_trees::data::DataField| {
            let identity = identities.intern(
                &checked_trees::CheckedMemberIdentity::of_member(
                    member.identity,
                    member.name.clone(),
                )
                .spelling(),
            );
            module
                .structural_type(carrier)
                .and_then(|declaration| match &declaration.shape {
                    terminal_psi::StructuralTypeShape::Record { fields, .. } => {
                        terminal_psi::field_by_identity(fields, identity)
                    }
                    _ => None,
                })
                .map(|field| field.id)
                .ok_or(LoweringError::Unsupported(
                    "checked storage field lost its emitted correspondence",
                ))
        };
        let domain = |carrier, source: language_semantics::SemanticDomainId| {
            module
                .structural_domains
                .iter()
                .find(|domain| {
                    domain.carrier == carrier && domain.semantic_domain.get() == u64::from(source.0)
                })
                .map(|domain| domain.id)
                .ok_or(LoweringError::Unsupported(
                    "checked storage qualification lost its emitted correspondence",
                ))
        };
        let backing = carrier(storage.carrier)?;
        let owner = match &storage.owner {
            AccessLifecycleOwner::Extent => terminal_psi::TerminalAccessOwner::Extent,
            AccessLifecycleOwner::Run {
                carrier: owner,
                storage: backing,
                count,
                singleton,
            } => {
                let owner = carrier(*owner)?;
                terminal_psi::TerminalAccessOwner::Run {
                    carrier: owner,
                    storage: field(identities, owner, backing)?,
                    count: field(identities, owner, count)?,
                    singleton: singleton.map(|source| domain(owner, source)).transpose()?,
                }
            }
        };
        let occupied_owner = match owner {
            terminal_psi::TerminalAccessOwner::Extent => backing,
            terminal_psi::TerminalAccessOwner::Run { carrier, .. } => carrier,
        };
        Ok(terminal_psi::TerminalAccessStorage {
            carrier: backing,
            owner,
            base: field(identities, backing, &storage.base)?,
            length: field(identities, backing, &storage.length)?,
            granted: domain(backing, storage.granted)?,
            vacant: storage
                .vacant
                .map(|source| domain(backing, source))
                .transpose()?,
            resident: storage
                .occupied
                .map(|source| domain(occupied_owner, source))
                .transpose()?,
        })
    }

    pub(crate) fn storage_fields(
        identities: &mut TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        extent: symbols::SymbolHandle,
        declaration: &terminal_psi::StructuralTypeDeclaration,
    ) -> Result<
        (
            semantic_vocabulary::StructuralFieldId,
            semantic_vocabulary::StructuralFieldId,
        ),
        LoweringError,
    > {
        let typed = &checked.typed;
        if !validation::is_core_extent(typed, extent) {
            return unsupported("placement storage is not the core extent");
        }
        let data = typed
            .data_definition_by_symbol(extent)
            .ok_or(LoweringError::Unsupported(
                "placement extent definition is absent",
            ))?;
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            return unsupported("placement extent is not a record");
        };
        let mut field = |name: &str| {
            let member = typed
                .data_members(data)
                .iter()
                .filter_map(|member| member.as_field())
                .find(|member| member.name.as_str() == name)
                .ok_or(LoweringError::Unsupported(
                    "placement extent lost a source field",
                ))?;
            let identity = identities.intern(
                &checked_trees::CheckedMemberIdentity::of_member(
                    member.identity,
                    member.name.clone(),
                )
                .spelling(),
            );
            terminal_psi::field_by_identity(fields, identity)
                .map(|field| field.id)
                .ok_or(LoweringError::Unsupported(
                    "placement extent lost its emitted field",
                ))
        };
        Ok((field("base")?, field("length")?))
    }
}
pub(crate) mod access_view_inputs {

    use crate::shared::checked_facts::flow_facts;
    use checked_trees::{CheckedAccessViewInput, CheckedTrees};
    use semantic_vocabulary::MachineId;
    use terminal_psi::{StructuralAccess, TerminalAccessViewInput};

    use crate::machine_lowering::declarations::nominal;
    use crate::shared::lowering_error::LoweringError;

    pub(crate) fn lower_access_view_input(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        input: &CheckedAccessViewInput,
        machine: MachineId,
    ) -> Result<TerminalAccessViewInput, LoweringError> {
        let identity = |symbol, missing| {
            checked
                .typed
                .normalized_hermetic_symbol_identity(symbol)
                .map_err(|_| LoweringError::Unsupported(missing))
        };
        let access = match input.reference_access {
            language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
            language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
            language_core::ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
        };
        let policy_identity = identity(
            input.policy,
            "access-view policy has no hermetic declaration identity",
        )?;
        let schema_identity = identity(
            input.schema,
            "access-view schema has no hermetic declaration identity",
        )?;
        let view_identity =
            terminal_psi::canonical_access_view_identity(&policy_identity, &schema_identity);
        let source_machine =
            checked
                .typed
                .machine_by_symbol(input.machine)
                .ok_or(LoweringError::Unsupported(
                    "access-view input lost its source machine",
                ))?;
        let state = typed_trees::state::state_by_symbol(
            checked.typed.machine_states(source_machine),
            input.state,
        )
        .ok_or(LoweringError::Unsupported(
            "access-view input lost its source state",
        ))?;
        let parameters = checked.typed.state_parameters(state);
        let parameter = parameters
            .get(input.position as usize)
            .filter(|parameter| parameter.symbol == input.parameter)
            .ok_or(LoweringError::Unsupported(
                "access-view input lost its exact source formal",
            ))?;
        let entry = flow_facts(checked)
            .terminal_unit_effects
            .callable_entry_for_machine(input.machine)
            .filter(|entry| entry.state == input.state)
            .ok_or(LoweringError::Unsupported(
                "access-view input has no exact checked callable entry",
            ))?;
        let runtime_position = entry
            .structural_parameters
            .iter()
            .position(|parameter| parameter.position == input.position)
            .and_then(|position| u32::try_from(position).ok())
            .ok_or(LoweringError::Unsupported(
                "access-view input has no checked structural formal",
            ))?;
        let carrier = validation::structural_type_identity(
            &checked.typed,
            parameter.type_reference,
            &[],
            &[],
        );
        let carrier_text =
            checked
                .typed
                .type_identity_text(carrier)
                .ok_or(LoweringError::Unsupported(
                    "access-view formal lost its structural carrier identity",
                ))?;
        let carrier_identity = identities.intern(&carrier_text);
        Ok(TerminalAccessViewInput {
            machine,
            position: input.position,
            runtime_position,
            carrier_identity,
            source_machine_identity: nominal(
                identities,
                checked,
                exact_toolchain_sources,
                input.machine,
            )
            .map_err(LoweringError::Unsupported)?
            .identity(),
            source_state_identity: nominal(
                identities,
                checked,
                exact_toolchain_sources,
                input.state,
            )
            .map_err(LoweringError::Unsupported)?
            .identity(),
            source_parameter_identity: identities.intern(&identity(
                input.parameter,
                "access-view parameter has no hermetic declaration identity",
            )?),
            access,
            binding_is_const: input.binding_is_const,
            binding_is_mutable: input.binding_is_mutable,
            view_identity: identities.intern(&view_identity),
            policy_identity: identities.intern(&policy_identity),
            policy_plan_machine_identity: identities.intern(&identity(
                input.policy_plan_machine,
                "access-view plan machine has no hermetic declaration identity",
            )?),
            schema_identity: identities.intern(&schema_identity),
            placement_report_fingerprint: input.placement.identity().compatibility_fingerprint(),
            placement_commitment: input.placement.content_interpretation().commitment(),
        })
    }
}
pub(crate) mod access_view_plans {
    use crate::machine_lowering::declarations::nominal;
    use crate::shared::lowering_error::LoweringError;
    use checked_trees::CheckedTrees;
    use terminal_psi::{
        TerminalAccessFieldPlan, TerminalAccessViewPlan, TerminalAccessor, TerminalAccessorKind,
        TerminalIdentityTableBuilder, TerminalModule,
    };

    pub(crate) fn retain(
        identities: &mut TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        module: &mut TerminalModule,
    ) -> Result<(), LoweringError> {
        let declared_type = |data| schema_type(checked, module, data);
        let mut plans = Vec::new();
        for view in &checked.typed.access_view_plans {
            let data = checked
                .typed
                .data_definition_by_symbol(view.data_symbol)
                .ok_or(LoweringError::Unsupported(
                    "placed plan lost exact view definition",
                ))?;
            let Some(carrier) = declared_type(data) else {
                continue;
            };
            let schema_data = checked
                .typed
                .data_definition_by_symbol(view.schema_symbol)
                .ok_or(LoweringError::Unsupported(
                    "placed plan lost exact schema definition",
                ))?;
            let schema = declared_type(schema_data).ok_or(LoweringError::Unsupported(
                "retained placed plan lost schema carrier",
            ))?;
            let mut fields = Vec::new();
            for field in &view.fields {
                let view_member = checked
                    .typed
                    .data_members(data)
                    .iter()
                    .filter_map(|member| member.as_field())
                    .find(|member| member.symbol == field.view_field_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "access field lost exact view member",
                    ))?;
                let schema_member = checked
                    .typed
                    .data_members(schema_data)
                    .iter()
                    .filter_map(|member| member.as_field())
                    .find(|member| member.symbol == field.field_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "access field lost exact schema member",
                    ))?;
                let view_member_identity = checked_trees::CheckedMemberIdentity::of_member(
                    view_member.identity,
                    view_member.name.clone(),
                )
                .spelling();
                let accessor = module
                    .structural_type(carrier)
                    .and_then(|declaration| match &declaration.shape {
                        terminal_psi::StructuralTypeShape::Record { fields, .. } => fields
                            .iter()
                            .find(|candidate| {
                                module.identities.text(candidate.identity)
                                    == Some(view_member_identity.as_str())
                            })
                            .and_then(|candidate| candidate.field_type.as_structural()),
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "retained access field lost its exact structural projection shape",
                    ))?;
                let mut operations = Vec::new();
                for target in &field.accessor_targets {
                    let identity = nominal(identities, checked, sources, target.state_symbol)
                        .map_err(LoweringError::Unsupported)?;
                    let machine_identity =
                        nominal(identities, checked, sources, target.machine_symbol)
                            .map_err(LoweringError::Unsupported)?;
                    if let Some(boundary) = module.boundary_machines.iter().find(|boundary| {
                        boundary.owner == identity || boundary.owner == machine_identity
                    }) {
                        let kind = match target.operation.as_str() {
                            "read" => TerminalAccessorKind::Read,
                            "take" => TerminalAccessorKind::Take,
                            "write" => TerminalAccessorKind::Write,
                            _ => {
                                return Err(LoweringError::Unsupported(
                                    "access field has unknown normalized operation",
                                ));
                            }
                        };
                        operations.push(TerminalAccessor {
                            boundary: boundary.id,
                            kind,
                        });
                    }
                }
                operations.sort_by_key(|operation| operation.boundary);
                fields.push(TerminalAccessFieldPlan {
                    access_slot: field.access_key.slot(),
                    view_field: identities.intern(
                        &checked_trees::CheckedMemberIdentity::of_member(
                            view_member.identity,
                            view_member.name.clone(),
                        )
                        .spelling(),
                    ),
                    schema_field: identities.intern(
                        &checked_trees::CheckedMemberIdentity::of_member(
                            schema_member.identity,
                            schema_member.name.clone(),
                        )
                        .spelling(),
                    ),
                    accessor,
                    operations,
                });
            }
            fields.sort_by_key(|field| field.access_slot);
            let policy = checked
                .typed
                .normalized_hermetic_symbol_identity(view.policy_symbol)
                .map_err(|_| LoweringError::Unsupported("placed plan lost policy identity"))?;
            let schema_identity = checked
                .typed
                .normalized_hermetic_symbol_identity(view.schema_symbol)
                .map_err(|_| LoweringError::Unsupported("placed plan lost schema identity"))?;
            plans.push(TerminalAccessViewPlan {
                carrier,
                schema,
                view_identity: identities.intern(&terminal_psi::canonical_access_view_identity(
                    &policy,
                    &schema_identity,
                )),
                policy_identity: identities.intern(&policy),
                policy_plan_machine_identity: identities.intern(
                    &checked
                        .typed
                        .normalized_hermetic_symbol_identity(view.policy_plan_machine_symbol)
                        .map_err(|_| {
                            LoweringError::Unsupported("placed plan lost policy machine identity")
                        })?,
                ),
                schema_identity: identities.intern(&schema_identity),
                placement: view.placement.clone(),
                fields,
            });
        }
        plans.sort_by_key(|plan| plan.carrier);
        module.access_view_plans = plans;
        Ok(())
    }

    pub(crate) fn schema_type(
        checked: &CheckedTrees,
        module: &TerminalModule,
        data: &typed_trees::data::DataDefinition,
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        let identity =
            if let Some(application) = data.generic_instance {
                checked.typed.type_identity(
                    typed_trees::type_identity::TypeIdentityRequest::ordinary(application),
                )
            } else {
                checked
                    .typed
                    .normalized_nominal_type_identity(data.symbol)?
            };
        let text = checked.typed.type_identity_text(identity)?;
        module
            .structural_types
            .iter()
            .find(|declaration| module.identities.text(declaration.identity) == Some(text.as_str()))
            .map(|declaration| declaration.id)
    }
}
pub(crate) mod closed_reach_applications {

    use checked_trees::CheckedTrees;
    use checked_trees::data::{
        MachineParameterContract, MachineParameterContractView, TypeParameterKind,
    };
    use language_semantics::ServiceReachId;
    use lowered_psi::LoweredSourceCallOccurrence;
    use semantic_vocabulary::{MachineId, ServiceId};
    use symbols::SymbolHandle;
    use terminal_psi::{
        ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
        ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema, ServiceDeclaration,
        TerminalModule,
    };

    use crate::shared::direct_calls::direct_call_parts;
    use crate::shared::lowering_error::LoweringError;
    use crate::shared::lowering_error::unsupported;
    use crate::shared::terminal_identities::hex_text;

    struct ReachApplicationView<'a> {
        template: SymbolHandle,
        instance: SymbolHandle,
        parameters: Vec<checked_trees::data::TypeParameter>,
        parameter_aliases: Vec<(SymbolHandle, SymbolHandle)>,
        invocation_start: usize,
        capture: Option<&'a typed_trees::typed_trees::CapturedMachineApplication>,
        normalized_template_identity: &'a str,
        type_argument_identities: Vec<String>,
        const_argument_identities: Vec<String>,
        machine_arguments: Vec<SymbolHandle>,
        machine_contracts: Vec<[u8; 32]>,
        evidence_bindings: Vec<(SymbolHandle, String)>,
    }

    impl ReachApplicationView<'_> {
        fn position(&self, symbol: SymbolHandle) -> Option<usize> {
            let symbol = self
                .parameter_aliases
                .iter()
                .find_map(|(original, applied)| (*original == symbol).then_some(*applied))
                .unwrap_or(symbol);
            typed_trees::data::type_parameter_position(&self.parameters, symbol)
        }
    }

    fn capture_view<'a>(
        checked: &'a CheckedTrees,
        capture: &'a typed_trees::typed_trees::CapturedMachineApplication,
    ) -> Result<ReachApplicationView<'a>, LoweringError> {
        use checked_trees::CheckedCaptureArgumentValue;
        let owner = checked
            .conformance_by_symbol(capture.conformance_application.declaration)
            .ok_or(LoweringError::Unsupported(
                "closed capture lost its selecting declaration",
            ))?;
        let parameters = checked.conformance_type_parameters(owner);
        let inputs = checked
            .facts
            .executable_bindings
            .captures
            .iter()
            .find(|inputs| inputs.instance == capture.instance)
            .ok_or(LoweringError::Unsupported(
                "closed capture lost its checked operands",
            ))?;
        let bindings = inputs.arguments.as_slice();
        if bindings.len() != parameters.len() {
            return unsupported("closed capture lost its complete typed substitution");
        }
        let mut types = Vec::new();
        let mut constants = Vec::new();
        for parameter in parameters {
            let mut matches = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let (Some(binding), None) = (matches.next(), matches.next()) else {
                return unsupported("closed capture has no unique exact parameter binding");
            };
            match &binding.value {
                CheckedCaptureArgumentValue::Type(identity) => {
                    types.push(checked.type_identity_text(*identity).ok_or(
                        LoweringError::Unsupported("closed capture type has no checked identity"),
                    )?);
                }
                CheckedCaptureArgumentValue::Const(identity) => {
                    constants.push(checked.type_identity_text(*identity).ok_or(
                        LoweringError::Unsupported("closed capture has an open const operand"),
                    )?);
                }
                CheckedCaptureArgumentValue::Machine => {}
                CheckedCaptureArgumentValue::RuntimeValue => {
                    return unsupported("closed capture needs a static value operand");
                }
                CheckedCaptureArgumentValue::Conformance => {
                    return unsupported("closed capture has no conformance-parameter route");
                }
            }
        }
        let identity = checked
            .facts
            .specialization_identities
            .for_instance(capture.instance)
            .ok_or(LoweringError::Unsupported(
                "closed capture lost its checked identity",
            ))?;
        Ok(ReachApplicationView {
            template: capture.template,
            instance: capture.instance,
            parameters: parameters.to_vec(),
            parameter_aliases: capture.parameters.clone(),
            invocation_start: parameters.len(),
            capture: Some(capture),
            normalized_template_identity: &capture.normalized_template_identity,
            type_argument_identities: types,
            const_argument_identities: constants,
            machine_arguments: inputs.machines.clone(),
            machine_contracts: identity.machine_argument_contract_commitments.to_vec(),
            evidence_bindings: Vec::new(),
        })
    }

    fn application_views<'a>(
        checked: &'a CheckedTrees,
        sources: &[(SymbolHandle, MachineId)],
    ) -> Result<Vec<ReachApplicationView<'a>>, LoweringError> {
        let mut applications = Vec::with_capacity(
            checked.machine_specializations.len() + checked.captured_machine_applications.len(),
        );
        for application in &checked.machine_specializations {
            if exact_machine(sources, application.instance)?.is_none() {
                continue;
            }
            let parameters = checked
                .data_type_parameters
                .span(application.template_parameters)
                .ok_or(LoweringError::Unsupported(
                    "closed invocation has a stale telescope",
                ))?;
            let identity = checked
                .facts
                .specialization_identities
                .for_instance(application.instance)
                .ok_or(LoweringError::Unsupported(
                    "closed invocation lost its checked identity",
                ))?;
            let mut captures = checked
                .captured_machine_applications
                .iter()
                .filter(|(_, capture)| capture.instance == application.template)
                .map(|(_, capture)| capture);
            let mut view = if let Some(capture) = captures.next() {
                if captures.next().is_some() {
                    return unsupported("closed invocation has ambiguous capture ownership");
                }
                capture_view(checked, capture)?
            } else {
                ReachApplicationView {
                    template: application.template,
                    instance: application.instance,
                    parameters: Vec::new(),
                    parameter_aliases: Vec::new(),
                    invocation_start: 0,
                    capture: None,
                    normalized_template_identity: &application.normalized_template_identity,
                    type_argument_identities: Vec::new(),
                    const_argument_identities: Vec::new(),
                    machine_arguments: Vec::new(),
                    machine_contracts: Vec::new(),
                    evidence_bindings: Vec::new(),
                }
            };
            view.instance = application.instance;
            view.evidence_bindings = closed_evidence_bindings(checked, application)?;
            view.normalized_template_identity = &application.normalized_template_identity;
            view.parameters.extend_from_slice(parameters);
            view.type_argument_identities
                .extend_from_slice(&application.type_argument_identities);
            view.const_argument_identities
                .extend_from_slice(&application.const_argument_identities);
            view.machine_arguments
                .extend_from_slice(&application.machine_arguments);
            view.machine_contracts
                .extend_from_slice(&identity.machine_argument_contract_commitments);
            applications.push(view);
        }
        for (_, capture) in checked.captured_machine_applications.iter() {
            if exact_machine(sources, capture.instance)?.is_some() {
                applications.push(capture_view(checked, capture)?);
            }
        }
        Ok(applications)
    }

    fn closed_evidence_bindings(
        checked: &CheckedTrees,
        application: &typed_trees::typed_trees::MachineSpecialization,
    ) -> Result<Vec<(SymbolHandle, String)>, LoweringError> {
        let template = checked
            .machine(application.template)
            .ok_or(LoweringError::Unsupported(
                "closed reach evidence lost its original machine",
            ))?;
        let bounds = &template.conformance_bounds;
        if bounds.len() != application.conformance_arguments.len()
            || bounds.len() != application.conformance_applications.len()
        {
            return unsupported("closed reach evidence lost its complete substitution");
        }
        bounds
            .iter()
            .enumerate()
            .map(|(position, bound)| {
                let selected_application = &application.conformance_applications[position];
                let declaration = declaration_identity(checked, selected_application.declaration);
                let commitment = hex_text(&selected_application.commitment.as_bytes());
                Ok((
                    bound.binder,
                    format!(
                        "closed-reach-source-conformance-v1({}:{}|{})",
                        declaration.len(),
                        declaration,
                        commitment
                    ),
                ))
            })
            .collect()
    }

    fn parameter_kind(parameter: &checked_trees::data::TypeParameter) -> u8 {
        match &parameter.kind {
            TypeParameterKind::Type => 0,
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => 1,
            TypeParameterKind::Proposition { .. } => 2,
            TypeParameterKind::Machine {
                contract: MachineParameterContract::RequirementIdentity,
            } => 4,
            TypeParameterKind::Machine { .. } => 3,
            TypeParameterKind::Implementation => 5,
        }
    }

    fn requirement_arguments(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        application: &ReachApplicationView<'_>,
        contract: &MachineParameterContract,
    ) -> Result<(Vec<u8>, Vec<terminal_psi::TerminalIdentityIndex>), LoweringError> {
        let MachineParameterContract::Nominal {
            trait_definition,
            trait_arguments,
            ..
        } = contract
        else {
            return Ok((Vec::new(), Vec::new()));
        };
        let declaration = checked
            .trait_definition_by_symbol(*trait_definition)
            .ok_or(LoweringError::Unsupported(
                "closed reach requirement lost its exact trait",
            ))?;
        let parameters = checked.trait_type_parameters(declaration);
        if parameters.len() != trait_arguments.len() {
            return unsupported("closed reach requirement lost its full trait argument telescope");
        }
        let mut binders = application
            .parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| (parameter.symbol, format!("reach-slot({position})")))
            .collect::<Vec<_>>();
        binders.extend(application.evidence_bindings.iter().cloned());
        for (original, applied) in &application.parameter_aliases {
            if let Some((symbol, name)) = binders
                .iter()
                .find(|(symbol, _)| symbol == applied || symbol == original)
            {
                let other = if symbol == applied {
                    *original
                } else {
                    *applied
                };
                let name = name.clone();
                if !binders.iter().any(|(symbol, _)| *symbol == other) {
                    binders.push((other, name));
                }
            }
        }
        let arguments = trait_arguments
            .iter()
            .map(|argument| {
                let text = checked.type_identity_request_text(
                    typed_trees::type_identity::TypeIdentityRequest {
                        type_reference: *argument,
                        binders: &binders,
                        substitutions: &[],
                        qualification:
                            typed_trees::type_identity::TypeIdentityQualification::PackageQualified,
                    },
                );
                if text.is_empty() {
                    return unsupported("closed reach requirement has an empty argument identity");
                }
                Ok(identities.intern(&text))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        Ok((parameters.iter().map(parameter_kind).collect(), arguments))
    }

    pub(crate) fn retain_closed_reach_applications(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        source_machines: &[(SymbolHandle, MachineId)],
        occurrences: &[LoweredSourceCallOccurrence],
        module: &mut TerminalModule,
    ) -> Result<(), LoweringError> {
        let applications = application_views(checked, source_machines)?;
        let operational = &checked.facts.operational;
        let inferred = &checked.facts.service_reaches;
        for specialization in &applications {
            let checked_identity = checked
                .facts
                .specialization_identities
                .for_instance(specialization.instance)
                .ok_or(LoweringError::Unsupported(
                    "closed reach application lost its checked specialization identity",
                ))?;
            let Some(owner) = exact_machine(source_machines, specialization.instance)? else {
                continue;
            };
            let parameters = &specialization.parameters;
            if u32::try_from(parameters.len()).is_err() {
                return unsupported(
                    "closed reach application lost its complete portable telescope",
                );
            }
            let template =
                inferred
                    .for_machine(specialization.template)
                    .ok_or(LoweringError::Unsupported(
                        "closed reach application lost the original template dependency",
                    ))?;
            let instance =
                inferred
                    .for_machine(specialization.instance)
                    .ok_or(LoweringError::Unsupported(
                        "closed reach application lost its checked owner row",
                    ))?;
            let mut dependencies = Vec::new();
            for dependency in inferred
                .dependency_parameters
                .span_or_empty(template.dependency.parameters)
            {
                let position =
                    specialization
                        .position(*dependency)
                        .ok_or(LoweringError::Unsupported(
                            "closed reach dependency is outside its full telescope",
                        ))?;
                dependencies.push(u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("closed reach telescope exceeds portable positions")
                })?);
            }
            dependencies.sort_unstable();
            let mut telescope = Vec::with_capacity(parameters.len());
            let mut types = specialization.type_argument_identities.iter();
            let mut constants = specialization.const_argument_identities.iter();
            let mut machines = specialization
                .machine_arguments
                .iter()
                .zip(specialization.machine_contracts.iter());
            for parameter in parameters {
                telescope.push(match &parameter.kind {
                    TypeParameterKind::Type => ClosedReachParameter::Type {
                        argument: types
                            .next()
                            .ok_or(LoweringError::Unsupported(
                                "closed reach telescope lost a type argument",
                            ))?
                            .clone(),
                    },
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                        ClosedReachParameter::Const {
                            argument: constants
                                .next()
                                .ok_or(LoweringError::Unsupported(
                                    "closed reach telescope lost a const argument",
                                ))?
                                .clone(),
                        }
                    }
                    TypeParameterKind::Machine { contract } => {
                        let (selected, commitment) =
                            machines.next().ok_or(LoweringError::Unsupported(
                                "closed reach telescope lost a selected machine contract",
                            ))?;
                        let (selected_machine, _) = checked
                            .machines()
                            .iter()
                            .find_map(|machine| {
                                typed_trees::state::state_by_symbol(
                                    checked.machine_states(machine),
                                    *selected,
                                )
                                .map(|state| (machine, state))
                            })
                            .ok_or(LoweringError::Unsupported(
                                "closed reach selection has no exact callable owner",
                            ))?;
                        let reach = checked
                            .facts
                            .service_reaches
                            .for_machine(selected_machine.symbol)
                            .ok_or(LoweringError::Unsupported(
                                "closed reach selection has no checked contract row",
                            ))?;
                        let callee = exact_machine(source_machines, selected_machine.symbol)?;
                        let schema =
                            if checked.machine_type_parameters(selected_machine).is_empty() {
                                None
                            } else {
                                checked
                                    .machine_specializations
                                    .iter()
                                    .find(|application| {
                                        application.template == selected_machine.symbol
                                            && source_machines
                                                .iter()
                                                .any(|(source, _)| *source == application.instance)
                                    })
                                    .map(|application| -> Result<ClosedReachSchema, LoweringError> {
                                        let identity = checked.facts.specialization_identities
                                        .for_instance(application.instance)
                                        .ok_or(LoweringError::Unsupported(
                                            "closed reach schema lost its checked template commitment",
                                        ))?;
                                        Ok(ClosedReachSchema {
                                            template_identity: identities
                                                .intern(&application.normalized_template_identity),
                                            template_commitment: identity
                                                .template_contract_commitment
                                                .as_bytes(),
                                        })
                                    })
                                    .transpose()?
                            };
                        let view = checked.machine_parameter_contract_view(contract).ok_or(
                            LoweringError::Unsupported(
                                "closed reach binder has no retained callable contract",
                            ),
                        )?;
                        let nominal_requirement = match view {
                            MachineParameterContractView::Structural(_) => None,
                            MachineParameterContractView::Nominal {
                                trait_definition,
                                requirement,
                            } => Some(format!(
                                "{}|{}",
                                declaration_identity(checked, trait_definition.symbol),
                                checked.trait_requirement_callable_identity_text(
                                    trait_definition,
                                    requirement
                                ),
                            )),
                        };
                        let upper_bound = retain_services(
                            identities,
                            checked,
                            checked
                                .service_reach_rows
                                .services(view.signature().service_reach_row),
                            module,
                        )?;
                        let selected_reach = retain_services(
                            identities,
                            checked,
                            checked.facts.service_reaches.rows.services(reach.effective),
                            module,
                        )?;
                        let (requirement_parameter_kinds, requirement_arguments) =
                            requirement_arguments(identities, checked, specialization, contract)?;
                        let upper_operational = validation::signature_operational(checked, view.signature());
                        let selected_operational = operational.machine_by_symbol(selected_machine.symbol).ok_or(LoweringError::Unsupported("closed selection lost its published operational contract"))?;
                        ClosedReachParameter::Machine(ClosedReachMachineBinding {
                            nominal_requirement,
                            requirement_parameter_kinds,
                            requirement_arguments,
                            upper_bound,
                            upper_bound_may_suspend: upper_operational.may_suspend,
                            upper_bound_may_block: upper_operational.may_block,
                            selected_may_suspend: selected_operational.published_may_suspend,
                            selected_may_block: selected_operational.published_may_block,
                            selected_identity: identities
                                .intern(&selected_static_machine_identity(checked, *selected)?),
                            selected_contract_commitment: *commitment,
                            selected_reach,
                            callee,
                            schema,
                        })
                    }
                    TypeParameterKind::Proposition { .. } => {
                        return unsupported(
                            "closed reach executable proposition argument is unavailable",
                        );
                    }
                    TypeParameterKind::Implementation => {
                        return unsupported(
                            "closed reach machine telescope retains a trait-only implementation binder",
                        );
                    }
                });
            }
            if types.next().is_some() || constants.next().is_some() || machines.next().is_some() {
                return unsupported("closed reach arguments differ from the complete telescope");
            }
            let mut calls = Vec::new();
            let caller = operational
                .machine_by_symbol(specialization.instance)
                .ok_or(LoweringError::Unsupported(
                    "closed reach application has no operational owner",
                ))?;
            for state in operational.states.span_or_empty(caller.states) {
                for call in operational.calls.span_or_empty(state.calls) {
                    if !call.static_machine_parameter.is_valid() {
                        continue;
                    }
                    let binder = specialization
                        .position(call.static_machine_parameter)
                        .ok_or(LoweringError::Unsupported(
                            "closed reach call lost its original binder",
                        ))?;
                    let statement_matches = occurrences
                        .iter()
                        .filter(|occurrence| {
                            occurrence.matches_statement(
                                state.symbol,
                                call.statement_index,
                                call.call_ordinal,
                            ) && resolved_source_entry(checked, occurrence.source_target)
                                == Some(call.target_state_symbol)
                        })
                        .collect::<Vec<_>>();
                    let owner_matches = statement_matches
                        .iter()
                        .copied()
                        .filter(|occurrence| {
                            module.machine(owner).is_some_and(|machine| {
                                machine
                                    .module_invocations()
                                    .any(|invocation| invocation.site == occurrence.terminal_site)
                            })
                        })
                        .collect::<Vec<_>>();
                    let matches = if owner_matches.is_empty() {
                        statement_matches
                    } else {
                        owner_matches
                    };
                    if matches.is_empty() {
                        continue;
                    }
                    let application = match &telescope[binder] {
                        ClosedReachParameter::Machine(binding) if binding.schema.is_some() => {
                            closed_call_application(
                                identities,
                                checked,
                                source_machines,
                                call.target_state_symbol,
                            )?
                        }
                        _ => None,
                    };
                    for occurrence in matches {
                        let site = occurrence.terminal_site;
                        let emitted = module
                            .machine(owner)
                            .and_then(|machine| {
                                machine
                                    .module_invocations()
                                    .find(|invocation| invocation.site == site)
                            })
                            .ok_or(LoweringError::Unsupported(
                                "closed reach call lost its exact emitted invocation",
                            ))?;
                        if emitted.inputs.callee.get() == 0 {
                            return unsupported("closed reach call has no module callee");
                        }
                        calls.push(ClosedReachCall {
                            site,
                            binder: binder as u32,
                            application: application.clone(),
                        });
                    }
                }
            }
            calls.sort_by_key(|call| call.site);
            if calls.windows(2).any(|pair| pair[0].site == pair[1].site) {
                return unsupported("closed reach call has ambiguous emitted occurrences");
            }
            let mut fixed = retain_services(
                identities,
                checked,
                inferred.rows.services(template.dependency.concrete),
                module,
            )?;
            fixed.extend(retain_services(
                identities,
                checked,
                inferred.rows.services(instance.cleanup_contribution),
                module,
            )?);
            fixed.sort();
            fixed.dedup();
            let terminal_owner = module
                .machines
                .iter_mut()
                .find(|machine| machine.id == owner)
                .ok_or(LoweringError::Unsupported(
                    "closed reach source owner has no Terminal machine",
                ))?;
            let mut row = fixed.clone();
            for dependency in &dependencies {
                let Some(ClosedReachParameter::Machine(binding)) =
                    telescope.get(*dependency as usize)
                else {
                    return unsupported("closed reach dependency is not a machine binder");
                };
                if binding.nominal_requirement.is_none() {
                    return unsupported("closed reach dependency refers to a structural binder");
                }
                row.extend(&binding.selected_reach);
            }
            row.sort();
            row.dedup();
            for call in &calls {
                let Some(ClosedReachParameter::Machine(_)) = telescope.get(call.binder as usize)
                else {
                    return unsupported("closed reach consumer does not select a machine binder");
                };
                match call.site {
                    semantic_vocabulary::InvocationSite::Operation(identity) => {
                        let operation = terminal_owner
                            .blocks
                            .iter_mut()
                            .flat_map(|block| &mut block.operations)
                            .find(|operation| operation.id == identity)
                            .ok_or(LoweringError::Unsupported(
                                "closed reach consumer has no exact owner operation",
                            ))?;
                        if direct_call_parts(&operation.kind).is_none() {
                            return unsupported(
                                "closed reach consumer is not a direct machine call",
                            );
                        }
                        operation.static_reach_binding = Some(call.binder);
                    }
                    semantic_vocabulary::InvocationSite::InvokeNoNormal(identity) => {
                        let binding = terminal_owner
                            .blocks
                            .iter_mut()
                            .find_map(|block| match &mut block.terminator {
                                terminal_psi::Terminator::InvokeNoNormal {
                                    edge,
                                    static_reach_binding,
                                    ..
                                } if *edge == identity => Some(static_reach_binding),
                                _ => None,
                            })
                            .ok_or(LoweringError::Unsupported(
                                "closed reach consumer has no exact invocation edge",
                            ))?;
                        *binding = Some(call.binder);
                    }
                    semantic_vocabulary::InvocationSite::ExternalCompletion(_) => {
                        return unsupported(
                            "closed reach consumer is not an ordinary module invocation",
                        );
                    }
                }
            }
            let capture = specialization
                .capture
                .map(|capture| {
                    let receipt = checked
                        .facts
                        .specialization_identities
                        .for_instance(capture.instance)
                        .ok_or(LoweringError::Unsupported(
                            "closed capture lost its checked receipt",
                        ))?;
                    let invocation = telescope.split_off(specialization.invocation_start);
                    let capture_telescope = std::mem::replace(&mut telescope, invocation);
                    Ok::<_, LoweringError>(terminal_psi::ClosedReachCapture {
                        template_identity: identities.intern(&capture.normalized_template_identity),
                        template_base_commitment: receipt.template_base_commitment,
                        suspension_dependency: receipt.suspension_dependency.clone(),
                        blocking_dependency: receipt.blocking_dependency.clone(),
                        conditional_carry: receipt.conditional_carry.to_vec(),
                        template_commitment: receipt.template_contract_commitment.as_bytes(),
                        specialization_commitment: receipt.commitment.as_bytes(),
                        parameter_kinds: specialization.parameters
                            [..specialization.invocation_start]
                            .iter()
                            .map(parameter_kind)
                            .collect(),
                        telescope: capture_telescope,
                    })
                })
                .transpose()?;
            terminal_owner.closed_reach_application = Some(ClosedReachApplication {
                template_identity: identities.intern(specialization.normalized_template_identity),
                template_base_commitment: checked_identity.template_base_commitment,
                suspension_dependency: checked_identity.suspension_dependency.clone(),
                blocking_dependency: checked_identity.blocking_dependency.clone(),
                conditional_carry: checked_identity.conditional_carry.to_vec(),
                template_commitment: checked_identity.template_contract_commitment.as_bytes(),
                specialization_commitment: checked_identity.commitment.as_bytes(),
                parameter_kinds: specialization.parameters[specialization.invocation_start..]
                    .iter()
                    .map(parameter_kind)
                    .collect(),
                telescope,
                capture,
                fixed,
                dependencies,
                calls,
            });
        }
        Ok(())
    }

    fn resolved_source_entry(checked: &CheckedTrees, target: SymbolHandle) -> Option<SymbolHandle> {
        checked.machines().iter().find_map(|machine| {
            let states = checked.machine_states(machine);
            if machine.symbol == target {
                states.first().map(|state| state.symbol)
            } else {
                typed_trees::state::state_by_symbol(states, target).map(|state| state.symbol)
            }
        })
    }

    fn closed_call_application(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(SymbolHandle, MachineId)],
        target_state: SymbolHandle,
    ) -> Result<Option<ClosedReachCallApplication>, LoweringError> {
        let source_machine = checked
            .machines()
            .iter()
            .find(|machine| {
                typed_trees::state::state_by_symbol(checked.machine_states(machine), target_state)
                    .is_some()
            })
            .ok_or(LoweringError::Unsupported(
                "closed schema call lost its source owner",
            ))?;
        let Some(callee) = exact_machine(sources, source_machine.symbol)? else {
            return Ok(None);
        };
        let specialization = checked
            .machine_specialization(source_machine.symbol)
            .ok_or(LoweringError::Unsupported(
                "closed schema call lost its specialization",
            ))?;
        let mut types = specialization.type_argument_identities.iter();
        let checked_identity = checked
            .facts
            .specialization_identities
            .for_instance(specialization.instance)
            .ok_or(LoweringError::Unsupported(
                "closed schema call lost its checked specialization identity",
            ))?;
        let mut constants = specialization.const_argument_identities.iter();
        let mut machines = specialization.machine_arguments.iter().zip(
            checked_identity
                .machine_argument_contract_commitments
                .iter(),
        );
        let mut arguments = Vec::with_capacity(specialization.template_parameters.len());
        for parameter in checked
            .data_type_parameters
            .span_or_empty(specialization.template_parameters)
        {
            arguments.push(match parameter.kind {
                TypeParameterKind::Type => {
                    ClosedReachArgument::Type(identities.intern(types.next().ok_or(
                        LoweringError::Unsupported("schema call lost a type argument"),
                    )?))
                }
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    ClosedReachArgument::Const(identities.intern(constants.next().ok_or(
                        LoweringError::Unsupported("schema call lost a const argument"),
                    )?))
                }
                TypeParameterKind::Machine { .. } => {
                    let (selected, commitment) = machines.next().ok_or(
                        LoweringError::Unsupported("schema call lost a machine argument"),
                    )?;
                    ClosedReachArgument::Machine {
                        identity: identities
                            .intern(&selected_static_machine_identity(checked, *selected)?),
                        contract_commitment: *commitment,
                    }
                }
                TypeParameterKind::Proposition { .. } => return Ok(None),
                TypeParameterKind::Implementation => {
                    return unsupported(
                        "schema machine telescope retains a trait-only implementation binder",
                    );
                }
            });
        }
        if types.next().is_some() || constants.next().is_some() || machines.next().is_some() {
            return unsupported("schema call arguments differ from its complete telescope");
        }
        Ok(Some(ClosedReachCallApplication {
            callee,
            specialization_commitment: checked_identity.commitment.as_bytes(),
            arguments,
        }))
    }

    pub(crate) fn selected_static_machine_identity(
        checked: &CheckedTrees,
        selected: SymbolHandle,
    ) -> Result<String, LoweringError> {
        let owner = checked
            .machine_holding_state(selected)
            .ok_or(LoweringError::Unsupported(
                "static machine argument lost its exact selected owner",
            ))?;
        let overload =
            checked
                .machine_callable_identity_text(owner)
                .ok_or(LoweringError::Unsupported(
                    "static machine argument has no normalized callable identity",
                ))?;
        Ok(format!(
            "{}|{}|selected={}",
            declaration_identity(checked, owner.symbol),
            overload,
            declaration_identity(checked, selected),
        ))
    }

    fn exact_machine(
        sources: &[(SymbolHandle, MachineId)],
        source: SymbolHandle,
    ) -> Result<Option<MachineId>, LoweringError> {
        let mut matches = sources.iter().filter(|(candidate, _)| *candidate == source);
        let result = matches.next().map(|(_, machine)| *machine);
        if matches.next().is_some() {
            return unsupported("closed reach selection has ambiguous source owners");
        }
        Ok(result)
    }

    fn declaration_identity(checked: &CheckedTrees, symbol: SymbolHandle) -> String {
        let path = checked.symbols.display_path(symbol, "::");
        match checked.symbols.symbol_package_identity(symbol) {
            Some(package) => {
                let owner = hex_text(&package.digest());
                format!("package:{owner}::{path}")
            }
            None => format!("unmanaged::{path}"),
        }
    }

    fn retain_services(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        services: &[ServiceReachId],
        module: &mut TerminalModule,
    ) -> Result<Vec<ServiceId>, LoweringError> {
        let mut result = Vec::with_capacity(services.len());
        for service in services {
            let definition = checked
                .facts
                .service_reaches
                .services
                .definition(*service)
                .ok_or(LoweringError::Unsupported(
                    "closed reach contract has an unknown service",
                ))?;
            let identity = crate::shared::terminal_identities::service_declaration_identity(
                identities,
                checked,
                definition.symbol,
            )?;
            let existing = module
                .services
                .iter()
                .find(|service| service.identity == identity)
                .map(|service| service.id);
            let id = if let Some(id) = existing {
                id
            } else {
                let parents = retain_services(identities, checked, &definition.parents, module)?;
                let next = module
                    .services
                    .iter()
                    .map(|service| service.id.get())
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "closed reach service identities are exhausted",
                    ))?;
                let id = crate::shared::terminal_identities::service_id(next);
                module.services.push(ServiceDeclaration {
                    id,
                    identity,
                    parents,
                });
                id
            };
            result.push(id);
        }
        result.sort();
        result.dedup();
        Ok(result)
    }
}
pub(crate) mod conformance_applications {
    use checked_trees::data::TypeParameterKind;
    use checked_trees::{CheckedTrees, ClosedConformanceConstArgument};
    use terminal_psi::{
        ClosedConformanceApplication, ClosedConformanceCallableResult,
        ClosedConformanceParameterBinding, ClosedConformanceParameterKind,
        ClosedConformanceRealizationCallable, ClosedConformanceRow, TargetMachineCallable,
        TerminalModule, closed_conformance_application_commitment,
        closed_conformance_application_report_fingerprint,
    };

    use super::LoweringError;
    use crate::machine_lowering::declarations::semantic_identity;

    pub(crate) fn lower_closed_conformance_applications(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        source_machines: &[symbols::SymbolHandle],
        source_boundaries: &[(
            symbols::SymbolHandle,
            semantic_vocabulary::BoundaryMachineId,
        )],
        module: &mut TerminalModule,
    ) -> Result<(), LoweringError> {
        let selected_applications = checked
            .typed
            .closed_conformance_application_sites_for_owners(source_machines);
        if selected_applications.is_empty() {
            module.closed_conformance_applications.clear();
            return Ok(());
        }
        if source_machines.len() != module.machines.len() {
            return Err(LoweringError::Unsupported(
                "closed conformance application ownership does not match the terminal machine closure",
            ));
        }

        let owners = source_machines
            .iter()
            .zip(&module.machines)
            .map(|(source, terminal)| (*source, terminal.id, &terminal.result))
            .collect::<Vec<_>>();
        let mut callables = owners
            .iter()
            .map(|(source, machine, result)| {
                let result = match result {
                    terminal_psi::TerminalMachineResult::Unit => {
                        ClosedConformanceCallableResult::Unit
                    }
                    terminal_psi::TerminalMachineResult::Scalar(result) => {
                        ClosedConformanceCallableResult::Scalar(result.scalar_type)
                    }
                    terminal_psi::TerminalMachineResult::Structural(result) => {
                        ClosedConformanceCallableResult::Structural(result.structural_type)
                    }
                };
                (*source, TargetMachineCallable::Machine(*machine), result)
            })
            .collect::<Vec<_>>();
        for (source, boundary) in source_boundaries {
            let declaration =
                module
                    .boundary_machine(*boundary)
                    .ok_or(LoweringError::Unsupported(
                        "static conformance boundary owner is absent",
                    ))?;
            let result = match &declaration.result {
                terminal_psi::BoundaryMachineResult::Unit => ClosedConformanceCallableResult::Unit,
                terminal_psi::BoundaryMachineResult::Scalar(result) => {
                    ClosedConformanceCallableResult::Scalar(result.scalar_type)
                }
                terminal_psi::BoundaryMachineResult::Structural(result) => {
                    ClosedConformanceCallableResult::Structural(result.structural_type)
                }
            };
            callables.push((*source, TargetMachineCallable::Boundary(*boundary), result));
        }
        module.closed_conformance_applications = collect_closed_conformance_applications(
            identities,
            checked,
            &owners,
            &callables,
            selected_applications
                .into_iter()
                .map(|(_, owner, _, application)| (owner, application)),
        )?;
        Ok(())
    }

    fn collect_closed_conformance_applications<'checked>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        owners: &[(
            symbols::SymbolHandle,
            semantic_vocabulary::MachineId,
            &terminal_psi::TerminalMachineResult,
        )],
        emitted_callables: &[(
            symbols::SymbolHandle,
            TargetMachineCallable,
            ClosedConformanceCallableResult,
        )],
        selected_applications: impl IntoIterator<
            Item = (
                symbols::SymbolHandle,
                &'checked typed_trees::typed_trees::ClosedConformanceApplication,
            ),
        >,
    ) -> Result<Vec<ClosedConformanceApplication>, LoweringError> {
        let mut proof_dispatches_by_owner = std::collections::HashMap::<
            symbols::SymbolHandle,
            Vec<checked_trees::StaticRequirementDispatchFact>,
        >::new();
        for (_, invocation) in checked.facts.proof.proof_output_calls.iter() {
            if let Some(dispatch) = invocation.static_requirement_dispatch {
                proof_dispatches_by_owner
                    .entry(invocation.caller_machine_symbol)
                    .or_default()
                    .push(dispatch);
            }
        }
        let mut cached_realized_calls = std::collections::HashMap::<
            symbols::SymbolHandle,
            Vec<(
                StaticExecutableCall,
                checked_trees::StaticRequirementDispatchFact,
            )>,
        >::new();
        let mut applications = Vec::new();
        for (source_owner, application) in selected_applications {
            let Some((_, owner, _)) = owners.iter().find(|(source, _, _)| *source == source_owner)
            else {
                continue;
            };
            let selected = checked
                .conformance_by_symbol(application.declaration)
                .ok_or(LoweringError::Unsupported(
                    "closed conformance application lost its declaration",
                ))?;
            let selected_rows = selected.rows.as_slice();
            let realized_calls = match cached_realized_calls.entry(source_owner) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(checked_static_dispatches(checked, source_owner)?)
                }
            };
            let telescope = checked_application_telescope(checked, application)?;
            let mut selected_calls = Vec::new();
            for (invocation, dispatch) in realized_calls.iter() {
                if dispatch.application_report_fingerprint != application.report_fingerprint
                    || dispatch.application_commitment != application.commitment
                {
                    continue;
                }
                let selected = checked.typed.selected_conformance_applications(
                    source_owner,
                    dispatch.selection_application,
                    dispatch.evidence_parameter,
                    dispatch.requirement,
                    dispatch.application_report_fingerprint,
                    dispatch.application_commitment,
                );
                let [source_application] = selected.as_slice() else {
                    return Err(LoweringError::Unsupported(
                        "static conformance call lost its unique source application",
                    ));
                };
                if source_application.declaration != application.declaration
                    || source_application.lifetime_arguments != application.lifetime_arguments
                    || source_application.trait_definition != application.trait_definition
                    || source_application.trait_lifetime_arguments
                        != application.trait_lifetime_arguments
                    || source_application.trait_arguments != application.trait_arguments
                    || source_application.rows != application.rows
                    || checked_application_telescope(checked, source_application)? != telescope
                {
                    continue;
                }
                selected_calls.push((invocation, dispatch, *source_application));
            }
            let realization_callables = selected_calls
                .into_iter()
                .map(|(invocation, dispatch, source_application)| {
                    let selected_row = selected_rows
                        .iter()
                        .find(|row| {
                            row.declaring_trait == dispatch.declaring_trait
                                && row.requirement == dispatch.requirement
                                && row.realization_machine == dispatch.realization_machine
                                && row.realization_state == dispatch.realization_state
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance dispatch lost its normalized row",
                        ))?;
                    let callable = emitted_callables
                        .iter()
                        .find_map(|(source, terminal, _)| {
                            (*source == invocation.machine).then_some(*terminal)
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance realization is absent from the Terminal closure",
                        ))?;
                    let realization_state = checked
                        .typed
                        .machines()
                        .iter()
                        .flat_map(|machine| checked.typed.machine_states(machine))
                        .find(|state| state.symbol == invocation.state)
                        .ok_or(LoweringError::Unsupported(
                            "static conformance realization state is absent",
                        ))?;
                    let requirements = checked
                        .typed
                        .trait_definition_by_symbol(dispatch.declaring_trait)
                        .map(|definition| {
                            checked
                                .typed
                                .trait_machine_signatures(definition)
                                .iter()
                                .filter(|requirement| requirement.symbol == dispatch.requirement)
                                .collect::<Vec<_>>()
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance requirement is absent",
                        ))?;
                    let [requirement] = requirements.as_slice() else {
                        return Err(LoweringError::Unsupported(
                            "static conformance requirement is absent or ambiguous",
                        ));
                    };
                    let emitted_result = emitted_callables
                        .iter()
                        .find(|(source, _, _)| *source == invocation.machine)
                        .map(|(_, _, result)| *result)
                        .ok_or(LoweringError::Unsupported(
                            "static conformance lost its emitted result signature",
                        ))?;
                    let classify = |return_type: checked_trees::types::TypeReferenceHandle| {
                        if !return_type.is_valid() {
                            return Ok(ClosedConformanceCallableResult::Unit);
                        }
                        if let Some(primitive) = checked.typed.primitive_type_reference(return_type)
                        {
                            return super::terminal_scalar_type(primitive)
                                .map(ClosedConformanceCallableResult::Scalar);
                        }
                        match emitted_result {
                            ClosedConformanceCallableResult::Structural(result) => {
                                Ok(ClosedConformanceCallableResult::Structural(result))
                            }
                            _ => Err(LoweringError::Unsupported(
                                "structured conformance result has no emitted structural signature",
                            )),
                        }
                    };
                    let result = classify(realization_state.return_type)?;
                    let public_signature = if dispatch.contract_signature.is_valid() {
                        checked
                            .typed
                            .closed_requirement_application(dispatch.contract_signature)
                            .ok_or(LoweringError::Unsupported(
                                "static conformance lost its applied public signature",
                            ))?
                            .signature
                    } else {
                        let scope = source_application
                            .trait_scope
                            .scope(&checked.typed)
                            .and_then(|scope| {
                                scope.declaring_scope(&checked.typed, dispatch.declaring_trait)
                            })
                            .ok_or(LoweringError::Unsupported(
                                "static conformance lost its exact declaring trait application",
                            ))?;
                        checked
                            .typed
                            .applied_callable_signature(&scope, dispatch.requirement)
                            .or_else(|| {
                                (scope.substitutions.is_empty()
                                    && scope.declaration.lifetime_parameters.is_empty())
                                .then_some(*requirement)
                            })
                            .ok_or(LoweringError::Unsupported(
                                "static conformance lost its applied public callable signature",
                            ))?
                    };
                    let public_return = checked_method_return_reference(
                        checked,
                        public_signature,
                        invocation.machine,
                    )?;
                    if classify(public_return)? != result {
                        return Err(LoweringError::Unsupported(
                            "static conformance requirement and realization result classes differ",
                        ));
                    }
                    let source_callable_identity = identities.intern(
                        &super::evidence_lowering::checked_evidence_machine_identity(
                            checked,
                            invocation.machine,
                        )?,
                    );
                    let applied = terminal_psi::ClosedConformanceStaticApplication {
                        public_requirement_identity: identities.intern(
                            &super::evidence_lowering::checked_evidence_requirement_identity(
                                checked,
                                dispatch.declaring_trait,
                                dispatch.requirement,
                            )?,
                        ),
                        realization_identity: identities.intern(
                            &checked
                                .symbols
                                .display_path(selected_row.realization_state, "::"),
                        ),
                        source_callable_identity,
                        callable,
                        application_commitment: checked
                            .facts
                            .specialization_identities
                            .for_instance(invocation.machine)
                            .map(|identity| identity.commitment.as_bytes()),
                        method_arguments: checked_method_arguments(
                            identities,
                            checked,
                            invocation.machine,
                        )?,
                    };
                    Ok((
                        ClosedConformanceRealizationCallable {
                            source_callable_identity: identities.intern(
                                &super::evidence_lowering::checked_evidence_machine_identity(
                                    checked,
                                    invocation.machine,
                                )?,
                            ),
                            callable,
                            result,
                        },
                        applied,
                    ))
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let (mut realization_callables, mut static_applications): (Vec<_>, Vec<_>) =
                realization_callables.into_iter().unzip();
            let table = identities.as_table();
            let mut keyed = static_applications
                .into_iter()
                .map(|row| {
                    terminal_semantics::closed_conformances::static_application_order_key(
                        table, &row,
                    )
                    .map(|key| (key, row))
                    .map_err(LoweringError::Unsupported)
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            static_applications = keyed.into_iter().map(|(_, row)| row).collect();
            let mut schemas = application
                .rows
                .iter()
                .map(|row| {
                    super::evidence_lowering::checked_requirement_schema(
                        identities,
                        checked,
                        row.declaring_trait,
                        row.requirement,
                        row.realization_state,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            schemas.sort_by(|left, right| {
                (
                    identities.text(left.declaring_trait_identity),
                    identities.text(left.public_requirement_identity),
                )
                    .cmp(&(
                        identities.text(right.declaring_trait_identity),
                        identities.text(right.public_requirement_identity),
                    ))
            });
            if schemas.windows(2).any(|pair| {
                pair[0].declaring_trait_identity == pair[1].declaring_trait_identity
                    && pair[0].public_requirement_identity == pair[1].public_requirement_identity
            }) {
                return Err(LoweringError::Unsupported(
                    "original conformance schema inventory repeats one requirement",
                ));
            }
            let table = identities.as_table();
            realization_callables.sort_by(|left, right| {
                table
                    .text(left.source_callable_identity)
                    .cmp(&table.text(right.source_callable_identity))
            });
            realization_callables.dedup();
            let mut rows = Vec::new();
            for row in &application.rows {
                let selected_row = selected_rows
                    .iter()
                    .find(|candidate| {
                        candidate.declaring_trait == row.declaring_trait
                            && candidate.requirement == row.requirement
                            && candidate.realization_state == row.realization_state
                    })
                    .ok_or(LoweringError::Unsupported(
                        "closed conformance application row no longer matches its declaration",
                    ))?;
                let declaring_trait_identity = checked
                    .symbols
                    .display_path(selected_row.declaring_trait, "::");
                let public_requirement_identity =
                    super::evidence_lowering::checked_evidence_requirement_identity(
                        checked,
                        selected_row.declaring_trait,
                        selected_row.requirement,
                    )?;
                let requirement_identity =
                    checked.symbols.display_path(selected_row.requirement, "::");
                for family_row in super::evidence_lowering::checked_static_conformance_rows(
                    checked,
                    selected_row.declaring_trait,
                    selected_row.requirement,
                    selected_row.realization_machine,
                    selected_row.realization_state,
                )? {
                    let selected_by_static_dispatch = proof_dispatches_by_owner
                        .get(&source_owner)
                        .is_some_and(|dispatches| {
                            dispatches.iter().any(|dispatch| {
                                dispatch.application_report_fingerprint
                                    == application.report_fingerprint
                                    && dispatch.application_commitment == application.commitment
                                    && dispatch.declaring_trait == selected_row.declaring_trait
                                    && dispatch.requirement == selected_row.requirement
                                    && dispatch.realization_machine
                                        == family_row.realization_machine
                                    && dispatch.realization_state == family_row.realization_state
                            })
                        });
                    let realization_callable_identity = selected_by_static_dispatch
                            .then(|| {
                                let callable_target = emitted_callables
                                    .iter()
                                    .find_map(|(source, terminal, _)| {
                                        (*source == family_row.realization_machine)
                                            .then_some(*terminal)
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "static conformance realization is absent from the Terminal closure",
                                    ))?;
                                let identity =
                                    super::evidence_lowering::checked_evidence_machine_identity(
                                        checked,
                                        family_row.realization_machine,
                                    )?;
                                realization_callables
                                    .iter()
                                    .any(|callable| {
                                        callable.source_callable_identity == identities.intern(&identity)
                                            && callable.callable == callable_target
                                    })
                                    .then_some(identity)
                                    .ok_or(LoweringError::Unsupported(
                                        "static conformance row lost its independent callable registry entry",
                                    ))
                            })
                            .transpose()?;
                    rows.push(ClosedConformanceRow {
                        requirement_declaration: terminal_psi::NominalDeclarationIdentity::new(
                            semantic_identity(identities, checked, row.requirement).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "conformance requirement lost its declaration owner",
                                    )
                                },
                            )?,
                        ),
                        declaring_trait_identity: identities
                            .intern(&declaring_trait_identity.clone()),
                        public_requirement_identity: identities
                            .intern(&public_requirement_identity.clone()),
                        family_tuple: family_row
                            .family_tuple
                            .iter()
                            .map(|identity| identities.intern(identity))
                            .collect(),
                        requirement_identity: identities.intern(&requirement_identity.clone()),
                        realization_identity: identities.intern(
                            &checked
                                .symbols
                                .display_path(family_row.realization_state, "::"),
                        ),
                        realization_callable_identity: realization_callable_identity
                            .as_ref()
                            .map(|identity| identities.intern(identity)),
                    });
                }
            }
            let telescope = checked_application_telescope(checked, application)?;
            let mut lowered = ClosedConformanceApplication {
                owner: *owner,
                declaration_identity: identities
                    .intern(&checked.symbols.display_path(application.declaration, "::")),
                telescope,
                trait_identity: identities.intern(
                    &checked
                        .symbols
                        .display_path(application.trait_definition, "::"),
                ),
                trait_lifetime_arguments: application.trait_lifetime_arguments.clone(),
                trait_arguments: application.trait_arguments.clone(),
                realization_callables,
                schemas,
                static_applications,
                rows,
                report_fingerprint: 0,
                commitment: Default::default(),
            };
            lowered.report_fingerprint =
                closed_conformance_application_report_fingerprint(identities.as_table(), &lowered);
            lowered.commitment =
                closed_conformance_application_commitment(identities.as_table(), &lowered);
            if !applications.contains(&lowered) {
                applications.push(lowered);
            }
        }
        sort_applications(identities.as_table(), &mut applications);
        Ok(applications)
    }

    fn sort_applications(
        identities: &terminal_psi::TerminalIdentityTable,
        applications: &mut [ClosedConformanceApplication],
    ) {
        applications.sort_by(|left, right| {
            (
                left.owner,
                identities
                    .text(left.declaration_identity)
                    .unwrap_or_default(),
                left.report_fingerprint,
            )
                .cmp(&(
                    right.owner,
                    identities
                        .text(right.declaration_identity)
                        .unwrap_or_default(),
                    right.report_fingerprint,
                ))
        });
    }

    pub(crate) fn checked_application_telescope(
        checked: &CheckedTrees,
        application: &typed_trees::typed_trees::ClosedConformanceApplication,
    ) -> Result<Vec<ClosedConformanceParameterBinding>, LoweringError> {
        let selected = checked
            .conformance_by_symbol(application.declaration)
            .ok_or(LoweringError::Unsupported(
                "closed conformance telescope lost its original declaration",
            ))?;
        if selected.lifetime_parameters.len() != application.lifetime_arguments.len() {
            return Err(LoweringError::Unsupported(
                "closed conformance application has an open lifetime telescope",
            ));
        }
        let mut telescope = selected
            .lifetime_parameters
            .iter()
            .zip(&application.lifetime_arguments)
            .map(|(parameter, argument)| ClosedConformanceParameterBinding {
                parameter: parameter.as_str().to_owned(),
                kind: ClosedConformanceParameterKind::Lifetime,
                argument: argument.clone(),
            })
            .collect::<Vec<_>>();
        let mut type_index = 0usize;
        let mut const_index = 0usize;
        let mut machine_index = 0usize;
        for parameter in checked.conformance_type_parameters(selected) {
            let (kind, argument) = match &parameter.kind {
                TypeParameterKind::Type => {
                    let argument = application.type_arguments.get(type_index).cloned();
                    type_index += 1;
                    (ClosedConformanceParameterKind::Type, argument)
                }
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    let argument = match application.const_arguments.get(const_index) {
                        Some(ClosedConformanceConstArgument::Evaluated { value, .. }) => Some(
                            language_semantics::const_value::CanonicalConstValue::new(
                                value.type_name.clone(),
                                value.encoding.clone(),
                                "",
                            )
                            .atom(),
                        ),
                        Some(
                            ClosedConformanceConstArgument::CallerExpression { .. }
                            | ClosedConformanceConstArgument::CallerBinder { .. },
                        ) => {
                            return Err(LoweringError::Unsupported(
                                "closed conformance application has an open const binder argument",
                            ));
                        }
                        None => None,
                    };
                    const_index += 1;
                    (ClosedConformanceParameterKind::Const, argument)
                }
                TypeParameterKind::Machine { .. } => {
                    if !application.callable_application.is_valid() {
                        return Err(LoweringError::Unsupported(
                            "closed conformance Machine tuple lost its checked occurrence",
                        ));
                    }
                    let selected = checked
                        .typed
                        .closed_application_selected_machines(application)
                        .get(machine_index)
                        .ok_or(LoweringError::Unsupported(
                            "closed conformance Machine tuple lost its exact selected callback",
                        ))?;
                    let owner = checked.typed.machine_holding_state(selected.symbol).ok_or(
                        LoweringError::Unsupported(
                            "closed conformance Machine tuple is not executable",
                        ),
                    )?;
                    let argument =
                        Some(super::evidence_lowering::checked_evidence_machine_identity(
                            checked,
                            owner.symbol,
                        )?);
                    machine_index += 1;
                    (ClosedConformanceParameterKind::Machine, argument)
                }
                TypeParameterKind::Proposition { .. } => {
                    return Err(LoweringError::Unsupported(
                        "closed conformance application retains a proposition parameter",
                    ));
                }
                TypeParameterKind::Implementation => {
                    return Err(LoweringError::Unsupported(
                        "closed conformance telescope retains a trait-only implementation binder",
                    ));
                }
            };
            telescope.push(ClosedConformanceParameterBinding {
                parameter: parameter.name.as_str().to_owned(),
                kind,
                argument: argument.ok_or(LoweringError::Unsupported(
                    "closed conformance application has an open static telescope",
                ))?,
            });
        }
        if type_index != application.type_arguments.len()
            || const_index != application.const_arguments.len()
            || machine_index != application.machine_arguments.len()
        {
            return Err(LoweringError::Unsupported(
                "closed conformance application telescope has extra arguments",
            ));
        }
        Ok(telescope)
    }

    pub(crate) fn checked_method_arguments(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
    ) -> Result<Vec<terminal_psi::ClosedReachArgument>, LoweringError> {
        let records = checked
            .machine_specializations
            .iter()
            .filter(|row| row.instance == machine)
            .collect::<Vec<_>>();
        let specialization = match records.as_slice() {
            [] => return Ok(Vec::new()),
            [record] => record,
            _ => {
                return Err(LoweringError::Unsupported(
                    "static method application has duplicate invocation ownership",
                ));
            }
        };
        let identity = checked
            .facts
            .specialization_identities
            .for_instance(machine)
            .ok_or(LoweringError::Unsupported(
                "static method application lost its checked invocation commitment",
            ))?;
        let mut types = specialization.type_argument_identities.iter();
        let mut constants = specialization.const_argument_identities.iter();
        let mut machines = specialization
            .machine_arguments
            .iter()
            .zip(identity.machine_argument_contract_commitments.iter());
        let mut arguments = Vec::new();
        for parameter in checked
            .typed
            .data_type_parameters
            .span_or_empty(specialization.template_parameters)
        {
            arguments.push(match parameter.kind {
                TypeParameterKind::Type => terminal_psi::ClosedReachArgument::Type(identities.intern(types.next().ok_or(LoweringError::Unsupported("static method tuple lost a Type argument"))?)),
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => terminal_psi::ClosedReachArgument::Const(identities.intern(constants.next().ok_or(LoweringError::Unsupported("static method tuple lost a const argument"))?)),
                TypeParameterKind::Machine { .. } => {
                    let (selected, commitment) = machines.next().ok_or(LoweringError::Unsupported("static method tuple lost a Machine argument"))?;
                    terminal_psi::ClosedReachArgument::Machine { identity: identities.intern(&super::closed_reach_applications::selected_static_machine_identity(checked, *selected)?), contract_commitment: *commitment }
                }
                _ => return Err(LoweringError::Unsupported("static method tuple contains an unsupported proposition or implementation argument")),
            });
        }
        if types.next().is_some() || constants.next().is_some() || machines.next().is_some() {
            return Err(LoweringError::Unsupported(
                "static method tuple has extra invocation arguments",
            ));
        }
        Ok(arguments)
    }

    struct StaticExecutableCall {
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
    }

    fn checked_static_dispatches(
        checked: &CheckedTrees,
        owner: symbols::SymbolHandle,
    ) -> Result<
        Vec<(
            StaticExecutableCall,
            checked_trees::StaticRequirementDispatchFact,
        )>,
        LoweringError,
    > {
        let control = &checked.facts.flow.control;
        let mut calls = Vec::new();
        for (_, state) in control
            .machine_states(owner)
            .into_iter()
            .map(|state| ((), state))
        {
            for call in control.calls.span_or_empty(state.calls) {
                let Some(dispatch) = call.static_requirement_dispatch else {
                    continue;
                };
                if control.is_retired(state.state_symbol, call) {
                    continue;
                }
                let target = call.target_symbol;
                let realized = checked.typed.machine_holding_state(target).ok_or(
                    LoweringError::Unsupported(
                        "static executable call lost its exact target owner",
                    ),
                )?;
                calls.push((
                    StaticExecutableCall {
                        machine: realized.symbol,
                        state: target,
                    },
                    dispatch,
                ));
            }
        }
        Ok(calls)
    }

    fn checked_method_return_reference(
        checked: &CheckedTrees,
        signature: &checked_trees::signature::StateSignature,
        target: symbols::SymbolHandle,
    ) -> Result<checked_trees::types::TypeReferenceHandle, LoweringError> {
        let result = signature.return_type;
        if !result.is_valid() {
            return Ok(result);
        }
        let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
            checked.typed.type_reference_table.type_reference(result)
        else {
            return Ok(result);
        };
        let parameters = checked.typed.state_signature_type_parameters(signature);
        let Some(position) = parameters
            .iter()
            .position(|parameter| parameter.symbol == *symbol)
        else {
            return Ok(result);
        };
        let specialization = checked
            .machine_specializations
            .iter()
            .find(|row| row.instance == target)
            .ok_or(LoweringError::Unsupported(
                "generic conformance result lost its exact method invocation",
            ))?;
        let private = checked
            .typed
            .data_type_parameters
            .span_or_empty(specialization.template_parameters)
            .get(position)
            .ok_or(LoweringError::Unsupported(
                "generic conformance result has no corresponding method binder",
            ))?;
        match checked
            .typed
            .specialization_binding(specialization, private.symbol)
        {
            Some(typed_trees::static_applications::StaticMachineBindingValue::Type(reference)) => {
                Ok(*reference)
            }
            Some(_) => Err(LoweringError::Unsupported(
                "generic conformance result binder is not a Type",
            )),
            None => Err(LoweringError::Unsupported(
                "generic conformance result has no unique applied Type",
            )),
        }
    }
}
pub(crate) mod external_providers {

    use crate::LoweringError;
    use crate::machine_lowering::declarations;
    use crate::unit::attached_unit::checked_unit_boundary_identity;
    use checked_trees::CheckedTrees;
    use terminal_psi::{
        ExternalProviderConformance, ExternalProviderOwner, TerminalIdentityTableBuilder,
        TerminalModule,
    };
    use typed_trees::machine::{SatisfiedDeclaration, resolve_satisfied_declaration};

    pub(crate) fn retain(
        identities: &mut TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        module: &mut TerminalModule,
    ) -> Result<(), LoweringError> {
        module.external_providers = project(
            identities,
            checked,
            sources,
            |identities, _, _, callable| {
                module
                    .boundary_machines
                    .iter()
                    .any(|boundary| identities.text(boundary.identity) == Some(callable))
                    || module
                        .boundary_application_demands
                        .iter()
                        .any(|demand| demand.requirement().overload() == callable)
            },
            |_, _| {},
        )?;
        Ok(())
    }

    pub(crate) fn project(
        identities: &mut TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        include: impl Fn(
            &TerminalIdentityTableBuilder,
            symbols::SymbolHandle,
            symbols::SymbolHandle,
            &str,
        ) -> bool,
        mut retain_declaration: impl FnMut(
            symbols::SymbolHandle,
            terminal_psi::NominalDeclarationIdentity,
        ),
    ) -> Result<Vec<ExternalProviderConformance>, LoweringError> {
        let typed = &checked.typed;
        let mut candidates = Vec::new();
        for machine in typed.machines() {
            for conformance in typed.machine_trait_conformances(machine) {
                let Some(binding) = conformance
                    .external_binding
                    .and_then(|binding| typed.external_bindings.identity(binding))
                else {
                    continue;
                };
                let Some(satisfied) = resolve_satisfied_declaration(typed, machine, conformance)
                else {
                    continue;
                };
                if matches!(satisfied, SatisfiedDeclaration::Trait { definition, .. } if !definition.is_boundary)
                {
                    continue;
                }
                let requirements = match satisfied {
                    SatisfiedDeclaration::TopLevelRequirement(requirement)
                        if !typed.machine_type_parameters(requirement).is_empty() =>
                    {
                        checked
                            .facts
                            .provider_closed_requirements
                            .iter()
                            .filter(|row| {
                                row.provider == machine.symbol
                                    && row.selecting_declaration == machine.symbol
                                    && row.template == requirement.symbol
                            })
                            .map(|row| row.closed_requirement)
                            .collect::<Vec<_>>()
                    }
                    _ => vec![satisfied.symbol()],
                };
                for requirement in requirements {
                    let callable = match satisfied {
                        SatisfiedDeclaration::Trait {
                            definition,
                            requirement,
                        } => {
                            let identity =
                                typed.trait_requirement_callable_identity(definition, requirement);
                            if !identity.is_valid() {
                                return Err(LoweringError::Unsupported(
                                    "external trait declaration has no exact callable identity",
                                ));
                            }
                            typed.callable_identity_text(identity)
                        }
                        _ => {
                            checked_unit_boundary_identity(checked, requirement)?.render(checked)?
                        }
                    };
                    if !include(identities, machine.symbol, requirement, &callable) {
                        continue;
                    }
                    let provider_symbol =
                        typed_trees::provider_declarations::satisfaction_provider_declaration(
                            typed, machine,
                        );
                    let provider = if let Some(provider_symbol) = provider_symbol {
                        Some(ExternalProviderOwner {
                            declaration: declarations::nominal(
                                identities,
                                checked,
                                sources,
                                provider_symbol,
                            )
                            .map_err(LoweringError::Unsupported)?,
                            identity: identities
                                .intern(&typed.symbols.display_path(provider_symbol, "::")),
                            package: checked.symbols.symbol_package_identity(provider_symbol),
                        })
                    } else {
                        None
                    };
                    let numeric_intrinsic = typed.numeric_intrinsic_execution(requirement);
                    let requirement_schema = typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement::by_symbol(typed, requirement)
                        .map(|requirement| terminal_psi::ExternalRequirementSchema {
                            identity: identities.intern(&requirement.schema_identity()),
                            method: identities.intern(requirement.method_name()),
                            owner: identities.intern(requirement.method_owner()),
                        });
                    if let Some(
                        language_semantics::numeric_intrinsics::NumericIntrinsic::Builtin {
                            function,
                            ..
                        },
                    ) = numeric_intrinsic
                    {
                        let symbol = checked.symbols.builtin_function_symbol(function).ok_or(
                            LoweringError::Unsupported(
                                "numeric builtin is absent from its exact root slot",
                            ),
                        )?;
                        if checked.symbols.builtin_function_for_symbol(symbol) != Some(function) {
                            return Err(LoweringError::Unsupported(
                                "numeric builtin does not round-trip through its exact root slot",
                            ));
                        }
                    }
                    let candidate = ExternalProviderConformance {
                        requirement: declarations::nominal(
                            identities,
                            checked,
                            sources,
                            requirement,
                        )
                        .map_err(LoweringError::Unsupported)?,
                        requirement_identity: identities.intern(&callable),
                        requirement_package: checked.symbols.symbol_package_identity(requirement),
                        numeric_intrinsic,
                        requirement_schema,
                        target: typed.machine_authored_target(machine).map(str::to_owned),
                        realization: declarations::identity(
                            identities,
                            checked,
                            sources,
                            machine.symbol,
                        )
                        .map_err(LoweringError::InvalidDeclarationIdentity)?,
                        realization_package: checked
                            .symbols
                            .symbol_package_identity(machine.symbol),
                        provider,
                        binding: binding.clone(),
                    };
                    retain_declaration(requirement, candidate.requirement.clone());
                    retain_declaration(machine.symbol, candidate.realization.machine.clone());
                    if let Some(provider) = &candidate.provider {
                        retain_declaration(
                            provider_symbol.ok_or(LoweringError::Unsupported(
                                "external provider lost its exact owner",
                            ))?,
                            provider.declaration.clone(),
                        );
                    }
                    candidates.push(candidate);
                }
            }
        }
        candidates.sort();
        candidates.dedup();
        Ok(candidates)
    }
}
pub(crate) mod operation_crash_contracts {

    use std::collections::BTreeMap;

    use checked_trees::signature::{SignatureContract, SignatureContractKind};
    use checked_trees::{
        CheckedCrashOperatorSite, CheckedOperatorUseHandle, CheckedTrees, CheckedValueOrigin,
    };
    use lowered_psi::{LoweredPsi, LoweredSelectedComparisonOperandOrder};
    use semantic_vocabulary::{MachineId, OperationId, ScalarTerm, ScalarType, ValueId};
    use terminal_psi::{OperationKind, TerminalMachine, TerminalOperationCrashContract};

    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::proofs::crash_routes::lower_formal_crash_routes;
    use crate::shared::direct_calls::direct_call_parts;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::dense_identity;

    pub(crate) fn retain_operation_crash_contracts(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        source_machines: &[symbols::SymbolHandle],
        projection_sources: &[(symbols::SymbolHandle, MachineId)],
        lowered: &mut LoweredPsi,
    ) -> Result<(), LoweringError> {
        let sites = source_machines
            .iter()
            .filter_map(|machine| checked.facts.contract_plans.for_machine(*machine))
            .flat_map(|plan| plan.crash.checked_operators())
            .collect::<Vec<_>>();
        validate_every_crash_qualified_use_has_a_site(checked, source_machines, &sites)?;
        let mut rows = sites
            .iter()
            .map(|site| lower_site(identities, checked, projection_sources, lowered, site))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| (row.machine, row.operation));
        if rows.windows(2).any(|pair| {
            (pair[0].machine, pair[0].operation) == (pair[1].machine, pair[1].operation)
        }) {
            return unsupported("operator crash sites collide on one Terminal operation");
        }
        if !lowered.semantic_module.operation_crash_contracts.is_empty() {
            return unsupported("operation crash contracts were already installed on this module");
        }
        lowered.semantic_module.operation_crash_contracts = rows;
        Ok(())
    }

    fn validate_every_crash_qualified_use_has_a_site(
        checked: &CheckedTrees,
        source_machines: &[symbols::SymbolHandle],
        sites: &[&CheckedCrashOperatorSite],
    ) -> Result<(), LoweringError> {
        let program = &checked.typed;
        let operators = &checked.facts.operators;
        let publishes_crash = |contracts: arena::HandleSpan<SignatureContract>| {
            program
                .signature_contracts
                .span_or_empty(contracts)
                .iter()
                .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
        };
        let source_machine_set: std::collections::HashSet<_> =
            source_machines.iter().copied().collect();
        let operator_site_set: std::collections::HashSet<_> = sites
            .iter()
            .filter(|site| site.operator_use.is_valid())
            .map(|site| site.operator_use)
            .collect();
        let named_site_set: std::collections::HashSet<_> = sites
            .iter()
            .filter(|site| site.named_use.is_valid())
            .map(|site| site.named_use)
            .collect();
        let in_closure = |origin: &CheckedValueOrigin| {
            matches!(origin, CheckedValueOrigin::StateStatement { machine_symbol, .. }
                if source_machine_set.contains(machine_symbol))
        };
        for (handle, operator_use) in operators.uses.iter() {
            if !in_closure(&operator_use.origin)
                || !operators
                    .selected_candidate(operator_use)
                    .is_some_and(|selected| publishes_crash(selected.contracts))
            {
                continue;
            }
            if !operator_site_set.contains(&handle) {
                return unsupported(
                    "selected operator crash invocation has no checked crash site to lower",
                );
            }
        }
        for (handle, named_use) in operators.named_uses.iter() {
            if !in_closure(&named_use.origin)
                || !operator_declaration(checked, named_use.selected_operator_symbol)
                    .is_some_and(|(_, contracts)| publishes_crash(contracts))
            {
                continue;
            }
            if !named_site_set.contains(&handle) {
                return unsupported(
                    "named operator crash invocation has no checked crash site to lower",
                );
            }
        }
        Ok(())
    }

    fn lower_site(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        projection_sources: &[(symbols::SymbolHandle, MachineId)],
        lowered: &LoweredPsi,
        site: &CheckedCrashOperatorSite,
    ) -> Result<Option<TerminalOperationCrashContract>, LoweringError> {
        if site.named_use.is_valid() {
            return validate_named_site_carrier(
                identities,
                checked,
                projection_sources,
                lowered,
                site,
            )
            .map(|()| None);
        }
        let (machine_id, operation_id, operand_order) = emitted_operation(lowered, site)?;
        let machine =
            lowered
                .semantic_module
                .machine(machine_id)
                .ok_or(LoweringError::Unsupported(
                    "operator crash site joins a Terminal machine outside the lowered module",
                ))?;
        let operation = machine
            .operation(operation_id)
            .ok_or(LoweringError::Unsupported(
                "operator crash site joins an operation outside its Terminal machine",
            ))?;
        let operands = positional_scalar_operands(&operation.kind)?;
        let operand_types = operands
            .iter()
            .map(|operand| {
                scalar_value_type(machine, *operand).ok_or(LoweringError::Unsupported(
                    "operator crash site operand has no Terminal scalar declaration",
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let authored_positions = (0..operands.len())
            .map(|ordinal| {
                operand_order
                    .terminal_operand_position(ordinal, operands.len())
                    .ok_or(LoweringError::Unsupported(
                        "operator crash site operand mapping does not address the emitted operand roster",
                    ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let authored_operand_types = authored_positions
            .iter()
            .map(|position| operand_types[*position])
            .collect::<Vec<_>>();
        validate_operator_signature(checked, site, &authored_operand_types)?;
        let published_routes = reindex_formal_telescope(
            lower_formal_crash_routes(
                identities,
                published_buckets(checked, site)?,
                &authored_operand_types,
            )?,
            &authored_positions,
            &authored_operand_types,
        )?;
        if published_routes.is_empty() {
            return unsupported("operator crash site publishes no lowerable crash route");
        }
        let substitutions = operands
            .iter()
            .zip(&operand_types)
            .enumerate()
            .map(|(ordinal, (operand, scalar_type))| {
                Ok((
                    formal_identity(ordinal)?,
                    ScalarTerm::value(*operand, *scalar_type),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
        let crash_continuations = terminal_semantics::substitution::substitute_crash_routes(
            &published_routes,
            &substitutions,
        );
        Ok(Some(TerminalOperationCrashContract {
            machine: machine_id,
            operation: operation_id,
            published_routes,
            crash_continuations,
        }))
    }

    fn validate_named_site_carrier(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        projection_sources: &[(symbols::SymbolHandle, MachineId)],
        lowered: &LoweredPsi,
        site: &CheckedCrashOperatorSite,
    ) -> Result<(), LoweringError> {
        let named_use = checked.facts.operators.named_uses.get(site.named_use);
        let CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            ..
        } = named_use.origin
        else {
            return unsupported("named operator crash invocation has no machine-local origin");
        };
        let source_site = checked_trees::NominalMachineUseSite::Expression(named_use.expression);
        let mut joins = lowered.source_call_occurrences.iter().filter(|occurrence| {
            occurrence.source_site == Some(source_site)
                && occurrence
                    .origin
                    .statement()
                    .is_some_and(|site| site.state == state_symbol)
                && occurrence.source_target == site.selected_operator
        });
        let join = match (joins.next(), joins.next()) {
            (Some(join), None) => join,
            (None, _) => {
                return unsupported(
                    "named operator crash invocation has no emitted boundary call to carry its contract",
                );
            }
            (Some(_), Some(_)) => {
                return unsupported(
                    "named operator crash invocation joins more than one emitted call",
                );
            }
        };
        let machine_id = projection_sources
            .iter()
            .find_map(|(source, id)| (*source == machine_symbol).then_some(*id))
            .ok_or(LoweringError::Unsupported(
                "named operator crash invocation's machine has no Terminal lowering",
            ))?;
        let machine = lowered
            .semantic_module
            .machine(machine_id)
            .ok_or(LoweringError::Unsupported(
                "named operator crash invocation joins a Terminal machine outside the lowered module",
            ))?;
        let operation = machine
            .operations()
            .find(|operation| join.terminal_site.operation() == Some(operation.id))
            .ok_or(LoweringError::Unsupported(
                "named operator crash invocation joins an operation outside its Terminal machine",
            ))?;
        let terminal_psi::OperationKind::BoundaryCall {
            boundary,
            arguments,
            ..
        } = &operation.kind
        else {
            return unsupported(
                "named operator crash invocation joins an operation that is not its boundary call",
            );
        };
        let declaration = lowered.semantic_module.boundary_machine(*boundary).ok_or(
            LoweringError::Unsupported(
                "named operator crash invocation's boundary call names no retained declaration",
            ),
        )?;
        if declaration.kind != semantic_vocabulary::BoundaryRequirementKind::Operator
            || arguments.len() != declaration.scalar_parameters.len()
        {
            return unsupported(
                "named operator crash invocation's boundary call disagrees with the operator requirement shape",
            );
        }
        validate_operator_signature(checked, site, &declaration.scalar_parameters)?;
        if declaration.crash_routes
            != lower_formal_crash_routes(
                identities,
                published_buckets(checked, site)?,
                &declaration.scalar_parameters,
            )?
        {
            return unsupported(
                "named operator crash invocation's boundary declaration publishes different crash routes than its checked site",
            );
        }
        Ok(())
    }

    fn emitted_operation(
        lowered: &LoweredPsi,
        site: &CheckedCrashOperatorSite,
    ) -> Result<
        (
            MachineId,
            OperationId,
            LoweredSelectedComparisonOperandOrder,
        ),
        LoweringError,
    > {
        let operator_use: CheckedOperatorUseHandle = site.operator_use;
        let mut joins = lowered
            .selected_comparison_occurrences
            .iter()
            .filter(|occurrence| occurrence.operator_use == operator_use)
            .map(|occurrence| {
                (
                    occurrence.terminal_machine,
                    occurrence.terminal_operation,
                    occurrence.operand_order,
                )
            });
        match (joins.next(), joins.next()) {
            (Some(join), None) => Ok(join),
            (None, _) => unsupported(
                "selected operator crash invocation has no emitted Terminal operation to carry its contract",
            ),
            (Some(_), Some(_)) => unsupported(
                "selected operator crash invocation joins more than one Terminal operation",
            ),
        }
    }

    fn reindex_formal_telescope(
        routes: Vec<terminal_psi::CrashRouteBucket>,
        authored_positions: &[usize],
        authored_operand_types: &[ScalarType],
    ) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
        if authored_positions
            .iter()
            .enumerate()
            .all(|(ordinal, position)| ordinal == *position)
        {
            return Ok(routes);
        }
        let renaming = authored_positions
            .iter()
            .zip(authored_operand_types)
            .enumerate()
            .map(|(ordinal, (position, scalar_type))| {
                Ok((
                    formal_identity(ordinal)?,
                    ScalarTerm::value(formal_identity(*position)?, *scalar_type),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
        Ok(terminal_semantics::substitution::substitute_crash_routes(
            &routes, &renaming,
        ))
    }

    fn formal_identity(ordinal: usize) -> Result<ValueId, LoweringError> {
        ValueId::new(dense_identity(ordinal)?).ok_or(LoweringError::Unsupported(
            "operator crash formal identity is zero",
        ))
    }

    fn published_buckets<'checked>(
        checked: &'checked CheckedTrees,
        site: &'checked CheckedCrashOperatorSite,
    ) -> Result<&'checked [checked_trees::CrashRouteBucket], LoweringError> {
        let Some(declaration) = checked
            .facts
            .contract_plans
            .for_machine(site.selected_operator)
        else {
            return Ok(site.published());
        };
        if declaration.crash.published() != site.published() {
            return unsupported(
                "operator crash site disagrees with its declaration's published crash routes",
            );
        }
        Ok(declaration.crash.published())
    }

    fn validate_operator_signature(
        checked: &CheckedTrees,
        site: &CheckedCrashOperatorSite,
        operand_types: &[ScalarType],
    ) -> Result<(), LoweringError> {
        let program = &checked.typed;
        let (parameters, _) = operator_declaration(checked, site.selected_operator).ok_or(
            LoweringError::Unsupported("operator crash site names no operator declaration"),
        )?;
        if parameters.len() != operand_types.len() {
            return unsupported(
                "operator crash site operand roster disagrees with the declared parameter count",
            );
        }
        for (parameter, operand_type) in parameters.iter().zip(operand_types) {
            let declared = program
                .primitive_type_reference(parameter.type_reference)
                .ok_or(LoweringError::Unsupported(
                    "operator crash contract requires an all-scalar operator signature",
                ))?;
            if terminal_scalar_type(declared)? != *operand_type {
                return unsupported(
                    "operator crash site operand type disagrees with its declared parameter",
                );
            }
        }
        Ok(())
    }

    fn operator_declaration(
        checked: &CheckedTrees,
        symbol: symbols::SymbolHandle,
    ) -> Option<(
        &[checked_trees::signature::StateParameter],
        arena::HandleSpan<SignatureContract>,
    )> {
        let program = &checked.typed;
        typed_trees::operator::declaration_by_symbol(program, symbol)
            .map(|operator| (program.operator_parameters(operator), operator.contracts))
    }

    fn positional_scalar_operands(kind: &OperationKind) -> Result<Vec<ValueId>, LoweringError> {
        if direct_call_parts(kind).is_some() || matches!(kind, OperationKind::BoundaryCall { .. }) {
            return unsupported("operator crash site joins a call operation");
        }
        let mut operands = Vec::new();
        let mut inventory = kind.clone();
        inventory.map_scalar_uses(&mut |operand| {
            operands.push(operand);
            operand
        });
        if operands.is_empty() {
            return unsupported("operator crash site joins an operation without scalar operands");
        }
        Ok(operands)
    }

    fn scalar_value_type(machine: &TerminalMachine, value: ValueId) -> Option<ScalarType> {
        machine
            .parameters
            .iter()
            .chain(machine.block_parameters())
            .map(|declaration| (declaration.id, declaration.scalar_type))
            .chain(
                machine
                    .operations()
                    .filter_map(|operation| operation.result.scalar())
                    .map(|declaration| (declaration.id, declaration.scalar_type)),
            )
            .find_map(|(id, scalar_type)| (id == value).then_some(scalar_type))
    }
}
pub(crate) mod payload_memberships {
    use crate::shared::lowering_error::LoweringError;
    pub(crate) mod membership_reachability {
        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::{
            StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
            StructuralTypeShape,
        };

        pub(crate) struct MembershipReachability {
            rows: Vec<(StructuralTypeId, bool)>,
        }
        impl MembershipReachability {
            pub(crate) fn new(
                types: &[StructuralTypeDeclaration],
                carriers: impl IntoIterator<Item = StructuralTypeId>,
            ) -> Self {
                let carriers: Vec<_> = carriers.into_iter().collect();
                let scalar_domains = carriers.iter().any(|carrier| {
                    types.iter().any(|declaration| {
                        declaration.id == *carrier
                            && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
                    })
                });
                let mut result = Self {
                    rows: types
                        .iter()
                        .map(|declaration| {
                            (
                                declaration.id,
                                carriers.contains(&declaration.id)
                                    || declaration.quotient_carrier.is_some()
                                    || declaration.scalar_restriction.is_some(),
                            )
                        })
                        .collect(),
                };
                loop {
                    let mut changed = false;
                    for (index, declaration) in types.iter().enumerate() {
                        if result.rows[index].1 {
                            continue;
                        }
                        let fields = |fields: &[StructuralFieldDeclaration]| {
                            fields
                                .iter()
                                .filter(|field| !field.relevance.is_erased())
                                .any(|field| match field.field_type {
                                    StructuralFieldType::Structural(child) => {
                                        result.may_contain(child)
                                    }
                                    StructuralFieldType::Scalar(_)
                                    | StructuralFieldType::IeeeFloat(_) => scalar_domains,
                                    StructuralFieldType::Erased { .. } => false,
                                    StructuralFieldType::Atomic(_)
                                    | StructuralFieldType::AtomicBoundedInteger(_)
                                    | StructuralFieldType::BoundedInteger(_)
                                    | StructuralFieldType::RefinedScalar(_)
                                    | StructuralFieldType::ByteSequence(_)
                                    | StructuralFieldType::PredicateByteSequence { .. } => true,
                                })
                        };
                        let reachable = match &declaration.shape {
                            StructuralTypeShape::Reference { referent, .. } => {
                                result.may_contain(*referent)
                            }
                            StructuralTypeShape::FixedArray { element, .. } => {
                                result.may_contain(*element)
                            }
                            StructuralTypeShape::ElementView { .. } => true,
                            StructuralTypeShape::Record { fields: rows, .. } => fields(rows),
                            StructuralTypeShape::Sum { cases } => {
                                cases.iter().any(|case| fields(&case.fields))
                            }
                            StructuralTypeShape::Mixed {
                                fields: rows,
                                cases,
                            } => fields(rows) || cases.iter().any(|case| fields(&case.fields)),
                            StructuralTypeShape::PrimitiveScalar(_) => scalar_domains,
                            StructuralTypeShape::ByteSequence(_)
                            | StructuralTypeShape::AtomicCell(_) => true,
                        };
                        if reachable {
                            result.rows[index].1 = true;
                            changed = true;
                        }
                    }
                    if !changed {
                        return result;
                    }
                }
            }
            pub(crate) fn may_contain(&self, identity: StructuralTypeId) -> bool {
                let mut rows = self.rows.iter().filter(|row| row.0 == identity);
                let Some(row) = rows.next() else {
                    return true;
                };
                rows.next().is_some() || row.1
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            fn id(value: u64) -> StructuralTypeId {
                StructuralTypeId::new(value).unwrap()
            }
            fn declaration(value: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
                StructuralTypeDeclaration {
                    requires: Vec::new(),
                    id: id(value),
                    identity: terminal_psi::TerminalIdentityIndex(0),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    copyable: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    shape,
                }
            }
            #[test]
            fn domain_reachability_follows_children_and_conservatively_keeps_unknowns() {
                let types = vec![
                    declaration(1, StructuralTypeShape::Sum { cases: Vec::new() }),
                    declaration(
                        2,
                        StructuralTypeShape::FixedArray {
                            element: id(1),
                            length: terminal_psi::ArrayExtent::Literal(4),
                        },
                    ),
                    declaration(3, StructuralTypeShape::Sum { cases: Vec::new() }),
                    declaration(
                        4,
                        StructuralTypeShape::Reference {
                            referent: id(2),
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                        },
                    ),
                    declaration(
                        5,
                        StructuralTypeShape::FixedArray {
                            element: id(5),
                            length: terminal_psi::ArrayExtent::Literal(1),
                        },
                    ),
                    declaration(
                        6,
                        StructuralTypeShape::Reference {
                            referent: id(9),
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                        },
                    ),
                ];
                let unrelated = MembershipReachability::new(&types, [id(3)]);
                assert!(!unrelated.may_contain(id(4)));
                assert!(!unrelated.may_contain(id(5)));
                assert!(unrelated.may_contain(id(6)));
                let nested = MembershipReachability::new(&types, [id(1)]);
                assert!(nested.may_contain(id(2)));
                assert!(nested.may_contain(id(4)));
                assert!(!nested.may_contain(id(3)));
                assert!(nested.may_contain(id(99)));
            }
        }
    }
    pub(crate) mod operand_memberships {
        use super::*;
        use terminal_psi::{StructuralPathQualification, StructuralPathSegment};

        pub(crate) fn retained(
            module: &TerminalModule,
            machine: &terminal_psi::TerminalMachine,
            operation: &terminal_psi::Operation,
            position: (semantic_vocabulary::BlockId, u32),
            analysis: &mut descriptors::DescriptorAnalysis<'_>,
            history: &mut descriptors::PayloadMembershipHistory<'_>,
            memberships: &super::membership_reachability::MembershipReachability,
        ) -> Option<PayloadMembershipContract> {
            let result = operation.result.structural()?;
            if !memberships.may_contain(result.structural_type) {
                return None;
            }
            let shape = &module.structural_type(result.structural_type)?.shape;
            let mut projected_qualifications = Vec::new();
            match &operation.kind {
                OperationKind::MoveStructuralField {
                    source,
                    path,
                    field,
                } => {
                    let root = descriptors::terminal_payload_contract(machine, *source)?;
                    let projection = terminal_semantics::structural_window_projection(
                        module.structural_types.iter(),
                        root.carrier,
                        path,
                        *field,
                    )?;
                    if projection.structural_type != result.structural_type {
                        return None;
                    }
                    let (qualifications, projected_qualifications) =
                        StructuralPathQualification::select_restorable(
                            &root.qualifications,
                            &root.projected_qualifications,
                            &projection.path,
                        )?;
                    return Some(PayloadMembershipContract {
                        scalar_observations: Vec::new(),
                        scalar_requirements: Vec::new(),
                        carrier: result.structural_type,
                        qualifications,
                        projected_qualifications,
                    });
                }
                OperationKind::EstablishReference { source, storage } => {
                    let root = descriptors::terminal_payload_contract(machine, source.place)?;
                    let path = terminal_semantics::reference_source_storage_path(
                        module.structural_types.iter(),
                        root.carrier,
                        &source.path,
                    );
                    projected_qualifications = terminal_semantics::reference_carrier_memberships(
                        module.structural_types.iter(),
                        root.carrier,
                        &root.qualifications,
                        &root.projected_qualifications,
                        path,
                        source.access,
                        result.structural_type,
                        storage.as_ref(),
                    )
                    .ok()?;
                    if let Ok(captured) = descriptors::reference_backing_memberships(
                        &module.structural_types,
                        &module.structural_domains,
                        &module.identities,
                        analysis,
                        machine.id,
                        position,
                        source,
                        storage.as_ref(),
                        result,
                        |place| descriptors::terminal_payload_contract(machine, place),
                    ) {
                        projected_qualifications.extend(captured);
                    }
                    if source.access == terminal_psi::StructuralAccess::SharedBorrow
                        && memberships.may_contain(root.carrier)
                        && let Ok(current) = history.current_selected_payload_contract_in(
                            analysis,
                            position,
                            source.place,
                            path,
                        )
                    {
                        StructuralPathQualification::append_under(
                            &current.qualifications,
                            &current.projected_qualifications,
                            &[StructuralPathSegment::Referent],
                            &mut projected_qualifications,
                        );
                    }
                }
                OperationKind::EstablishStructuralArray { elements } => {
                    for (ordinal, argument) in elements.iter().enumerate() {
                        append(
                            machine,
                            argument,
                            &[StructuralPathSegment::FixedIndex(
                                u64::try_from(ordinal).ok()?,
                            )],
                            &mut projected_qualifications,
                        )?;
                    }
                }
                OperationKind::EstablishRecord { fields } => {
                    append_fields(
                        machine,
                        fields,
                        shape.record_fields()?,
                        &[],
                        &mut projected_qualifications,
                    )?;
                }
                OperationKind::EstablishStructuralCase {
                    result_case,
                    fields,
                } => {
                    let common = shape.direct_fields().unwrap_or(&[]);
                    let case = shape
                        .sum_cases()?
                        .iter()
                        .find(|case| case.id == *result_case)?;
                    let (common_values, case_values) = fields.split_at_checked(
                        common
                            .iter()
                            .filter(|field| !field.relevance.is_erased())
                            .count(),
                    )?;
                    append_fields(
                        machine,
                        common_values,
                        common,
                        &[],
                        &mut projected_qualifications,
                    )?;
                    append_fields(
                        machine,
                        case_values,
                        &case.fields,
                        &[StructuralPathSegment::Case(*result_case)],
                        &mut projected_qualifications,
                    )?;
                }
                _ => return None,
            }
            if projected_qualifications.is_empty() {
                return None;
            }
            projected_qualifications.sort();
            projected_qualifications.dedup();
            Some(PayloadMembershipContract {
                scalar_observations: Vec::new(),
                scalar_requirements: Vec::new(),
                carrier: result.structural_type,
                qualifications: Vec::new(),
                projected_qualifications,
            })
        }

        fn append_fields(
            machine: &terminal_psi::TerminalMachine,
            values: &[terminal_psi::RecordFieldInitializer],
            declarations: &[terminal_psi::StructuralFieldDeclaration],
            prefix: &[StructuralPathSegment],
            destination: &mut Vec<StructuralPathQualification>,
        ) -> Option<()> {
            let declarations = declarations
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            if values.len() != declarations.len() {
                return None;
            }
            for (value, declaration) in values.iter().zip(declarations) {
                if value.field != declaration.id {
                    return None;
                }
                if let terminal_psi::RecordFieldValue::Structural(argument) = &value.value
                    && matches!(
                        declaration.field_type,
                        terminal_psi::StructuralFieldType::Structural(_)
                    )
                {
                    let mut path = prefix.to_vec();
                    path.push(StructuralPathSegment::Field(declaration.identity));
                    append(machine, argument, &path, destination)?;
                }
            }
            Some(())
        }

        fn append(
            machine: &terminal_psi::TerminalMachine,
            argument: &terminal_psi::StructuralArgument,
            prefix: &[StructuralPathSegment],
            destination: &mut Vec<StructuralPathQualification>,
        ) -> Option<()> {
            let root = descriptors::terminal_payload_contract(machine, argument.place)?;
            let (whole, projected) = StructuralPathQualification::select(
                &root.qualifications,
                &root.projected_qualifications,
                &argument.path,
            );
            StructuralPathQualification::append_under(&whole, &projected, prefix, destination);
            Some(())
        }
    }
    pub(crate) mod view_memberships {
        use super::*;

        pub(crate) fn retained(
            module: &TerminalModule,
            machine: &terminal_psi::TerminalMachine,
            operation: &terminal_psi::Operation,
            position: (semantic_vocabulary::BlockId, u32),
            analysis: &mut descriptors::DescriptorAnalysis<'_>,
            memberships: &super::membership_reachability::MembershipReachability,
        ) -> Option<PayloadMembershipContract> {
            let result = operation.result.structural()?;
            if !memberships.may_contain(result.structural_type) {
                return None;
            }
            let terminal_psi::StructuralTypeShape::ElementView { element, .. } =
                terminal_psi::find_structural_type(
                    &module.structural_types,
                    result.structural_type,
                )?
                .shape
            else {
                return None;
            };
            let source = match &operation.kind {
                OperationKind::EstablishElementView { source, .. }
                | OperationKind::EstablishMutableElementView { source, .. }
                | OperationKind::ElementViewSubslice { source, .. } => source,
                _ => return None,
            };
            let evidence = descriptors::element_payload_memberships(
                &module.structural_types,
                &module.structural_domains,
                machine,
                analysis,
                position,
                source,
                element,
            )?;
            let mut projected_qualifications = Vec::new();
            match evidence {
                descriptors::ElementPayloadMemberships::Present(contract) => {
                    terminal_psi::StructuralPathQualification::append_under(
                        &contract.qualifications,
                        &contract.projected_qualifications,
                        &[terminal_psi::StructuralPathSegment::AllElements],
                        &mut projected_qualifications,
                    );
                }
                descriptors::ElementPayloadMemberships::Empty => {
                    let same_element_carrier = |candidate| {
                        terminal_semantics::borrowed_referent_presentation(
                            module.structural_types.iter(),
                            result.structural_type,
                            candidate,
                        ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                    };
                    let valid_universal = |row: &&terminal_psi::StructuralPathQualification| {
                        matches!(
                            row.path.first(),
                            Some(terminal_psi::StructuralPathSegment::AllElements)
                        ) && terminal_semantics::qualification_path_carrier(
                            module.structural_types.iter(),
                            result.structural_type,
                            &row.path,
                        )
                        .is_some_and(|carrier| {
                            module
                                .structural_domains
                                .iter()
                                .any(|domain| domain.id == row.domain && domain.carrier == carrier)
                        })
                    };
                    for machine in &module.machines {
                        for parameter in machine.structural_parameters.iter().chain(
                            machine
                                .blocks
                                .iter()
                                .flat_map(|block| block.structural_parameters.iter()),
                        ) {
                            if same_element_carrier(parameter.structural_type) {
                                projected_qualifications.extend(
                                    parameter
                                        .projected_qualifications
                                        .iter()
                                        .filter(valid_universal)
                                        .cloned(),
                                );
                            }
                        }
                        if let Some(expected) = machine.result.structural()
                            && same_element_carrier(expected.structural_type)
                        {
                            projected_qualifications.extend(
                                expected
                                    .projected_qualifications
                                    .iter()
                                    .filter(valid_universal)
                                    .cloned(),
                            );
                        }
                    }
                }
            }
            if projected_qualifications.is_empty() {
                return None;
            }
            projected_qualifications.sort();
            projected_qualifications.dedup();
            Some(PayloadMembershipContract {
                scalar_observations: Vec::new(),
                scalar_requirements: Vec::new(),
                carrier: result.structural_type,
                qualifications: Vec::new(),
                projected_qualifications,
            })
        }
    }
    use terminal_psi::{OperationKind, OperationResult, TerminalModule};
    use terminal_semantics::descriptor_results::{self as descriptors, PayloadMembershipContract};

    pub(crate) fn retain(module: &mut TerminalModule) -> Result<(), LoweringError> {
        if module.structural_domains.is_empty() {
            return Ok(());
        }
        let memberships = membership_reachability::MembershipReachability::new(
            &module.structural_types,
            module
                .structural_domains
                .iter()
                .map(|domain| domain.carrier),
        );
        let candidate_machines = module
            .machines
            .iter()
            .map(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| block.operations.iter())
                    .any(|operation| {
                        operation
                            .result
                            .structural()
                            .is_some_and(|result| memberships.may_contain(result.structural_type))
                    })
            })
            .collect::<Vec<_>>();
        if !candidate_machines.iter().any(|&has| has) {
            return Ok(());
        }
        let types = module.structural_types.clone();
        let graph = descriptors::terminal_graph_from(
            module,
            module
                .machines
                .iter()
                .zip(&candidate_machines)
                .filter(|(_, candidate)| **candidate)
                .map(|(machine, _)| machine.id),
        );
        let mut analysis = descriptors::DescriptorAnalysis::new(&types, &graph);
        for (machine_index, has_candidate) in candidate_machines.into_iter().enumerate() {
            if !has_candidate {
                continue;
            }
            loop {
                let mut copies = Vec::new();
                let reference_memberships =
                    terminal_semantics::ReferenceResultMemberships::new(module);
                {
                    let machine = &module.machines[machine_index];
                    let mut history = descriptors::PayloadMembershipHistory::new(
                        &types,
                        &module.structural_domains,
                        &graph,
                        machine,
                    );
                    for (block_index, block) in machine.blocks.iter().enumerate() {
                        for (ordinal, operation) in block.operations.iter().enumerate() {
                            let Some(result) = operation.result.structural() else {
                                continue;
                            };
                            if !memberships.may_contain(result.structural_type) {
                                continue;
                            }
                            let position = (
                                block.id,
                                u32::try_from(ordinal).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "membership operation ordinal overflow",
                                    )
                                })?,
                            );
                            if let Ok(projected_qualifications) =
                                reference_memberships.for_operation(machine, operation)
                                && !projected_qualifications.is_empty()
                            {
                                copies.push((
                                    block_index,
                                    ordinal,
                                    PayloadMembershipContract {
                                        scalar_observations: Vec::new(),
                                        scalar_requirements: Vec::new(),
                                        carrier: result.structural_type,
                                        qualifications: Vec::new(),
                                        projected_qualifications,
                                    },
                                ));
                            }
                            if let Some(contract) = view_memberships::retained(
                                module,
                                machine,
                                operation,
                                position,
                                &mut analysis,
                                &memberships,
                            ) {
                                copies.push((block_index, ordinal, contract));
                            }
                            if let Some(contract) = operand_memberships::retained(
                                module,
                                machine,
                                operation,
                                position,
                                &mut analysis,
                                &mut history,
                                &memberships,
                            ) {
                                copies.push((block_index, ordinal, contract));
                            }
                            let selected_source = match &operation.kind {
                                OperationKind::MoveStructuralValue { source } => {
                                    Some((source.place, source.path.as_slice()))
                                }
                                OperationKind::StructuralLeafCopy { source, path } => {
                                    Some((*source, path.as_slice()))
                                }
                                _ => None,
                            };
                            if let Some((source, path)) = selected_source
                                && let Ok(root) = history.current_payload_contract_in(
                                    &mut analysis,
                                    position,
                                    source,
                                )
                                && terminal_semantics::qualification_path_carrier(
                                    types.iter(),
                                    root.carrier,
                                    path,
                                ) == Some(result.structural_type)
                            {
                                let (qualifications, projected_qualifications) =
                                    terminal_psi::StructuralPathQualification::select(
                                        &root.qualifications,
                                        &root.projected_qualifications,
                                        path,
                                    );
                                if !qualifications.is_empty()
                                    || !projected_qualifications.is_empty()
                                {
                                    copies.push((
                                        block_index,
                                        ordinal,
                                        PayloadMembershipContract {
                                            scalar_observations: Vec::new(),
                                            scalar_requirements: Vec::new(),
                                            carrier: result.structural_type,
                                            qualifications,
                                            projected_qualifications,
                                        },
                                    ));
                                }
                            }
                            let OperationKind::StructuralLeafCopy { source, path } =
                                &operation.kind
                            else {
                                continue;
                            };
                            let Some(root) =
                                descriptors::terminal_payload_contract(machine, *source)
                            else {
                                continue;
                            };
                            let Some(query) = descriptors::descriptor_contents_query(
                                &types,
                                *source,
                                root.carrier,
                                path,
                                None,
                            ) else {
                                continue;
                            };
                            let Some(targets) =
                                analysis.payload_targets(machine.id, position, &query)
                            else {
                                continue;
                            };
                            let Some(contract) = descriptors::payload_memberships(
                                &types,
                                &module.structural_domains,
                                targets,
                                |place| descriptors::terminal_payload_contract(machine, place),
                            ) else {
                                continue;
                            };
                            if contract.carrier == result.structural_type
                                && (!contract.qualifications.is_empty()
                                    || !contract.projected_qualifications.is_empty())
                            {
                                copies.push((block_index, ordinal, contract));
                            }
                        }
                    }
                }
                drop(reference_memberships);
                let mut changed = false;
                let machine = &mut module.machines[machine_index];
                for (block, operation, contract) in copies {
                    let OperationResult::Structural(result) =
                        &mut machine.blocks[block].operations[operation].result
                    else {
                        continue;
                    };
                    changed |= merge(&mut result.qualifications, contract.qualifications);
                    changed |= merge(
                        &mut result.projected_qualifications,
                        contract.projected_qualifications,
                    );
                }
                if !changed {
                    break;
                }
            }
        }
        Ok(())
    }

    fn merge<T: Ord>(destination: &mut Vec<T>, source: Vec<T>) -> bool {
        let old_length = destination.len();
        destination.extend(source);
        destination.sort();
        destination.dedup();
        destination.len() != old_length
    }
}
pub(crate) mod reborrow_restored_call_use {
    use arena::Handle;
    use checked_trees::{
        CheckedParentBorrowResource, CheckedReborrowResourceDisposition,
        CheckedReborrowResourceDispositionEvent,
    };
    use semantic_vocabulary::MachineId;
    use terminal_psi::{
        TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse,
        TerminalReborrowSharedCohortMember,
    };

    use crate::shared::lowering_error::LoweringError;
    use crate::shared::lowering_error::unsupported;
    use checked_trees::CheckedTrees;
    use lowered_psi::LoweredSourceCallOccurrence;

    pub(crate) mod places {

        use checked_trees::{BorrowAccessKind, BorrowLoanOwnerSegment, FlowInvalidationSource};
        use terminal_psi::{
            StructuralAccess, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
            TerminalBorrowPlace, TerminalBorrowPlaceSegment,
        };

        use crate::digest_encoder::DomainDigestEncoder;
        use crate::shared::lowering_error::LoweringError;
        use crate::shared::lowering_error::unsupported;
        use crate::shared::terminal_identities::hex_text;
        use checked_trees::CheckedTrees;

        pub(crate) fn identity(
            checked: &CheckedTrees,
            symbol: symbols::SymbolHandle,
        ) -> Result<String, LoweringError> {
            if let Ok(identity) = checked.typed.normalized_hermetic_symbol_identity(symbol) {
                return Ok(identity);
            }
            if !symbol.is_valid() {
                return unsupported("reborrow custody contains an unresolved symbol");
            }
            let mut digest = DomainDigestEncoder::new(b"omega-terminal-reborrow-symbol-v1");
            digest.raw(checked.typed.symbols.display_path(symbol, "::").as_bytes());
            digest.raw(&symbol.arena_index().to_le_bytes());
            digest.raw(&symbol.generation().to_le_bytes());
            Ok(format!("terminal-borrow:{}", hex_text(&digest.finish())))
        }

        fn ordinal(value: usize) -> Result<u64, LoweringError> {
            value
                .try_into()
                .map_err(|_| LoweringError::Unsupported("reborrow custody coordinate is too large"))
        }

        pub(crate) fn storage_root_identity(
            checked: &CheckedTrees,
            root: facts::PlaceRoot,
        ) -> Result<String, LoweringError> {
            match root {
                facts::PlaceRoot::Symbol(symbol) => identity(checked, symbol),
                facts::PlaceRoot::Expression(expression) if expression.is_valid() => {
                    let mut digest =
                        DomainDigestEncoder::new(b"omega-terminal-computed-backing-v1");
                    digest.raw(&expression.arena_index().to_le_bytes());
                    digest.raw(&expression.generation().to_le_bytes());
                    Ok(format!(
                        "terminal-borrow-storage:{}",
                        hex_text(&digest.finish())
                    ))
                }
                _ => unsupported("reborrow custody contains an unresolved storage root"),
            }
        }

        pub(crate) fn boundary(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            source: FlowInvalidationSource,
        ) -> Result<TerminalBorrowBoundarySource, LoweringError> {
            Ok(match source {
                FlowInvalidationSource::Statement { statement_index } => {
                    TerminalBorrowBoundarySource::Statement {
                        statement_index: ordinal(statement_index)?,
                    }
                }
                FlowInvalidationSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol,
                } => TerminalBorrowBoundarySource::Call {
                    statement_index: ordinal(statement_index)?,
                    call_ordinal: ordinal(call_ordinal)?,
                    target_identity: identities.intern(&identity(checked, target_symbol)?),
                },
            })
        }

        pub(crate) fn owner_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            path: &[BorrowLoanOwnerSegment],
        ) -> Result<Vec<TerminalBorrowOwnerSegment>, LoweringError> {
            path.iter()
                .map(|segment| {
                    Ok(match segment {
                        BorrowLoanOwnerSegment::Field(symbol) => TerminalBorrowOwnerSegment::Field(
                            identities.intern(&identity(checked, *symbol)?),
                        ),
                        BorrowLoanOwnerSegment::Case(symbol) => TerminalBorrowOwnerSegment::Case(
                            identities.intern(&identity(checked, *symbol)?),
                        ),
                        BorrowLoanOwnerSegment::FixedIndex(index) => {
                            TerminalBorrowOwnerSegment::FixedIndex(ordinal(*index)?)
                        }
                        BorrowLoanOwnerSegment::DynamicIndex => {
                            TerminalBorrowOwnerSegment::DynamicIndex
                        }
                    })
                })
                .collect()
        }

        #[derive(Clone, Copy)]
        pub(crate) struct Capture {
            pub state: symbols::SymbolHandle,
            pub loan: arena::Handle<checked_trees::BorrowLoanFact>,
        }

        impl Capture {
            fn selector_identity(
                self,
                checked: &CheckedTrees,
                expression: typed_trees::expression::ExpressionHandle,
            ) -> Result<String, LoweringError> {
                let loan = checked.facts.borrow.loans.get(self.loan);
                if !expression.is_valid() {
                    return unsupported("reborrow custody selector has no captured occurrence");
                }
                let mut digest =
                    DomainDigestEncoder::new(b"omega-terminal-captured-borrow-selector-v1");
                digest.raw(identity(checked, self.state)?.as_bytes());
                digest.raw(&self.loan.arena_index().to_le_bytes());
                digest.raw(&self.loan.generation().to_le_bytes());
                digest.raw(&ordinal(loan.statement_index)?.to_le_bytes());
                digest.raw(&loan.formation_expression.arena_index().to_le_bytes());
                digest.raw(&loan.formation_expression.generation().to_le_bytes());
                digest.raw(&expression.arena_index().to_le_bytes());
                digest.raw(&expression.generation().to_le_bytes());
                Ok(format!(
                    "terminal-borrow-selector:{}",
                    hex_text(&digest.finish())
                ))
            }
        }

        pub(crate) fn place_segments(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            segments: &[facts::PlaceSegment],
            capture: Capture,
        ) -> Result<Vec<TerminalBorrowPlaceSegment>, LoweringError> {
            segments
                .iter()
                .map(|segment| {
                    Ok(match segment {
                        facts::PlaceSegment::Field { symbol } => TerminalBorrowPlaceSegment::Field(
                            identities.intern(&identity(checked, *symbol)?),
                        ),
                        facts::PlaceSegment::Case { variant } => TerminalBorrowPlaceSegment::Case(
                            identities.intern(&identity(checked, *variant)?),
                        ),
                        facts::PlaceSegment::FixedIndex { index } => {
                            TerminalBorrowPlaceSegment::FixedIndex(ordinal(*index)?)
                        }
                        facts::PlaceSegment::FixedRange { start, end } => {
                            TerminalBorrowPlaceSegment::FixedRange {
                                start: ordinal(*start)?,
                                end: ordinal(*end)?,
                            }
                        }
                        facts::PlaceSegment::Index { expression } => {
                            TerminalBorrowPlaceSegment::CapturedSelector(
                                identities
                                    .intern(&capture.selector_identity(checked, *expression)?),
                            )
                        }
                        facts::PlaceSegment::OpaqueRange => {
                            return unsupported(
                                "reborrow custody window has no captured occurrence",
                            );
                        }
                    })
                })
                .collect()
        }

        fn captured_root_identity(
            checked: &CheckedTrees,
            place: &checked_trees::CapturedPlace,
            capture: Capture,
        ) -> Result<String, LoweringError> {
            let root = storage_root_identity(checked, place.root)?;
            if !place.incoming_reference.is_valid() {
                return Ok(root);
            }
            let mut digest = DomainDigestEncoder::new(b"omega-terminal-incoming-borrow-root-v1");
            digest.bytes(root.as_bytes());
            digest.raw(&place.incoming_reference.arena_index().to_le_bytes());
            digest.raw(&place.incoming_reference.generation().to_le_bytes());
            digest.len(place.incoming_owner_path.len());
            for segment in &place.incoming_owner_path {
                match segment {
                    facts::PlaceSegment::Field { symbol } => {
                        digest.byte(0);
                        digest.bytes(identity(checked, *symbol)?.as_bytes());
                    }
                    facts::PlaceSegment::Case { variant } => {
                        digest.byte(1);
                        digest.bytes(identity(checked, *variant)?.as_bytes());
                    }
                    facts::PlaceSegment::FixedIndex { index } => {
                        digest.byte(2);
                        digest.u64(ordinal(*index)?);
                    }
                    facts::PlaceSegment::Index { expression } => {
                        digest.byte(3);
                        digest.bytes(capture.selector_identity(checked, *expression)?.as_bytes());
                    }
                    facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                        return unsupported(
                            "incoming descriptor selection is not an exact captured leaf",
                        );
                    }
                }
            }
            Ok(format!(
                "terminal-borrow-incoming:{}",
                hex_text(&digest.finish())
            ))
        }

        pub(crate) fn place(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            place: &checked_trees::CapturedPlace,
            capture: Capture,
        ) -> Result<TerminalBorrowPlace, LoweringError> {
            Ok(TerminalBorrowPlace {
                root_identity: identities.intern(&captured_root_identity(checked, place, capture)?),
                segments: place_segments(identities, checked, &place.segments, capture)?,
            })
        }

        pub(crate) fn projected_place(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            parent: &checked_trees::CapturedPlace,
            published_parent: &TerminalBorrowPlace,
            child: &checked_trees::CapturedPlace,
            capture: Capture,
        ) -> Result<(TerminalBorrowPlace, Vec<TerminalBorrowPlaceSegment>), LoweringError> {
            if parent.root != child.root
                || parent.incoming_reference != child.incoming_reference
                || parent.incoming_owner_path != child.incoming_owner_path
            {
                return unsupported("reborrow custody child changes its captured root");
            }
            let remainder = child
                .segments
                .strip_prefix(parent.segments.as_slice())
                .ok_or(LoweringError::Unsupported(
                    "reborrow custody child place is not under its parent",
                ))?;
            let projection = place_segments(identities, checked, remainder, capture)?;
            let mut place = published_parent.clone();
            place.segments.extend_from_slice(&projection);
            Ok((place, projection))
        }

        pub(crate) fn access(kind: BorrowAccessKind) -> StructuralAccess {
            match kind {
                BorrowAccessKind::Read => StructuralAccess::SharedBorrow,
                BorrowAccessKind::Mutable => StructuralAccess::MutableBorrow,
                BorrowAccessKind::WriteOnly => StructuralAccess::WriteOnlyBorrow,
            }
        }
    }

    use places::{Capture, access, boundary, identity, owner_path, place, projected_place};

    pub(crate) fn retain_selected_reborrow_restored_call_uses(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        source_machine: symbols::SymbolHandle,
        terminal_machine: MachineId,
        source_calls: &[LoweredSourceCallOccurrence],
        terminal_machines: &[terminal_psi::TerminalMachine],
        rows: &mut Vec<TerminalReborrowRestoredCallUse>,
    ) -> Result<(), LoweringError> {
        let borrow = &checked.facts.borrow;
        let flow = &checked.facts.flow;
        for (_, event) in borrow.reborrow_disposition_events.iter() {
            if event.machine_symbol != source_machine || !event.restored_call.is_valid() {
                continue;
            }
            let shared_freeze_cohort = restored_kind(event)?;
            if !borrow
                .reborrow_loan_resources
                .is_valid(event.child_resource)
                || !matches!(
                    event.parent_resource,
                    CheckedParentBorrowResource::DirectRoot { resource }
                        if borrow.direct_loan_resources.is_valid(resource)
                )
                || !flow
                    .borrow_lifetimes
                    .weakenings
                    .is_valid(event.child_weakening)
                || !flow
                    .borrow_lifetimes
                    .weakenings
                    .is_valid(child_parent_weakening_handle(borrow, event)?)
                || !flow.control.calls.is_valid(event.restored_call)
                || !borrow.calls.is_valid(event.restored_borrow_call)
                || !borrow
                    .argument_accesses
                    .is_valid(event.restored_call_access)
            {
                return unsupported(
                    "restored reborrow call use contains an invalid checked handle",
                );
            }

            let child = borrow.reborrow_loan_resources.get(event.child_resource);
            let CheckedParentBorrowResource::DirectRoot {
                resource: parent_resource,
            } = event.parent_resource
            else {
                return unsupported("restored reborrow call use names a non-root parent");
            };
            let parent = borrow.direct_loan_resources.get(parent_resource);
            let child_activation = flow
                .borrow_lifetimes
                .activations
                .get(child.parent_suspension.child_activation);
            let child_weakening = flow
                .borrow_lifetimes
                .weakenings
                .get(child.retention.final_weakening);
            let parent_weakening = flow
                .borrow_lifetimes
                .weakenings
                .get(parent.retention.final_weakening);
            let call = flow.control.calls.get(event.restored_call);
            if !flow
                .contexts
                .constraint_refs
                .is_valid(child.parent_suspension.parent_entry_constraint)
            {
                return unsupported(
                    "restored reborrow call use formation constraint handle is invalid",
                );
            }
            if child.machine_symbol != event.machine_symbol
                || child.state_symbol != event.state_symbol
                || child.loan != event.child_loan
                || child.parent_loan != event.parent_loan
                || child.parent_resource != event.parent_resource
                || child.parent_suspension.child_activation != event.child_activation
                || child.parent_end_status.child_weakening != event.child_weakening
                || child.weakening_source != event.boundary_source
                || parent.loan != event.parent_loan
            {
                return unsupported("restored reborrow call use checked rows disagree");
            }

            let direct_root_place = place(
                identities,
                checked,
                &parent.captured_place,
                Capture {
                    state: parent.state_symbol,
                    loan: parent.loan,
                },
            )?;
            let mut shared_cohort = Vec::new();
            if shared_freeze_cohort {
                for member_handle in &event.shared_cohort {
                    let member = borrow.reborrow_loan_resources.get(*member_handle);
                    let member_activation = flow
                        .borrow_lifetimes
                        .activations
                        .get(member.parent_suspension.child_activation);
                    let member_weakening = flow
                        .borrow_lifetimes
                        .weakenings
                        .get(member.retention.final_weakening);
                    shared_cohort.push(TerminalReborrowSharedCohortMember {
                        child_owner_identity: identities
                            .intern(&identity(checked, member.owner_symbol)?),
                        child_owner_path: owner_path(identities, checked, &member.owner_path)?,
                        child_place: projected_place(
                            identities,
                            checked,
                            &parent.captured_place,
                            &direct_root_place,
                            &member.captured_place,
                            Capture {
                                state: member.state_symbol,
                                loan: member.loan,
                            },
                        )?
                        .0,
                        child_access: access(member.access.clone()),
                        child_activation: boundary(identities, checked, member_activation.source)?,
                        child_weakening: boundary(identities, checked, member_weakening.source)?,
                    });
                }
            }

            let occurrences = source_calls
                .iter()
                .filter(|occurrence| {
                    occurrence.matches_statement(
                        event.state_symbol,
                        call.statement_index,
                        call.call_ordinal,
                    ) && occurrence.source_target == call.target_symbol
                })
                .collect::<Vec<_>>();
            let [occurrence] = occurrences.as_slice() else {
                return unsupported(
                    "restored reborrow call use does not name one exact Terminal call occurrence",
                );
            };
            let terminal_operation =
                occurrence
                    .terminal_site
                    .operation()
                    .ok_or(LoweringError::Unsupported(
                        "external completion cannot establish a restored reborrow receipt",
                    ))?;
            let caller = terminal_machines
                .iter()
                .find(|machine| machine.id == terminal_machine)
                .ok_or(LoweringError::Unsupported(
                    "restored reborrow call use names an absent Terminal machine",
                ))?;
            let operations = caller
                .operations()
                .filter(|operation| operation.id == terminal_operation)
                .collect::<Vec<_>>();
            let [operation] = operations.as_slice() else {
                return unsupported(
                    "restored reborrow call use does not name one exact Terminal operation",
                );
            };
            let terminal_psi::OperationKind::CallUnit {
                callee,
                structural_arguments,
                ..
            } = &operation.kind
            else {
                return unsupported("restored reborrow call use does not name a Unit call");
            };
            let [_] = structural_arguments.as_slice() else {
                return unsupported(
                    "restored reborrow call use requires one exact structural argument",
                );
            };
            let callee_id = *callee;
            let callee = terminal_machines
                .iter()
                .find(|machine| machine.id == callee_id)
                .ok_or(LoweringError::Unsupported(
                    "restored reborrow call use names an absent Terminal callee",
                ))?;
            let [_] = callee.structural_parameters.as_slice() else {
                return unsupported(
                    "restored reborrow call use callee does not have one structural parameter",
                );
            };
            let child_owner_identity = identity(checked, child.owner_symbol)?;
            let child_owner_path = owner_path(identities, checked, &child.owner_path)?;
            let (child_place, projection_remainder) = projected_place(
                identities,
                checked,
                &parent.captured_place,
                &direct_root_place,
                &child.captured_place,
                Capture {
                    state: child.state_symbol,
                    loan: child.loan,
                },
            )?;
            let child_access = access(child.access.clone());
            let child_activation = boundary(identities, checked, child_activation.source)?;
            let child_weakening = boundary(identities, checked, child_weakening.source)?;
            rows.push(TerminalReborrowRestoredCallUse {
                machine: terminal_machine,
                operation: terminal_operation,
                restoration_class: if shared_freeze_cohort {
                    TerminalReborrowRestorationClass::SharedFreezeRestoration
                } else {
                    TerminalReborrowRestorationClass::ExclusiveReactivation
                },
                call_boundary: terminal_psi::TerminalBorrowBoundarySource::Call {
                    statement_index: u64::try_from(call.statement_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "restored reborrow call statement index exceeds Terminal range",
                        )
                    })?,
                    call_ordinal: u64::try_from(call.call_ordinal).map_err(|_| {
                        LoweringError::Unsupported(
                            "restored reborrow call ordinal exceeds Terminal range",
                        )
                    })?,
                    target_identity: identities.intern(&identity(checked, call.target_symbol)?),
                },
                call_target_machine: callee_id,
                source_machine_identity: identities
                    .intern(&identity(checked, event.machine_symbol)?),
                source_state_identity: identities.intern(&identity(checked, event.state_symbol)?),
                direct_root_owner_identity: identities
                    .intern(&identity(checked, parent.owner_symbol)?),
                direct_root_owner_path: owner_path(identities, checked, &parent.owner_path)?,
                direct_root_place,
                direct_root_activation: boundary(identities, checked, parent.activation_source)?,
                direct_root_weakening: boundary(identities, checked, parent_weakening.source)?,
                direct_root_lifetime_identity: identities.intern(&places::storage_root_identity(
                    checked,
                    parent.parent_lifetime.root,
                )?),
                child_owner_identity: identities.intern(&child_owner_identity),
                child_owner_path,
                child_place,
                projection_remainder,
                child_access,
                child_activation,
                formation_boundary: boundary(identities, checked, child.parent_suspension.source)?,
                child_weakening,
                shared_cohort,
            });
        }
        rows.sort();
        if !rows.windows(2).all(|pair| pair[0] < pair[1]) {
            return unsupported("duplicate restored reborrow call-use publication");
        }
        Ok(())
    }

    fn restored_kind(
        event: &CheckedReborrowResourceDispositionEvent,
    ) -> Result<bool, LoweringError> {
        match event.disposition {
            CheckedReborrowResourceDisposition::Reactivate => Ok(false),
            CheckedReborrowResourceDisposition::RestoreSharedCohort => {
                if !matches!(event.shared_cohort.len(), 1..=3) {
                    return unsupported(
                        "restored shared-freeze reborrow cohort is outside the retained bound",
                    );
                }
                Ok(true)
            }
            _ => unsupported("restored reborrow call use names a non-restoration disposition"),
        }
    }

    fn child_parent_weakening_handle(
        borrow: &checked_trees::BorrowFacts,
        event: &CheckedReborrowResourceDispositionEvent,
    ) -> Result<Handle<checked_trees::FlowBorrowWeakeningFact>, LoweringError> {
        if !borrow
            .reborrow_loan_resources
            .is_valid(event.child_resource)
        {
            return unsupported("restored reborrow call use child resource handle is invalid");
        }
        let weakening = match borrow
            .reborrow_loan_resources
            .get(event.child_resource)
            .parent_end_status
            .parent_end
        {
            checked_trees::CheckedBorrowParentEnd::Lexical { weakening } => weakening,
            checked_trees::CheckedBorrowParentEnd::CompletedOutput { holder_weakening } => {
                holder_weakening
            }
        };
        Ok(weakening)
    }
}
pub(crate) mod retained_borrow_custody {
    use super::{
        BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineId,
        BoundaryMachineResult, CheckedTrees, ContentPlaceVersion, DomainSemanticId, LoweringError,
        Multiplicity, StructuralAccess, StructuralMultiplicity, TerminalModule,
        checked_unit_boundary_identity, content_conservation, unsupported,
    };
    use crate::machine_lowering::declarations::nominal;
    use checked_trees::types::TypeReferenceNode;

    fn lower_projection(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        projection: &language_semantics::content::ContentProjectionPlan,
    ) -> Result<terminal_psi::RetainedBorrowContentProjection, LoweringError> {
        let exact = checked
            .facts
            .qualifications
            .content
            .for_semantic_domain(projection.semantic_domain)
            .ok_or(LoweringError::Unsupported(
                "retained-borrow projection is absent from checked content facts",
            ))?;
        let projection = content_conservation::lower_structural_content_projection(
            checked,
            projection.semantic_domain,
            &projection.carrier_identity,
        )?
        .ok_or(LoweringError::Unsupported(
            "retained-borrow projection has no complete Terminal definition",
        ))?;
        Ok(terminal_psi::RetainedBorrowContentProjection {
            semantic_domain: DomainSemanticId::new(projection.identity.domain.get())
                .ok_or(LoweringError::InvalidContentDomainIdentity)?,
            carrier_identity: identities.intern(&exact.carrier_identity.clone()),
            projection,
        })
    }

    fn lower_custody(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        fact: &checked_trees::RetainedBorrowCustodyFact,
    ) -> Result<terminal_psi::RetainedBorrowCustody, LoweringError> {
        let requirements = checked
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| checked.executable_trait_machine_instances(definition))
            .filter(|signature| signature.symbol == fact.callable)
            .collect::<Vec<_>>();
        let [signature] = requirements.as_slice() else {
            return unsupported("retained-borrow callable is not one exact boundary requirement");
        };
        let callable_identity =
            checked_unit_boundary_identity(checked, fact.callable)?.render(checked)?;
        let callable_lifetime_parameter_count = u32::try_from(signature.lifetime_parameters.len())
            .map_err(|_| {
                LoweringError::Unsupported("retained-borrow callable lifetime count exceeds u32")
            })?;
        if usize::try_from(fact.callable_lifetime_parameter_ordinal)
            .ok()
            .and_then(|ordinal| signature.lifetime_parameters.get(ordinal))
            != Some(&fact.lifetime)
        {
            return unsupported("retained-borrow callable lifetime ordinal does not replay");
        }

        let language_semantics::content::ContentPlaceRoot::Parameter {
            position,
            symbol,
            name,
        } = &fact.source.root
        else {
            return unsupported("retained-borrow source is not a direct parameter");
        };
        let parameter = checked
            .state_signature_parameters(signature)
            .get(*position as usize)
            .ok_or(LoweringError::Unsupported(
                "retained-borrow source parameter is out of range",
            ))?;
        if parameter.symbol != *symbol
            || parameter.name.as_str() != name
            || fact.source.version != language_semantics::content::ContentPlaceVersion::Entry
            || !fact.source.segments.is_empty()
            || !matches!(
                fact.access,
                language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable
            )
        {
            return unsupported("retained-borrow source place or access does not replay");
        }
        let source_ty = checked
            .type_reference_table
            .unconstrained_type_reference(parameter.type_reference);
        let TypeReferenceNode::Reference {
            lifetime: Some(_), ..
        } = checked.type_reference_table.type_reference(source_ty)
        else {
            return unsupported("retained-borrow source is not one explicit direct reference");
        };

        if fact.result.version != language_semantics::content::ContentPlaceVersion::Current
            || !matches!(
                fact.result.root,
                language_semantics::content::ContentPlaceRoot::Result
            )
            || !fact.result.segments.is_empty()
        {
            return unsupported("retained-borrow result place does not replay");
        }
        let result_ty = checked
            .type_reference_table
            .unconstrained_type_reference(signature.return_type);
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = checked.type_reference_table.type_reference(result_ty)
        else {
            return unsupported(
                "retained-borrow result is not the bounded nominal lifetime carrier",
            );
        };
        if *base_symbol != fact.result_data {
            return unsupported("retained-borrow result nominal owner does not replay");
        }
        if lifetime_arguments.as_slice() != [fact.lifetime.clone()]
            || fact.result_lifetime_argument_ordinal != 0
        {
            return unsupported("retained-borrow result lifetime slot does not replay");
        }
        if !checked
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty()
        {
            return unsupported("retained-borrow result has runtime generic arguments");
        }
        if checked.type_multiplicity(signature.return_type) != Multiplicity::Linear {
            return unsupported("retained-borrow result is not linear");
        }
        if fact.retained_semantic_domain != fact.result_projection.semantic_domain
            || fact.source_projection.algebra != fact.result_projection.algebra
        {
            return unsupported("retained-borrow result projection does not replay");
        }

        let source_identity = parameter.name.as_str().to_owned();
        let source_projection = lower_projection(identities, checked, &fact.source_projection)?;
        let result_projection = lower_projection(identities, checked, &fact.result_projection)?;
        Ok(terminal_psi::RetainedBorrowCustody {
            callable_identity: identities.intern(&callable_identity),
            source: terminal_psi::RetainedBorrowPlace {
                version: ContentPlaceVersion::Entry,
                root: terminal_psi::RetainedBorrowPlaceRoot::Parameter {
                    position: *position,
                    identity: identities.intern(&source_identity),
                },
                segments: Vec::new(),
            },
            result: terminal_psi::RetainedBorrowPlace {
                version: ContentPlaceVersion::Current,
                root: terminal_psi::RetainedBorrowPlaceRoot::Result,
                segments: Vec::new(),
            },
            access: match fact.access {
                language_semantics::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
                language_semantics::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
                language_semantics::ReferenceAccess::WriteOnly => {
                    return unsupported("retained-borrow access requires readable storage");
                }
            },
            callable_lifetime_parameter_count,
            callable_lifetime_parameter_ordinal: fact.callable_lifetime_parameter_ordinal,
            result_nominal_identity: identities
                .intern(&fact.result_projection.carrier_identity.clone()),
            result_multiplicity: StructuralMultiplicity::Linear,
            result_lifetime_argument_count: 1,
            result_lifetime_argument_ordinal: fact.result_lifetime_argument_ordinal,
            result_lifetime_slot_is_erased: true,
            retained_semantic_domain: result_projection.semantic_domain,
            source_projection,
            result_projection,
        })
    }

    pub(crate) fn retain_foreign_borrow_custodies(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        module: &mut TerminalModule,
    ) -> Result<(), LoweringError> {
        let mut rows = checked
            .facts
            .qualifications
            .content
            .retained_borrow_custodies
            .iter()
            .map(|fact| {
                let owner = checked
                    .traits()
                    .iter()
                    .find(|definition| {
                        checked
                            .executable_trait_machine_instances(definition)
                            .any(|signature| signature.symbol == fact.callable)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "retained-borrow callable lost its schema owner",
                    ))?;
                let owner = nominal(identities, checked, exact_toolchain_sources, owner.symbol)
                    .map_err(LoweringError::Unsupported)?;
                Ok((lower_custody(identities, checked, fact)?, owner))
            })
            .collect::<Result<Vec<_>, _>>()?;
        rows.sort_by_key(|left| left.0.callable_identity);
        if rows
            .windows(2)
            .any(|pair| pair[0].0.callable_identity == pair[1].0.callable_identity)
        {
            return unsupported("retained-borrow callable has multiple custody rows");
        }
        let mut next_boundary = module
            .boundary_machines
            .iter()
            .map(|boundary| boundary.id.get())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "retained-borrow boundary identity space is exhausted",
            ))?;
        for (row, owner) in rows {
            if let Some(boundary) = module
                .boundary_machines
                .iter_mut()
                .find(|boundary| boundary.identity == row.callable_identity)
            {
                if boundary.kind != terminal_psi::BoundaryRequirementKind::Trait {
                    return unsupported(
                        "retained-borrow custody does not name a trait requirement",
                    );
                }
                boundary
                    .content_guarantees
                    .push(BoundaryContentGuarantee::RetainedBorrow(row));
                boundary.content_guarantees.sort();
                continue;
            }
            let id = BoundaryMachineId::new(next_boundary).ok_or(LoweringError::Unsupported(
                "retained-borrow boundary identity is invalid",
            ))?;
            next_boundary = next_boundary
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "retained-borrow boundary identity space is exhausted",
                ))?;
            module.boundary_machines.push(BoundaryMachineDeclaration {
                numeric_intrinsic: None,
                normal_qualifications: Vec::new(),
                scalar_domain_requirements: Vec::new(),
                declaration: None,
                role: terminal_psi::BoundaryDeclarationRole::ContractEvidence,
                unconditional_termination: false,
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                kind: terminal_psi::BoundaryRequirementKind::Trait,
                scalar_ensures: Vec::new(),
                scalar_requires: Vec::new(),
                parameter_order: Vec::new(),
                fixed_service_reach: Vec::new(),
                owning_service: None,
                id,
                identity: row.callable_identity,
                owner,
                attachment: None,
                scalar_parameters: Vec::new(),
                crash_routes: Vec::new(),
                structural_parameters: Vec::new(),
                result: BoundaryMachineResult::Unit,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: vec![BoundaryContentGuarantee::RetainedBorrow(row)],
                result_reference_regions: Vec::new(),
                reference_outputs: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
        }
        Ok(())
    }
}
pub(crate) mod suspension_call_plan {

    use super::{
        CheckedTrees, LoweredSourceCallOccurrence, LoweringError, OperationKind, TerminalModule,
        ValueDeclaration, ValueId, invalid_suspension, terminal_scalar_type, unsupported,
    };
    use crate::expression_preparation::bindings::structural_paths::lower_static_argument_path;
    use crate::shared::direct_calls::direct_call_parts;
    use crate::shared::terminal_identities::lookup_claim_id;
    use arena::{Handle, HandleSpan};
    use terminal_psi::{TerminalSuspensionPlace, TerminalSuspensionValueType};
    pub(crate) fn retain_suspension_call_plans(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        source_machines: &[symbols::SymbolHandle],
        source_calls: &[LoweredSourceCallOccurrence],
        state_blocks: &[lowered_psi::LoweredStateBlock],
        module: &mut TerminalModule,
    ) -> Result<(), LoweringError> {
        if !module.suspension_call_plans.is_empty() {
            return unsupported("suspension call plans must be produced exactly once");
        }

        let mut plans = Vec::new();
        for crossing in checked
            .facts
            .carry
            .suspension_crossings
            .iter()
            .filter(|crossing| {
                source_machines.contains(&crossing.machine)
                    || state_blocks
                        .iter()
                        .any(|block| block.state == crossing.state)
            })
        {
            if !crossing.condition.parameters.is_empty() {
                return unsupported(
                    "closed suspension crossing retains an unresolved callback carry condition",
                );
            }
            let mut occurrences = source_calls.iter().filter(|occurrence| {
                occurrence.matches_statement(
                    crossing.state,
                    crossing.statement_index,
                    crossing.call_ordinal,
                )
            });
            let Some(occurrence) = occurrences.next() else {
                if fused_component_transfer(checked, state_blocks, crossing) {
                    continue;
                }
                return unsupported(
                    "possibly-suspending checked call has no exact Terminal operation join",
                );
            };
            let terminal_operation =
                occurrence
                    .terminal_site
                    .operation()
                    .ok_or(LoweringError::Unsupported(
                        "external completion cannot park a continuing activation",
                    ))?;
            if occurrences.next().is_some()
                || owning_callable_symbol(checked, occurrence.source_target)
                    != owning_callable_symbol(checked, crossing.target)
            {
                return unsupported("suspension crossing call join is duplicate or redirected");
            }
            let (machine_index, block_index, operation_index) = module
                .machines
                .iter()
                .enumerate()
                .flat_map(|(machine_index, machine)| {
                    machine
                        .blocks
                        .iter()
                        .enumerate()
                        .flat_map(move |(block_index, block)| {
                            block.operations.iter().enumerate().map(
                                move |(operation_index, operation)| {
                                    (machine_index, block_index, operation_index, operation)
                                },
                            )
                        })
                })
                .find(|(.., operation)| operation.id == terminal_operation)
                .map(|(machine_index, block_index, operation_index, _)| {
                    (machine_index, block_index, operation_index)
                })
                .ok_or(LoweringError::Unsupported(
                    "suspension crossing Terminal operation is absent",
                ))?;
            let operation =
                &module.machines[machine_index].blocks[block_index].operations[operation_index];
            let target = terminal_call_target(&operation.kind).ok_or(
                LoweringError::Unsupported("suspension crossing Terminal operation is not a call"),
            )?;

            let mut live_values = Vec::with_capacity(crossing.live_values.len());
            for live in &crossing.live_values {
                let (place, value_type) = match live.origin {
                    checked_trees::SuspensionCrossingValueOrigin::Parameter {
                        position, ..
                    }
                    | checked_trees::SuspensionCrossingValueOrigin::Local {
                        environment_position: position,
                        ..
                    } => source_binding(&occurrence.frontier_before_call, position)?,
                    checked_trees::SuspensionCrossingValueOrigin::CallArgument { position } => {
                        call_argument_binding(
                            checked,
                            crossing.target,
                            &operation.kind,
                            module,
                            position,
                        )?
                    }
                };
                if let TerminalSuspensionPlace::Scalar(value) = place {
                    if !scalar_value_available_before(
                        &module.machines[machine_index],
                        block_index,
                        operation_index,
                        value,
                    ) {
                        return unsupported("suspension frontier value is unavailable at its call");
                    }
                    let source_type = checked
                        .primitive_type_reference(live.type_reference)
                        .ok_or(LoweringError::Unsupported(
                            "suspension scalar frontier has no checked scalar type",
                        ))?;
                    if TerminalSuspensionValueType::Scalar(terminal_scalar_type(source_type)?)
                        != value_type
                    {
                        return unsupported("suspension frontier scalar type join is inconsistent");
                    }
                } else if let TerminalSuspensionPlace::Structural(argument) = &place
                    && !module.machines[machine_index]
                        .structural_places
                        .iter()
                        .any(|declaration| declaration.id == argument.place)
                {
                    return unsupported(
                        "suspension structural frontier has no caller storage declaration",
                    );
                }
                let mut claims = live
                    .claims
                    .iter()
                    .map(|identity| {
                        lookup_claim_id(
                            &occurrence.frontier_before_call.claims,
                            identity.claim_identity,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                claims.sort();
                claims.dedup();
                live_values.push((
                    live,
                    terminal_psi::TerminalSuspensionLiveValue {
                        place,
                        value_type,
                        storage: match live.storage {
                            checked_trees::SuspensionCrossingStorage::Parameter => {
                                terminal_psi::TerminalSuspensionStorage::Parameter
                            }
                            checked_trees::SuspensionCrossingStorage::Local => {
                                terminal_psi::TerminalSuspensionStorage::Local
                            }
                            checked_trees::SuspensionCrossingStorage::CallArgument => {
                                terminal_psi::TerminalSuspensionStorage::CallArgument
                            }
                        },
                        claim_count: u32::try_from(claims.len()).map_err(|_| {
                            LoweringError::Unsupported(
                                "suspension frontier claim count exceeds the Terminal vocabulary",
                            )
                        })?,
                        claims,
                        effective: live.effective,
                        loans: terminal_psi::TerminalSuspensionValueLoans::Unresolved,
                    },
                ));
            }
            live_values.sort_by(|left, right| {
                (
                    &left.1.place,
                    left.1.storage,
                    left.1.value_type,
                    left.1.effective,
                )
                    .cmp(&(
                        &right.1.place,
                        right.1.storage,
                        right.1.value_type,
                        right.1.effective,
                    ))
            });
            let loans = retain_loan_graph(
                identities,
                checked,
                crossing,
                occurrence,
                &mut live_values,
                source_calls,
                module,
            )?;
            let live_values = live_values
                .into_iter()
                .map(|(_, live)| live)
                .collect::<Vec<_>>();
            let crossing_id =
                checked_trees::canonical_suspension_crossing_id(&checked.typed, crossing).ok_or(
                    LoweringError::Unsupported(
                        "suspension crossing identity cannot resolve its source symbols",
                    ),
                )?;
            module.machines[machine_index].blocks[block_index].operations[operation_index]
                .suspension_crossing = Some(crossing_id);
            plans.push(terminal_psi::TerminalSuspensionCallPlan {
                operation: terminal_operation,
                crossing: crossing_id,
                target,
                effective: crossing.effective,
                live_value_count: u32::try_from(live_values.len()).map_err(|_| {
                    LoweringError::Unsupported(
                        "suspension live-value count exceeds the Terminal vocabulary",
                    )
                })?,
                live_values,
                loans,
            });
        }
        plans.sort_by_key(|plan| (plan.operation, plan.crossing));
        module.suspension_call_plan_count = u32::try_from(plans.len()).map_err(|_| {
            LoweringError::Unsupported("suspension call plan count exceeds the Terminal vocabulary")
        })?;
        module.suspension_call_sites = plans
            .iter()
            .map(|plan| terminal_psi::TerminalSuspensionCallSite {
                operation: plan.operation,
                crossing: plan.crossing,
                target: plan.target,
                frontier_commitment: terminal_psi::suspension_frontier_commitment(
                    &identities.snapshot(),
                    plan,
                ),
            })
            .collect();
        module.suspension_call_plans = plans;
        Ok(())
    }

    fn retain_loan_graph(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        crossing: &checked_trees::SuspensionCrossingCarryFact,
        occurrence: &LoweredSourceCallOccurrence,
        live_values: &mut [(
            &checked_trees::SuspensionCrossingLiveValueFact,
            terminal_psi::TerminalSuspensionLiveValue,
        )],
        source_calls: &[LoweredSourceCallOccurrence],
        module: &TerminalModule,
    ) -> Result<terminal_psi::TerminalSuspensionLoans, LoweringError> {
        let types = &module.structural_types;
        use checked_trees::{
            BorrowLoanLineage, CheckedCallBorrowLoanSource, CheckedParentBorrowResource,
            SuspensionCrossingLoanBacking, SuspensionCrossingLoanSource,
            SuspensionValueBorrowFrontier,
        };
        use terminal_psi::{
            StructuralAccess, TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
            TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource,
            TerminalSuspensionValueLoans,
        };
        let carry = &checked.facts.carry;
        let borrow = &checked.facts.borrow;
        let rows = carry.suspension_loans.span(crossing.borrows.loans).ok_or(
            LoweringError::Unsupported("suspension loan roster is unavailable"),
        )?;
        let mut graph = terminal_psi::TerminalSuspensionLoans::default();
        let mut loan_identities = Vec::new();
        for (position, row) in rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.retained_for_crossing)
        {
            let terminal = graph.occurrences.insert(TerminalSuspensionLoan::default());
            loan_identities.push((
                crossing.borrows.loans.handle_at(position as u32),
                row.stored_loan(),
                terminal,
            ));
        }
        let dependency = |loan: Handle<checked_trees::BorrowLoanFact>| {
            if !loan.is_valid() {
                return Handle::invalid();
            }
            loan_identities
                .iter()
                .find(|(_, source, _)| *source == loan)
                .map_or(Handle::invalid(), |(_, _, terminal)| *terminal)
        };
        for (source_row, source_loan, terminal) in &loan_identities {
            let row = carry.suspension_loans.get(*source_row);
            let (capture, resource, parent, suppliers, access) = match row.source {
                SuspensionCrossingLoanSource::Incoming {
                    identity,
                    ref access,
                } => {
                    let (position, path, _) = incoming_reference_binding(
                        identities,
                        checked,
                        identity,
                        rows,
                        live_values,
                        types,
                    )?;
                    (
                        TerminalSuspensionLoanCapture::RetainedValue {
                            live_value: u32::try_from(position).map_err(|_| {
                                LoweringError::Unsupported(
                                    "incoming loan index exceeds Terminal vocabulary",
                                )
                            })?,
                            path: graph.paths.insert_many(path),
                        },
                        TerminalSuspensionLoanResource::IncomingReference,
                        Handle::invalid(),
                        HandleSpan::empty(),
                        access.clone(),
                    )
                }
                SuspensionCrossingLoanSource::LifetimeDependency { loan, resource } => {
                    if !borrow.output_lifetime_dependencies.is_valid(resource)
                        || borrow.output_lifetime_dependencies.get(resource).loan != loan
                    {
                        return invalid_suspension(
                            "suspension lifetime dependency lost its checked resource",
                        );
                    }
                    let mut suppliers = HandleSpan::empty();
                    for source in borrow
                        .loan_suppliers
                        .span_or_empty(borrow.loans.get(loan).supplying_loans)
                    {
                        let supplier = dependency(*source);
                        if !supplier.is_valid() {
                            return invalid_suspension("suspension dependency supplier is absent");
                        }
                        suppliers.push_contiguous(graph.supplier_links.insert(supplier));
                    }
                    (
                        TerminalSuspensionLoanCapture::InputDependencies,
                        TerminalSuspensionLoanResource::LifetimeDependency,
                        Handle::invalid(),
                        suppliers,
                        borrow.loans.get(loan).kind.clone(),
                    )
                }
                SuspensionCrossingLoanSource::OutputInput { input, .. } => {
                    let input = borrow.supplied_output_inputs.get(input);
                    let contract = checked
                        .facts
                        .flow
                        .control
                        .operand_retained_suppliers
                        .get(input.source)
                        .contract
                        .ok_or(LoweringError::InvalidSuspensionPlan(
                            "suspension output input has no original contract",
                        ))?;
                    let calls = checked.facts.flow.control.calls.iter().filter(|(_, call)| {
                        call.statement_index == contract.statement_index
                            && call.call_ordinal == contract.call_ordinal
                            && call.authored_expression == contract.invocation
                            && call
                                .argument_operands()
                                .handles()
                                .any(|operand| operand == contract.input)
                    });
                    let mut calls = calls;
                    let (call, _) = calls.next().ok_or(LoweringError::InvalidSuspensionPlan(
                        "suspension input has no original invocation",
                    ))?;
                    if calls.next().is_some() {
                        return invalid_suspension("suspension input invocation is ambiguous");
                    }
                    let path = borrow.call_loan_paths.span(input.capture_path).ok_or(
                        LoweringError::InvalidSuspensionPlan(
                            "suspension input capture path is unavailable",
                        ),
                    )?;
                    let (operation, argument, path) = captured_call_leaf(
                        identities,
                        checked,
                        source_calls,
                        module,
                        contract.state_symbol,
                        call,
                        contract.parameter_index,
                        path,
                    )?;
                    let mut suppliers = HandleSpan::empty();
                    let access = match input.permission {
                        checked_trees::CheckedOutputBorrowInputPermission::Borrowed {
                            resource,
                        } => {
                            let resource = borrow.call_loan_resources.get(resource);
                            let parents = match resource.source {
                                CheckedCallBorrowLoanSource::StoredLoan { loan } => vec![loan],
                                CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                                    borrow.call_loan_parents.span_or_empty(parents).to_vec()
                                }
                            };
                            for parent in parents {
                                let parent = dependency(parent);
                                if !parent.is_valid() {
                                    return invalid_suspension(
                                        "suspension input ancestor is absent",
                                    );
                                }
                                suppliers.push_contiguous(graph.supplier_links.insert(parent));
                            }
                            resource.access.clone()
                        }
                        checked_trees::CheckedOutputBorrowInputPermission::Incoming {
                            access,
                            ..
                        } => access.into(),
                    };
                    (
                        TerminalSuspensionLoanCapture::CallInput {
                            call: operation,
                            argument,
                            path: graph.paths.insert_many(path),
                            stored: contract.source == checked_trees::ViewReturnInputSource::Stored,
                        },
                        TerminalSuspensionLoanResource::OutputInput,
                        Handle::invalid(),
                        suppliers,
                        access,
                    )
                }
                SuspensionCrossingLoanSource::Stored { ref resource, .. } => {
                    if !borrow.loans.is_valid(*source_loan) {
                        return unsupported("suspension loan occurrence is unavailable");
                    }
                    let loan = borrow.loans.get(*source_loan);
                    let mut captures = occurrence
                        .frontier_before_call
                        .loan_captures
                        .iter()
                        .filter(|capture| capture.loan == *source_loan);
                    let capture = if loan.lineage == BorrowLoanLineage::UnretainedDerived
                        && (!loan.supplying_loans.is_empty()
                            || borrow.retained_call_inputs(*source_loan).is_some())
                    {
                        TerminalSuspensionLoanCapture::DescriptorTransfer
                    } else if let CheckedParentBorrowResource::SuppliedOutput { resource } =
                        resource
                    {
                        let output = borrow.supplied_output_resources.get(*resource);
                        let incoming = borrow.incoming_references.get(output.identity);
                        let checked_trees::IncomingReferenceOrigin::Output { identity } =
                            incoming.origin
                        else {
                            return invalid_suspension(
                                "suspension output lost its exact namespace",
                            );
                        };
                        let call = checked.facts.flow.control.calls.get(output.call);
                        let parameters =
                            validation::call_target_parameters(checked, call.target_symbol).ok_or(
                                LoweringError::InvalidSuspensionPlan(
                                    "suspension output call has no formal telescope",
                                ),
                            )?;
                        let position = parameters
                            .iter()
                            .position(|parameter| parameter.symbol == identity.parameter_symbol)
                            .ok_or(LoweringError::InvalidSuspensionPlan(
                                "suspension output has no original output argument",
                            ))?;
                        let path = validation::borrow_contracts::structural_owner_path(
                            checked,
                            borrow.owner_segments.span_or_empty(incoming.owner_path),
                        )
                        .ok_or(LoweringError::InvalidSuspensionPlan(
                            "suspension output path has no declaration",
                        ))?;
                        let (operation, argument, path) = captured_call_leaf(
                            identities,
                            checked,
                            source_calls,
                            module,
                            output.state_symbol,
                            output.call,
                            position,
                            &path,
                        )?;
                        TerminalSuspensionLoanCapture::CompletedOutput {
                            call: operation,
                            argument,
                            path: graph.paths.insert_many(path),
                        }
                    } else if let Some(capture) = captures.next() {
                        if captures.next().is_some() {
                            return unsupported("suspension loan has multiple formation captures");
                        }
                        let mut path = HandleSpan::empty();
                        for segment in &capture.source.path {
                            path.push_contiguous(graph.paths.insert(segment.clone()));
                        }
                        TerminalSuspensionLoanCapture::Reference {
                            formation: capture.formation,
                            carrier: capture.carrier,
                            source: capture.source.place,
                            source_access: capture.source.access,
                            source_path: path,
                        }
                    } else {
                        TerminalSuspensionLoanCapture::Unresolved
                    };
                    let resource = match resource {
                        CheckedParentBorrowResource::DirectRoot { resource }
                            if borrow.direct_loan_resources.is_valid(*resource) =>
                        {
                            TerminalSuspensionLoanResource::DirectRoot
                        }
                        CheckedParentBorrowResource::Reborrow { resource }
                            if borrow.reborrow_loan_resources.is_valid(*resource) =>
                        {
                            TerminalSuspensionLoanResource::Reborrow
                        }
                        CheckedParentBorrowResource::SuppliedOutput { resource }
                            if borrow.supplied_output_resources.is_valid(*resource) =>
                        {
                            TerminalSuspensionLoanResource::CompletedOutput
                        }
                        _ if loan.lineage == BorrowLoanLineage::UnretainedDerived
                            && (!loan.supplying_loans.is_empty()
                                || borrow.retained_call_inputs(*source_loan).is_some()) =>
                        {
                            TerminalSuspensionLoanResource::DescriptorTransfer
                        }
                        _ => TerminalSuspensionLoanResource::Unresolved,
                    };
                    let parent = match loan.lineage {
                        BorrowLoanLineage::Reborrow { parent_loan } => dependency(parent_loan),
                        _ => Handle::invalid(),
                    };
                    let mut suppliers = HandleSpan::empty();
                    for source in borrow.loan_suppliers.span_or_empty(loan.supplying_loans) {
                        let supplier = dependency(*source);
                        if !supplier.is_valid() {
                            return unsupported("suspension loan supplier is not retained");
                        }
                        suppliers.push_contiguous(graph.supplier_links.insert(supplier));
                    }
                    if let Some(inputs) = borrow.retained_call_inputs(*source_loan) {
                        for input in inputs {
                            let supplier = loan_identities.iter().find(|(source, _, _)| matches!(carry.suspension_loans.get(*source).source,
                                SuspensionCrossingLoanSource::CapturedActual { resource } if resource == *input))
                                .map(|(_, _, target)| *target).ok_or(LoweringError::InvalidSuspensionPlan("saved reference's original call input is absent"))?;
                            suppliers.push_contiguous(graph.supplier_links.insert(supplier));
                        }
                    }
                    (capture, resource, parent, suppliers, loan.kind.clone())
                }
                SuspensionCrossingLoanSource::CapturedActual { resource } => {
                    if !borrow.call_loan_resources.is_valid(resource) {
                        return unsupported("suspension captured actual resource is unavailable");
                    }
                    let resource_handle = resource;
                    let resource = borrow.call_loan_resources.get(resource);
                    let historical = rows.iter().any(|row| {
                        let loan = row.stored_loan();
                        loan.is_valid()
                            && borrow
                                .retained_call_inputs(loan)
                                .is_some_and(|inputs| inputs.contains(&resource_handle))
                    });
                    let capture = if historical {
                        let call = checked.facts.flow.control.calls.get(resource.call);
                        let position = call
                            .argument_operands()
                            .handles()
                            .position(|operand| operand == resource.owner_operand)
                            .ok_or(LoweringError::InvalidSuspensionPlan(
                                "saved input has no original call argument",
                            ))?;
                        let path = borrow.call_loan_paths.span(resource.capture_path).ok_or(
                            LoweringError::InvalidSuspensionPlan(
                                "saved input's original capture path is unavailable",
                            ),
                        )?;
                        let (operation, argument, path) = captured_call_leaf(
                            identities,
                            checked,
                            source_calls,
                            module,
                            resource.state_symbol,
                            resource.call,
                            position,
                            path,
                        )?;
                        TerminalSuspensionLoanCapture::CallInput {
                            call: operation,
                            argument,
                            path: graph.paths.insert_many(path),
                            stored: !resource.capture_path.is_empty(),
                        }
                    } else {
                        let position = crossing
                            .borrows
                            .operands
                            .handles()
                            .position(|operand| operand == resource.owner_operand)
                            .ok_or(LoweringError::Unsupported(
                                "suspension loan has no captured actual",
                            ))?;
                        let live_value = live_values
                            .iter()
                            .position(|(live, _)| {
                                live.origin
                                    == checked_trees::SuspensionCrossingValueOrigin::CallArgument {
                                        position,
                                    }
                            })
                            .ok_or(LoweringError::Unsupported(
                                "suspension captured actual is not retained",
                            ))?;
                        let path = borrow.call_loan_paths.span(resource.capture_path).ok_or(
                            LoweringError::Unsupported(
                                "suspension actual loan path is unavailable",
                            ),
                        )?;
                        let TerminalSuspensionValueType::Structural(root) =
                            live_values[live_value].1.value_type
                        else {
                            return unsupported(
                                "suspension captured loan has no structural operand",
                            );
                        };
                        let path = lower_static_argument_path(identities, path, root, types)?;
                        let path = graph.paths.insert_many(path);
                        TerminalSuspensionLoanCapture::RetainedValue {
                            live_value: u32::try_from(live_value).map_err(|_| {
                                LoweringError::Unsupported(
                                    "suspension actual index exceeds Terminal vocabulary",
                                )
                            })?,
                            path,
                        }
                    };
                    let (kind, parent, suppliers) = match resource.source {
                        CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                            let supplier = dependency(loan);
                            if !supplier.is_valid() {
                                return unsupported(
                                    "suspension actual loan supplier is not retained",
                                );
                            }
                            let kind = TerminalSuspensionLoanResource::DescriptorTransfer;
                            (
                                kind,
                                Handle::invalid(),
                                graph.supplier_links.insert_many([supplier]),
                            )
                        }
                        CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                            let parents = borrow.call_loan_parents.span(parents).ok_or(
                                LoweringError::Unsupported(
                                    "suspension actual parent roster is unavailable",
                                ),
                            )?;
                            if parents.iter().any(|parent| !dependency(*parent).is_valid()) {
                                return unsupported("suspension actual parent is not retained");
                            }
                            let parent = parents
                                .first()
                                .map_or(Handle::invalid(), |parent| dependency(*parent));
                            (
                                if parent.is_valid() {
                                    TerminalSuspensionLoanResource::Reborrow
                                } else {
                                    TerminalSuspensionLoanResource::DirectRoot
                                },
                                parent,
                                graph
                                    .supplier_links
                                    .insert_many(parents.iter().map(|parent| dependency(*parent))),
                            )
                        }
                    };
                    (capture, kind, parent, suppliers, resource.access.clone())
                }
            };
            let backing = match row.backing {
                SuspensionCrossingLoanBacking::Unresolved => {
                    TerminalSuspensionLoanBacking::Unresolved
                }
                SuspensionCrossingLoanBacking::StateValue { origin } => {
                    let position = live_values
                        .iter()
                        .position(|(live, _)| live.origin == origin)
                        .ok_or(LoweringError::Unsupported(
                            "suspension loan backing is not retained",
                        ))?;
                    TerminalSuspensionLoanBacking::RetainedOwner {
                        live_value: u32::try_from(position).map_err(|_| {
                            LoweringError::Unsupported(
                                "suspension backing index exceeds Terminal vocabulary",
                            )
                        })?,
                    }
                }
                SuspensionCrossingLoanBacking::CompletedOutput { resource } => {
                    let loan = dependency(borrow.supplied_output_resources.get(resource).loan);
                    if !loan.is_valid() {
                        return invalid_suspension("suspension completed output is not retained");
                    }
                    TerminalSuspensionLoanBacking::CompletedOutput { loan }
                }
                SuspensionCrossingLoanBacking::InputDependencies => {
                    TerminalSuspensionLoanBacking::InputDependencies
                }
                SuspensionCrossingLoanBacking::IncomingReferent { .. }
                    if matches!(row.source, SuspensionCrossingLoanSource::OutputInput { .. })
                        || matches!(row.source, SuspensionCrossingLoanSource::CapturedActual { resource } if rows.iter().any(|row| {
                            let loan = row.stored_loan();
                            loan.is_valid() && borrow.retained_call_inputs(loan).is_some_and(|inputs| inputs.contains(&resource))
                        })) =>
                {
                    TerminalSuspensionLoanBacking::CapturedInput
                }
                SuspensionCrossingLoanBacking::IncomingReferent { identity } => {
                    let (_, _, argument) = incoming_reference_binding(
                        identities,
                        checked,
                        identity,
                        rows,
                        live_values,
                        types,
                    )?;
                    TerminalSuspensionLoanBacking::IncomingReferent { argument }
                }
            };
            let mut inputs = HandleSpan::empty();
            if matches!(
                row.source,
                SuspensionCrossingLoanSource::Stored {
                    resource: CheckedParentBorrowResource::SuppliedOutput { .. },
                    ..
                } | SuspensionCrossingLoanSource::LifetimeDependency { .. }
            ) {
                for (candidate, _, target) in &loan_identities {
                    if matches!(carry.suspension_loans.get(*candidate).source,
                        SuspensionCrossingLoanSource::OutputInput { owner, .. } if owner == *source_loan)
                    {
                        inputs.push_contiguous(graph.input_links.insert(*target));
                    }
                }
            }
            *graph.occurrences.get_mut(*terminal) = TerminalSuspensionLoan {
                capture,
                backing,
                resource,
                parent,
                suppliers,
                inputs,
                access: match access {
                    checked_trees::BorrowAccessKind::Read => StructuralAccess::SharedBorrow,
                    checked_trees::BorrowAccessKind::Mutable => StructuralAccess::MutableBorrow,
                    checked_trees::BorrowAccessKind::WriteOnly => StructuralAccess::WriteOnlyBorrow,
                },
                active_at_call: row.active_at_call,
            };
        }
        for (source, live) in live_values {
            live.loans = match source.borrow_frontier {
                SuspensionValueBorrowFrontier::ViewFree => TerminalSuspensionValueLoans::ViewFree,
                SuspensionValueBorrowFrontier::RetainedLoans { loans } => {
                    let links = carry.suspension_value_loans.span(loans).ok_or(
                        LoweringError::Unsupported("suspension value loan edges are unavailable"),
                    )?;
                    let mut retained = HandleSpan::empty();
                    for link in links {
                        let terminal = loan_identities
                            .iter()
                            .find(|(source, _, _)| source == link)
                            .map(|(_, _, terminal)| *terminal)
                            .ok_or(LoweringError::Unsupported(
                                "suspension value names an unretained loan",
                            ))?;
                        retained.push_contiguous(graph.value_links.insert(terminal));
                    }
                    TerminalSuspensionValueLoans::Retained { links: retained }
                }
                SuspensionValueBorrowFrontier::Unresolved => {
                    TerminalSuspensionValueLoans::Unresolved
                }
                SuspensionValueBorrowFrontier::TemplateDependent { .. } => {
                    return invalid_suspension(
                        "closed suspension crossing retains a template-dependent loan frontier",
                    );
                }
            };
        }
        Ok(graph)
    }

    fn incoming_reference_binding(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        identity: Handle<checked_trees::IncomingReferenceIdentity>,
        loans: &[checked_trees::SuspensionCrossingLoanFact],
        live_values: &[(
            &checked_trees::SuspensionCrossingLiveValueFact,
            terminal_psi::TerminalSuspensionLiveValue,
        )],
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<
        (
            usize,
            Vec<terminal_psi::StructuralPathSegment>,
            terminal_psi::StructuralArgument,
        ),
        LoweringError,
    > {
        let borrow = &checked.facts.borrow;
        if !borrow.incoming_references.is_valid(identity) {
            return unsupported("suspension incoming referent is unavailable");
        }
        let incoming = borrow.incoming_references.get(identity);
        let (position, (_, live)) = live_values.iter().enumerate().find(|(_, (source, _))| {
            matches!(source.origin, checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, .. }
                if symbol == incoming.parameter_symbol())
        }).ok_or(LoweringError::Unsupported("suspension ingress parameter is not retained"))?;
        let (
            TerminalSuspensionPlace::Structural(argument),
            TerminalSuspensionValueType::Structural(root),
        ) = (&live.place, live.value_type)
        else {
            return unsupported("suspension ingress permission has no structural argument");
        };
        let owner_path =
            borrow
                .owner_segments
                .span(incoming.owner_path)
                .ok_or(LoweringError::Unsupported(
                    "suspension ingress descriptor path is unavailable",
                ))?;
        let path = validation::borrow_contracts::structural_owner_path(checked, owner_path).ok_or(
            LoweringError::Unsupported(
                "suspension ingress path has no exact declaration selectors",
            ),
        )?;
        let path = lower_static_argument_path(identities, &path, root, types)?;
        let mut referent = argument.clone();
        let access = loans
            .iter()
            .find_map(|loan| match &loan.source {
                checked_trees::SuspensionCrossingLoanSource::Incoming {
                    identity: permission,
                    access,
                } if *permission == identity
                    && loan.active_at_call
                    && loan.retained_for_crossing =>
                {
                    Some(access)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "suspension ingress access has no retained permission",
            ))?;
        referent.access = match access {
            checked_trees::BorrowAccessKind::Read => terminal_psi::StructuralAccess::SharedBorrow,
            checked_trees::BorrowAccessKind::Mutable => {
                terminal_psi::StructuralAccess::MutableBorrow
            }
            checked_trees::BorrowAccessKind::WriteOnly => {
                terminal_psi::StructuralAccess::WriteOnlyBorrow
            }
        };
        referent.path.extend(path.iter().cloned());
        let root_borrow = matches!(
            incoming.origin,
            checked_trees::IncomingReferenceOrigin::ParameterReferent { .. }
        );
        if !root_borrow {
            referent
                .path
                .push(terminal_psi::StructuralPathSegment::Referent);
        }
        Ok((position, path, referent))
    }

    fn source_binding(
        bindings: &lowered_psi::LoweredSourceFrontierBindings,
        position: usize,
    ) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
        if let Some(binding) = bindings
            .scalars
            .iter()
            .find(|binding| binding.source_position == position)
        {
            return Ok((
                TerminalSuspensionPlace::Scalar(binding.value.id),
                TerminalSuspensionValueType::Scalar(binding.value.scalar_type),
            ));
        }
        let binding = bindings
            .structural
            .iter()
            .find(|binding| binding.source_position == position)
            .ok_or(LoweringError::Unsupported(
                "suspension frontier source position has no storage binding",
            ))?;
        Ok((
            TerminalSuspensionPlace::Structural(binding.argument.clone()),
            TerminalSuspensionValueType::Structural(binding.structural_type),
        ))
    }

    fn call_argument_binding(
        checked: &CheckedTrees,
        target: symbols::SymbolHandle,
        operation: &OperationKind,
        module: &TerminalModule,
        position: usize,
    ) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
        let parameters = validation::call_target_parameters(&checked.typed, target).ok_or(
            LoweringError::Unsupported("suspension call argument has no checked formal telescope"),
        )?;
        let parameter = parameters
            .get(position)
            .filter(|parameter| !parameter.relevance.is_erased())
            .ok_or(LoweringError::Unsupported(
                "suspension call argument has no runtime formal",
            ))?;
        let scalar = checked
            .primitive_type_reference(parameter.type_reference)
            .is_some();
        let mut lane_position = parameters[..position]
            .iter()
            .filter(|parameter| {
                !parameter.relevance.is_erased()
                    && checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                        == scalar
            })
            .count();
        let (scalars, structural, structural_type) = if let Some(call) =
            direct_call_parts(operation)
        {
            let callee = module
                .machines
                .iter()
                .find(|machine| machine.id == call.callee)
                .ok_or(LoweringError::Unsupported(
                    "suspension call argument has no emitted callee",
                ))?;
            (
                call.arguments,
                call.structural_arguments,
                callee
                    .structural_parameters
                    .get(lane_position)
                    .map(|parameter| parameter.structural_type),
            )
        } else if let OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            ..
        } = operation
        {
            let callee = module
                .boundary_machines
                .iter()
                .find(|callee| callee.id == *boundary)
                .ok_or(LoweringError::Unsupported(
                    "suspension call argument has no emitted requirement",
                ))?;
            (
                arguments.as_slice(),
                structural_arguments.as_slice(),
                callee
                    .structural_parameters
                    .get(lane_position)
                    .map(|parameter| parameter.structural_type),
            )
        } else if let OperationKind::CallDynamic {
            selector,
            interface,
            slot,
            arguments,
            structural_arguments,
            ..
        } = operation
        {
            if scalar && lane_position == 0 {
                return scalar_argument_binding(module, *selector);
            }
            if scalar {
                lane_position = lane_position
                    .checked_sub(1)
                    .ok_or(LoweringError::Unsupported(
                        "suspension dynamic scalar argument has no data lane",
                    ))?;
            }
            let signature = module
                .dynamic_dispatch
                .interface(*interface)
                .and_then(|interface| interface.slots.get(usize::try_from(*slot).ok()?))
                .ok_or(LoweringError::Unsupported(
                    "suspension dynamic call has no public slot",
                ))?;
            (
                arguments.as_slice(),
                structural_arguments.as_slice(),
                signature
                    .structural_parameters
                    .get(lane_position)
                    .map(|parameter| parameter.structural_type),
            )
        } else {
            return unsupported("suspension call argument is not a call");
        };
        if scalar {
            let value = scalars
                .get(lane_position)
                .copied()
                .ok_or(LoweringError::Unsupported(
                    "suspension call scalar argument has no emitted value",
                ))?;
            return scalar_argument_binding(module, value);
        }
        let argument = structural
            .get(lane_position)
            .ok_or(LoweringError::Unsupported(
                "suspension call structural argument has no emitted storage",
            ))?;
        let structural_type = structural_type.ok_or(LoweringError::Unsupported(
            "suspension call structural argument has no emitted formal",
        ))?;
        Ok((
            TerminalSuspensionPlace::Structural(argument.clone()),
            TerminalSuspensionValueType::Structural(structural_type),
        ))
    }

    fn scalar_argument_binding(
        module: &TerminalModule,
        value: ValueId,
    ) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
        let declaration = scalar_declaration(module, value).ok_or(LoweringError::Unsupported(
            "suspension call scalar argument has no emitted value",
        ))?;
        Ok((
            TerminalSuspensionPlace::Scalar(value),
            TerminalSuspensionValueType::Scalar(declaration.scalar_type),
        ))
    }

    fn fused_component_transfer(
        checked: &CheckedTrees,
        state_blocks: &[lowered_psi::LoweredStateBlock],
        crossing: &checked_trees::SuspensionCrossingCarryFact,
    ) -> bool {
        let target = validation::call_target_entry_symbol(checked, crossing.target);
        let planned = crate::shared::checked_facts::flow_facts(checked)
            .terminal_unit_effects
            .composed_machines
            .iter()
            .flat_map(|plan| &plan.states)
            .filter(|state| state.state == crossing.state)
            .flat_map(|state| state.successors())
            .any(|successor| {
                successor.statement_ordinal as usize == crossing.statement_index
                    && successor.target_state == target
            });
        let mut holders = state_blocks
            .iter()
            .filter(|block| block.state == crossing.state)
            .peekable();
        planned
            && holders.peek().is_some()
            && holders.all(|holder| {
                state_blocks
                    .iter()
                    .any(|block| block.machine == holder.machine && block.state == target)
            })
    }

    fn owning_callable_symbol(
        checked: &CheckedTrees,
        symbol: symbols::SymbolHandle,
    ) -> symbols::SymbolHandle {
        checked
            .machines()
            .iter()
            .find(|machine| {
                typed_trees::state::state_by_symbol(checked.machine_states(machine), symbol)
                    .is_some()
            })
            .map_or(symbol, |machine| machine.symbol)
    }

    fn terminal_call_target(
        kind: &OperationKind,
    ) -> Option<terminal_psi::TerminalSuspensionCallTarget> {
        if let Some(call) = direct_call_parts(kind) {
            return Some(terminal_psi::TerminalSuspensionCallTarget::Machine(
                call.callee,
            ));
        }
        match kind {
            OperationKind::BoundaryCall { boundary, .. } => Some(
                terminal_psi::TerminalSuspensionCallTarget::Boundary(*boundary),
            ),
            OperationKind::CallDynamic {
                interface, slot, ..
            } => Some(terminal_psi::TerminalSuspensionCallTarget::Dynamic {
                interface: *interface,
                slot: *slot,
            }),
            _ => None,
        }
    }

    fn scalar_declaration(module: &TerminalModule, value: ValueId) -> Option<ValueDeclaration> {
        module.machines.iter().find_map(|machine| {
            machine
                .parameters
                .iter()
                .chain(machine.block_parameters())
                .chain(
                    machine
                        .operations()
                        .filter_map(|operation| operation.result.scalar_ref()),
                )
                .find(|declaration| declaration.id == value)
                .copied()
        })
    }

    fn scalar_value_available_before(
        machine: &terminal_psi::TerminalMachine,
        block_index: usize,
        operation_index: usize,
        value: ValueId,
    ) -> bool {
        let block = &machine.blocks[block_index];
        if machine
            .parameters
            .iter()
            .chain(block.parameters.iter())
            .chain(
                block.operations[..operation_index]
                    .iter()
                    .filter_map(|candidate| candidate.result.scalar_ref()),
            )
            .any(|declaration| declaration.id == value)
        {
            return true;
        }
        let Some(definition) = machine.blocks.iter().find(|candidate| {
            candidate
                .parameters
                .iter()
                .chain(
                    candidate
                        .operations
                        .iter()
                        .filter_map(|operation| operation.result.scalar_ref()),
                )
                .any(|declaration| declaration.id == value)
        }) else {
            return false;
        };
        definition.id != block.id
            && terminal_semantics::control_graph::dominators(machine)
                .dominates(definition.id, block.id)
    }

    fn captured_call_leaf(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        source_calls: &[LoweredSourceCallOccurrence],
        module: &TerminalModule,
        state: symbols::SymbolHandle,
        call: Handle<checked_trees::FlowCallFact>,
        position: usize,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Result<
        (
            semantic_vocabulary::OperationId,
            terminal_psi::StructuralArgument,
            Vec<terminal_psi::StructuralPathSegment>,
        ),
        LoweringError,
    > {
        let call = checked.facts.flow.control.calls.get(call);
        let mut occurrences = source_calls.iter().filter(|occurrence| {
            occurrence.matches_statement(state, call.statement_index, call.call_ordinal)
        });
        let occurrence = occurrences
            .next()
            .ok_or(LoweringError::InvalidSuspensionPlan(
                "captured output/input call has no emitted occurrence",
            ))?;
        if occurrences.next().is_some()
            || owning_callable_symbol(checked, occurrence.source_target)
                != owning_callable_symbol(checked, call.target_symbol)
        {
            return invalid_suspension("captured output/input call is ambiguous or redirected");
        }
        let operation =
            occurrence
                .terminal_site
                .operation()
                .ok_or(LoweringError::InvalidSuspensionPlan(
                    "captured output/input has no normal call operation",
                ))?;
        let emitted = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find(|candidate| candidate.id == operation)
            .ok_or(LoweringError::InvalidSuspensionPlan(
                "captured output/input operation is unavailable",
            ))?;
        let (
            TerminalSuspensionPlace::Structural(argument),
            TerminalSuspensionValueType::Structural(root),
        ) = call_argument_binding(checked, call.target_symbol, &emitted.kind, module, position)?
        else {
            return invalid_suspension("captured output/input has no structural argument");
        };
        let path = lower_static_argument_path(identities, path, root, &module.structural_types)?;
        Ok((operation, argument, path))
    }
}

pub(crate) use conformance_applications::{
    checked_application_telescope, checked_method_arguments,
};
