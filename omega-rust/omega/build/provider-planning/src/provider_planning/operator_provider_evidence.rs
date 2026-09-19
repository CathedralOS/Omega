//! Selected operator provider evidence and intrinsic realization checks.

pub(crate) use crate::exact_checked_adapter;
use crate::provider_planning::intrinsic_execution::{
    primitive_float_binary_intrinsic_execution_identity_for,
    primitive_integer_comparison_intrinsic_execution_identity_for,
};
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use typed_trees::TypedTrees;

pub(crate) fn plan_selected_operator_provider_evidence(
    checked: &checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<
    (
        Vec<(
            arena::Handle<checked_trees::CheckedOperatorUseFact>,
            u64,
            checked_trees::CheckedProviderPlanCommitment,
        )>,
        Vec<(
            arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
            u64,
            checked_trees::CheckedProviderPlanCommitment,
        )>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    // Validate selected operator plans independently of use-site discovery.
    // A malformed realization is invalid policy even when dead code happens
    // not to mention its requirement, and later annotation may consume only
    // plans that passed this gate.
    let mut diagnostics = Vec::new();
    for plan in selected.plans() {
        let operator = checked.typed.operators().iter().find(|operator| {
            crate::service_schema::schema_binds_exact_boundary_operator(
                &checked.typed,
                &plan.schema,
                operator,
            )
        });
        let Some(operator) = operator else {
            continue;
        };
        if let Err(diagnostic) = selected_operator_provider_evidence(
            checked,
            candidates,
            selected,
            operator.symbol,
            None,
        ) {
            diagnostics.push(diagnostic);
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let spelled = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, operator_use)| {
            (
                handle,
                operator_use.application_site(),
                operator_use.selected_operator_symbol,
            )
        })
        .collect::<Vec<_>>();
    let named = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| {
            (
                handle,
                operator_use.expression,
                operator_use.origin,
                operator_use.selected_operator_symbol,
            )
        })
        .collect::<Vec<_>>();
    let mut spelled_updates = Vec::new();
    for (handle, site, symbol) in spelled {
        match selected_operator_provider_evidence(checked, candidates, selected, symbol, Some(site))
        {
            Ok(Some((report_fingerprint, commitment))) => {
                spelled_updates.push((handle, report_fingerprint, commitment));
            }
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    let mut named_updates = Vec::new();
    for (handle, expression, origin, symbol) in named {
        match selected_operator_provider_evidence(
            checked,
            candidates,
            selected,
            symbol,
            Some(
                checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                    expression,
                    origin,
                },
            ),
        ) {
            Ok(Some((report_fingerprint, commitment))) => {
                named_updates.push((handle, report_fingerprint, commitment));
            }
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    if diagnostics.is_empty() {
        Ok((spelled_updates, named_updates))
    } else {
        Err(diagnostics)
    }
}

/// Stamp the selected plan onto each direct call to a public receiver-free
/// top-level requirement, the requirement-spelling twin of the named
/// operator-use stamping above. A checked-adapter plan is the direct-call
/// adapter route; a compiler-intrinsic plan must name a compiler-known
/// realization that satisfies exactly this requirement as an external leaf.
/// An evaluated `via` binding keeps the call on the requirement's retained
/// boundary seam: native realization joins the selected row's normalized
/// import or syscall to the demanded boundary by requirement identity. Only
/// mechanisms with a freestanding host-ABI route can serve a direct call —
/// vtable and table rows have no receiver table here.
pub(crate) fn plan_selected_requirement_provider_evidence(
    checked: &checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<
    Vec<(
        arena::Handle<checked_trees::CheckedNamedRequirementUseFact>,
        u64,
        checked_trees::CheckedProviderPlanCommitment,
    )>,
    Vec<diagnostics::Diagnostic>,
> {
    let mut updates = Vec::new();
    let mut diagnostics = Vec::new();
    let uses = checked
        .facts
        .operators
        .named_requirement_uses
        .iter()
        .map(|(handle, requirement_use)| (handle, requirement_use.requirement_symbol))
        .collect::<Vec<_>>();
    for (handle, symbol) in uses {
        match selected_requirement_provider_evidence(checked, candidates, selected, symbol) {
            Ok(Some((report_fingerprint, commitment))) => {
                updates.push((handle, report_fingerprint, commitment));
            }
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    if diagnostics.is_empty() {
        Ok(updates)
    } else {
        Err(diagnostics)
    }
}

fn selected_requirement_provider_evidence(
    checked: &checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<Option<(u64, checked_trees::CheckedProviderPlanCommitment)>, diagnostics::Diagnostic> {
    let Some(requirement) =
        crate::IntrinsicRequirement::by_symbol(&checked.typed, requirement_symbol)
    else {
        return Ok(None);
    };
    if requirement.kind != crate::IntrinsicRequirementKind::TopLevelRequirement {
        return Ok(None);
    }
    let slot = requirement.display();
    if !candidates
        .iter()
        .any(|candidate| requirement.schema_binds(&checked.typed, &candidate.schema))
    {
        return Ok(None);
    }
    let matching_selected = selected
        .plans()
        .iter()
        .filter(|plan| requirement.schema_binds(&checked.typed, &plan.schema))
        .collect::<Vec<_>>();
    let [plan] = matching_selected.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "boundary requirement `{slot}` in package {:?} resolves to {} selected ProviderPlans; expected exactly one",
            requirement.package_identity,
            matching_selected.len(),
        )));
    };
    let plan = *plan;
    let [row] = plan.rows.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-requirement ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        )));
    };
    match &row.binding {
        ProviderBinding::CheckedAdapter { .. } => {}
        ProviderBinding::CompilerIntrinsic { machine, .. } => {
            compiler_intrinsic_diagnostic_label_for(&checked.typed, &requirement).ok_or_else(
                || {
                    diagnostics::Diagnostic::error(format!(
                        "selected boundary-requirement ProviderPlan `{}` targets `{slot}`, which has no compiler-known migrated intrinsic",
                        plan.name,
                    ))
                },
            )?;
            if !requirement.intrinsic_realization_matches(&checked.typed, machine) {
                return Err(diagnostics::Diagnostic::error(format!(
                    "selected boundary-requirement ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact slot `{slot}` as an external leaf",
                    plan.name,
                )));
            }
        }
        ProviderBinding::Import { .. } | ProviderBinding::Syscall { .. } => {}
        binding => {
            return Err(diagnostics::Diagnostic::error(format!(
                "selected boundary-requirement ProviderPlan `{}` uses unsupported binding `{binding:?}` for a direct call; a directly called requirement takes a checked adapter, compiler intrinsic, or evaluated import/syscall binding",
                plan.name,
            )));
        }
    }
    Ok(Some((
        plan.report_fingerprint(),
        checked_trees::CheckedProviderPlanCommitment::from_digest(
            *plan.identity_digest().as_bytes(),
        ),
    )))
}

