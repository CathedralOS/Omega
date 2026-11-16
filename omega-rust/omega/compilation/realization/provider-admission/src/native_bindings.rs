//! A Build binding supplies one exact boundary without naming a nominal
//! provider. Keep its evaluated receipt and full requirement schema together;
//! the locator alone establishes neither behavior nor execution authority.

use std::sync::Arc;

use effects::provider_plan::{EvaluatedBindingReceipt, EvaluatedNativeBinding, ServiceSchema};
use semantic_vocabulary::BoundaryMachineId;
use terminal_psi::{TerminalIdentityTable, TerminalIdentityTableBuilder};

#[derive(Debug, Clone, Copy)]
pub struct NativeBindingInput<'input> {
    pub boundary: BoundaryMachineId,
    pub binding: &'input EvaluatedNativeBinding,
    pub identities: &'input TerminalIdentityTable,
    pub schema: &'input ServiceSchema,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedNativeBinding {
    pub boundary: BoundaryMachineId,
    pub binding: EvaluatedNativeBinding,
    pub schema: Arc<ServiceSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RetainedNativeBindings {
    pub identities: Arc<TerminalIdentityTable>,
    pub rows: Vec<RetainedNativeBinding>,
}

impl RetainedNativeBindings {
    pub fn inputs(&self) -> Vec<NativeBindingInput<'_>> {
        self.rows
            .iter()
            .map(|row| NativeBindingInput {
                boundary: row.boundary,
                binding: &row.binding,
                identities: &self.identities,
                schema: &row.schema,
            })
            .collect()
    }

    pub fn identity_digest(&self) -> [u8; 32] {
        let mut digest =
            crate::digest_encoder::DomainDigestEncoder::new(b"omega.native-build-bindings.v1\0");
        digest.u64(self.rows.len() as u64);
        for row in &self.rows {
            digest.u64(row.boundary.get());
            digest.raw(&row.binding.receipt().identity_digest());
            digest.raw(row.schema.identity_digest().as_bytes());
        }
        digest.finish()
    }

    pub fn retain(inputs: &[NativeBindingInput<'_>]) -> Result<Self, String> {
        let mut identities = TerminalIdentityTableBuilder::default();
        let mut schemas = std::collections::BTreeMap::<
            effects::provider_plan::ServiceSchemaDigest,
            Arc<ServiceSchema>,
        >::new();
        let mut rows = Vec::with_capacity(inputs.len());
        let mut boundaries = inputs
            .iter()
            .map(|input| input.boundary)
            .collect::<Vec<_>>();
        boundaries.sort_unstable();
        if boundaries.windows(2).any(|rows| rows[0] == rows[1]) {
            return Err("native Build bindings repeat one exact boundary".into());
        }
        for input in inputs {
            validate_receipt(input.binding, input.identities)?;
            let schema = Arc::clone(
                schemas
                    .entry(input.schema.identity_digest())
                    .or_insert_with(|| Arc::new(input.schema.clone())),
            );
            rows.push(RetainedNativeBinding {
                boundary: input.boundary,
                binding: input
                    .binding
                    .reintern_producer(input.identities, &mut identities)?,
                schema,
            });
        }
        Ok(Self {
            identities: Arc::new(identities.build()),
            rows,
        })
    }
}

