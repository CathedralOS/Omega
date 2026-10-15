//! Exact establishment custody for the Fused fields of one selected
//! `ProgramEntry` receiver, at whatever nested-record depth they sit.
//!
//! One traversal, `collect_service_fields`, decides which receiver fields are
//! service carriers. Establishment reads it here, and consumer nomination
//! (package review, the canary harness) reads the same traversal through
//! `program_entry_service_requirements` before any provider is selected. A
//! second walk would let the two disagree: a nomination that sees only direct
//! fields leaves a nested `Binding<R>` (such as `fs: Filesystem` holding
//! `Binding<FilesystemHost>`) without a selected provider, and establishment
//! then rejects the entry for the missing Fused selection.

use super::{
    CheckedTrees, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape, CompositionMode,
    DataDefinition, DataField, Diagnostic, SelectedProviderReviewProvenance, data_field_identity,
};
use checked_trees::{CheckedUnitStructuralFieldPlan, CheckedUnitStructuralTypePlan};
use typed_trees::service::ExactServiceCarrier;
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct ServiceFieldApplication<'program> {
    field: &'program DataField,
    carrier: ExactServiceCarrier,
    application: AppliedTraitScope<'program>,
    path: Vec<String>,
}

#[derive(Clone, Copy)]
struct ServiceFieldSource<'program> {
    definition: &'program DataDefinition,
    storage_reference: TypeReferenceHandle,
    reference: TypeReferenceHandle,
    lexical_owner: symbols::SymbolHandle,
}

impl<'program> ServiceFieldSource<'program> {
    fn entry(
        checked: &'program CheckedTrees,
        context: &provider_planning::program_entry::ProvisionedEntryContext<'program>,
        lexical_owner: symbols::SymbolHandle,
    ) -> Option<Self> {
        let TypeReferenceNode::Reference { referee, .. } = checked
            .type_reference_table
            .type_reference(context.parameter.type_reference)
        else {
            return None;
        };
        let (reference, _) = checked.project_enclosing_data_type_reference(*referee, &[])?;
        let owner = checked
            .type_reference_table
            .type_reference(reference)
            .type_symbol(&checked.type_reference_table);
        Some(Self {
            definition: checked.unique_data_definition_by_symbol(owner)?,
            storage_reference: *referee,
            reference,
            lexical_owner,
        })
    }
}

/// The boundary requirements ProgramEntry machine `machine_symbol` demands:
/// every `Binding<R>` carrier on its provisioned context record, in authored
/// route order, found by the same nested-record traversal establishment
/// uses, then every boundary its own service reach names.
///
/// This is the nomination surface: it runs before provider selection, so it
/// cannot demand a selected Fused provider. It also leaves invalid carrier
/// shapes and ambiguous receiver rejoins to establishment, which diagnoses
/// them; here they contribute no requirement. A ProgramEntry is root-bound,
/// not a package callable, so package review never reads its reach rows as a
/// callable's; a direct `Requirement::operation(..)` call in the entry, or in
/// a helper it calls, nominates its boundary through the reach read here.
pub fn program_entry_service_requirements(
    checked: &CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
) -> Vec<symbols::SymbolHandle> {
    let mut requirements = program_entry_carrier_requirements(checked, machine_symbol);
    for service in program_entry_reached_services(checked, machine_symbol) {
        if !requirements.contains(&service) {
            requirements.push(service);
        }
    }
    requirements
}

/// The boundaries the entry's published ceiling and checked-body reach name:
/// the same rows package review reads for a public callable.
fn program_entry_reached_services(
    checked: &CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
) -> Vec<symbols::SymbolHandle> {
    let reaches = &checked.facts.service_reaches;
    let Some(reach) = reaches.for_machine(machine_symbol) else {
        return Vec::new();
    };
    let published = match reach.interface {
        language_semantics::ServiceReachInterface::PublishedCeiling(row) => Some(row),
        language_semantics::ServiceReachInterface::InternalInferred => None,
    };
    published
        .into_iter()
        .chain([reach.inferred_transitive, reach.concrete_transitive])
        .flat_map(|row| reaches.rows.services(row))
        .filter_map(|&service| reaches.services.definition(service))
        .map(|definition| definition.symbol)
        .collect()
}