fn selected_operator_provider_evidence(
    checked: &checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
    operator_symbol: symbols::SymbolHandle,
    use_site: Option<checked_trees::CheckedBoundaryOperatorApplicationUseSite>,
) -> Result<Option<(u64, checked_trees::CheckedProviderPlanCommitment)>, diagnostics::Diagnostic> {
    let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.symbol == operator_symbol)
    else {
        return Ok(None);
    };
    let slot =
        typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    let operator_package = checked
        .typed
        .symbols
        .symbol_package_identity(operator.symbol);
    // Canonical overload identities are package-blind; every candidate and
    // selected-plan join must also bind the exact declaration package or a
    // same-spelled foreign coordinate would substitute for this slot.
    if !candidates.iter().any(|candidate| {
        candidate.schema.trait_name == slot
            && candidate.schema.trait_package_identity == operator_package
    }) {
        return Ok(None);
    }
    let matching_selected = selected
        .plans()
        .iter()
        .filter(|plan| {
            plan.schema.trait_name == slot && plan.schema.trait_package_identity == operator_package
        })
        .collect::<Vec<_>>();
    let [plan] = matching_selected.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "boundary operator `{slot}` in package {:?} resolves to {} selected ProviderPlans; expected exactly one",
            operator_package,
            matching_selected.len(),
        )));
    };
    let plan = *plan;
    let [row] = plan.rows.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        )));
    };
    if let ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = &row.binding
    {
        let [namespace, requirement] = checked.typed.operator_path_members(operator.name) else {
            return Err(diagnostics::Diagnostic::error(format!(
                "selected checked boundary-operator ProviderPlan `{}` targets `{slot}`, whose source path is not the supported `Namespace::requirement` shape",
                plan.name,
            )));
        };
        let demanded_application = match use_site {
            None => None,
            Some(site) => {
                let origin = match site {
                    checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                        origin,
                        ..
                    }
                    | checked_trees::CheckedBoundaryOperatorApplicationUseSite::MatchEquality {
                        origin,
                        ..
                    } => origin,
                    _ => {
                        return Err(diagnostics::Diagnostic::error(
                            "selected operator provider requires an exact value occurrence",
                        ));
                    }
                };
                if use_site_is_generic_template(checked, origin) {
                    // Generic templates are not executable provider evidence,
                    // even when one call happens not to mention a template
                    // binder. Concrete clones are annotated independently
                    // after final substitution. An emitted non-generic use
                    // still requires exactly one closed demand below.
                    return Ok(None);
                }
                let matching = checked
                    .facts
                    .operators
                    .boundary_applications
                    .iter()
                    .filter(|application| {
                        application.requirement_symbol == operator.symbol
                            && application.site == site
                    })
                    .collect::<Vec<_>>();
                let [application] = matching.as_slice() else {
                    return Err(diagnostics::Diagnostic::error(format!(
                        "selected checked boundary-operator use retains {} exact application demands; expected one",
                        matching.len(),
                    )));
                };
                Some(*application)
            }
        };
        let direct_provider = exact_checked_adapter(&checked.typed, plan, row);
        let specialized_providers = checked
            .typed
            .machine_specializations
            .iter()
            .filter(|specialization| {
                validation::machine_specialization_matches_template_identity(
                    &checked.typed,
                    specialization,
                    machine_identity,
                    *machine_package_identity,
                ) && specialization
                    .operator_realizations
                    .iter()
                    .any(|realization| {
                        realization.requirement_symbol == operator.symbol
                            && demanded_application.is_none_or(|application| {
                                validation::checked_operator_application_matches_realization(
                                    &checked.typed,
                                    application,
                                    realization,
                                )
                            })
                    })
            })
            .filter_map(|specialization| {
                checked
                    .typed
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == specialization.instance)
            })
            .filter(|machine| {
                checked
                    .typed
                    .symbols
                    .symbol_package_identity(machine.symbol)
                    == *machine_package_identity
            })
            .collect::<Vec<_>>();
        let requires_specialization =
            demanded_application.is_some_and(|application| !application.arguments.is_empty());
        let checked_providers = if requires_specialization {
            if specialized_providers.len() != 1 {
                return Err(diagnostics::Diagnostic::error(format!(
                    "selected checked boundary-operator ProviderPlan `{}` resolves to {} exact specializations for one demanded application",
                    plan.name,
                    specialized_providers.len(),
                )));
            }
            specialized_providers
        } else {
            match direct_provider {
                Ok(provider) => vec![provider],
                Err(direct_error) if specialized_providers.is_empty() => {
                    return Err(direct_error);
                }
                Err(_) => specialized_providers,
            }
        };
        let satisfies_slot = checked_providers.iter().all(|checked_provider| {
            checked
                .typed
                .machine_trait_conformances(checked_provider)
                .iter()
                .any(|conformance| {
                    conformance.external_binding.is_none()
                        && (typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
                            &checked.typed,
                            checked_provider,
                            conformance,
                        )
                        .is_some_and(|resolved| resolved.symbol == operator.symbol)
                            || typed_trees::operator::resolve_specialized_checked_operator_application(
                                &checked.typed,
                                checked_provider,
                                namespace.as_str(),
                                requirement.as_str(),
                            )
                            .is_some_and(|(resolved, _)| resolved.symbol == operator.symbol))
                })
        });
        if !satisfies_slot {
            return Err(diagnostics::Diagnostic::error(format!(
                "selected boundary-operator ProviderPlan `{}` binds checked adapter `{machine_identity}`, but that machine does not satisfy exact slot `{slot}` with a checked body",
                plan.name,
            )));
        }
        return Ok(Some((
            plan.report_fingerprint(),
            checked_trees::CheckedProviderPlanCommitment::from_digest(
                *plan.identity_digest().as_bytes(),
            ),
        )));
    }
    let ProviderBinding::CompilerIntrinsic { machine, .. } = &row.binding else {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` uses unsupported binding `{:?}`; boundary operators require a checked adapter or compiler intrinsic",
            plan.name, row.binding,
        )));
    };
    compiler_intrinsic_diagnostic_label(&checked.typed, operator).ok_or_else(|| {
        diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` targets `{slot}`, which has no compiler-known migrated intrinsic",
            plan.name,
        ))
    })?;
    if !intrinsic_realization_matches_operator(&checked.typed, machine, operator) {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact slot `{slot}` as an external leaf",
            plan.name,
        )));
    }
    Ok(Some((
        plan.report_fingerprint(),
        checked_trees::CheckedProviderPlanCommitment::from_digest(
            *plan.identity_digest().as_bytes(),
        ),
    )))
}

