//! Proposed compiler-only coordinator. Not yet compiled or applied.
//! Source declaration selection and invocation stay here; the completed plans
//! consumed by native/provider planning retain canonical metadata only.

use std::sync::Arc;

use build_time_evaluation::{
    InvocationTarget, PreparedBuildMachineProgram, SourceCompletionAccounting,
};
use calling_conventions::BoundaryCallingApplication;
use diagnostics::Diagnostic;
use effects::provider_plan::{
    BoundaryCallingPlanCommitment, BoundaryCallingPlanPromise, ServiceCallingApplication,
    ServiceCallingPlan, ServiceSchema,
};
use package_compilation::PackageCompilationInputs;
use program_entry_plan::{
    ProgramEntryPhysicalContractPackageSourceDigest, ProgramEntryPhysicalContractPlan,
    SelectedProgramEntrySignature, SelectedProgramStorageEntryPlan,
};
use provider_planning::calling_policy_plans::{
    CanonicalCallingSignature, decode_boundary_plan_result, encode_calling_policy_signature,
    validate_calling_policy_result,
};
use provider_planning::program_entry::SelectedProgramEntryCallingPlans;
use psi_compiler::PortableTerminalProgram;
use semantic_vocabulary::{CallingSignatureId, callback_nominal_report_fingerprint};

use crate::CheckedCompilation;

fn failure(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason.into())]
}