fn program_entry_carrier_requirements(
    checked: &CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
) -> Vec<symbols::SymbolHandle> {
    let Some(machine) = checked.machine(machine_symbol) else {
        return Vec::new();
    };
    let Some(entry) = checked.machine_states(machine).first() else {
        return Vec::new();
    };
    let Ok(Some(context)) =
        provider_planning::program_entry::provisioned_entry_context(checked, machine, entry)
    else {
        return Vec::new();
    };
    let Some(source) = ServiceFieldSource::entry(checked, &context, machine.symbol) else {
        return Vec::new();
    };
    let mut service_fields = Vec::new();
    let mut invalid_shapes = Vec::new();
    collect_service_fields(
        checked,
        context.definition,
        source,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut service_fields,
        &mut invalid_shapes,
    );
    service_fields
        .into_iter()
        .map(|field| field.carrier.requirement)
        .collect()
}

/// Discovery tolerance follows exact carrier applications, not the base-symbol
/// nomination list. Otherwise a selected descendant falsely appears unsettled,
/// or a differently applied ancestor appears to supply a field it cannot own.
pub(crate) fn program_entry_has_unsettled_service_fields(
    checked: &CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
    selected: &[SelectedProviderReviewProvenance],
) -> bool {
    let Some(machine) = checked.machine(machine_symbol) else {
        return false;
    };
    let Some(entry) = checked.machine_states(machine).first() else {
        return false;
    };
    let Ok(Some(context)) =
        provider_planning::program_entry::provisioned_entry_context(checked, machine, entry)
    else {
        return false;
    };
    let Some(source) = ServiceFieldSource::entry(checked, &context, machine.symbol) else {
        return false;
    };
    let mut fields = Vec::new();
    collect_service_fields(
        checked,
        context.definition,
        source,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut fields,
        &mut Vec::new(),
    );
    fields.iter().any(|field| {
        !super::has_fused_selection(
            checked,
            selected,
            &field.application,
            field.carrier.arguments(checked),
        )
    })
}

