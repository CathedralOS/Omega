//! Publish evaluated lifecycle demand through the call's emission correspondence.
//!
//! An unexecuted source requirement and a statically selected checked adapter
//! do not create an external operation. Only an emitted boundary call carries
//! a whole-value plan into installation. Its selected provider, actual profile
//! invocation and live allocation remain separate execution obligations.

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
                LoweringError::Unsupported("access lifecycle lost its authenticated declaration")
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
        let schema_data =
            checked
                .typed
                .data_definition_by_symbol(*schema)
                .ok_or(LoweringError::Unsupported(
                    "access lifecycle lost its exact schema definition",
                ))?;
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
                    AccessLifecycleOperation::MoveOut => TerminalAccessLifecycleOperation::MoveOut,
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

/// Resolve the normalized policy's complete source-field roster while source
/// ownership still exists. Runtime consumers address dense declarations;
/// inaccessible fields and arrays are not reconstructed from accessor shapes.
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
    let mut profile_type = None;
    let mut profile_requirements = Vec::new();
    let profile_positions: &[usize] = match declaration.operation() {
        AccessLifecycleOperation::Initialize => &[2],
        AccessLifecycleOperation::MoveOut | AccessLifecycleOperation::View => &[1],
        AccessLifecycleOperation::Relocate => &[2, 3],
    };
    for (role_position, position) in profile_positions.iter().enumerate() {
        let state = checked.typed.state_by_symbol(declaration.state()).ok_or(
            LoweringError::Unsupported("placement primitive entry is absent"),
        )?;
        let parameters = checked.typed.state_parameters(state);
        let profile = parameters.get(*position).ok_or(LoweringError::Unsupported(
            "placement primitive profile input is absent",
        ))?;
        let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = checked
            .typed
            .type_reference_table
            .type_reference(profile.type_reference)
        else {
            return unsupported("placement primitive must borrow its original profile result");
        };
        let data =
            checked
                .typed
                .closed_data_definition(*referee)
                .ok_or(LoweringError::Unsupported(
                    "placement primitive profile type is absent",
                ))?;
        let retained_type = super::access_view_plans::schema_type(checked, module, data).ok_or(
            LoweringError::Unsupported("placement primitive profile type was not emitted"),
        )?;
        if profile_type.is_some_and(|previous| previous != retained_type) {
            return unsupported("placement primitive profiles have different core carriers");
        }
        profile_type = Some(retained_type);
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

/// Bind the core requirement while its exact declaration still exists. Neither
/// an unrelated same-shaped provider nor a direct implementation call is the
/// admitted ResourceProfileSupply crossing. Retain its dense coordinate so
/// execution need not resolve names or inspect source. Independent supply roles
/// may coexist; only the primitive's exact closed role can select its receipt.
fn profile_requirement(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    lowered: &LoweredPsi,
    role: typed_trees::types::TypeReferenceHandle,
) -> Result<terminal_psi::TerminalAccessProfileRequirement, LoweringError> {
    const LAYOUT_SOURCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/layout.omg"
    ));
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
                source.origin == source::SourceOrigin::Toolchain
                    && source.package_identity.is_none()
                    && source.path.strip_prefix(&source.package_root).ok()
                        == Some(std::path::Path::new("layout.omg"))
                    && source.source.as_bytes() == LAYOUT_SOURCE
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
        // Derive the expected nominal owner and exact overload from the core
        // source, not from the boundary selected by this emission coordinate.
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

/// Translate the evaluated plan's checked roles into emitted identities.
/// No source shape or qualification selection is repeated during publication.
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
            &checked_trees::CheckedMemberIdentity::of_member(member.identity, member.name.clone())
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

/// Resolve core geometry roles while the authenticated source is still present.
/// Lifecycles and reference formation use the same projection. A view need not
/// be accompanied by an initialization call in this module to retain its owner.
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
    if !validation::is_extent_declaration(typed, extent) || typed.symbols.name(extent) != "Extent" {
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
            &checked_trees::CheckedMemberIdentity::of_member(member.identity, member.name.clone())
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
