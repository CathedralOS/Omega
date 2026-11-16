//! Coordinate actual source policy invocation for a selected program entry.
//! Source declaration selection and invocation stay here; the completed plans
//! consumed by native/provider planning retain canonical metadata only.

use std::sync::Arc;

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
use provider_planning::calling_policy_plans::{CallingProgram, CanonicalCallingSignature};
use provider_planning::program_entry::SelectedProgramEntryCallingPlans;
use semantic_vocabulary::{CallingSignatureId, callback_nominal_report_fingerprint};
use source_compiler::PortableTerminalProgram;

use crate::CheckedCompilation;

fn failure(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason.into())]
}

pub(crate) fn prepare(
    checked: &CheckedCompilation,
    program: &PortableTerminalProgram,
    inputs: Option<&PackageCompilationInputs>,
    signature: &SelectedProgramEntrySignature,
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
    let selected_policy = *instance
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
        if rows.next().is_some()
            || !row
                .projections
                .iter()
                .any(|projection| projection.source == symbol)
        {
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
    let semantic = CanonicalCallingSignature::materialize(
        program.artifact().closed_module(),
        semantic_id,
        profile,
        &extents,
    )
    .map_err(|reason| failure(format!("semantic entry ABI: {reason}")))?;
    let physical = CanonicalCallingSignature::materialize(
        program.artifact().closed_module(),
        physical_id,
        profile,
        &extents,
    )
    .map_err(|reason| failure(format!("physical entry ABI: {reason}")))?;
    if semantic.abi != semantic_identity || physical.abi != physical_identity {
        return Err(failure(
            "materialized entry signature differs from its exact accepted ABI",
        ));
    }
    let mut policies = module
        .calling_policy_requests
        .iter()
        .flat_map(|request| request.instances.as_ref().into_iter().flatten())
        .filter(|instance| {
            module.identities.text(instance.declaring_application) == Some(root_application)
                && instance.signatures.as_ref().is_ok_and(|signatures| {
                    signatures.contains(&semantic_id) && signatures.contains(&physical_id)
                })
        });
    let policy = policies
        .next()
        .filter(|_| policies.next().is_none())
        .ok_or_else(|| failure("entry has no unique policy in its shared checked module"))?;
    let selected_machine = selected_policy
        .realization(owner, selected_policy.slot)
        .map_err(failure)?;
    let expected_policy = owner
        .machine_callable_identity_text(selected_machine)
        .ok_or_else(|| failure("entry policy lost its complete selected callable identity"))?;
    let policy_machine = policy
        .policy
        .as_ref()
        .map_err(|reason| failure(reason.clone()))?;
    let terminal_psi::TargetMachineCallable::Machine(policy_machine) = *policy_machine else {
        return Err(failure("entry Calling policy has no checked machine body"));
    };
    let published = module
        .machines
        .iter()
        .find(|machine| machine.id == policy_machine)
        .and_then(|machine| machine.declaration.as_ref())
        .ok_or_else(|| failure("entry Calling policy lost its checked declaration"))?;
    if module.identities.text(published.callable) != Some(expected_policy.as_str())
        || policy.policy_declaration.as_ref().ok().copied()
            != module
                .scalar_applications
                .proof_scope
                .callable_declaration(terminal_psi::TargetMachineCallable::Machine(policy_machine))
    {
        return Err(failure(
            "entry Calling policy differs from its exact selected declaration",
        ));
    }
    let requester = match (
        owner.symbols.symbol_package_identity(boundary),
        owner.symbols.symbol_source_origin(boundary),
    ) {
        (Some(package), _) => {
            semantic_vocabulary::compiler_evaluation::DeclarationCustody::Package(package)
        }
        (_, Some(source::SourceOrigin::Toolchain)) => {
            semantic_vocabulary::compiler_evaluation::DeclarationCustody::Toolchain
        }
        (_, Some(source::SourceOrigin::ToolchainContract)) => {
            semantic_vocabulary::compiler_evaluation::DeclarationCustody::ToolchainContract
        }
        _ => {
            return Err(failure(
                "entry Calling policy has no retained declaration custody",
            ));
        }
    };
    let calling = CallingProgram {
        artifact: Arc::clone(program.artifact()),
        evaluator: build_settings::CALLING_POLICY_EVALUATOR,
    };
    let authority = inputs.map(|inputs| {
        inputs as &dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority
    });
    let evaluate = |id, expected: CanonicalCallingSignature| {
        let (signature, plan) = calling.evaluate(policy, profile, id, &[], requester, authority)?;
        if signature.abi != expected.abi {
            return Err("entry policy evaluated another exact declaration ABI".to_owned());
        }
        let mut application = signature.native_application(&plan, None)?;
        application.calling_owner_application = Some(root_application.to_owned());
        application.commitment = application.recomputed_commitment()?;
        application.report_fingerprint = callback_nominal_report_fingerprint(
            b"omega.boundary-plan-application-report.v3",
            &[&application.commitment],
        );
        Ok(application)
    };
    let semantic_calling_application = evaluate(semantic_id, semantic)
        .map_err(|reason| failure(format!("semantic entry Calling policy: {reason}")))?;
    let physical_calling_application = evaluate(physical_id, physical)
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
        physical_type_identities(owner, catalog, physical_id, physical_symbol).map_err(failure)?;
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
    Ok(SelectedProgramEntryCallingPlans {
        semantic_calling_application,
        physical_calling_application,
        storage_entry,
    })
}

fn physical_type_identities(
    typed: &typed_trees::TypedTrees,
    catalog: &typed_trees::service_declarations::signatures::PreparedBoundarySignatures,
    id: CallingSignatureId,
    source: symbols::SymbolHandle,
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
    // The selected entry's source is already authenticated above. A Build
    // copy of that declaration does not describe this Product entry's types.
    for projection in row
        .projections
        .iter()
        .filter(|projection| projection.source == source)
    {
        let projection = projection.storage.as_ref().map_err(Clone::clone)?;
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