pub(crate) fn derive_fused_program_entry_establishments(
    checked: &CheckedTrees,
    calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    entry: &provider_planning::program_entry::SelectedCompilerProgramEntry,
    selected: &[SelectedProviderReviewProvenance],
    permit_unsettled: bool,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>, Vec<Diagnostic>> {
    let source = entry.source_signature();
    let Some(receiver_identity) = source.receiver().normalized_type_identity() else {
        return Ok(Vec::new());
    };
    let receiver_identity_text = source
        .identities()
        .text(receiver_identity)
        .unwrap_or_default()
        .to_owned();
    let matching_machines = checked
        .machines()
        .iter()
        .filter(|machine| machine.symbol == entry.machine_symbol())
        .collect::<Vec<_>>();
    let [machine] = matching_machines.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected ProgramEntry establishment rejoins {} source machines; expected one",
            matching_machines.len(),
        ))]);
    };
    let matching_states = checked
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == entry.state_symbol())
        .collect::<Vec<_>>();
    let [state] = matching_states.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected ProgramEntry establishment rejoins {} entry states; expected one",
            matching_states.len(),
        ))]);
    };
    let context =
        provider_planning::program_entry::provisioned_entry_context(checked, machine, state)
            .map_err(|diagnostic| vec![diagnostic])?
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected provisioned ProgramEntry has no source context parameter",
                )]
            })?;
    if source.receiver().source_position() != Some(context.source_position)
        || entry.context_parameter() != context.parameter.symbol
    {
        return Err(vec![Diagnostic::error(
            "selected ProgramEntry context occurrence drifted before Fused establishment",
        )]);
    }
    let receiver_parameter = context.parameter;
    let owner = context.definition;
    let source_fields =
        ServiceFieldSource::entry(checked, &context, machine.symbol).ok_or_else(|| {
            vec![Diagnostic::error(
                "selected ProgramEntry lost its context application",
            )]
        })?;
    if checked.normalized_type_identity_text(receiver_parameter.type_reference)
        != source
            .identities()
            .text(receiver_identity)
            .unwrap_or_default()
    {
        return Err(vec![Diagnostic::error(
            "selected ProgramEntry receiver identity drifted before Fused establishment",
        )]);
    }
    let mut diagnostics = Vec::new();
    let mut service_fields = Vec::new();
    collect_service_fields(
        checked,
        owner,
        source_fields,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut service_fields,
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    if service_fields.is_empty() {
        return Ok(Vec::new());
    }

    // A dependency-discovery pass tolerates fields whose provider selection
    // does not exist yet: nominating that selection is what the pass is for.
    // Unsettled fields leave the derivation instead of diagnosing; every other
    // establishment check still applies to the fields that keep a selection.
    if permit_unsettled {
        service_fields.retain(|field| {
            super::has_fused_selection(
                checked,
                selected,
                &field.application,
                field.carrier.arguments(checked),
            )
        });
        if service_fields.is_empty() {
            return Ok(Vec::new());
        }
    }

    // Psi retains every logical carrier without a provider decision. This
    // realization currently supports only Fused entry establishment; missing
    // or Independent selection cannot authorize zero-payload storage.
    for field in &service_fields {
        if !super::has_fused_selection(
            checked,
            selected,
            &field.application,
            field.carrier.arguments(checked),
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Binding field `{}::{}` requires a selected Fused provider for boundary `{}`",
                owner.name,
                field.path.join("::"),
                checked.symbols.display_path(field.carrier.requirement, "::"),
            )));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    // One derivation over one kind of evidence: the selected entry state's
    // checked structural-parameter roster. Every Unit-plan family publishes
    // that roster, so a plan family is admitted here by supplying its roster,
    // not by matching a shape of its own. A plan that retains its own data
    // owner as runtime storage is a different establishment contract and
    // nominates no services; that judgment stays with the rosters that can
    // carry such an attachment, and is applied where they are read.
    let context_identity = |parameters: &[checked_trees::CheckedUnitStructuralParameterPlan]| {
        let TypeReferenceNode::Reference { referee, .. } = checked
            .type_reference_table
            .type_reference(receiver_parameter.type_reference)
        else {
            return None;
        };
        let mut matching = parameters
            .iter()
            .filter(|parameter| parameter.position == context.source_position);
        let parameter = matching.next()?;
        if matching.next().is_some()
            || parameter.access != checked_trees::CheckedStructuralAccess::MutableBorrow
            || parameter.type_identity != checked.normalized_type_identity(*referee)
        {
            return None;
        }
        Some(parameter.type_identity)
    };
    let attachment_identities = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .filter(|plan| {
            plan.machine == machine.symbol
                && plan.state == state.symbol
                && plan.attachment_type_identity.is_none()
        })
        .filter_map(|plan| context_identity(&plan.structural_parameters))
        .chain(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter()
                .filter(|plan| {
                    plan.machine == machine.symbol
                        && plan.attachment_type_identity.is_none()
                        && plan.states.iter().any(|plan| plan.state == state.symbol)
                })
                .flat_map(|plan| {
                    plan.states
                        .iter()
                        .filter(|candidate| candidate.state == state.symbol)
                        .filter_map(|state| context_identity(&state.structural_parameters))
                }),
        )
        .collect::<Vec<_>>();
    let [attachment_type_identity] = attachment_identities.as_slice() else {
        // An entry with no plan reports the construct that removed it.
        if let Some(gap) = checked
            .facts
            .flow
            .terminal_unit_effects
            .gap_diagnostic(machine.symbol, &checked.symbols)
        {
            return Err(vec![gap]);
        }
        return Err(vec![Diagnostic::error(format!(
            "selected ProgramEntry establishment rejoins {} Terminal context identities; expected one",
            attachment_identities.len(),
        ))]);
    };

    let structural_types = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter()
        .filter(|plan| plan.identity == *attachment_type_identity)
        .collect::<Vec<_>>();
    let [structural_type] = structural_types.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected ProgramEntry receiver `{receiver_identity_text}` with attachment `{}` rejoins {} Terminal structural types; expected one (available: {})",
            checked
                .type_identity_text(*attachment_type_identity)
                .unwrap_or_default(),
            structural_types.len(),
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .map(|plan| {
                    checked
                        .type_identity_text(plan.identity)
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(", "),
        ))]);
    };
    let CheckedUnitStructuralTypeShape::Record { fields, .. } = &structural_type.shape else {
        return Err(vec![Diagnostic::error(
            "selected ProgramEntry receiver is not one exact Terminal record",
        )]);
    };

    let mut rows = Vec::new();
    for ServiceFieldApplication {
        field,
        carrier,
        application,
        path: field_path,
    } in service_fields
    {
        let field_identity = data_field_identity(field);
        let checked_field = match checked_field_for_path(
            &checked.facts.flow.terminal_unit_effects.structural_types,
            fields,
            &field_path,
        ) {
            Ok(checked_field) => checked_field,
            Err(message) => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected ProgramEntry Binding field `{}::{}` {message}",
                    owner.name,
                    field_path.join("::"),
                )));
                continue;
            }
        };
        let CheckedUnitStructuralFieldType::BindingBacked {
            provider_type_identity,
            receipt,
        } = &checked_field.field_type
        else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Binding field `{}::{}` lacks exact Fused Terminal custody",
                owner.name,
                field_path.join("::"),
            )));
            continue;
        };
        let carrier_type_identity = checked.normalized_type_identity_text(field.type_reference);
        if checked
            .type_identity_text(*provider_type_identity)
            .unwrap_or_default()
            != carrier_type_identity
            || receipt.requirement != carrier.requirement
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Binding field `{}::{}` substituted its carrier or requirement",
                owner.name, field_path.join("::"),
            )));
            continue;
        }
        let Some(schema) = super::carrier_schema(
            checked,
            calling_plans,
            &application,
            carrier.arguments(checked),
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Binding field `{}::{}` cannot reconstruct its schema",
                owner.name,
                field_path.join("::"),
            )));
            continue;
        };
        let matching_plans = selected
            .iter()
            .filter_map(|candidate| {
                if candidate.selected_by.composition_mode() != Ok(CompositionMode::Fused) {
                    return None;
                }
                provider_planning::provider_receiver_application(
                    checked,
                    &candidate.plan,
                    &application,
                    &schema,
                )
                .map(|view| (candidate, view))
            })
            .collect::<Vec<_>>();
        let [(selected_plan, receiving_application)] = matching_plans.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry Binding field `{}::{}` rejoins {} exact Fused provider plans; expected one",
                owner.name,
                field_path.join("::"),
                matching_plans.len(),
            )));
            continue;
        };
        let carrier_base_identity = unconstrained_type_identity(checked, field.type_reference);
        let receiving_requirements =
            super::receiving_requirements(checked, receiving_application, identities)
                .map_err(|message| vec![Diagnostic::error(message)])?;
        let row = program_entry_plan::ProgramEntryFusedServiceEstablishment::new(
            source.identity(),
            source.target_slot(),
            receiver_identity.to_owned(),
            checked
                .type_identity_text(*attachment_type_identity)
                .unwrap_or_default(),
            field_identity,
            field_path,
            carrier_type_identity,
            carrier_base_identity,
            program_entry_plan::ProgramEntryFusedServiceEstablishment::requirement_identity_for_schema(&schema),
            schema.identity_digest(),
            selected_plan.plan.identity_digest(),
            receiving_application.declaration_view,
            receiving_requirements,
            identities,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        rows.push(row);
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    // Rows share the signature's table, so field-path order follows the
    // rendered text ordering the String fields carried before.
    rows.sort_by_cached_key(|row| {
        row.field_path()
            .iter()
            .map(|segment| identities.text(*segment).unwrap_or_default().to_owned())
            .collect::<Vec<_>>()
    });
    if rows
        .windows(2)
        .any(|pair| pair[0].field_path() == pair[1].field_path())
    {
        return Err(vec![Diagnostic::error(
            "selected ProgramEntry repeats one Fused Service establishment field route",
        )]);
    }
    Ok(rows)
}

