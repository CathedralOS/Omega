//! Review canonical numeric calls through the same declaration and target
//! realization checks as native production. They are not authored Build
//! selections. The intrinsic execution retains their complete catalog meaning;
//! source custody and the full checked contract must agree before projection.

use super::PackageReviewInput;
use super::declarations::unavailable;
use crate::record::{
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryRealization,
    PackagePolicyProviderBinding, PackagePolicyProviderPlan, PackagePolicyProviderRow,
    PackagePolicyServiceAuthority, PackagePolicyServiceSignature, PackageReviewBoundaryApplication,
    PackageReviewCompilerIntrinsicExecution, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewOperatorCoordinate, PackageReviewSignatureParameter,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, MachineId, TargetMachineCallable};
use target::TargetProfile;
use typed_trees::declarations::intrinsic_requirements::{
    IntrinsicRequirement, IntrinsicRequirementKind,
};

pub(super) struct Projection {
    pub boundary: BoundaryMachineId,
    pub dependency: crate::record::PackagePolicySemanticDependency,
    pub realization: Option<(PackagePolicyProviderPlan, Application)>,
}

pub(super) struct Application {
    plan_name: String,
    target: TargetProfile,
    coordinate: PackageReviewOperatorCoordinate,
    requirement: String,
    execution: PackageReviewCompilerIntrinsicExecution,
}

impl Application {
    pub(super) fn finish(
        self,
        plans: &[PackagePolicyProviderPlan],
    ) -> Result<PackagePolicyBoundaryApplicationRealization, Vec<Diagnostic>> {
        let begin = plans.partition_point(|plan| plan.plan_name < self.plan_name);
        let end = plans.partition_point(|plan| plan.plan_name <= self.plan_name);
        let mut matching = plans[begin..end].iter().enumerate().filter(|(_, plan)| {
            plan.realizing_package.is_none()
                && plan.provider_type_declaration.is_none()
                && plan.schema_declaration == self.coordinate.identity
                && plan.target == self.target.target_name()
        });
        let (Some((offset, _)), None) = (matching.next(), matching.next()) else {
            return Err(unavailable("one exact numeric application target plan"));
        };
        let index = begin + offset;
        Ok(PackagePolicyBoundaryApplicationRealization {
            operator_coordinate: self.coordinate,
            requirement_identity: self.requirement,
            application: PackageReviewBoundaryApplication::Empty,
            selected_plan_index: u32::try_from(index)
                .map_err(|_| unavailable("a numeric target plan index within u32"))?,
            realization: PackagePolicyBoundaryRealization::ExactCompilerIntrinsic {
                execution: self.execution,
            },
        })
    }
}

