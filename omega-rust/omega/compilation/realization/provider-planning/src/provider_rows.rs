// Retained provider plans and canonical direct-boundary data. Source declaration
// discharge objects have no producer after the old checker connection is cut.
use crate::DerivedProviderPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanCandidate {
    source: DerivedProviderPlan,
}

impl ProviderPlanCandidate {
    pub fn plan(&self) -> &ProviderPlan {
        &self.source.plan
    }
    pub fn direct_source(
        &self,
        input: &DirectBoundaryInput,
    ) -> Result<DirectBoundarySource<'_>, Diagnostic> {
        derived_direct_source(&self.source, input.row_index)
    }
    pub fn matches_selection(&self, selected: &crate::SelectedProviderReviewProvenance) -> bool {
        self.plan() == &selected.plan && self.source.provenance == selected.provider
    }
}

fn derived_direct_source(
    source: &DerivedProviderPlan,
    row_index: usize,
) -> Result<DirectBoundarySource<'_>, Diagnostic> {
    let provenance = &source.provenance;
    let row = source
        .plan
        .rows
        .get(row_index)
        .ok_or_else(|| Diagnostic::error("direct boundary lost its original provider row"))?;
    let requirement = provenance
        .row_requirements
        .get(row_index)
        .copied()
        .ok_or_else(|| Diagnostic::error("direct boundary lost its original requirement"))?;
    let realization = provenance
        .row_realizations
        .get(row_index)
        .and_then(crate::ProviderRowRealization::authored)
        .ok_or_else(|| Diagnostic::error("direct boundary lost its authored realization"))?;
    Ok(DirectBoundarySource {
        plan: &source.plan,
        row,
        schema: provenance.schema,
        provider: provenance.provider_type,
        requirement,
        realization,
    })
}