pub fn validate_receipt(
    binding: &EvaluatedNativeBinding,
    identities: &TerminalIdentityTable,
) -> Result<(), String> {
    let receipt = binding.receipt();
    let producer = identities
        .text(receipt.producer_callable_identity())
        .filter(|identity| !identity.is_empty())
        .ok_or("native Build receipt lost its exact producer")?;
    let origin = receipt
        .build_origin()
        .ok_or("native binding has no completed Build occurrence")?;
    if receipt.producer_package().is_none()
        || receipt.locator_identity_digest() != binding.identity_digest()
    {
        return Err("native Build receipt changed its supplier or locator".into());
    }
    let mut reconstructed_identities = TerminalIdentityTableBuilder::default();
    let mut reconstructed = EvaluatedBindingReceipt::from_evaluation(
        receipt.producer_package(),
        producer,
        &mut reconstructed_identities,
        receipt.producer_closure_digest(),
        receipt.evaluator_semantics_marker(),
        receipt.evaluation_usage(),
        receipt.evaluation_digest(),
        receipt.materializer_schema_version(),
        receipt.materialization_digest(),
        receipt.locator_identity_digest(),
    )?;
    let mut reconstructed_origin = origin.clone();
    let requirement = identities
        .text(origin.requirement_identity)
        .filter(|identity| !identity.is_empty())
        .ok_or("native Build receipt lost its exact requirement")?;
    reconstructed_origin.requirement_identity = reconstructed_identities.intern(requirement);
    reconstructed = reconstructed
        .with_build_origin(reconstructed_origin, reconstructed_identities.as_table())?;
    if reconstructed.identity_digest() != receipt.identity_digest() {
        return Err("native Build receipt does not match its retained evaluation".into());
    }
    // Reconstruct the materialized mechanism as well as its receipt.
    match binding {
        EvaluatedNativeBinding::Import(import) => {
            effects::provider_plan::EvaluatedForeignImport::from_retained_evidence(
                import.locator().clone(),
                receipt.clone(),
            )?;
        }
        EvaluatedNativeBinding::Syscall(syscall) => {
            effects::provider_plan::EvaluatedForeignSyscall::from_retained_evidence(
                syscall.target(),
                u64::try_from(syscall.number()).map_err(|_| "native syscall number is negative")?,
                receipt.clone(),
            )?;
        }
    }
    Ok(())
}

pub(crate) fn settle<'execution>(
    input: &crate::AdmissionSource<'_>,
    request: &crate::provider_admission::ProviderAdmissionRequest<'_, 'execution>,
) -> Result<
    (
        Vec<crate::AdmittedBoundarySettlement<'execution>>,
        Vec<crate::provider_admission::AdmittedTerminalMechanism>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    use crate::AdmittedBoundaryExecution;
    use target_operations::{
        BoundarySettlementRealization, NormalizedForeignCallBinding,
        NormalizedForeignCallMechanism, NormalizedForeignExecution, NormalizedSystemCallBinding,
    };
    let retained = request.native_bindings;
    let refuse = |reason: String| {
        vec![diagnostics::Diagnostic::error(format!(
            "native Build binding admission: {reason}"
        ))]
    };
    let mut settlements = Vec::new();
    let mut mechanisms = Vec::new();
    let mut boundaries = retained
        .rows
        .iter()
        .map(|row| row.boundary)
        .collect::<Vec<_>>();
    boundaries.sort_unstable();
    if boundaries.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(refuse("duplicate exact boundary supply".into()));
    }
    for row in &retained.rows {
        let module = input.module();
        let boundary = validate_native_contract(row, &retained.identities, module, request.profile)
            .map_err(refuse)?;
        let requirement = module
            .identities
            .text(boundary.identity)
            .ok_or_else(|| refuse("boundary has no terminal identity".into()))?;
        if request
            .selected_provider_plans
            .selected_requirement_row(requirement)
            .is_some()
            || request
                .compiler_builtins
                .iter()
                .any(|builtin| builtin.boundary == row.boundary)
        {
            return Err(refuse(
                "boundary has competing native and provider executions".into(),
            ));
        }
        // The module may retain choices for other roots. Only the actual
        // admitted execution scope produces a physical settlement here.
        if input.plan().boundary_machine(row.boundary).is_none() {
            continue;
        }
        let abi = module.calling_abi_identity(row.boundary).map_err(refuse)?;
        let mut applications = request
            .installed_calling
            .iter()
            .filter(|application| application.abi == abi);
        let application = applications
            .next()
            .filter(|_| applications.next().is_none())
            .ok_or_else(|| refuse("binding has no unique exact calling application".into()))?;
        let execution =
            validate_native_calling(row, module, request.profile, boundary, application)
                .map_err(refuse)?;
        let receipt = row.binding.receipt().identity_digest();
        let entry = application.plan.plan().clone();
        let (mechanism, physical) = match &row.binding {
            EvaluatedNativeBinding::Import(import) => {
                if entry.call.policy
                    != calling_conventions::CallingPolicy::native_for_target(request.target())
                {
                    return Err(refuse("import has another target calling policy".into()));
                }
                if request.profile.program_entry_slot().schema
                    != target::ProgramEntrySchema::HostedApplication
                {
                    return Err(refuse(
                        "foreign import needs admitted demand on a fixed or freestanding stack"
                            .into(),
                    ));
                }
                let physical = crate::provider_admission::normalized_foreign_terminal_mechanism(
                    import.locator(),
                    &entry,
                )
                .map_err(refuse)?;
                (
                    NormalizedForeignCallMechanism::HostedImport {
                        locator: import.locator().clone(),
                        receipt,
                        requirement_identity: requirement.to_owned(),
                    },
                    physical,
                )
            }
            EvaluatedNativeBinding::Syscall(syscall) => {
                let policy =
                    calling_conventions::CallingPolicy::native_syscall_for_target(request.target())
                        .ok_or_else(|| refuse("syscall target is unsupported".into()))?;
                let signature = calling_conventions::CallSignature::of_placements(
                    &entry.call.parameters,
                    entry.call.result.as_ref(),
                );
                if calling_conventions::evaluate_call_plan(policy, &signature).as_ref()
                    != Ok(&entry.call)
                {
                    return Err(refuse(
                        "syscall has another exact kernel calling policy".into(),
                    ));
                }
                let physical = crate::provider_admission::conservative_syscall_terminal_mechanism(
                    request.profile,
                    syscall.number(),
                    input.plan(),
                    row.boundary,
                    input.closed_module(),
                )
                .map_err(refuse)?;
                (
                    NormalizedForeignCallMechanism::SystemCall(NormalizedSystemCallBinding {
                        target: request.profile,
                        number: syscall.number(),
                        requirement_identity: requirement.to_owned(),
                        execution: NormalizedForeignExecution::NativeBinding(receipt),
                    }),
                    physical,
                )
            }
        };
        let foreign = NormalizedForeignCallBinding {
            boundary_entry_plan: entry,
            mechanism,
        };
        if !foreign.has_valid_entry_mechanism() {
            return Err(refuse(
                "binding has an invalid physical entry mechanism".into(),
            ));
        }
        settlements.push(crate::AdmittedBoundarySettlement {
            boundary: row.boundary,
            execution: AdmittedBoundaryExecution::NativeBinding {
                execution,
                schema: row.schema.identity_digest(),
            },
            realization: BoundarySettlementRealization::NormalizedForeignCall(foreign),
        });
        mechanisms.push(crate::provider_admission::AdmittedTerminalMechanism {
            boundary: row.boundary,
            mechanism: physical,
        });
    }
    Ok((settlements, mechanisms))
}

