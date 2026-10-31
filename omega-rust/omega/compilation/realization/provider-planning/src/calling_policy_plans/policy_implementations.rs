//! Select a calling policy through its explicit named implementation map.
//! Boundary applications close their authored telescope before the exact
//! CallingPolicy requirement row selects an invocation. Names are diagnostic
//! payload; neither attached machine namespaces nor target defaults select code.

use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::types::TypeReferenceHandle;

pub(crate) struct BoundaryPolicyInstance {
    pub arguments: Vec<TypeReferenceHandle>,
    pub argument_owner: symbols::SymbolHandle,
}

pub(crate) fn boundary_policy_instances(
    typed: &TypedTrees,
    boundary: &checked_trees::typed_trees::trait_definition::TraitDefinition,
) -> Result<Vec<BoundaryPolicyInstance>, String> {
    let parameters = typed.trait_type_parameters(boundary);
    if parameters.is_empty() {
        return Ok(vec![BoundaryPolicyInstance {
            arguments: Vec::new(),
            argument_owner: boundary.symbol,
        }]);
    }

    let mut instances = Vec::new();
    for conformance in typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.trait_symbol == boundary.symbol)
    {
        // Every named implementation of the generic boundary trait is one
        // concrete application; its trait arguments carry every data type.
        let arguments = typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .to_vec();
        if arguments.len() != parameters.len() {
            return Err(format!(
                "conformance `{} satisfies {}` supplies {} argument(s), expected {}",
                typed.symbols.display_path(conformance.symbol, "::"),
                conformance.trait_name,
                arguments.len(),
                parameters.len()
            ));
        }
        instances.push(BoundaryPolicyInstance {
            arguments,
            argument_owner: conformance.symbol,
        });
    }

    // A generic declaration is not itself a callable ABI. With no concrete
    // conformance there is no requirement identity to publish yet.
    Ok(instances)
}

