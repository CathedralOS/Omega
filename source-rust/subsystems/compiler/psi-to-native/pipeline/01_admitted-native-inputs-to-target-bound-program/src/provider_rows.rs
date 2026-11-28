// Retained provider plans and canonical direct-boundary data. Source declaration
// discharge objects have no producer after the old checker connection is cut.
use diagnostics::Diagnostic;
use build_providers::plans::ProviderPlan;

pub(crate) struct DirectBoundaryInput {
    declaration_identity: String,
    callable_identity: String,
    realization_owner: String,
    realization_package: Option<semantic_vocabulary::PackageKeyIdentity>,
}

impl DirectBoundaryInput {
    /// A Build binds this requirement itself; there is no forwarding machine
    /// or supplied conformance to invent for its external execution.
    pub(crate) fn from_boundary(
        module: &terminal_psi::TerminalModule,
        boundary: &terminal_psi::BoundaryMachineDeclaration,
        realization_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) -> Result<Self, Diagnostic> {
        let declaration = boundary
            .declaration
            .as_ref()
            .ok_or_else(|| Diagnostic::error("direct binding lost its source declaration"))?;
        let text = |id| {
            module
                .identities
                .text(id)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    Diagnostic::error("direct binding has an unresolved source identity")
                })
        };
        Ok(Self {
            declaration_identity: text(declaration.source.semantic_declaration)?,
            callable_identity: text(declaration.source.callable)?,
            // A direct requirement has no selected named implementation.
            realization_owner: String::new(),
            realization_package,
        })
    }

    /// Keep the exact external declaration the checker admitted. This adapter
    /// describes a native leaf; receiving execution still admits its binding.
    pub(crate) fn from_checked(
        source: &terminal_psi::ExternalProviderConformance,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, Diagnostic> {
        source.validate(identities).map_err(Diagnostic::error)?;
        let text = |identity| {
            identities
                .text(identity)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    Diagnostic::error("native leaf lost a complete checked declaration identity")
                })
        };
        Ok(Self {
            declaration_identity: text(source.realization.machine.identity())?,
            callable_identity: text(source.realization.callable)?,
            realization_owner: source
                .provider
                .as_ref()
                .map(|provider| text(provider.identity))
                .transpose()?
                .unwrap_or_default(),
            realization_package: source
                .provider
                .as_ref()
                .and_then(|provider| provider.package),
        })
    }
    pub(crate) fn finish(
        &self,
        original: &ProviderPlan,
        mut execution: direct_boundary_bindings::DirectBoundaryExecution,
    ) -> Result<direct_boundary_bindings::DirectBoundaryLeaf, Diagnostic> {
        let row = original.rows.first().ok_or_else(|| {
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
        if plan.rows[0].calling_signature.is_none() {
            plan.rows[0].declared_requirement_identity = plan.rows[0].requirement_identity;
        }
        let [method] = declaration.methods.as_mut_slice() else {
            return Err(Diagnostic::error(
                "boundary leaf does not bind one exact requirement contract",
            ));
        };
        // An ABI-selected native instance still implements the original
        // method. A concrete realization without that instance keeps its
        // existing private callable projection instead.
        if plan.rows[0].calling_signature.is_none() {
            method.requirement_identity = declaration.identities.intern(&self.callable_identity);
        }
        plan.schema.calling_applications.clear();
        if let direct_boundary_bindings::DirectBoundaryExecution::Foreign(Ok(binding)) =
            &mut execution
        {
            binding.trait_name = self.callable_identity.clone();
            binding.requirement_identity = self.callable_identity.clone();
            binding.provider_type = self.realization_owner.clone();
        }
        let leaf = direct_boundary_bindings::DirectBoundaryLeaf {
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
    schemas: &[build_providers::plans::ServiceSchema],
    target: &str,
    requested: impl IntoIterator<Item = semantic_vocabulary::BoundaryMachineId>,
) -> Result<Vec<direct_boundary_bindings::DirectBoundaryLeaf>, Vec<Diagnostic>> {
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
    for boundary in &module.interfaces.boundary_machines {
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
                        native_intrinsics::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
                    }
                    _ => return Err(fail("hosted primitive has no native catalog projection")),
                };
                let canonical = module
                    .boundary_machine(requirement)
                    .ok_or_else(|| fail("hosted primitive lost its canonical requirement link"))?;
                (canonical, execution, None)
            }
            terminal_psi::CheckedBoundaryExecution::NumericPrimitive => {
                let meaning = boundary
                    .numeric_intrinsic
                    .ok_or_else(|| fail("numeric primitive lost its semantic meaning"))?;
                let entry = provider_admission::direct_numeric_target(module, boundary, target)?;
                (
                    boundary,
                    native_intrinsics::CompilerIntrinsicExecutionIdentity::from(meaning),
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
        let mut original = build_providers::plans::ProviderPlan {
            target: target.to_owned(),
            schema: schema.clone(),
            ..Default::default()
        };
        let coordinate = original.identities.intern(requirement);
        original.rows.push(build_providers::plans::ProviderPlanRow {
            method: method.name.clone(),
            requirement_identity: coordinate,
            declared_requirement_identity: coordinate,
            calling_signature: None,
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: build_providers::plans::ProviderBinding::CompilerIntrinsic {
                machine: text(declaration.source.semantic_declaration)?.to_owned(),
            },
        });
        let input = DirectBoundaryInput {
            declaration_identity: text(declaration.source.semantic_declaration)?.to_owned(),
            callable_identity: text(declaration.source.callable)?.to_owned(),
            realization_owner: text(declaration.source.machine.identity())?.to_owned(),
            // Receiving has already authenticated both declarations as exact
            // core members; the canonical schema retains that package custody.
            realization_package: original.schema.declaration.trait_package_identity,
        };
        let mut leaf = input
            .finish(
                &original,
                direct_boundary_bindings::DirectBoundaryExecution::Intrinsic(Some(execution)),
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

// The tests check that target preparation and native publication consult the
// same canonical table, and that a declaration no selected root reaches
// imposes no target obligation.
#[cfg(test)]
mod tests {
    use super::*;
    use build_providers::plans::ProviderPlanRow;
    use build_providers::plans::{ProviderBinding, ServiceMethod, ServiceSchema, ServiceSurface};
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
            declaration_identity: "primitive declaration".into(),
            callable_identity: "primitive callable(i32)->core::Never".into(),
            realization_owner: "core::ProcessExitPrimitive".into(),
            realization_package: None,
        };
        let leaf = input
            .finish(
                &original,
                direct_boundary_bindings::DirectBoundaryExecution::Intrinsic(Some(
                    native_intrinsics::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
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