fn validate_schema_declaration(schema: &ServiceSchema, checked: [u8; 32]) -> Result<(), String> {
    schema
        .declaration
        .validate_receiver_applications()
        .map_err(str::to_owned)?;
    if checked == [0; 32] || checked != schema.declaration.declaration_commitment() {
        return Err("native binding service schema differs from the checked declaration".into());
    }
    Ok(())
}

fn validate_native_contract<'module>(
    row: &RetainedNativeBinding,
    identities: &TerminalIdentityTable,
    module: &'module terminal_psi::TerminalModule,
    profile: target::TargetProfile,
) -> Result<&'module terminal_psi::BoundaryMachineDeclaration, String> {
    validate_receipt(&row.binding, identities)?;
    if row.binding.target() != profile {
        return Err("binding belongs to another target".into());
    }
    let boundary = module
        .boundary_machine(row.boundary)
        .ok_or_else(|| "binding has no canonical boundary declaration".to_owned())?;
    let declaration = boundary
        .declaration
        .as_ref()
        .ok_or_else(|| "binding has no checked requirement contract".to_owned())?;
    validate_schema_declaration(&row.schema, declaration.service_declaration)?;
    let callable = module
        .identities
        .text(declaration.source.callable)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "binding requirement lost its exact callable identity".to_owned())?;
    let origin = row
        .binding
        .receipt()
        .build_origin()
        .ok_or_else(|| "binding lost its Build occurrence".to_owned())?;
    if identities.text(origin.requirement_identity) != Some(callable) {
        return Err("receipt designates another exact requirement".into());
    }
    let mut methods = row
        .schema
        .declaration
        .methods
        .iter()
        .filter(|method| row.schema.identity_text(method.requirement_identity) == callable);
    let method = methods
        .next()
        .filter(|_| methods.next().is_none())
        .ok_or_else(|| "full service schema has no unique exact requirement".to_owned())?;
    if method.completion != boundary.completion {
        return Err("native requirement changes its declared completion".into());
    }
    if boundary.unconditional_termination
        != (method.terminates_guarantee && method.termination_premises.is_empty())
    {
        return Err("native requirement changes its declared termination guarantee".into());
    }
    if method.may_suspend != declaration.source.published_may_suspend {
        return Err("native requirement changes its declared suspension behavior".into());
    }
    if method.may_block != declaration.source.published_may_block {
        return Err("native requirement changes its declared blocking behavior".into());
    }
    validate_native_declared_types(
        method,
        &declaration.source.parameter_types,
        declaration.source.result_type,
        &boundary.result,
        &module.identities,
    )?;
    Ok(boundary)
}