/// Close the selected implementation before selecting its exact requirement
/// row. A same-named attached machine is never an implementation selection.
pub(crate) fn selected_policy_machine<'program>(
    typed: &'program TypedTrees,
    boundary: &checked_trees::typed_trees::trait_definition::TraitDefinition,
    boundary_arguments: &[TypeReferenceHandle],
    argument_owner: symbols::SymbolHandle,
    calling_trait: &checked_trees::typed_trees::trait_definition::TraitDefinition,
    calling_policy: &checked_trees::typed_trees::trait_definition::TraitDefinition,
    target_profile: target::TargetProfile,
) -> Result<&'program checked_trees::typed_trees::machine::Machine, String> {
    use checked_trees::typed_trees::trait_definition::{
        AppliedTraitScope, TraitApplicationArguments,
    };
    let boundary_scope = AppliedTraitScope::new(
        typed,
        boundary,
        TraitApplicationArguments {
            lexical_owner: argument_owner,
            enclosing_data_applications: &[],
            arguments: boundary_arguments,
            lifetimes: &[],
        },
    )
    .ok_or("calling policy has no complete boundary application")?;
    let calling_scope = boundary_scope
        .declaring_scope(typed, calling_trait.symbol)
        .ok_or("calling policy has no unique exact Calling application")?;
    let selections = typed
        .trait_implementation_applications
        .iter()
        .filter(|(_, selected)| {
            selected.source_scope.scope(typed).is_some_and(|scope| {
                scope.argument_owner == argument_owner
                    && scope.same_application(typed, &calling_scope)
            })
        })
        .map(|(_, selected)| selected)
        .collect::<Vec<_>>();
    let [selected] = selections.as_slice() else {
        return Err("calling-policy implementation application has no uniquely prepared closed source selection; unresolved static binders cannot execute".to_owned());
    };
    // Settlement runs on the admitted program: the conformance application's
    // arity, kind and carrier obligations already passed checking, so the
    // realization join needs only the conformance's exact trait and rows.
    let Some(conformance) = typed.conformance_by_symbol(selected.application.declaration) else {
        return Err(format!(
            "static argument `{}` does not name a package-scoped conformance",
            typed
                .symbols
                .display_path(selected.application.declaration, "::")
        ));
    };
    let trait_definition = typed
        .trait_definition_by_symbol(conformance.trait_symbol)
        .ok_or_else(|| {
            format!(
                "conformance `{}` names unresolved trait `{}`",
                conformance
                    .alias
                    .as_ref()
                    .map_or("<unnamed-conformance>", |name| name.as_str()),
                conformance.trait_name
            )
        })?;
    if trait_definition.symbol != calling_policy.symbol || !conformance.arguments.is_empty() {
        return Err(
            "calling policy must explicitly select a complete CallingPolicy implementation"
                .to_owned(),
        );
    }
    let requirements = typed.trait_machine_signatures(calling_policy);
    let [requirement] = requirements else {
        return Err("CallingPolicy must declare exactly one canonical plan requirement".to_owned());
    };
    if requirement.name.as_str() != "plan" {
        return Err("CallingPolicy has no canonical plan requirement".to_owned());
    }
    let rows = conformance
        .rows
        .iter()
        .filter(|row| {
            row.declaring_trait == calling_policy.symbol && row.requirement == requirement.symbol
        })
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return Err(
            "calling policy implementation must retain exactly one CallingPolicy::plan row"
                .to_owned(),
        );
    };
    let machine = typed
        .machine(row.realization_machine)
        .ok_or_else(|| "calling policy row has no exact realization machine".to_owned())?;
    if typed
        .machine_states(machine)
        .first()
        .is_none_or(|state| state.symbol != row.realization_state)
    {
        return Err(
            "calling policy row must select its realization machine's entry state".to_owned(),
        );
    }
    let machine = if typed.declaration_scope_parent(machine.symbol) == conformance.symbol
        && (!typed.conformance_type_parameters(conformance).is_empty()
            || !conformance.lifetime_parameters.is_empty())
    {
        let captures = typed
            .captured_machine_applications
            .iter()
            .filter(|(_, capture)| {
                capture.template == machine.symbol
                    && capture.conformance_application == selected.application
                    && typed
                        .expression_table
                        .static_machine_application(capture.supplying_application)
                        .lexical_owner
                        == argument_owner
            })
            .map(|(_, capture)| capture)
            .collect::<Vec<_>>();
        let [capture] = captures.as_slice() else {
            return Err(
                "calling policy has no unique checked realization of its exact map capture"
                    .to_owned(),
            );
        };
        let entry = typed
            .captured_machine_states
            .span_or_empty(capture.states)
            .iter()
            .filter(|state| state.template == row.realization_state)
            .collect::<Vec<_>>();
        if entry.len() != 1 {
            return Err("calling policy capture lost its exact entry correspondence".to_owned());
        }
        let realized = typed
            .machine(capture.instance)
            .ok_or("calling policy capture has no executable body")?;
        if typed
            .machine_states(realized)
            .first()
            .is_none_or(|state| state.symbol != entry[0].instance)
        {
            return Err(
                "calling policy capture does not select its checked entry state".to_owned(),
            );
        }
        realized
    } else {
        machine
    };
    if !typed.machine_type_parameters(machine).is_empty() || !machine.conformance_bounds.is_empty()
    {
        return Err("generic calling-policy realization requires an explicitly specialized machine before policy evaluation".to_owned());
    }
    // The implementation row selects the authored callable. A family head
    // is its portable contract, not an executable policy body. Resolve that
    // exact family on the selected deployment before build-time admission,
    // as external binding producers and provider rows already do.
    typed
        .machine_for_target(machine, target_profile.target_name())
        .ok_or_else(|| {
            format!(
                "calling policy realization `{}` has no body for target `{}`",
                machine.name,
                target_profile.target_name()
            )
        })
}