pub(crate) fn prepare(
    checked: &CheckedCompilation,
    program: &PortableTerminalProgram,
    inputs: Option<&PackageCompilationInputs>,
    signature: &SelectedProgramEntrySignature,
    accounting: &mut SourceCompletionAccounting,
) -> Result<SelectedProgramEntryCallingPlans, Vec<Diagnostic>> {
    let owner = checked.source_inspection().program();
    checked
        .sources()
        .require_owner(owner)
        .map_err(|error| vec![error])?;
    checked
        .terminal_publication()
        .require_source_owner(owner)
        .map_err(|error| vec![error])?;
    let accepted = checked.terminal_publication().accepted()?;
    let module = program.artifact().semantic_module();
    if module != accepted.module() {
        return Err(failure(
            "entry calling plans belong to another accepted module",
        ));
    }
    let profile = checked
        .execution()
        .target_profile()
        .unwrap_or_else(target::TargetProfile::host);
    let slot = signature.target_slot();
    if slot != profile.program_entry_slot() {
        return Err(failure(
            "entry calling plans differ from the selected target slot",
        ));
    }
    let boundary = package_compilation::selected_program_entry_boundary(
        owner,
        slot,
        inputs,
        checked.sources().exact_toolchain_sources(),
    )?;
    let inventory = checked.source_inspection().inventory();
    let services = inventory.services_for(owner).map_err(|error| vec![error])?;
    let surface = services
        .declaration(
            typed_trees::service_declarations::ProviderSchemaDeclaration::BoundaryTrait(boundary),
        )
        .ok_or_else(|| failure("selected entry boundary lost its same-source inventory surface"))?;
    let requirements = surface.exact_requirements().map_err(|error| vec![error])?;
    let root_application = surface
        .declaration
        .root_application_identity()
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| failure("selected entry schema has no exact applied owner"))?;
    let catalog = owner.boundary_signatures();
    let mut requests = catalog
        .calling()
        .requests()
        .filter(|row| row.boundary == boundary);
    let request = requests
        .next()
        .ok_or_else(|| failure("selected entry schema has no Calling request"))?;
    if requests.next().is_some() {
        return Err(failure(
            "selected entry schema has competing Calling requests",
        ));
    }
    let instances = catalog.calling().instances(request)?;
    let [instance] = instances else {
        return Err(failure(
            "selected target entry needs one actual Calling policy instance",
        ));
    };
    let policy = *instance
        .policy
        .as_ref()
        .map_err(|reason| failure(reason.clone()))?;
    let physical_role = slot
        .physical_arrival_requirement
        .ok_or_else(|| failure("selected target has no physical entry role"))?;
    let (_, semantic_requirement) = typed_trees::product_operand::core_arrival_requirement(
        owner,
        slot.semantic_arrival_requirement,
    )
    .map_err(failure)?;
    if !requirements.contains(&semantic_requirement.symbol) {
        return Err(failure(
            "selected entry surface does not inherit the exact core semantic arrival",
        ));
    }
    let boundary_source = owner
        .symbols
        .symbol_provenance_source_span(boundary)
        .ok_or_else(|| failure("entry boundary lost its retained source owner"))?;
    let mut semantic = None;
    let mut physical = None;
    for applied in catalog.calling().signatures(instance) {
        if let Some(reason) = &applied.abi_refusal {
            return Err(failure(reason.clone()));
        }
        let declaration = catalog
            .applied_declaration(applied)
            .ok_or_else(|| failure("entry Calling application lost its source declaration"))?;
        if !requirements.contains(&declaration.symbol) {
            return Err(failure(
                "entry Calling application is foreign to its selected source surface",
            ));
        }
        let (declaring_trait, source_signature) = owner
            .trait_machine_signature(declaration.symbol)
            .ok_or_else(|| failure("entry Calling application lost its exact trait signature"))?;
        let role = format!("{}::{}", declaring_trait.name, source_signature.name);
        let [id] = applied.abi_signatures.as_slice() else {
            return Err(failure(
                "entry Calling application is not one closed ABI declaration",
            ));
        };
        let slot = if declaration.symbol == semantic_requirement.symbol {
            &mut semantic
        } else if role == physical_role {
            let source = owner
                .symbols
                .symbol_provenance_source_span(declaration.symbol)
                .ok_or_else(|| failure("physical arrival lost its exact source owner"))?;
            if source.source_id != boundary_source.source_id {
                return Err(failure(
                    "physical arrival is foreign to its authenticated target contract source",
                ));
            }
            &mut physical
        } else {
            return Err(failure(
                "selected entry schema has an unsupported additional calling role",
            ));
        };
        if slot.replace((declaration.symbol, *id)).is_some() {
            return Err(failure("selected entry schema repeats an arrival role"));
        }
    }
    let (semantic_symbol, semantic_id) =
        semantic.ok_or_else(|| failure("selected entry schema omitted semantic arrival"))?;
    let (physical_symbol, physical_id) =
        physical.ok_or_else(|| failure("selected entry schema omitted physical arrival"))?;
    if semantic_symbol == physical_symbol || semantic_id == physical_id {
        return Err(failure(
            "semantic and physical entry arrivals have collapsed identities",
        ));
    }
    let semantic_row = module
        .calling_abi_signature(semantic_id)
        .ok_or_else(|| failure("semantic entry ABI is absent from the accepted module"))?;
    let physical_row = module
        .calling_abi_signature(physical_id)
        .ok_or_else(|| failure("physical entry ABI is absent from the accepted module"))?;
    let semantic_identity = semantic_row.identity(&module.identities).map_err(failure)?;
    let physical_identity = physical_row.identity(&module.identities).map_err(failure)?;
    for (symbol, id, accepted_identity) in [
        (semantic_symbol, semantic_id, &semantic_identity),
        (physical_symbol, physical_id, &physical_identity),
    ] {
        let mut rows = catalog.abi_signatures().iter().filter(|row| row.id == id);
        let row = rows
            .next()
            .ok_or_else(|| failure("entry ABI lost its original source application"))?;
        if rows.next().is_some() || !row.source_templates.contains(&symbol) {
            return Err(failure(
                "entry ABI has competing or foreign source declaration custody",
            ));
        }
        let source_identity = semantic_vocabulary::CallingAbiIdentity {
            requirement_identity: row.requirement_identity.clone(),
            declaring_application: row.declaring_application.clone(),
            operands: row.operands.clone(),
        };
        source_identity.validate().map_err(failure)?;
        if &source_identity != accepted_identity || source_identity.id() != id {
            return Err(failure(
                "accepted entry ABI differs from its complete source application key",
            ));
        }
    }
    // A missing dynamic extent is a concrete materialization refusal. No zero,
    // default extent, or unrelated scalar completion is supplied.
    let extents = |id| {
        Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
            id,
        ))
    };
    let semantic = CanonicalCallingSignature::materialize(module, semantic_id, profile, &extents)
        .map_err(|reason| failure(format!("semantic entry ABI: {reason}")))?;
    let physical = CanonicalCallingSignature::materialize(module, physical_id, profile, &extents)
        .map_err(|reason| failure(format!("physical entry ABI: {reason}")))?;
    if semantic.abi != semantic_identity || physical.abi != physical_identity {
        return Err(failure(
            "materialized entry signature differs from its exact accepted ABI",
        ));
    }
    let source_program = PreparedBuildMachineProgram::prepare(
        owner,
        inputs.map(|inputs| {
            Arc::new(inputs.clone()) as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        }),
    )?;
    let entry = source_program.entry(boundary, policy).map_err(failure)?;
    let layout = profile
        .layout_target_facts()
        .ok_or_else(|| failure("entry Calling policy target has no retained layout facts"))?;
    let address_model = target::TargetSemantics::close_address_model(profile).map(|model| {
        terminal_installation::ScalarAddressModel {
            target: profile.target_name().to_owned(),
            carrier: model.carrier(),
            semantics_version: model.semantics_version(),
            commitment: model.commitment(),
        }
    });
    let invocation_target = InvocationTarget {
        name: profile.target_name(),
        layout_target: Some(&layout),
        carrier_layout: profile
            .native_realization()
            .map(runtime_abi::target_carrier_layout),
        address_model: address_model.as_ref(),
    };
    let semantic_calling_application = evaluate(
        &entry,
        invocation_target,
        semantic,
        root_application,
        accounting,
    )
    .map_err(|reason| failure(format!("semantic entry Calling policy: {reason}")))?;
    let physical_calling_application = evaluate(
        &entry,
        invocation_target,
        physical,
        root_application,
        accounting,
    )
    .map_err(|reason| failure(format!("physical entry Calling policy: {reason}")))?;

    let mut schema = ServiceSchema::from_declaration(Arc::clone(surface.declaration));
    for application in [&semantic_calling_application, &physical_calling_application] {
        let requirement = schema
            .declaration
            .identities
            .id_at(&application.requirement_identity)
            .ok_or_else(|| {
                failure("evaluated entry plan is absent from its selected source schema")
            })?;
        schema.calling_applications.push(ServiceCallingApplication {
            requirement,
            abi: application.abi.clone(),
            plan: ServiceCallingPlan {
                report_fingerprint: application.report_fingerprint,
                commitment: BoundaryCallingPlanCommitment::from_digest(application.commitment),
                promise: BoundaryCallingPlanPromise::from_digest(
                    application.plan.contract_commitment_digest(),
                ),
            },
        });
    }
    schema
        .validate_calling_applications()
        .map_err(|reason| failure(reason.to_string()))?;
    let storage_entry = SelectedProgramStorageEntryPlan::from_target_slot(
        slot,
        schema,
        semantic_identity.requirement_identity,
    )
    .map_err(|reason| failure(reason.to_string()))?;
    let package = slot
        .physical_contract_package
        .ok_or_else(|| failure("selected target physical entry has no source package"))?;
    let physical_source = owner
        .symbols
        .symbol_provenance_source_span(physical_symbol)
        .and_then(|span| owner.symbols.source_file(span))
        .ok_or_else(|| failure("physical entry declaration lost its retained source bytes"))?;
    if physical_source.source_id != boundary_source.source_id {
        return Err(failure(
            "physical entry is not owned by its authenticated target contract source",
        ));
    }
    let source_digest = ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        package,
        physical_source.source.as_bytes(),
    );
    let source_report = callback_nominal_report_fingerprint(
        b"omega.program-entry-retained-source-report.v1",
        &[source_digest.as_bytes()],
    );
    let (parameters, result) =
        physical_type_identities(owner, catalog, physical_id).map_err(failure)?;
    let physical_contract = ProgramEntryPhysicalContractPlan::new(
        slot,
        physical_identity.requirement_identity,
        package,
        source_digest,
        source_report,
        parameters,
        result,
        physical_calling_application
            .plan
            .contract_report_fingerprint(),
        physical_calling_application.plan.plan().clone(),
    )
    .map_err(failure)?;
    let storage_entry = storage_entry
        .with_physical_contract(physical_contract)
        .map_err(|reason| failure(reason.to_string()))?;
    accounting.validate_charges().map_err(failure)?;
    Ok(SelectedProgramEntryCallingPlans {
        semantic_calling_application,
        physical_calling_application,
        storage_entry,
    })
}