pub struct DirectBoundarySource<'a> {
    pub plan: &'a ProviderPlan,
    pub row: &'a ProviderPlanRow,
    pub schema: crate::ProviderSchemaDeclaration,
    pub provider: Option<symbols::SymbolHandle>,
    pub requirement: symbols::SymbolHandle,
    pub realization: symbols::SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectBoundaryMechanism {
    Intrinsic,
    Foreign(Result<calling_conventions::ExternalBindingRow, Vec<Diagnostic>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectBoundaryInput {
    row_index: usize,
    declaration_identity: String,
    callable_identity: String,
    realization_owner: String,
    realization_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub mechanism: DirectBoundaryMechanism,
}

impl DirectBoundaryInput {
    pub fn finish(
        &self,
        original: &ProviderPlan,
        mut execution: backend_plan::DirectBoundaryExecution,
    ) -> Result<backend_plan::DirectBoundaryLeaf, Diagnostic> {
        let row = original.rows.get(self.row_index).ok_or_else(|| {
            Diagnostic::error("direct boundary projection lost its original provider row")
        })?;
        let requirement = original.identities.text(row.declared_requirement_identity);
        let satisfied_schema = original.schema.clone();
        let mut plan = original.clone();
        plan.rows = vec![row.clone()];
        // A direct callable is not a selected provider declaration. Its one
        // row retains no separate external realization declaration.
        plan.provider_declaration = None;
        plan.row_numeric_realizations = vec![None];
        std::sync::Arc::make_mut(&mut plan.schema.declaration)
            .methods
            .retain(|method| {
                original.schema.identity_text(method.requirement_identity) == requirement
            });
        let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
        declaration.receiver_applications.clear();
        plan.name = format!("direct-boundary:{}:{}", plan.target, self.callable_identity);
        plan.provider_type = self.realization_owner.clone();
        plan.provider_type_package_identity = self.realization_package;
        plan.origin_package_identity = self.realization_package;
        plan.provider_application = plan.identities.intern(&self.realization_owner);
        declaration.trait_name = declaration.identities.intern(&self.callable_identity);
        declaration.trait_package_identity = self.realization_package;
        plan.rows[0].requirement_identity = plan.identities.intern(&self.callable_identity);
        plan.rows[0].declared_requirement_identity = plan.rows[0].requirement_identity;
        plan.rows[0].calling_signature = None;
        let [method] = declaration.methods.as_mut_slice() else {
            return Err(Diagnostic::error(
                "boundary leaf does not bind one exact requirement contract",
            ));
        };
        method.requirement_identity = declaration.identities.intern(&self.callable_identity);
        plan.schema.calling_applications.clear();
        if let backend_plan::DirectBoundaryExecution::Foreign(Ok(binding)) = &mut execution {
            binding.trait_name = self.callable_identity.clone();
            binding.requirement_identity = self.callable_identity.clone();
            binding.provider_type = self.realization_owner.clone();
        }
        let leaf = backend_plan::DirectBoundaryLeaf {
            declaration_identity: self.declaration_identity.clone(),
            callable_identity: self.callable_identity.clone(),
            satisfied_schema,
            plan,
            satisfied_requirement_identity: requirement.to_owned(),
            execution,
            target_realization: None,
        };
        leaf.validate_calling_custody().map_err(Diagnostic::error)?;
        Ok(leaf)
    }
}

/// Realize the checked core primitive at its own callable coordinate, retaining
/// the canonical requirement's complete schema as its semantic owner.
/// No source callback or provider-name classification supplies execution here.
/// Only requested declarations acquire a target realization; unused library
/// declarations remain available for source checking without being deployed.
pub fn direct_compiler_primitive_leaves(
    module: &terminal_psi::TerminalModule,
    schemas: &[effects::provider_plan::ServiceSchema],
    target: &str,
    requested: impl IntoIterator<Item = semantic_vocabulary::BoundaryMachineId>,
) -> Result<Vec<backend_plan::DirectBoundaryLeaf>, Vec<Diagnostic>> {
    let fail = |reason: &str| vec![Diagnostic::error(reason)];
    if target.is_empty() {
        return Err(fail("hosted primitive requires one explicit native target"));
    }
    let mut requested = requested.into_iter().collect::<Vec<_>>();
    requested.sort_unstable();
    if requested.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(fail("requested primitive declaration is repeated"));
    }
    let mut boundaries = Vec::with_capacity(requested.len());
    for boundary in &module.boundary_machines {
        if requested.binary_search(&boundary.id).is_ok() {
            boundaries.push(boundary);
        }
    }
    if boundaries.len() != requested.len() {
        return Err(fail(
            "requested primitive has no declaration in this module",
        ));
    }
    let text = |identity| {
        module
            .identities
            .text(identity)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| fail("hosted primitive lost its exact declaration identity"))
    };
    let mut leaves = Vec::new();
    for boundary in boundaries {
        let Some(declaration) = &boundary.declaration else {
            continue;
        };
        let (canonical, execution, target_realization) = match declaration.execution {
            terminal_psi::CheckedBoundaryExecution::HostedPrimitive {
                operation,
                requirement,
            } => {
                let execution = match operation {
                    language_core::sealed_intrinsics::HostedOperation::ProcessExit => {
                        effects::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
                    }
                    _ => return Err(fail("hosted primitive has no native catalog projection")),
                };
                let canonical = module
                    .boundary_machine(requirement)
                    .ok_or_else(|| fail("hosted primitive lost its canonical requirement link"))?;
                (canonical, execution, None)
            }
            terminal_psi::CheckedBoundaryExecution::NumericPrimitive => {
                terminal_semantics::numeric_primitives::validate_boundary(module, boundary)
                    .map_err(fail)?;
                let meaning = boundary
                    .numeric_intrinsic
                    .ok_or_else(|| fail("numeric primitive lost its semantic meaning"))?;
                let entry = canonical_numeric_realization(module, boundary, target)?;
                (
                    boundary,
                    effects::CompilerIntrinsicExecutionIdentity::from(meaning),
                    Some(entry),
                )
            }
            _ => continue,
        };
        let requirement = text(canonical.identity)?;
        let mut originals = schemas.iter().flat_map(|schema| {
            schema.declaration.methods.iter().filter_map(move |method| {
                (schema.identity_text(method.requirement_identity) == requirement)
                    .then_some((schema, method))
            })
        });
        let (Some((schema, method)), None) = (originals.next(), originals.next()) else {
            return Err(fail(
                "hosted primitive needs one exact canonical requirement schema",
            ));
        };
        let mut original = effects::provider_plan::ProviderPlan {
            target: target.to_owned(),
            schema: schema.clone(),
            ..Default::default()
        };
        let coordinate = original.identities.intern(requirement);
        original.rows.push(effects::provider_plan::ProviderPlanRow {
            method: method.name.clone(),
            requirement_identity: coordinate,
            declared_requirement_identity: coordinate,
            calling_signature: None,
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                machine: text(declaration.source.semantic_declaration)?.to_owned(),
            },
        });
        let row_index = 0;
        let input = DirectBoundaryInput {
            row_index,
            declaration_identity: text(declaration.source.semantic_declaration)?.to_owned(),
            callable_identity: text(declaration.source.callable)?.to_owned(),
            realization_owner: text(declaration.source.machine.identity())?.to_owned(),
            // Receiving has already authenticated both declarations as exact
            // core members; the canonical schema retains that package custody.
            realization_package: original.schema.declaration.trait_package_identity,
            mechanism: DirectBoundaryMechanism::Intrinsic,
        };
        let mut leaf = input
            .finish(
                &original,
                backend_plan::DirectBoundaryExecution::Intrinsic(Some(execution)),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        leaf.target_realization = target_realization;
        if !leaf
            .bind_to_terminal(module)
            .map_err(|reason| fail(&reason))?
        {
            return Err(fail(
                "hosted primitive lost its exact receiving declaration",
            ));
        }
        leaves.push(leaf);
    }
    Ok(leaves)
}

