//! Publish evaluated lifecycle demand through the call's emission correspondence.
//!
//! An unexecuted source requirement and a statically selected checked adapter
//! do not create an external operation. Only an emitted boundary call carries
//! a whole-value plan into installation. Its selected provider, actual profile
//! invocation and live allocation remain separate execution obligations.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use terminal_psi::{
    OperationKind, TerminalIdentityTableBuilder, TerminalPlacementLifecycleOperation,
    TerminalPlacementLifecyclePlan,
};
use typed_trees::typed_trees::{PlacementLifecycleOperation, PlacementLifecyclePlan};

use crate::lowering_error::LoweringError;

pub(crate) fn retain(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    lowered: &mut LoweredPsi,
) -> Result<(), LoweringError> {
    let module = &lowered.semantic_module;
    let mut plans = Vec::new();
    for (_, row) in checked.typed.placement_lifecycle_plans.iter() {
        let PlacementLifecyclePlan::Evaluated {
            callable,
            operation,
            policy,
            schema,
            policy_machine,
            placement,
            ..
        } = row
        else {
            continue;
        };
        let private_declaration =
            typed_trees::typed_trees::PlacementExecutionDeclaration::authenticate(
                &checked.typed,
                *callable,
            )
            .map_err(|_| {
                LoweringError::Unsupported("placement lifecycle lost its authenticated declaration")
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
                return Err(LoweringError::Unsupported(
                    "placement lifecycle lost its emitted call occurrence",
                ));
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
                    "placement lifecycle lost its exact schema definition",
                ))?;
        let schema_type = super::placed_view_plans::schema_type(checked, module, schema_data)
            .ok_or(LoweringError::Unsupported(
                "placement lifecycle lost its emitted schema type",
            ))?;
        let mut identity = |symbol| {
            checked
                .typed
                .normalized_hermetic_symbol_identity(symbol)
                .map(|text| identities.intern(&text))
                .map_err(|_| {
                    LoweringError::Unsupported("placement lifecycle lost declaration identity")
                })
        };
        let policy_identity = identity(*policy)?;
        let policy_plan_machine_identity = identity(*policy_machine)?;
        let schema_identity = identity(*schema)?;
        let storage = storage_contract(identities, checked, module, *callable)?;
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
            plans.push(TerminalPlacementLifecyclePlan {
                boundary,
                operation: match operation {
                    PlacementLifecycleOperation::Initialize => {
                        TerminalPlacementLifecycleOperation::Initialize
                    }
                    PlacementLifecycleOperation::MoveOut => {
                        TerminalPlacementLifecycleOperation::MoveOut
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
    plans.sort_unstable_by_key(|plan| plan.boundary);
    if plans
        .windows(2)
        .any(|pair| pair[0].boundary == pair[1].boundary)
    {
        return Err(LoweringError::Unsupported(
            "placement boundary has competing lifecycle plans",
        ));
    }
    lowered.semantic_module.placement_lifecycle_plans = plans;
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
        return Err(LoweringError::Unsupported(
            "placement image lost its exact record schema",
        ));
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
) -> Result<terminal_psi::TerminalPlacementLifecycleExecution, LoweringError> {
    use terminal_psi::TerminalPlacementLifecycleExecution;
    let module = &lowered.semantic_module;
    let declaration = typed_trees::typed_trees::PlacementExecutionDeclaration::authenticate(
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
            return Err(LoweringError::Unsupported(
                "placement lifecycle has neither a requirement nor a compiler execution contract",
            ));
        }
        return Ok(TerminalPlacementLifecycleExecution::SelectedProvider);
    };
    let profile_type =
        {
            let state = checked.typed.state_by_symbol(declaration.state()).ok_or(
                LoweringError::Unsupported("placement primitive entry is absent"),
            )?;
            let parameters = checked.typed.state_parameters(state);
            let profile = match (declaration.operation(), parameters) {
                (PlacementLifecycleOperation::Initialize, [_, _, profile])
                | (PlacementLifecycleOperation::MoveOut, [_, profile]) => profile,
                _ => {
                    return Err(LoweringError::Unsupported(
                        "placement primitive profile input is absent",
                    ));
                }
            };
            let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = checked
                .typed
                .type_reference_table
                .type_reference(profile.type_reference)
            else {
                return Err(LoweringError::Unsupported(
                    "placement primitive must borrow its original profile result",
                ));
            };
            let data = checked.typed.closed_data_definition(*referee).ok_or(
                LoweringError::Unsupported("placement primitive profile type is absent"),
            )?;
            Some(
                super::placed_view_plans::schema_type(checked, module, data).ok_or(
                    LoweringError::Unsupported("placement primitive profile type was not emitted"),
                )?,
            )
        };
    let profile_requirement = if profile_type.is_some() {
        Some(profile_requirement(
            identities,
            checked,
            exact_toolchain_sources,
            lowered,
        )?)
    } else {
        None
    };
    Ok(TerminalPlacementLifecycleExecution::CompilerPrimitive {
        profile_type,
        profile_requirement,
    })
}

/// Bind the core requirement while its exact declaration still exists. Neither
/// an unrelated same-shaped provider nor a direct implementation call is the
/// admitted ResourceProfileSupply crossing. Retain its dense coordinate so
/// execution need not resolve names or inspect source.
fn profile_requirement(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    lowered: &LoweredPsi,
) -> Result<terminal_psi::TerminalPlacementProfileRequirement, LoweringError> {
    const LAYOUT_SOURCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/layout.omg"
    ));
    let typed = &checked.typed;
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
        let Some(operation) = occurrence.terminal_site.operation() else {
            continue;
        };
        let Some(operation) = lowered
            .semantic_module
            .operations()
            .find(|candidate| candidate.id == operation)
        else {
            return Err(LoweringError::Unsupported(
                "profile supply lost its emitted call",
            ));
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
            return Err(LoweringError::Unsupported(
                "profile supply lost its exact overload identity",
            ));
        }
        let requirement = terminal_psi::TerminalPlacementProfileRequirement {
            boundary,
            owner,
            callable: identities.intern(&typed.callable_identity_text(callable)),
        };
        if selected
            .as_ref()
            .is_some_and(|previous| previous != &requirement)
        {
            return Err(LoweringError::Unsupported(
                "profile supply has competing requirement coordinates",
            ));
        }
        selected = Some(requirement);
    }
    selected.ok_or(LoweringError::Unsupported(
        "placement initialization requires its core profile supply call",
    ))
}

/// Resolve roles while authenticated source declarations still exist. Terminal
/// consumers receive dense coordinates; they never recognize field names or
/// guess which qualification on an arbitrary linear record means residency.
fn storage_contract(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    module: &terminal_psi::TerminalModule,
    callable: symbols::SymbolHandle,
) -> Result<terminal_psi::TerminalPlacementStorage, LoweringError> {
    let typed = &checked.typed;
    let (parameters, return_type) =
        if let Some(application) = typed.closed_requirement_application(callable) {
            (
                typed.state_signature_parameters(application.signature),
                application.signature.return_type,
            )
        } else {
            let machine = typed.machine(callable).ok_or(LoweringError::Unsupported(
                "placement execution lost its source declaration",
            ))?;
            if typed_trees::typed_trees::PlacementExecutionDeclaration::authenticate(
                typed,
                machine.symbol,
            )
            .map_err(|_| LoweringError::Unsupported("placement execution contract is invalid"))?
            .is_none()
            {
                return Err(LoweringError::Unsupported(
                    "placement storage has no authenticated execution contract",
                ));
            }
            let [state] = typed.machine_states(machine) else {
                return Err(LoweringError::Unsupported(
                    "placement execution lost its sole entry",
                ));
            };
            (typed.state_parameters(state), state.return_type)
        };
    let parameter = parameters.first().ok_or(LoweringError::Unsupported(
        "placement storage lost its source parameter",
    ))?;
    let reference = typed
        .type_reference_table
        .peeled_type_reference(parameter.type_reference);
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(reference)
    else {
        return Err(LoweringError::Unsupported(
            "placement storage is not the core extent",
        ));
    };
    if !validation::is_extent_declaration(typed, *symbol) || typed.symbols.name(*symbol) != "Extent"
    {
        return Err(LoweringError::Unsupported(
            "placement storage is not the core extent",
        ));
    }
    let data = typed
        .data_definition_by_symbol(*symbol)
        .ok_or(LoweringError::Unsupported(
            "placement extent definition is absent",
        ))?;
    let carrier = super::placed_view_plans::schema_type(checked, module, data).ok_or(
        LoweringError::Unsupported("placement extent carrier is absent"),
    )?;
    let declaration = terminal_psi::find_structural_type(&module.structural_types, carrier).ok_or(
        LoweringError::Unsupported("placement extent declaration is absent"),
    )?;
    let (base, length) = storage_fields(identities, checked, *symbol, declaration)?;
    let domains =
        validation::field_domains::declared_type_domain_rows(typed, parameter.type_reference)
            .into_iter()
            .chain(validation::field_domains::declared_type_domain_rows(
                typed,
                return_type,
            ))
            .collect::<Vec<_>>();
    let domain = |name: &str| {
        let mut matches = domains
            .iter()
            .filter(|row| {
                validation::is_extent_declaration(typed, row.domain_symbol)
                    && typed
                        .domain_definition(row.domain_symbol)
                        .is_some_and(|definition| {
                            definition.name.as_str().rsplit("::").next() == Some(name)
                        })
            })
            .filter_map(|row| {
                module
                    .structural_domains
                    .iter()
                    .find(|domain| {
                        domain.carrier == carrier
                            && domain.semantic_domain.get() == u64::from(row.semantic_domain.0)
                    })
                    .map(|domain| domain.id)
            })
            .collect::<Vec<_>>();
        matches.sort_unstable();
        matches.dedup();
        match matches.as_slice() {
            [domain] => Ok(*domain),
            _ => Err(LoweringError::Unsupported(
                "placement extent lost an exact source qualification",
            )),
        }
    };
    Ok(terminal_psi::TerminalPlacementStorage {
        carrier,
        base,
        length,
        granted: domain("Granted")?,
        vacant: domain("Vacant")?,
        resident: domain("Resident")?,
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
        return Err(LoweringError::Unsupported(
            "placement storage is not the core extent",
        ));
    }
    let data = typed
        .data_definition_by_symbol(extent)
        .ok_or(LoweringError::Unsupported(
            "placement extent definition is absent",
        ))?;
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err(LoweringError::Unsupported(
            "placement extent is not a record",
        ));
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