pub(super) fn project(
    input: &PackageReviewInput<'_>,
    accepted: &source_compiler::AcceptedTerminalSource,
    target: TargetProfile,
    roots: impl IntoIterator<Item = MachineId>,
) -> Result<Vec<Projection>, Vec<Diagnostic>> {
    let module = accepted.module();
    let numeric = module
        .boundary_machines
        .iter()
        .filter(|boundary| {
            boundary.declaration.as_ref().is_some_and(|declaration| {
                declaration.execution == terminal_psi::CheckedBoundaryExecution::NumericPrimitive
            })
        })
        .collect::<Vec<_>>();
    if numeric.is_empty() {
        return Ok(Vec::new());
    }
    // Declaration checking does not deploy a reusable library's unused bodies.
    // The existing call closure includes normal and tail calls. If an indirect
    // route prevents an exact closure, retain every numeric demand conservatively.
    let roots = roots.into_iter().collect::<Vec<_>>();
    let reached = if roots.is_empty() {
        Some(Vec::new())
    } else {
        accepted.closure().elaboration.call_closure(roots)
    };
    let demanded = reached
        .map(|machines| {
            let mut boundaries = Vec::new();
            for machine in machines {
                let tables = accepted
                    .closure()
                    .elaboration
                    .tables
                    .machines
                    .get(machine)
                    .ok_or_else(|| unavailable("the reached machine's checked call inventory"))?;
                for call in tables.invocations.values() {
                    if let terminal_check_tables::edges::Callee::Declared(
                        TargetMachineCallable::Boundary(boundary),
                    ) = call.callee
                    {
                        boundaries.push(boundary);
                    }
                }
            }
            boundaries.sort_unstable();
            boundaries.dedup();
            Ok::<_, Vec<Diagnostic>>(boundaries)
        })
        .transpose()?;
    let schemas = provider_planning::compiler_primitive_schemas(module)
        .map_err(|reason| unavailable(&reason))?;
    let mut leaves = provider_planning::direct_compiler_primitive_leaves(
        module,
        &schemas,
        target.target_name(),
        numeric
            .iter()
            .filter(|boundary| {
                demanded
                    .as_ref()
                    .is_none_or(|demanded| demanded.binary_search(&boundary.id).is_ok())
            })
            .map(|boundary| boundary.id),
    )?;
    leaves.sort_unstable_by(|left, right| left.callable_identity.cmp(&right.callable_identity));
    if leaves
        .windows(2)
        .any(|pair| pair[0].callable_identity == pair[1].callable_identity)
    {
        return Err(unavailable("one exact numeric target realization"));
    }
    let typed = input.typed();
    let mut projected = Vec::with_capacity(numeric.len());
    for boundary in numeric {
        let original = *accepted
            .closure()
            .elaboration
            .boundaries
            .get(boundary.id)
            .ok_or_else(|| unavailable("the numeric requirement's original declaration"))?;
        // Reuse the prepared declaration index rather than scanning every
        // source machine again for each numeric boundary.
        let requirement = typed
            .operator_definition_by_symbol(original)
            .and_then(|operator| IntrinsicRequirement::from_operator(typed, operator))
            .or_else(|| {
                typed
                    .machine_by_own_or_state_symbol(original)
                    .filter(|machine| {
                        machine.symbol == original
                            || typed
                                .machine_states(machine)
                                .first()
                                .is_some_and(|entry| entry.symbol == original)
                    })
                    .and_then(|machine| IntrinsicRequirement::from_requirement(typed, machine))
            })
            .ok_or_else(|| unavailable("the numeric requirement's complete source signature"))?;
        let nominal = input.nominal(requirement.symbol)?;
        if !matches!(nominal.owner, PackageReviewNominalOwner::ToolchainSource(_)) {
            return Err(unavailable(
                "the numeric requirement's exact toolchain owner",
            ));
        }
        if typed.symbols.sealed_intrinsic(requirement.symbol)
            != Some(language_core::sealed_intrinsics::SealedIntrinsic::FloatOperationRequirement)
            || typed.numeric_intrinsic_execution(requirement.symbol) != boundary.numeric_intrinsic
        {
            return Err(unavailable(
                "the numeric declaration's exact catalog meaning",
            ));
        }
        let callable = match requirement.kind {
            IntrinsicRequirementKind::TopLevelRequirement => typed
                .machine_by_symbol(requirement.symbol)
                .map(|machine| typed.machine_callable_identity(machine)),
            IntrinsicRequirementKind::Operator => requirement
                .as_operator()
                .map(|operator| typed.boundary_operator_requirement_callable_identity(operator)),
        }
        .filter(|identity| identity.is_valid())
        .ok_or_else(|| unavailable("the numeric requirement's exact callable identity"))?;
        let coordinate = PackageReviewOperatorCoordinate {
            identity: nominal.clone(),
            parameter_dispatch: typed
                .callable_parameters_text(typed.callable_identity_parameters(callable)),
            result_dispatch: typed
                .callable_result_dispatch_text(typed.callable_identity_result_dispatch(callable)),
        };
        let identity = module
            .identities
            .text(boundary.identity)
            .ok_or_else(|| unavailable("the accepted numeric callable coordinate"))?;
        if !coordinate.matches_policy_requirement(identity) {
            return Err(unavailable(
                "the numeric declaration's exact accepted application",
            ));
        }
        let dependency = crate::record::PackagePolicySemanticDependency {
            consumer: crate::record::PackagePolicySemanticDependencyConsumer::PackageImplementation,
            dependency: nominal.clone(),
            exposure: crate::record::PackageReviewSemanticDependencyExposure::PrivateImplementation,
            kind: crate::record::PackageReviewSemanticDependencyKind::NominalIdentity,
        };
        if demanded
            .as_ref()
            .is_some_and(|demanded| demanded.binary_search(&boundary.id).is_err())
        {
            projected.push(Projection {
                boundary: boundary.id,
                dependency,
                realization: None,
            });
            continue;
        }
        let index = leaves
            .binary_search_by(|leaf| leaf.callable_identity.as_str().cmp(identity))
            .map_err(|_| unavailable("the numeric declaration's target realization"))?;
        let leaf = &leaves[index];
        leaf.validate_bound_calling_custody(module)
            .map_err(|reason| unavailable(&reason))?;
        let backend_plan::DirectBoundaryExecution::Intrinsic(Some(execution)) = leaf.execution
        else {
            return Err(unavailable(
                "the numeric declaration's closed intrinsic execution",
            ));
        };
        let execution = numeric_execution(execution)?;
        let [method] = leaf.plan.schema.declaration.methods.as_slice() else {
            return Err(unavailable("one complete numeric method schema"));
        };
        let [row] = leaf.plan.rows.as_slice() else {
            return Err(unavailable("one complete numeric realization row"));
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
        else {
            return Err(unavailable("the numeric method's intrinsic binding"));
        };
        let parameters = requirement
            .parameters
            .iter()
            .map(|parameter| {
                Ok(PackageReviewSignatureParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: input
                        .type_identity_for_private_contract(parameter.type_reference)?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let result = input.type_identity_for_private_contract(requirement.return_type)?;
        let requirement_identity = PackageReviewNominalIdentity {
            owner: nominal.owner,
            path: identity.to_owned(),
        };
        let plan = PackagePolicyProviderPlan {
            plan_name: leaf.plan.name.clone(),
            realizing_package: None,
            schema_declaration: nominal.clone(),
            provider_type: leaf.plan.provider_type.clone(),
            provider_type_declaration: None,
            target: target.target_name().to_owned(),
            methods: vec![super::providers::service_method_policy(
                method,
                &leaf.plan.schema,
                requirement_identity.clone(),
                nominal,
                PackagePolicyServiceSignature {
                    schema_arguments: Vec::new(),
                    schema_lifetime_parameter_count: 0,
                    schema_lifetime_arguments: Vec::new(),
                    application_lifetime_parameter_count: 0,
                    requirement_arguments: Vec::new(),
                    requirement_lifetime_arguments: Vec::new(),
                    requirement_lifetime_parameter_count: 0,
                    static_parameters: Vec::new(),
                    parameters,
                    result: Some(result),
                },
                PackagePolicyServiceAuthority {
                    service_reach: Vec::new(),
                    synchronous_invocations: Vec::new(),
                    progress_premises: Vec::new(),
                },
            )],
            rows: vec![PackagePolicyProviderRow {
                method: row.method.clone(),
                requirement: requirement_identity,
                requirement_application: None,
                realization: None,
                native_build: None,
                requirement_lifetime_partition: row.requirement_lifetime_partition.clone(),
                admitted_same_stack_bytes: row.admitted_same_stack_bytes,
                binding: PackagePolicyProviderBinding::CompilerIntrinsic {
                    machine: machine.clone(),
                },
                compiler_intrinsic_execution: Some(execution),
                installation_reach: None,
            }],
            grants: Vec::new(),
        };
        let application = Application {
            plan_name: plan.plan_name.clone(),
            target,
            coordinate,
            requirement: identity.to_owned(),
            execution,
        };
        projected.push(Projection {
            boundary: boundary.id,
            dependency,
            realization: Some((plan, application)),
        });
    }
    Ok(projected)
}

fn numeric_execution(
    execution: effects::CompilerIntrinsicExecutionIdentity,
) -> Result<PackageReviewCompilerIntrinsicExecution, Vec<Diagnostic>> {
    use PackageReviewCompilerIntrinsicExecution as Policy;
    use effects::CompilerIntrinsicExecutionIdentity as Source;
    Ok(match execution {
        Source::PrimitiveFloatBinary { operation, format } => {
            Policy::PrimitiveFloatBinary { operation, format }
        }
        Source::NamedFloatNegation(format) => Policy::NamedFloatNegation(format),
        Source::NamedFloatConversion {
            source,
            target,
            domain,
        } => Policy::NamedFloatConversion {
            source,
            target,
            domain,
        },
        Source::BuiltinFunction(function) => Policy::BuiltinFunction(function),
        _ => return Err(unavailable("a supported numeric intrinsic policy")),
    })
}