/// Target preparation and native publication consult the same canonical table.
/// A declaration that no selected root reaches imposes no target obligation.
pub(crate) fn canonical_numeric_realization(
    module: &terminal_psi::TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    target: &str,
) -> Result<target::CanonicalFloatRealization, Vec<Diagnostic>> {
    let fail = |reason: &str| vec![Diagnostic::error(reason)];
    if boundary.numeric_intrinsic.is_none() {
        return Err(fail("numeric primitive lost its semantic meaning"));
    }
    let profile = target::TargetProfile::from_canonical_target_name(target)
        .map_err(|_| fail("numeric primitive has no recognized target"))?;
    let requirement = module
        .identities
        .text(boundary.identity)
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| fail("numeric primitive lost its exact declaration identity"))?;
    let entry = profile
        .canonical_float_realization(requirement)
        .ok_or_else(|| fail("numeric primitive has no canonical target realization entry"))?;
    if let target::CanonicalFloatRealization::Unsupported { constraint } = entry {
        return Err(vec![Diagnostic::error(format!(
            "canonical float operation `{requirement}` has no realization on target `{target}`: {constraint}"
        ))]);
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use effects::provider_plan::{ProviderBinding, ServiceMethod, ServiceSchema, ServiceSurface};
    use semantic_vocabulary::{BoundaryCompletion, ExternalCompletionKind};

    /// Isolate contract projection, not source admission or execution authority.
    /// Changing any part of the original exit contract must break custody.
    #[test]
    fn direct_projection_preserves_completion_nominal_result_and_progress() {
        let mut surface = ServiceSurface::default();
        surface.trait_name = surface.identities.intern("core::ProcessExit");
        let requirement = surface.identities.intern("core::ProcessExit::exit_process");
        let parameter = surface.identities.intern("i32");
        let result = surface.identities.intern("core::Never");
        surface.methods.push(ServiceMethod {
            completion: BoundaryCompletion::External(ExternalCompletionKind::ProcessExit),
            name: "exit_process".into(),
            requirement_owner: "core::ProcessExit".into(),
            requirement_identity: requirement,
            parameter_count: 1,
            parameter_type_identities: vec![parameter],
            has_result: true,
            result_type_identity: result,
            service_reach: vec!["core::ProcessExit".into()],
            terminates_guarantee: true,
            ..Default::default()
        });
        let mut original = ProviderPlan {
            target: "linux_x86_64".into(),
            schema: ServiceSchema::from_declaration(std::sync::Arc::new(surface)),
            ..Default::default()
        };
        let requirement = original
            .identities
            .intern("core::ProcessExit::exit_process");
        original.rows.push(ProviderPlanRow {
            method: "exit_process".into(),
            requirement_identity: requirement,
            declared_requirement_identity: requirement,
            calling_signature: None,
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: ProviderBinding::CompilerIntrinsic {
                machine: "core::ProcessExitPrimitive".into(),
            },
        });
        let input = DirectBoundaryInput {
            row_index: 0,
            declaration_identity: "primitive declaration".into(),
            callable_identity: "primitive callable(i32)->core::Never".into(),
            realization_owner: "core::ProcessExitPrimitive".into(),
            realization_package: None,
            mechanism: DirectBoundaryMechanism::Intrinsic,
        };
        let leaf = input
            .finish(
                &original,
                backend_plan::DirectBoundaryExecution::Intrinsic(Some(
                    effects::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
                )),
            )
            .unwrap();
        assert_eq!(leaf.satisfied_schema, original.schema);
        assert!(leaf.validate_calling_custody().is_ok());
        let errors = leaf.plan.validate_against_schema();
        assert!(
            errors.is_empty(),
            "the complete logical exit contract: {errors:?}"
        );
        let mut missing_result_identity = leaf.clone();
        std::sync::Arc::make_mut(&mut missing_result_identity.plan.schema.declaration).methods[0]
            .result_type_identity = Default::default();
        assert!(
            missing_result_identity
                .plan
                .validate_against_schema()
                .iter()
                .any(|error| error
                    .contains("result presence disagrees with its exact result type identity"))
        );
        for change in 0..3 {
            let mut changed = leaf.clone();
            let schema = std::sync::Arc::make_mut(&mut changed.plan.schema.declaration);
            match change {
                0 => schema.methods[0].completion = BoundaryCompletion::NormalReturn,
                1 => schema.methods[0].terminates_guarantee = false,
                _ => schema.methods[0].result_type_identity = schema.identities.intern("Unit"),
            }
            assert!(changed.validate_calling_custody().is_err());
        }
    }
}
