//! Complete provider tuples derived from exact authored trait applications.
//!
//! The source edge owns the contract application. Static proposal inference
//! only discovers candidate operands; complete applied identity must still
//! agree before a tuple is retained. The remaining provider telescope uses the
//! same exact-scope invocation classification as source conformance checking.
//! No runtime parameter shape, body result or provider spelling supplies a row.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::machine::{Machine, TraitConformance};
use typed_trees::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
    StaticMachineLifetimeBinding,
};
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, TypeIdentityQualification,
    applied_static_parameter_identity, applied_type_identity,
};
use typed_trees::types::TypeReferenceHandle;

pub(super) struct DeclaredTraitProviderApplication {
    pub(super) bindings: Vec<StaticMachineBinding>,
    pub(super) lifetimes: Vec<StaticMachineLifetimeBinding>,
}

pub(super) fn provider_application(
    source: &mut TypedTrees,
    provider: &Machine,
    edge: &TraitConformance,
    named_conformance: arena::Handle<typed_trees::trait_definition::Conformance>,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    application: StaticMachineApplicationHandle,
    required_bindings: &[StaticMachineBinding],
) -> Result<Option<DeclaredTraitProviderApplication>, Diagnostic> {
    // The source map and machine satisfaction edge own distinct lifetime
    // telescopes. Freeze once before reading either original applied scope.
    let original = source.clone();
    let root_scope = if named_conformance.is_valid() {
        let declaration = original.conformances.get(named_conformance);
        if declaration.trait_symbol != edge.symbol {
            return Ok(None);
        }
        AppliedTraitScope::from_conformance(&original, declaration)
    } else {
        AppliedTraitScope::from_machine_conformance(&original, provider, edge)
    };
    let Some(root_scope) = root_scope else {
        return Ok(None);
    };
    let Some(region_ordinals) = root_scope.declaring_lifetime_ordinals(&original, declaring_trait)
    else {
        return Err(Diagnostic::error(
            "declared provider lost its complete inherited region application",
        ));
    };
    let Some(scope) = root_scope.declaring_scope(&original, declaring_trait) else {
        return Ok(None);
    };
    if edge.requirement_symbol != requirement {
        return Ok(None);
    }
    let signature = source
        .trait_machine_signature_by_symbol(scope.declaration, requirement)
        .ok_or_else(|| Diagnostic::error("declared provider edge has no original requirement"))?;
    let required_parameters = source.state_signature_type_parameters(signature).to_vec();
    let invocation_regions = signature.lifetime_parameters.clone();
    let invocation =
        validation::requirement_invocation_parameters(source, provider, provider, &scope);
    if invocation.len() != required_parameters.len()
        || invocation
            .iter()
            .zip(&required_parameters)
            .any(|(actual, required)| {
                std::mem::discriminant(&actual.kind) != std::mem::discriminant(&required.kind)
            })
    {
        return Err(Diagnostic::error(
            "declared trait provider changes the requirement's universal static telescope",
        ));
    }
    let trait_parameters = source.trait_type_parameters(scope.declaration).to_vec();
    let mut arguments = Vec::new();
    // Original scopes index an immutable snapshot. Project their complete graph
    // before proposal inference, retaining inherited compound substitutions.
    for parameter in &trait_parameters {
        let reference = scope.parameter_argument(&original, parameter.symbol);
        if !reference.is_valid() {
            return Err(Diagnostic::error(
                "declared trait provider lost a scoped application operand",
            ));
        }
        let expected = required_bindings
            .iter()
            .find(|row| row.parameter == parameter.symbol)
            .ok_or_else(|| {
                Diagnostic::error("required application lost a declared trait binder")
            })?;
        arguments.push((parameter.clone(), reference, expected.value.clone()));
    }
    let parameters = source.machine_type_parameters(provider).to_vec();
    let type_parameters = parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Type
                    | TypeParameterKind::Machine { .. }
                    | TypeParameterKind::Implementation
            )
        })
        .map(|parameter| (parameter.symbol, parameter.name.as_str().to_owned()))
        .collect::<Vec<_>>();
    let const_parameters = parameters
        .iter()
        .filter_map(|parameter| match &parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => Some((
                parameter.symbol,
                parameter.name.as_str().to_owned(),
                *type_reference,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut type_proposals = Vec::new();
    let mut const_proposals = Vec::new();
    let mut projected = Vec::new();
    for (parameter, required, expected) in arguments {
        let required = super::signature_cloning::copy_scoped_type_reference(
            &original, source, required, &scope,
        )?;
        let actual = binding_reference(source, &expected)?;
        projected.push((parameter, required, actual));
    }
    // Reuse ordinary argument materialization after relocating the complete
    // graphs. Literal array extents and domain endpoints are existing typed
    // operands, and inference needs the same retained atom on every entrance.
    super::materialize_static_argument_types(source);
    for (_, required, actual) in &projected {
        super::selection::infer_static_bindings(
            source,
            *required,
            *actual,
            &type_parameters,
            &const_parameters,
            None,
            0,
            &mut type_proposals,
            &mut const_proposals,
        );
    }
    let mut bindings = Vec::new();
    for (index, (symbol, _)) in type_parameters.iter().enumerate() {
        let proposals = type_proposals
            .iter()
            .filter(|(_, ordinal, _)| *ordinal == index)
            .map(|(_, _, actual)| *actual)
            .collect::<Vec<_>>();
        if let Some(actual) = super::selection::coherent_type_proposal(source, &proposals) {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == *symbol)
                .expect("type proposal has its original parameter");
            let value = match parameter.kind {
                TypeParameterKind::Type => StaticMachineBindingValue::Type(actual),
                TypeParameterKind::Machine { .. } => StaticMachineBindingValue::Machine(
                    super::application_arguments::static_argument_from_type_reference(
                        source, actual,
                    )
                    .ok_or_else(|| {
                        Diagnostic::error("selected machine operand lost its typed application")
                    })?,
                ),
                TypeParameterKind::Implementation => StaticMachineBindingValue::Conformance(
                    super::application_arguments::static_argument_from_type_reference(
                        source, actual,
                    )
                    .ok_or_else(|| {
                        Diagnostic::error("selected conformance operand lost its typed application")
                    })?,
                ),
                _ => unreachable!("classified proposal parameter"),
            };
            bindings.push(StaticMachineBinding {
                parameter: *symbol,
                value,
            });
        } else if !proposals.is_empty() {
            return Ok(None);
        }
    }
    for (index, (symbol, _, _)) in const_parameters.iter().enumerate() {
        let proposals = const_proposals
            .iter()
            .filter(|(_, ordinal, _)| *ordinal == index)
            .map(|(_, _, actual)| *actual)
            .collect::<Vec<_>>();
        if let Some(actual) = super::selection::coherent_type_proposal(source, &proposals) {
            bindings.push(StaticMachineBinding {
                parameter: *symbol,
                value: StaticMachineBindingValue::Const(actual),
            });
        } else if !proposals.is_empty() {
            return Ok(None);
        }
    }
    for (actual, required) in invocation.iter().zip(&required_parameters) {
        let selected = required_bindings
            .iter()
            .find(|row| row.parameter == required.symbol)
            .ok_or_else(|| Diagnostic::error("required application lost an invocation binder"))?;
        if bindings.iter().any(|row| row.parameter == actual.symbol) {
            return Err(Diagnostic::error(
                "declared trait edge also captures a fresh invocation binder",
            ));
        }
        bindings.push(StaticMachineBinding {
            parameter: actual.symbol,
            value: selected.value.clone(),
        });
    }
    if parameters
        .iter()
        .any(|parameter| !bindings.iter().any(|row| row.parameter == parameter.symbol))
    {
        // A requirement tuple cannot choose an independent source-edge slot.
        // Retain absence instead of inventing a provider actual for that slot.
        return Ok(None);
    }
    let substitutions = identity_operands(&bindings);
    let lifetimes = source
        .expression_table
        .static_machine_application_lifetimes(application);
    let mut regions = Vec::new();
    let mut provider_lifetimes: Vec<StaticMachineLifetimeBinding> = Vec::new();
    for (parameter, root_ordinal) in scope
        .declaration
        .lifetime_parameters
        .iter()
        .zip(&region_ordinals)
    {
        let ordinal = usize::try_from(*root_ordinal)
            .ok()
            .and_then(|ordinal| edge.trait_lifetime_arguments.get(ordinal))
            .ok_or_else(|| {
                Diagnostic::error("declared provider lost an inherited captured region slot")
            })?;
        let caller = lifetimes
            .iter()
            .find(|binding| &binding.parameter == parameter)
            .ok_or_else(|| {
                Diagnostic::error("required application lost a captured trait region")
            })?;
        if caller.argument.named().is_none() {
            return Err(Diagnostic::error(
                "captured provider region has no named source correspondence",
            ));
        }
        let provider_region = usize::try_from(*ordinal)
            .ok()
            .and_then(|ordinal| provider.lifetime_parameters.get(ordinal))
            .ok_or_else(|| {
                Diagnostic::error("declared provider maps a region outside its original telescope")
            })?;
        if let Some(known) = provider_lifetimes
            .iter()
            .find(|row| &row.parameter == provider_region)
        {
            if known.argument != caller.argument {
                return Ok(None);
            }
        } else {
            regions.push((
                provider_region.as_str().to_owned(),
                caller.argument.scope_argument(),
            ));
            provider_lifetimes.push(StaticMachineLifetimeBinding {
                parameter: provider_region.clone(),
                argument: caller.argument.clone(),
            });
        }
    }
    for ordinal in &edge.trait_lifetime_arguments {
        let region = usize::try_from(*ordinal)
            .ok()
            .and_then(|ordinal| provider.lifetime_parameters.get(ordinal))
            .ok_or_else(|| {
                Diagnostic::error("declared provider region lies outside its original family")
            })?;
        if !provider_lifetimes
            .iter()
            .any(|row| &row.parameter == region)
        {
            return Ok(None);
        }
    }
    let remaining_regions = provider
        .lifetime_parameters
        .iter()
        .enumerate()
        .filter(|(ordinal, _)| {
            !edge
                .trait_lifetime_arguments
                .iter()
                .any(|captured| usize::try_from(*captured).ok() == Some(*ordinal))
        })
        .map(|(_, region)| region)
        .collect::<Vec<_>>();
    if std::env::var_os("OMEGA_MAIN21_SCOPE_TRACE").is_some() {
        eprintln!(
            "main21-provider-regions provider={} named={named_conformance:?} requirement={requirement:?} original={:?} captured={provider_lifetimes:?} remaining={remaining_regions:?} invocation={invocation_regions:?} caller={lifetimes:?}",
            provider.name, provider.lifetime_parameters
        );
    }
    if remaining_regions.len() != invocation_regions.len() {
        return Err(Diagnostic::error(
            "declared trait provider changes the universal invocation lifetime family",
        ));
    }
    for (required, provider_region) in invocation_regions.iter().zip(remaining_regions) {
        if let Some(caller) = lifetimes
            .iter()
            .find(|binding| &binding.parameter == required)
        {
            regions.push((
                provider_region.as_str().to_owned(),
                caller.argument.scope_argument(),
            ));
            provider_lifetimes.push(StaticMachineLifetimeBinding {
                parameter: provider_region.clone(),
                argument: caller.argument.clone(),
            });
        }
    }
    let lexical = source
        .expression_table
        .static_machine_application(application)
        .type_identity_owner(source)
        .ok_or_else(|| {
            Diagnostic::error("required application lost its original lexical region owner")
        })?;
    let applied = AppliedTypeIdentityScope {
        region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
        declaration: provider.symbol,
        arguments: &substitutions,
        lifetimes: &regions,
    };
    let required_operands = identity_operands(required_bindings);
    let required_regions = lifetimes
        .iter()
        .map(|row| {
            (
                row.parameter.as_str().to_owned(),
                row.argument.scope_argument(),
            )
        })
        .collect::<Vec<_>>();
    let required_scope = AppliedTypeIdentityScope {
        region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
        declaration: scope.declaration.symbol,
        arguments: &required_operands,
        lifetimes: &required_regions,
    };
    // An outer operand retains its declared category and carrier. Original
    // source-edge telescopes follow provider substitutions, so each replacement
    // sees only earlier scopes. Caller operands belong to the required application.
    // Anonymous values land only through this independently admitted slot;
    // typed values retain their own width/policy, and missing custody declines.
    let agrees = scope.with_identity_scopes(source, |source_scopes| {
        let mut provider_scopes = Vec::with_capacity(source_scopes.len() + 1);
        provider_scopes.push(applied);
        provider_scopes.extend_from_slice(source_scopes);
        projected.iter().all(|(parameter, required, actual)| {
            let operand = |reference| {
                if matches!(parameter.kind, TypeParameterKind::Type) {
                    IndexIdentityArgument::TypeArgument(reference)
                } else {
                    IndexIdentityArgument::TypeReference(reference)
                }
            };
            let required = applied_static_parameter_identity(
                source,
                parameter,
                operand(*required),
                lexical,
                &provider_scopes,
                TypeIdentityQualification::PackageQualified,
            );
            let actual = applied_static_parameter_identity(
                source,
                parameter,
                operand(*actual),
                lexical,
                &[required_scope],
                TypeIdentityQualification::PackageQualified,
            );
            required.is_some() && required == actual
        })
    });
    if !agrees {
        return Ok(None);
    }
    let extra_evidence = provider_evidence_bindings(
        source,
        provider,
        scope.declaration,
        required_bindings,
        &bindings,
        application,
        &regions,
    )?;
    bindings.extend(extra_evidence);
    provider_lifetimes.sort_by_key(|row| {
        provider
            .lifetime_parameters
            .iter()
            .position(|parameter| parameter == &row.parameter)
    });
    Ok(Some(DeclaredTraitProviderApplication {
        bindings,
        lifetimes: provider_lifetimes,
    }))
}

fn binding_reference(
    program: &mut TypedTrees,
    value: &StaticMachineBindingValue,
) -> Result<TypeReferenceHandle, Diagnostic> {
    match value {
        StaticMachineBindingValue::Type(reference)
        | StaticMachineBindingValue::Const(reference) => Ok(*reference),
        StaticMachineBindingValue::Machine(argument)
        | StaticMachineBindingValue::Conformance(argument) => {
            super::application_arguments::static_argument_type_reference(program, argument)
                .ok_or_else(|| {
                    Diagnostic::error("trait application operand lost its typed declaration graph")
                })
        }
        StaticMachineBindingValue::RuntimeValue { .. } => Err(Diagnostic::error(
            "runtime subject is not an immutable trait application operand",
        )),
    }
}

fn provider_evidence_bindings(
    program: &TypedTrees,
    provider: &Machine,
    requirement: &typed_trees::trait_definition::TraitDefinition,
    required_bindings: &[StaticMachineBinding],
    bindings: &[StaticMachineBinding],
    application: StaticMachineApplicationHandle,
    provider_regions: &[(String, Option<String>)],
) -> Result<Vec<StaticMachineBinding>, Diagnostic> {
    let actual_operands = identity_operands(bindings);
    let required_operands = identity_operands(required_bindings);
    let required_regions = program
        .expression_table
        .static_machine_application_lifetimes(application)
        .iter()
        .map(|row| {
            (
                row.parameter.as_str().to_owned(),
                row.argument.scope_argument(),
            )
        })
        .collect::<Vec<_>>();
    let lexical = program
        .expression_table
        .static_machine_application(application)
        .type_identity_owner(program)
        .ok_or_else(|| {
            Diagnostic::error("provider evidence lost the required application's lexical owner")
        })?;
    let actual_scope = AppliedTypeIdentityScope {
        region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
        declaration: provider.symbol,
        arguments: &actual_operands,
        lifetimes: provider_regions,
    };
    let required_scope = AppliedTypeIdentityScope {
        region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
        declaration: requirement.symbol,
        arguments: &required_operands,
        lifetimes: &required_regions,
    };
    let mut selected = Vec::new();
    for bound in &provider.conformance_bounds {
        if bindings.iter().any(|row| row.parameter == bound.binder) {
            continue;
        }
        let matches =
            requirement
                .conformance_bounds
                .iter()
                .filter(|required| {
                    required.trait_symbol == bound.trait_symbol
                        && required.arguments.len() == bound.arguments.len()
                        && required.arguments.iter().zip(&bound.arguments).all(
                            |(expected, actual)| {
                                let expected = applied_type_identity(
                                    program,
                                    *expected,
                                    lexical,
                                    &[required_scope],
                                    TypeIdentityQualification::PackageQualified,
                                );
                                let actual = applied_type_identity(
                                    program,
                                    *actual,
                                    lexical,
                                    &[actual_scope],
                                    TypeIdentityQualification::PackageQualified,
                                );
                                expected.is_some() && expected == actual
                            },
                        )
                })
                .filter_map(|required| {
                    required_bindings
                        .iter()
                        .find(|row| row.parameter == required.binder)
                })
                .collect::<Vec<_>>();
        let [required] = matches.as_slice() else {
            return Err(Diagnostic::error(
                "declared provider evidence has no unique exact public bound correspondence",
            ));
        };
        let StaticMachineBindingValue::Conformance(argument) = &required.value else {
            return Err(Diagnostic::error(
                "declared provider evidence lost its selected conformance operand",
            ));
        };
        selected.push(StaticMachineBinding {
            parameter: bound.binder,
            value: StaticMachineBindingValue::Conformance(argument.clone()),
        });
    }
    Ok(selected)
}

fn identity_operands(
    bindings: &[StaticMachineBinding],
) -> Vec<(SymbolHandle, IndexIdentityArgument<'_>)> {
    bindings
        .iter()
        .map(|row| {
            let actual = match &row.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference) => {
                    IndexIdentityArgument::TypeReference(*reference)
                }
                StaticMachineBindingValue::Machine(argument)
                | StaticMachineBindingValue::Conformance(argument) => {
                    IndexIdentityArgument::StaticArgument(argument)
                }
                StaticMachineBindingValue::RuntimeValue { subject, .. } => {
                    IndexIdentityArgument::Subject(*subject)
                }
            };
            (row.parameter, actual)
        })
        .collect()
}
