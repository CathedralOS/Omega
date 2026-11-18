//! Review canonical numeric calls through the target plans retained for this package. They are not authored Build
//! selections. The intrinsic execution retains their complete catalog meaning;
//! source custody and the full checked contract must agree before projection.

use super::declarations::unavailable;
use super::input::PolicySource;
use crate::record::{
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryRealization,
    PackagePolicyProviderBinding, PackagePolicyProviderPlan, PackagePolicyProviderRow,
    PackagePolicyServiceAuthority, PackagePolicyServiceSignature, PackageReviewBoundaryApplication,
    PackageReviewCompilerIntrinsicExecution, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewOperatorCoordinate, PackageReviewSignatureParameter,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use source_compiler::source_state::declarations::intrinsic_requirements::{
    IntrinsicRequirement, IntrinsicRequirementKind,
};
use target::TargetProfile;

pub(super) struct Projection {
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

/// Source declarations and source-owned signatures needed by the reached numeric methods.
pub(super) struct PreparedNumeric {
    target: TargetProfile,
    declarations: Vec<NumericDeclaration>,
    demanded: usize,
}

struct NumericDeclaration {
    boundary: BoundaryMachineId,
    dependency: crate::record::PackagePolicySemanticDependency,
    method: Option<NumericMethod>,
}

struct NumericMethod {
    identity: String,
    coordinate: PackageReviewOperatorCoordinate,
    signature: PackagePolicyServiceSignature,
}

pub(super) fn prepare(
    input: &PolicySource<'_>,
    target: TargetProfile,
    provider_bodies: impl IntoIterator<Item = MachineId>,
) -> Result<PreparedNumeric, Vec<Diagnostic>> {
    let accepted = input.accepted();
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
    // Bind the review roster back to the exact entry and provider bodies the
    // package owns. No provider choice or ABI plan is made during inspection.
    let demanded = if numeric.is_empty() {
        Vec::new()
    } else {
        accepted
            .retained_package_review_numeric_requirements(provider_bodies)
            .map_err(unavailable)?
    };
    let typed = input.typed();
    let mut declarations = Vec::with_capacity(numeric.len());
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
        if demanded.binary_search(&boundary.id).is_err() {
            declarations.push(NumericDeclaration {
                boundary: boundary.id,
                dependency,
                method: None,
            });
            continue;
        }
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
        declarations.push(NumericDeclaration {
            boundary: boundary.id,
            dependency,
            method: Some(NumericMethod {
                identity: identity.to_owned(),
                coordinate,
                signature: PackagePolicyServiceSignature {
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
            }),
        });
    }
    if declarations
        .iter()
        .filter(|row| row.method.is_some())
        .count()
        != demanded.len()
    {
        return Err(unavailable("one exact numeric target realization"));
    }
    Ok(PreparedNumeric {
        target,
        declarations,
        demanded: demanded.len(),
    })
}

impl PreparedNumeric {
    pub(super) fn boundaries(&self) -> impl Iterator<Item = BoundaryMachineId> + '_ {
        self.declarations.iter().map(|row| row.boundary)
    }

    pub(super) fn finish(
        &self,
        checked: &products::CheckedCompilation,
        target: TargetProfile,
    ) -> Result<Vec<Projection>, Vec<Diagnostic>> {
        if target != self.target || checked.execution().target_profile() != Some(target) {
            return Err(unavailable("the numeric declaration's target realization"));
        }
        let leaves: &[backend_plan::package_review::ReviewedNumericBinding] =
            match checked.execution().package_review() {
                Some(reviewed) => reviewed.numeric.as_ref().map_err(Clone::clone)?,
                None if self.demanded == 0 => &[],
                None => return Err(unavailable("the numeric declaration's target realization")),
            };
        if leaves.len() != self.demanded {
            return Err(unavailable("one exact numeric target realization"));
        }
        self.declarations
            .iter()
            .map(|row| {
                let realization = row
                    .method
                    .as_ref()
                    .map(|method| {
                        let index = leaves
                            .binary_search_by(|binding| {
                                binding.leaf().callable_identity.cmp(&method.identity)
                            })
                            .map_err(|_| {
                                unavailable("the numeric declaration's target realization")
                            })?;
                        let binding = &leaves[index];
                        if binding.boundary() != row.boundary
                            || binding.leaf().plan.target != target.target_name()
                        {
                            return Err(unavailable(
                                "the numeric declaration's target realization",
                            ));
                        }
                        method.realize(binding.leaf(), target)
                    })
                    .transpose()?;
                Ok(Projection {
                    dependency: row.dependency.clone(),
                    realization,
                })
            })
            .collect()
    }
}

impl NumericMethod {
    fn realize(
        &self,
        leaf: &backend_plan::DirectBoundaryLeaf,
        target: TargetProfile,
    ) -> Result<(PackagePolicyProviderPlan, Application), Vec<Diagnostic>> {
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
        let nominal = self.coordinate.identity.clone();
        let requirement_identity = PackageReviewNominalIdentity {
            owner: nominal.owner,
            path: self.identity.clone(),
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
                self.signature.clone(),
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
            coordinate: self.coordinate.clone(),
            requirement: self.identity.clone(),
            execution,
        };
        Ok((plan, application))
    }
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