fn evaluate(
    entry: &build_time_evaluation::PreparedBuildMachineEntry,
    target: InvocationTarget<'_>,
    signature: CanonicalCallingSignature,
    owner: &str,
    accounting: &mut SourceCompletionAccounting,
) -> Result<BoundaryCallingApplication, String> {
    let arguments = [encode_calling_policy_signature(&signature.policy)?];
    let result = entry.invoke_recorded(target, &arguments, accounting)?;
    let plan = decode_boundary_plan_result(result.value())?;
    let validated = validate_calling_policy_result(plan, &signature.policy)?;
    // These source-declared arrivals have no synthetic dispatch prefix.
    let mut application = signature.native_application(&validated, 0)?;
    application.calling_owner_application = Some(owner.to_owned());
    application.commitment = application.recomputed_commitment()?;
    application.report_fingerprint = callback_nominal_report_fingerprint(
        b"omega.boundary-plan-application-report.v3",
        &[&application.commitment],
    );
    Ok(application)
}

fn physical_type_identities(
    typed: &typed_trees::TypedTrees,
    catalog: &typed_trees::service_declarations::signatures::PreparedBoundarySignatures,
    id: CallingSignatureId,
) -> Result<(Vec<String>, String), String> {
    let row = catalog
        .abi_signatures()
        .iter()
        .find(|row| row.id == id)
        .ok_or("physical entry ABI lost its actual source storage projection")?;
    let text = |reference| {
        if !typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("physical entry type lost its actual source reference".to_owned());
        }
        typed
            .type_identity_text(typed.normalized_type_identity(reference))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| "physical entry type has no complete retained identity".to_owned())
    };
    let mut previous = None;
    for projection in &row.projections {
        let projection = projection.as_ref().map_err(Clone::clone)?;
        if !projection.substitutions.is_empty() {
            return Err("physical entry types require retained substitutions".into());
        }
        let parameters = projection
            .parameters
            .iter()
            .map(|reference| text(*reference))
            .collect::<Result<Vec<_>, _>>()?;
        let result = text(projection.result)?;
        let current = (parameters, result);
        if let Some(previous) = &previous {
            if previous != &current {
                return Err("physical entry has inconsistent source type projections".into());
            }
        } else {
            previous = Some(current);
        }
    }
    previous.ok_or_else(|| "physical entry ABI has no source type projection".into())
}