/// Collect every authored bound-service field on the receiver's record-field
/// tree. Each carrier retains its field, applied scope and route at whatever
/// depth nested records place it. A plain record extends both the field route
/// and the enclosing application scopes through its members. Nested `data` definitions
/// rejoin exactly like the eligibility walk: their complete closed application
/// supplies the fields, and checked zero membership still gates the roster.
fn collect_service_fields<'a>(
    checked: &'a CheckedTrees,
    owner: &DataDefinition,
    source: ServiceFieldSource<'a>,
    enclosing_data_applications: &mut Vec<TypeReferenceHandle>,
    field_path: &mut Vec<String>,
    service_fields: &mut Vec<ServiceFieldApplication<'a>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    enclosing_data_applications.push(source.reference);
    for member in checked.data_members(source.definition) {
        let Some(field) = member.as_field() else {
            continue;
        };
        let field_identity = data_field_identity(field);
        // Specialization gives storage fields new symbols. Rejoin the authored
        // member through its retained generic origin and the exact reference
        // used by storage eligibility, including occurrence-specific field
        // views. The original field operand and its supplying prefix still
        // own region custody; the prepared storage view cannot replace them.
        let Some(storage_field) = checked.closed_data_constructor_field(
            source.storage_reference,
            source.definition.symbol,
            field.symbol,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry field `{field_identity}` lost its retained storage declaration"
            )));
            continue;
        };
        let Some((reference, frames)) = checked.project_enclosing_data_type_reference(
            field.type_reference,
            enclosing_data_applications,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "selected ProgramEntry field `{field_identity}` lost its original supplying application"
            )));
            continue;
        };
        let mut field_applications = enclosing_data_applications[..frames].to_vec();
        match typed_trees::service::classify_exact_bound_service_carrier(
            checked,
            typed_trees::service::ServiceCarrierOccurrence {
                type_reference: reference,
                lexical_owner: source.lexical_owner,
                enclosing_data_applications: &field_applications,
            },
        ) {
            Ok(Some(carrier)) => {
                let Some(application) =
                    carrier.application(checked, source.lexical_owner, &field_applications)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                            "selected ProgramEntry field `{}::{}` lost its exact enclosing data application",
                            owner.name, field_identity,
                        )));
                    continue;
                };
                field_path.push(field_identity);
                service_fields.push(ServiceFieldApplication {
                    field: storage_field,
                    carrier,
                    application,
                    path: field_path.clone(),
                });
                field_path.pop();
            }
            Ok(None) => {
                if nested_source_record(checked, storage_field.type_reference).is_none() {
                    continue;
                }
                let owner_symbol = checked
                    .type_reference_table
                    .type_reference(reference)
                    .type_symbol(&checked.type_reference_table);
                let Some(declaration) = checked.unique_data_definition_by_symbol(owner_symbol)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "selected ProgramEntry field `{field_identity}` lost its original record declaration"
                    )));
                    continue;
                };
                field_path.push(field_identity);
                collect_service_fields(
                    checked,
                    owner,
                    ServiceFieldSource {
                        definition: declaration,
                        storage_reference: storage_field.type_reference,
                        reference,
                        lexical_owner: source.lexical_owner,
                    },
                    &mut field_applications,
                    field_path,
                    service_fields,
                    diagnostics,
                );
                field_path.pop();
            }
            Err(reason) => {
                let mut path = field_path.clone();
                path.push(field_identity);
                diagnostics.push(Diagnostic::error(format!(
                    "selected ProgramEntry field `{}::{}` has invalid Binding establishment shape: {reason}",
                    owner.name,
                    path.join("::"),
                )))
            }
        }
    }
    enclosing_data_applications.pop();
}

