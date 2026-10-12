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
    let requirement_symbol = signature.symbol;
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
    let caller_view =
        typed_trees::static_applications::StaticApplicationView::new(source, application)
            .map_err(Diagnostic::error)?;
    let lifetimes = source
        .expression_table
        .static_machine_application_lifetimes(application);
    let lexical = caller_view
        .application()
        .type_identity_owner(source)
        .ok_or_else(|| {
            Diagnostic::error("required application lost its original lexical region owner")
        })?;
    let caller_owner = match lexical {
        typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(owner) => owner,
        typed_trees::type_identity::AppliedTypeIdentityOwner::Captured(capture) => {
            if !source.retains_source_declaration_symbol(capture.owner(), capture.symbol()) {
                return Err(Diagnostic::error(
                    "provider input application lost its original caller declaration",
                ));
            }
            capture.symbol()
        }
    };
    let (source_owner, source_lifetimes) = if named_conformance.is_valid() {
        let declaration = original.conformances.get(named_conformance);
        (
            declaration.symbol,
            declaration.lifetime_parameters.as_slice(),
        )
    } else {
        (provider.symbol, provider.lifetime_parameters.as_slice())
    };
    let mut explicit_regions = Vec::new();
    for (ordinal, root_ordinal) in region_ordinals.iter().enumerate() {
        let source_ordinal = usize::try_from(*root_ordinal)
            .ok()
            .and_then(|ordinal| edge.trait_lifetime_arguments.get(ordinal))
            .and_then(|ordinal| usize::try_from(*ordinal).ok())
            .ok_or_else(|| {
                Diagnostic::error("declared provider lost an inherited captured region slot")
            })?;
        if source_ordinal >= source_lifetimes.len() {
            return Err(Diagnostic::error(
                "declared source maps a region outside its original telescope",
            ));
        }
        let argument =
            required_region_argument(&caller_view, scope.declaration.symbol, ordinal, false)?;
        let Some(name) = argument.named() else {
            return Err(Diagnostic::error(
                "captured provider region has no named source correspondence",
            ));
        };
        let identity = caller_region_identity(source, lexical, name)?;
        explicit_regions.push((
            source_ordinal,
            validation::borrow_contracts::InputLifetimeArgument::Captured(identity),
        ));
    }
    let provider_substitutions = bindings
        .iter()
        .filter_map(|binding| match &binding.value {
            StaticMachineBindingValue::Type(reference)
            | StaticMachineBindingValue::Const(reference) => Some((binding.parameter, *reference)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let type_inputs = projected
        .iter()
        .map(|(_, actual, required)| (*actual, *required))
        .collect::<Vec<_>>();
    let source_application = validation::borrow_contracts::infer_type_input_lifetime_application(
        source,
        validation::borrow_contracts::InputTypeRegionScope {
            declaration: source_owner,
            lifetime_parameters: source_lifetimes,
            substitutions: &provider_substitutions,
            argument_owner: caller_owner,
        },
        caller_owner,
        &type_inputs,
        if named_conformance.is_valid() {
            0
        } else {
            invocation_regions.len()
        },
        &explicit_regions,
    )
    .map_err(Diagnostic::error)?;
    let source_regions = source_application
        .iter()
        .map(|binding| {
            let argument = receiving_region_argument(
                source,
                lexical,
                &caller_view,
                requirement_symbol,
                &binding.argument,
            )?;
            Ok((
                binding.parameter.as_str().to_owned(),
                argument.scope_argument(),
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let source_applied = AppliedTypeIdentityScope {
        region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Supplied,
        declaration: source_owner,
        arguments: &[],
        lifetimes: &source_regions,
    };
    let original_signature = original
        .trait_machine_signature_by_symbol(scope.declaration, requirement)
        .ok_or_else(|| {
            Diagnostic::error("provider input correspondence lost its original requirement")
        })?;
    let state = original.machine_states(provider).first().ok_or_else(|| {
        Diagnostic::error("provider input correspondence lost its original entry")
    })?;
    let invocation_binders = invocation
        .iter()
        .zip(&required_parameters)
        .enumerate()
        .flat_map(|(ordinal, (actual, required))| {
            let name = format!("$provider-invocation-{ordinal}");
            [(actual.symbol, name.clone()), (required.symbol, name)]
        })
        .collect::<Vec<_>>();
    let input_application = scope
        .with_identity_scopes(&original, |scopes| {
            validation::borrow_contracts::infer_callable_input_lifetime_application(
                &original,
                validation::borrow_contracts::CallableBorrowContract {
                    lexical_owner: original_signature.symbol,
                    lifetime_parameters: &original_signature.lifetime_parameters,
                    parameters: original.state_signature_parameters(original_signature),
                    return_type: original_signature.return_type,
                    substitutions: &scope.substitutions,
                    substitution_scope_starts: &scope.substitution_scope_starts,
                    application_scopes: scopes,
                    binders: &invocation_binders,
                    lifetime_binding_declaration: SymbolHandle::invalid(),
                    lifetime_bindings: &[],
                    argument_owner: scope.argument_owner,
                    supplied_telescope: false,
                },
                validation::borrow_contracts::CallableBorrowContract {
                    lexical_owner: provider.symbol,
                    lifetime_parameters: &provider.lifetime_parameters,
                    parameters: original.state_parameters(state),
                    return_type: state.return_type,
                    substitutions: &[],
                    substitution_scope_starts: &[],
                    application_scopes: &[],
                    binders: &invocation_binders,
                    lifetime_binding_declaration: SymbolHandle::invalid(),
                    lifetime_bindings: &[],
                    argument_owner: provider.symbol,
                    supplied_telescope: false,
                },
            )
        })
        .map_err(Diagnostic::error)?;
    let mut regions = Vec::new();
    let mut provider_lifetimes = Vec::new();
    for binding in input_application {
        let argument = match binding.argument {
            validation::borrow_contracts::InputLifetimeArgument::Captured(identity)
                if identity.declaration == original.lifetime_declaration_owner(source_owner) =>
            {
                source_application
                    .get(identity.ordinal)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "provider input lost its captured source application slot",
                        )
                    })?
                    .argument
                    .clone()
            }
            argument => argument,
        };
        let argument = receiving_region_argument(
            source,
            lexical,
            &caller_view,
            requirement_symbol,
            &argument,
        )?;
        regions.push((
            binding.parameter.as_str().to_owned(),
            argument.scope_argument(),
        ));
        provider_lifetimes.push(StaticMachineLifetimeBinding {
            parameter: binding.parameter,
            argument,
        });
    }
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
        let mut provider_scopes = Vec::with_capacity(source_scopes.len() + 2);
        provider_scopes.push(source_applied);
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

fn required_region_argument<'owner>(
    view: &typed_trees::static_applications::StaticApplicationView<'owner>,
    declaration: SymbolHandle,
    ordinal: usize,
    invocation: bool,
) -> Result<&'owner typed_trees::static_applications::StaticMachineLifetimeArgument, Diagnostic> {
    let mut matching = view.lifetimes().iter().filter(|(formal, _)| {
        formal.declaration == declaration
            && formal.ordinal == ordinal
            && formal.invocation == invocation
    });
    let (_, binding) = matching.next().ok_or_else(|| {
        Diagnostic::error("required application lost its original lifetime coordinate")
    })?;
    if matching.next().is_some() {
        return Err(Diagnostic::error(
            "required lifetime coordinate has competing bindings",
        ));
    }
    let binding = *binding;
    Ok(&binding.argument)
}

fn caller_region_identity(
    program: &TypedTrees,
    owner: typed_trees::type_identity::AppliedTypeIdentityOwner<'_>,
    name: &typed_trees::name::Identifier,
) -> Result<typed_trees::type_identity::LifetimeParameterIdentity, Diagnostic> {
    let identity = match owner {
        typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(owner) => {
            typed_trees::type_identity::lifetime_parameter_identity(program, owner, name.as_str())
        }
        typed_trees::type_identity::AppliedTypeIdentityOwner::Captured(capture) => {
            typed_trees::type_identity::lifetime_parameter_identity(
                capture.owner(),
                capture.symbol(),
                name.as_str(),
            )
        }
    };
    identity.ok_or_else(|| {
        Diagnostic::error("input region lost its original caller declaration coordinate")
    })
}

fn receiving_region_argument(
    program: &TypedTrees,
    owner: typed_trees::type_identity::AppliedTypeIdentityOwner<'_>,
    caller: &typed_trees::static_applications::StaticApplicationView<'_>,
    requirement: SymbolHandle,
    argument: &validation::borrow_contracts::InputLifetimeArgument,
) -> Result<typed_trees::static_applications::StaticMachineLifetimeArgument, Diagnostic> {
    use validation::borrow_contracts::InputLifetimeArgument;
    match argument {
        InputLifetimeArgument::Invocation(ordinal) => {
            required_region_argument(caller, requirement, *ordinal, true).cloned()
        }
        InputLifetimeArgument::Captured(identity) => {
            let name = match owner {
                typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(_) => {
                    typed_trees::type_identity::lifetime_parameter_name(program, *identity)
                }
                typed_trees::type_identity::AppliedTypeIdentityOwner::Captured(capture) => {
                    typed_trees::type_identity::lifetime_parameter_name(capture.owner(), *identity)
                }
            }
            .ok_or_else(|| {
                Diagnostic::error("captured input region lost its original payload owner")
            })?;
            if caller_region_identity(program, owner, name)? != *identity {
                return Err(Diagnostic::error(
                    "captured input region differs from its caller scope",
                ));
            }
            Ok(name.clone().into())
        }
        InputLifetimeArgument::AnonymousInvocation(identity) => {
            let argument =
                required_region_argument(caller, identity.declaration, identity.ordinal, true)?;
            if !matches!(
                argument,
                typed_trees::static_applications::StaticMachineLifetimeArgument::Anonymous
            ) {
                return Err(Diagnostic::error(
                    "anonymous input region lost its original invocation formal",
                ));
            }
            Ok(argument.clone())
        }
    }
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