fn validate_native_declared_types(
    method: &effects::provider_plan::ServiceMethod,
    parameter_types: &[terminal_psi::TerminalIdentityIndex],
    result_type: terminal_psi::TerminalIdentityIndex,
    result: &terminal_psi::BoundaryMachineResult,
    identities: &TerminalIdentityTable,
) -> Result<(), String> {
    if method.parameter_count != parameter_types.len()
        || method.parameter_type_identities.len() != method.parameter_count
    {
        return Err("native requirement changes its declared parameter roster".into());
    }
    if !method.has_result && (!result.is_unit() || method.result_type_identity.is_valid()) {
        return Err("native requirement with an omitted result must retain exact Unit".into());
    }
    let parameters = parameter_types
        .iter()
        .enumerate()
        .map(|(ordinal, identity)| {
            identities
                .text(*identity)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| format!("native parameter {ordinal} lost its exact declared type"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = identities
        .text(result_type)
        .filter(|identity| !identity.is_empty())
        .ok_or("native result lost its exact declared type")?;
    let signature = language_semantics::service_surface::declared_type_signature(
        parameters.iter().copied(),
        result,
    )?;
    if method.declared_type_signature == [0; 32] || method.declared_type_signature != signature {
        return Err(format!(
            "native declared types differ from their authenticated service signature (parameters {parameters:?}, result `{result}`)"
        ));
    }
    Ok(())
}

pub fn validate_native_binding_execution(
    row: &RetainedNativeBinding,
    identities: &TerminalIdentityTable,
    module: &terminal_psi::TerminalModule,
    profile: target::TargetProfile,
    application: &calling_conventions::BoundaryCallingApplication,
) -> Result<target_operations::NativeBindingExecution, String> {
    let boundary = validate_native_contract(row, identities, module, profile)?;
    validate_native_calling(row, module, profile, boundary, application)
}

fn validate_native_calling(
    row: &RetainedNativeBinding,
    module: &terminal_psi::TerminalModule,
    profile: target::TargetProfile,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    application: &calling_conventions::BoundaryCallingApplication,
) -> Result<target_operations::NativeBindingExecution, String> {
    let abi = module.calling_abi_identity(row.boundary)?;
    if application.abi != abi
        || application.target != profile.native_target()
        || application.dispatch_only_formal.is_some()
    {
        return Err("calling application changed its exact ABI, target or dispatch".into());
    }
    validate_native_calling_plan(application)?;
    row.schema
        .validate_calling_applications()
        .map_err(str::to_owned)?;
    for retained in row.schema.calling_applications.iter().filter(|calling| {
        row.schema.identity_text(calling.requirement) == application.abi.requirement_identity
    }) {
        if retained.abi != application.abi
            || retained.plan.report_fingerprint != application.report_fingerprint
            || retained.plan.commitment.as_bytes() != application.commitment
            || retained.plan.promise.as_bytes() != application.plan.contract_commitment_digest()
        {
            return Err("native schema changed its retained calling application".into());
        }
    }
    let telescope = calling_conventions::TerminalCallingTelescope::for_application(
        module,
        row.boundary,
        application,
    )?;
    let parameters = application
        .signature
        .native_parameters
        .span(application.signature.native_parameter_rows)
        .ok_or("calling application has stale parameter storage")?;
    if parameters != telescope.parameters.as_slice() {
        return Err("calling application differs from the canonical argument telescope".into());
    }
    validate_native_geometry(module, profile, boundary, application)?;
    if !telescope.binders.is_empty() {
        return Err(
            "native Build callbacks need their admitted calling and retention resource".into(),
        );
    }
    let declaration = boundary
        .declaration
        .as_ref()
        .ok_or("native binding lost its checked requirement")?;
    target_operations::NativeBindingExecution::from_commitments(
        row.binding.receipt().identity_digest(),
        declaration.source.contract,
        application.commitment,
    )
    .ok_or_else(|| "native binding has an incomplete admitted commitment".into())
}

fn validate_native_calling_plan(
    application: &calling_conventions::BoundaryCallingApplication,
) -> Result<(), String> {
    // Check placements against the declared value geometry. A plan can be
    // internally valid and still use the wrong width for this requirement.
    // Recovery already owns that correspondence and the complete commitment.
    calling_conventions::BoundaryCallingApplicationCandidate {
        abi: application.abi.clone(),
        calling_owner_application: application.calling_owner_application.clone(),
        requirement_identity: application.requirement_identity.clone(),
        target: application.target,
        report_fingerprint: application.report_fingerprint,
        commitment: application.commitment,
        plan: application.plan.plan().clone(),
        dispatch_only_formal: application.dispatch_only_formal,
        signature: application.signature.clone(),
        opaque_applications: application.opaque_applications.clone(),
        callback_context: application.callback_context.clone(),
        callback_context_closed: application.callback_context_closed,
    }
    .validate()?;
    Ok(())
}

/// Replay the evaluated mechanism as well as its contract and calling plan.
pub fn validate_native_binding_realization(
    row: &RetainedNativeBinding,
    identities: &TerminalIdentityTable,
    module: &terminal_psi::TerminalModule,
    profile: target::TargetProfile,
    application: &calling_conventions::BoundaryCallingApplication,
    binding: &target_operations::NormalizedForeignCallBinding,
) -> Result<target_operations::NativeBindingExecution, String> {
    let execution =
        validate_native_binding_execution(row, identities, module, profile, application)?;
    if binding.boundary_entry_plan != *application.plan.plan()
        || !binding.has_valid_entry_mechanism()
    {
        return Err("native binding changed its exact physical calling plan".into());
    }
    let mechanism_matches = match (&row.binding, &binding.mechanism) {
        (
            EvaluatedNativeBinding::Import(evaluated),
            target_operations::NormalizedForeignCallMechanism::HostedImport {
                locator,
                receipt,
                requirement_identity,
            },
        ) => {
            let declared = module
                .boundary_machine(row.boundary)
                .and_then(|boundary| module.identities.text(boundary.identity));
            evaluated.locator() == locator
                && *receipt == execution.receipt()
                && declared == Some(requirement_identity.as_str())
        }
        (
            EvaluatedNativeBinding::Syscall(evaluated),
            target_operations::NormalizedForeignCallMechanism::SystemCall(syscall),
        ) => {
            evaluated.target() == syscall.target
                && evaluated.number() == syscall.number
                && module
                    .boundary_machine(row.boundary)
                    .and_then(|boundary| module.identities.text(boundary.identity))
                    == Some(syscall.requirement_identity.as_str())
        }
        _ => false,
    };
    if !mechanism_matches
        || !binding.matches_execution(target_operations::ExternalExecutionBinding::NativeBinding(
            execution,
        ))
    {
        return Err(
            "native binding changed its evaluated locator, syscall or execution receipt".into(),
        );
    }
    Ok(execution)
}

fn validate_native_geometry(
    module: &terminal_psi::TerminalModule,
    profile: target::TargetProfile,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    application: &calling_conventions::BoundaryCallingApplication,
) -> Result<(), String> {
    use calling_conventions::BoundaryValueClass;
    let scalar = |kind: semantic_vocabulary::ScalarType, root: u16| -> Result<(), String> {
        let expected = calling_conventions::terminal_layout::scalar_shape(kind)
            .map_err(|error| format!("native scalar layout is unavailable: {error:?}"))?;
        let actual = application
            .signature
            .shapes
            .get(usize::from(root))
            .ok_or("native scalar has no retained value shape")?;
        let class = match expected.class {
            calling_conventions::ValueClass::Integer => BoundaryValueClass::Integer,
            calling_conventions::ValueClass::Float => BoundaryValueClass::Float,
            _ => return Err("native scalar has no scalar calling class".into()),
        };
        if actual.class != class
            || u32::from(actual.byte_size) != expected.byte_size
            || actual.alignment != expected.alignment
        {
            return Err("native scalar calling shape differs from its canonical carrier".into());
        }
        Ok(())
    };
    let owned = |kind: semantic_vocabulary::StructuralTypeId, root: u16| -> Result<(), String> {
        let shape = module
            .structural_type(kind)
            .ok_or("native owned value has no canonical type")?;
        if let terminal_psi::StructuralTypeShape::PrimitiveScalar(kind) = shape.shape {
            return scalar(kind, root);
        }
        if matches!(
            shape.shape,
            terminal_psi::StructuralTypeShape::Record { .. }
                | terminal_psi::StructuralTypeShape::FixedArray { .. }
        ) {
            return validate_owned_aggregate(module, profile, application, kind, root);
        }
        if !shape.shape.is_view_descriptor()
            && !matches!(
                shape.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            )
        {
            return Err(
                "native owned aggregate needs complete canonical field calling correspondence"
                    .into(),
            );
        }
        let actual = application
            .signature
            .shapes
            .get(usize::from(root))
            .ok_or("native descriptor has no retained value shape")?;
        let geometry =
            calling_conventions::terminal_layout::NativeGeometry::new(&module.structural_types)
                .map_err(|error| format!("native type catalog: {error:?}"))?;
        let expected = geometry
            .shape(kind)
            .map_err(|error| format!("native descriptor layout: {error:?}"))?;
        if actual.class != BoundaryValueClass::Reference
            || u32::from(actual.byte_size) != expected.byte_size
            || actual.alignment != expected.alignment
        {
            return Err("native descriptor calling shape differs from its canonical type".into());
        }
        Ok(())
    };
    let mut scalars = boundary.scalar_parameters.iter();
    let mut structural = boundary.structural_parameters.iter();
    for (kind, root) in boundary
        .parameter_order
        .iter()
        .zip(&application.signature.parameters)
    {
        match kind {
            terminal_psi::BoundaryParameterKind::Scalar => scalar(
                *scalars
                    .next()
                    .ok_or("native scalar telescope is incomplete")?,
                *root,
            )?,
            terminal_psi::BoundaryParameterKind::Structural => {
                let formal = structural
                    .next()
                    .ok_or("native structural telescope is incomplete")?;
                if formal.access == terminal_psi::StructuralAccess::Owned {
                    owned(formal.structural_type, *root)?;
                }
                // Borrowed presentation is checked against the canonical foreign
                // descriptor role by TerminalCallingTelescope.
            }
        }
    }
    match (&boundary.result, application.signature.result) {
        (terminal_psi::BoundaryMachineResult::Unit, None) => Ok(()),
        (terminal_psi::BoundaryMachineResult::Scalar(kind), Some(root)) => {
            scalar(kind.scalar_type, root)
        }
        (terminal_psi::BoundaryMachineResult::Structural(kind), Some(root)) => {
            owned(kind.structural_type, root)
        }
        (terminal_psi::BoundaryMachineResult::Structural(kind), None) => {
            if terminal_semantics::inhabitation::external_result_is_erased(
                boundary.completion,
                Some(kind.structural_type),
                &module.structural_types,
            )? {
                Ok(())
            } else {
                Err("native calling result erased a type without external completion".into())
            }
        }
        _ => Err("native calling result differs from its canonical requirement".into()),
    }
}

fn validate_owned_aggregate(
    module: &terminal_psi::TerminalModule,
    profile: target::TargetProfile,
    application: &calling_conventions::BoundaryCallingApplication,
    kind: semantic_vocabulary::StructuralTypeId,
    root: u16,
) -> Result<(), String> {
    // Use the same public storage projection as the calling-plan producer.
    // A retained ABI length cannot establish an unresolved semantic extent.
    let extents = |id| {
        Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
            id,
        ))
    };
    let mut recipes = layout::TerminalBoundaryRecipes::new(module, profile.target_name(), &extents);
    let recipe = recipes.value(kind)?;
    let mut expected =
        layout::BoundaryValueLayout::new(layout::LayoutTarget::for_profile(profile)?);
    let (_, canonical) = expected.value_type(recipes.recipes(), recipe)?;
    let actual = calling_conventions::BoundaryValueGraph {
        shapes: &application.signature.shapes,
        fields: &application.signature.fields,
    };
    if !expected.matches_value(canonical, &actual, root) {
        return Err("native owned aggregate differs from its canonical field layout".into());
    }
    let (_, _, opaque_occurrences) = expected.into_parts();
    if !opaque_occurrences.is_empty() {
        return Err("native owned aggregate needs admitted opaque representation custody".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use effects::provider_plan::*;

    #[test]
    fn native_declared_types_accept_i32_and_reject_a_different_type() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let i32_type = identities.intern("compiler-type(atom(i32))");
        let i64_type = identities.intern("compiler-type(atom(i64))");
        let mut surface_types = ProviderIdentityTable::default();
        let i32_surface = surface_types.intern("i32");
        let method = ServiceMethod {
            parameter_count: 1,
            parameter_type_identities: vec![i32_surface],
            has_result: true,
            result_type_identity: i32_surface,
            declared_type_signature: language_semantics::service_surface::declared_type_signature(
                ["compiler-type(atom(i32))"].into_iter(),
                "compiler-type(atom(i32))",
            )
            .unwrap(),
            ..Default::default()
        };
        let result = terminal_psi::BoundaryMachineResult::Scalar(
            semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                    .unwrap(),
            )
            .into(),
        );
        validate_native_declared_types(
            &method,
            &[i32_type],
            i32_type,
            &result,
            identities.as_table(),
        )
        .unwrap();
        for (parameters, result_type) in [(vec![i64_type], i32_type), (vec![i32_type], i64_type)] {
            assert!(
                validate_native_declared_types(
                    &method,
                    &parameters,
                    result_type,
                    &result,
                    identities.as_table()
                )
                .unwrap_err()
                .contains("declared types differ")
            );
        }
    }

    #[test]
    fn native_omitted_unit_result_retains_its_declared_type() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let unit = identities.intern("unit");
        let scalar = identities.intern("compiler-type(atom(i32))");
        let method = ServiceMethod {
            declared_type_signature: language_semantics::service_surface::declared_type_signature(
                std::iter::empty::<&str>(),
                "unit",
            )
            .unwrap(),
            ..Default::default()
        };
        validate_native_declared_types(
            &method,
            &[],
            unit,
            &terminal_psi::BoundaryMachineResult::Unit,
            identities.as_table(),
        )
        .unwrap();
        assert!(
            validate_native_declared_types(
                &method,
                &[],
                scalar,
                &terminal_psi::BoundaryMachineResult::Unit,
                identities.as_table()
            )
            .unwrap_err()
            .contains("declared types differ")
        );
        let structural = terminal_psi::BoundaryMachineResult::Structural(
            terminal_psi::BoundaryStructuralResultDeclaration {
                structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
        );
        assert!(
            validate_native_declared_types(&method, &[], unit, &structural, identities.as_table())
                .unwrap_err()
                .contains("omitted result must retain exact Unit")
        );
    }

    #[test]
    fn native_calling_placements_must_match_the_declared_value_width() {
        use calling_conventions::{
            BoundaryCallingApplication, BoundarySignatureGeometry, BoundaryValueClass,
            BoundaryValueShape, CallSignature, CallingPolicy, ValueClass, ValueShape,
        };
        let plan_for = |bytes: u16| {
            calling_conventions::evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::MicrosoftX64,
                &CallSignature {
                    parameters: Vec::new(),
                    result: Some(ValueShape {
                        class: ValueClass::Integer,
                        byte_size: u32::from(bytes),
                        alignment: bytes,
                    }),
                },
            )
            .unwrap()
        };
        let mut application = BoundaryCallingApplication {
            abi: semantic_vocabulary::CallingAbiIdentity {
                requirement_identity: "package::Raw::number".into(),
                declaring_application: "package::Raw".into(),
                operands: Vec::new(),
            },
            calling_owner_application: None,
            requirement_identity: "package::Raw::number".into(),
            target: target::NativeTarget::windows_x64(),
            report_fingerprint: 1,
            commitment: [0; 32],
            plan: plan_for(4),
            dispatch_only_formal: None,
            signature: BoundarySignatureGeometry {
                shapes: vec![BoundaryValueShape {
                    class: BoundaryValueClass::Integer,
                    byte_size: 4,
                    alignment: 4,
                }],
                fields: Vec::new(),
                parameters: Vec::new(),
                result: Some(0),
                native_parameters: Default::default(),
                native_parameter_rows: Default::default(),
                callback_layouts: Default::default(),
                callback_layout_rows: Default::default(),
                callback_bindings: Default::default(),
                callback_binding_rows: Default::default(),
            },
            opaque_applications: Vec::new(),
            callback_context: Default::default(),
            callback_context_closed: false,
        };
        application.commitment = application.recomputed_commitment().unwrap();
        validate_native_calling_plan(&application).unwrap();

        // This is a valid 64-bit plan, with an honestly recomputed commitment,
        // but the requirement still returns a 32-bit value.
        application.plan = plan_for(8);
        application.commitment = application.recomputed_commitment().unwrap();
        let error = validate_native_calling_plan(&application).unwrap_err();
        assert!(error.contains("boundary calling plan"), "{error}");
    }

    fn evaluated(with_origin: bool) -> (EvaluatedNativeBinding, TerminalIdentityTable) {
        let mut identities = TerminalIdentityTableBuilder::default();
        let requirement = identities.intern("Raw::number()->u64");
        let receipt = EvaluatedBindingReceipt::from_evaluation(
            semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
            "Build::build",
            &mut identities,
            EvaluatedBindingProducerClosureDigest::from_bytes([2; 32]).unwrap(),
            1,
            EvaluatedBindingUsage::from_evaluator(1, 1, 1, 100, 0, 0, 1, 0, 1, 0).unwrap(),
            EvaluatedBindingEvaluationDigest::from_bytes([3; 32]).unwrap(),
            1,
            EvaluatedBindingMaterializationDigest::from_bytes([4; 32]).unwrap(),
            evaluated_syscall_identity_digest(target::TargetProfile::LinuxX64, 39),
        )
        .unwrap();
        let receipt = if with_origin {
            receipt
                .with_build_origin(
                    BuildNativeBindingOrigin {
                        requirement_identity: requirement,
                        source_occurrence_digest: [5; 32],
                        terminal_code: terminal_psi::TerminalPsiIdentity {
                            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes(
                                [6; 32],
                            ),
                        },
                        work_schedule: 1,
                        storage_schedule: 1,
                        observation_digest: [7; 32],
                    },
                    identities.as_table(),
                )
                .unwrap()
        } else {
            receipt
        };
        (
            EvaluatedNativeBinding::Syscall(
                EvaluatedForeignSyscall::from_retained_evidence(
                    target::TargetProfile::LinuxX64,
                    39,
                    receipt,
                )
                .unwrap(),
            ),
            identities.build(),
        )
    }

    #[test]
    fn retained_native_receipts_require_the_original_build_occurrence() {
        let (binding, identities) = evaluated(true);
        validate_receipt(&binding, &identities).unwrap();
        let (unowned, identities) = evaluated(false);
        assert!(
            validate_receipt(&unowned, &identities)
                .unwrap_err()
                .contains("completed Build occurrence")
        );
    }

    #[test]
    fn native_roster_keeps_one_shared_identity_table_and_rejects_competing_rows() {
        let (binding, identities) = evaluated(true);
        let schema = ServiceSchema::default();
        let input = NativeBindingInput {
            boundary: BoundaryMachineId::new(1).unwrap(),
            binding: &binding,
            identities: &identities,
            schema: &schema,
        };
        let retained = RetainedNativeBindings::retain(&[input]).unwrap();
        validate_receipt(&retained.rows[0].binding, &retained.identities).unwrap();
        assert_eq!(
            retained.rows[0].binding.receipt().identity_digest(),
            binding.receipt().identity_digest()
        );
        assert!(
            RetainedNativeBindings::retain(&[input, input])
                .unwrap_err()
                .contains("repeat")
        );
    }

    #[test]
    fn native_schema_cannot_change_declared_reach_or_claims() {
        let mut declaration = ServiceSurface::default();
        declaration.trait_name = declaration.identities.intern("Raw");
        declaration.methods.push(ServiceMethod {
            name: "number".into(),
            requirement_owner: "Raw".into(),
            requirement_identity: declaration.identities.intern("Raw::number()->u64"),
            ..Default::default()
        });
        let original = ServiceSchema::from_declaration(Arc::new(declaration));
        let checked = original.declaration.declaration_commitment();
        validate_schema_declaration(&original, checked).unwrap();
        for change in [0, 1, 2, 3] {
            let mut changed = original.clone();
            let method = &mut Arc::make_mut(&mut changed.declaration).methods[0];
            match change {
                0 => method.service_reach.push("ProcessExit".into()),
                1 => method.synchronous_invocations.push("Raw::callback".into()),
                2 => method.result_claims.push(ServiceResultClaim {
                    domain: "Granted".into(),
                    effective_carry: Default::default(),
                }),
                _ => method.declared_type_signature[0] ^= 1,
            }
            assert!(validate_schema_declaration(&changed, checked).is_err());
        }
    }
}