/// The nested source data definition a record field names, under the same
/// guards the receiver-eligibility walk applies: its exact closed application
/// must have a checked zero value in its declared domain.
/// Data `where` facts the zero value satisfies constrain other fields; they
/// do not hide a nested service field. Refined references stay unresolved
/// rather than stripping a constraint to reach nested fields.
fn nested_source_record(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    checked
        .closed_data_definition(reference)
        .filter(|definition| !validation::data_requires_establishment(checked, definition))
}

/// Rejoin one collected service field's route to its checked structural
/// field: each intermediate segment must be an exact `Structural` record
/// child, and the leaf lands on the field the route ends at.
fn checked_field_for_path<'a>(
    structural_types: &'a [CheckedUnitStructuralTypePlan],
    fields: &'a [CheckedUnitStructuralFieldPlan],
    field_path: &[String],
) -> Result<&'a CheckedUnitStructuralFieldPlan, String> {
    let mut current_fields = fields;
    for (index, segment) in field_path.iter().enumerate() {
        let matching = current_fields
            .iter()
            .filter(|candidate| candidate.identity.spelling() == *segment)
            .collect::<Vec<_>>();
        let [field] = matching.as_slice() else {
            return Err(format!(
                "rejoins {} checked fields at route segment `{segment}`; expected one",
                matching.len(),
            ));
        };
        if index + 1 == field_path.len() {
            return Ok(field);
        }
        let CheckedUnitStructuralFieldType::Structural { type_identity } = &field.field_type else {
            return Err(format!(
                "route segment `{segment}` does not stay a nested record field"
            ));
        };
        let matching_plans = structural_types
            .iter()
            .filter(|plan| plan.identity == *type_identity)
            .collect::<Vec<_>>();
        let [child_plan] = matching_plans.as_slice() else {
            return Err(format!(
                "route segment `{segment}` rejoins {} checked record plans; expected one",
                matching_plans.len(),
            ));
        };
        current_fields = match &child_plan.shape {
            CheckedUnitStructuralTypeShape::Record { fields, .. }
            | CheckedUnitStructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
            _ => {
                return Err(format!(
                    "route segment `{segment}` does not stay a nested record field"
                ));
            }
        };
    }
    Err("route has no field segments".to_owned())
}

fn unconstrained_type_identity(
    checked: &CheckedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> String {
    while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
        checked.type_reference_table.type_reference(type_reference)
    {
        type_reference = *base_type;
    }
    checked.normalized_type_identity_text(type_reference)
}