fn use_site_is_generic_template(
    checked: &checked_trees::CheckedTrees,
    origin: checked_trees::CheckedValueOrigin,
) -> bool {
    let checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. } = origin else {
        return false;
    };
    checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .is_some_and(|machine| !checked.typed.machine_type_parameters(machine).is_empty())
}

pub fn intrinsic_realization_matches_operator(
    typed: &TypedTrees,
    realization_machine_identity: &str,
    operator: &typed_trees::operator::OperatorDefinition,
) -> bool {
    crate::IntrinsicRequirement::from_operator(typed, operator).is_some_and(|requirement| {
        requirement.intrinsic_realization_matches(typed, realization_machine_identity)
    })
}

/// Render the compiler-known float realization selected by an exact checked
/// operator. This label is diagnostic-only: provider identity and dispatch use
/// the normalized realization-machine symbol retained in `ProviderBinding`.
pub fn compiler_intrinsic_diagnostic_label(
    typed: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<String> {
    let requirement = crate::IntrinsicRequirement::from_operator(typed, operator)?;
    compiler_intrinsic_diagnostic_label_for(typed, &requirement)
}

/// The same label keyed on the requirement view, so a `boundary requirement`
/// spelling of the same `Owner::name` signature renders identically.
pub fn compiler_intrinsic_diagnostic_label_for(
    typed: &TypedTrees,
    requirement: &crate::IntrinsicRequirement<'_>,
) -> Option<String> {
    if let Some(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format }) =
        primitive_float_binary_intrinsic_execution_identity_for(typed, requirement)
    {
        return Some(format!("Float::{}.{}", operation.name(), format.name()));
    }
    if let Some(CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
        operation,
        integer_type,
    }) = primitive_integer_comparison_intrinsic_execution_identity_for(typed, requirement)
    {
        // No canonical requirement namespace exists for integer comparisons;
        // the authored token is the operation identity, so the diagnostic
        // label names the closed comparison shape directly.
        return Some(format!(
            "integer-comparison.{}.{}",
            operation.name(),
            integer_type.name()
        ));
    }
    let namespace = requirement.namespace.as_str();
    let requirement_name = requirement.name.as_str();
    let return_type = requirement.return_type;
    let parameters = requirement.parameters;
    let (operation, primitive, expected_result) = match namespace {
        "F32" | "F64" => {
            if matches!(requirement_name, "from_f64" | "from_f32") {
                let (expected_source, expected_result, source_name) =
                    match (namespace, requirement_name) {
                        ("F32", "from_f64") => (
                            typed_trees::types::PrimitiveType::F64,
                            typed_trees::types::PrimitiveType::F32,
                            "f64",
                        ),
                        ("F64", "from_f32") => (
                            typed_trees::types::PrimitiveType::F32,
                            typed_trees::types::PrimitiveType::F64,
                            "f32",
                        ),
                        _ => return None,
                    };
                let [value] = parameters else {
                    return None;
                };
                if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                    || typed.primitive_type_reference(return_type) != Some(expected_result)
                {
                    return None;
                }
                return Some(format!("{}::{}.{source_name}", namespace, requirement_name));
            }
            if let Some(source_name) = requirement_name.strip_prefix("from_") {
                let expected_source = match source_name {
                    "i8" => typed_trees::types::PrimitiveType::I8,
                    "i16" => typed_trees::types::PrimitiveType::I16,
                    "i32" => typed_trees::types::PrimitiveType::I32,
                    "i64" => typed_trees::types::PrimitiveType::I64,
                    "u8" => typed_trees::types::PrimitiveType::U8,
                    "u16" => typed_trees::types::PrimitiveType::U16,
                    "u32" => typed_trees::types::PrimitiveType::U32,
                    "u64" => typed_trees::types::PrimitiveType::U64,
                    _ => return None,
                };
                let expected_result = if namespace == "F32" {
                    typed_trees::types::PrimitiveType::F32
                } else {
                    typed_trees::types::PrimitiveType::F64
                };
                let [value] = parameters else {
                    return None;
                };
                if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                    || typed.primitive_type_reference(return_type) != Some(expected_result)
                {
                    return None;
                }
                return Some(format!("{}::{}.{source_name}", namespace, requirement_name));
            }
            let operation = match requirement_name {
                "minimum" | "maximum" => requirement_name,
                "negate"
                | "square_root"
                | "square_root_toward_zero"
                | "square_root_toward_positive"
                | "square_root_toward_negative"
                | "classify"
                | "is_nan"
                | "is_finite"
                | "is_infinite"
                | "is_normal"
                | "is_subnormal" => requirement_name,
                "multiply_then_add"
                | "fused_multiply_add"
                | "fused_multiply_add_toward_zero"
                | "fused_multiply_add_toward_positive"
                | "fused_multiply_add_toward_negative" => requirement_name,
                "add_toward_zero" | "add_toward_positive" | "add_toward_negative" => {
                    requirement_name
                }
                "subtract_toward_zero"
                | "subtract_toward_positive"
                | "subtract_toward_negative" => requirement_name,
                "multiply_toward_zero"
                | "multiply_toward_positive"
                | "multiply_toward_negative" => requirement_name,
                "divide_toward_zero" | "divide_toward_positive" | "divide_toward_negative" => {
                    requirement_name
                }
                _ => return None,
            };
            let expected_primitive = if namespace == "F32" {
                typed_trees::types::PrimitiveType::F32
            } else {
                typed_trees::types::PrimitiveType::F64
            };
            match parameters {
                [value]
                    if matches!(
                        operation,
                        "negate"
                            | "square_root"
                            | "square_root_toward_zero"
                            | "square_root_toward_positive"
                            | "square_root_toward_negative"
                            | "classify"
                            | "is_nan"
                            | "is_finite"
                            | "is_infinite"
                            | "is_normal"
                            | "is_subnormal"
                    ) =>
                {
                    if typed.primitive_type_reference(value.type_reference)
                        != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                [left, right]
                    if matches!(
                        operation,
                        "minimum"
                            | "maximum"
                            | "add_toward_zero"
                            | "add_toward_positive"
                            | "add_toward_negative"
                            | "subtract_toward_zero"
                            | "subtract_toward_positive"
                            | "subtract_toward_negative"
                            | "multiply_toward_zero"
                            | "multiply_toward_positive"
                            | "multiply_toward_negative"
                            | "divide_toward_zero"
                            | "divide_toward_positive"
                            | "divide_toward_negative"
                    ) =>
                {
                    if typed.primitive_type_reference(left.type_reference)
                        != Some(expected_primitive)
                        || typed.primitive_type_reference(right.type_reference)
                            != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                [left, right, addend]
                    if matches!(
                        operation,
                        "multiply_then_add"
                            | "fused_multiply_add"
                            | "fused_multiply_add_toward_zero"
                            | "fused_multiply_add_toward_positive"
                            | "fused_multiply_add_toward_negative"
                    ) =>
                {
                    if typed.primitive_type_reference(left.type_reference)
                        != Some(expected_primitive)
                        || typed.primitive_type_reference(right.type_reference)
                            != Some(expected_primitive)
                        || typed.primitive_type_reference(addend.type_reference)
                            != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                _ => return None,
            }
            if operation == "classify" {
                if typed.display_type_reference(return_type) != "FloatClass" {
                    return None;
                }
                let format = if expected_primitive == typed_trees::types::PrimitiveType::F32 {
                    "f32"
                } else {
                    "f64"
                };
                return Some(format!("{}::classify.{format}", namespace));
            }
            let expected_result = if matches!(
                operation,
                "is_nan" | "is_finite" | "is_infinite" | "is_normal" | "is_subnormal"
            ) {
                typed_trees::types::PrimitiveType::Bool
            } else {
                expected_primitive
            };
            (operation, expected_primitive, expected_result)
        }
        "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64" => {
            let expected_result = match namespace {
                "I8" => typed_trees::types::PrimitiveType::I8,
                "I16" => typed_trees::types::PrimitiveType::I16,
                "I32" => typed_trees::types::PrimitiveType::I32,
                "I64" => typed_trees::types::PrimitiveType::I64,
                "U8" => typed_trees::types::PrimitiveType::U8,
                "U16" => typed_trees::types::PrimitiveType::U16,
                "U32" => typed_trees::types::PrimitiveType::U32,
                "U64" => typed_trees::types::PrimitiveType::U64,
                _ => unreachable!(),
            };
            let (expected_source, source_name) = match requirement_name {
                "from_f32" => (typed_trees::types::PrimitiveType::F32, "f32"),
                "from_f64" => (typed_trees::types::PrimitiveType::F64, "f64"),
                _ => return None,
            };
            let [value] = parameters else {
                return None;
            };
            if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                || typed.primitive_type_reference(return_type) != Some(expected_result)
            {
                return None;
            }
            let policy = match typed.type_reference_table.arithmetic_domain(return_type) {
                numerics::arithmetic::ArithmeticDomain::Exact => "exact",
                numerics::arithmetic::ArithmeticDomain::Trapping => "trapping",
                numerics::arithmetic::ArithmeticDomain::Saturating => "saturating",
                numerics::arithmetic::ArithmeticDomain::Wrapping => return None,
            };
            return Some(format!(
                "{}::{}.{source_name}.{policy}",
                namespace, requirement_name
            ));
        }
        _ => return None,
    };
    if typed.primitive_type_reference(return_type) != Some(expected_result) {
        return None;
    }
    let format = match primitive {
        typed_trees::types::PrimitiveType::F32 => "f32",
        typed_trees::types::PrimitiveType::F64 => "f64",
        _ => return None,
    };
    Some(format!("{}::{operation}.{format}", namespace))
}

pub(crate) fn provider_type_package_identity(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<semantic_vocabulary::PackageKeyIdentity> {
    provider_type_symbol(typed, machine)
        .and_then(|symbol| typed.symbols.symbol_package_identity(symbol))
}

pub(crate) fn provider_type_symbol(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<symbols::SymbolHandle> {
    let attached_data = machine.attached_data.as_ref()?;
    let mut owners = typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == *attached_data);
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner.symbol)
}
