//! Select a calling policy through its explicit named implementation map.
//! Boundary applications close their authored telescope before the exact
//! CallingPolicy requirement row selects an invocation. Names are diagnostic
//! payload; neither attached machine namespaces nor target defaults select code.

use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::static_applications::{
    StaticMachineBinding, StaticMachineBindingValue,
};
use checked_trees::typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) struct BoundaryPolicySourceApplication<'program> {
    pub(super) arguments: Vec<TypeReferenceHandle>,
    pub(super) policy: TypeReferenceHandle,
    pub(super) lexical_owner: symbols::SymbolHandle,
    pub(super) lifetime_arguments: Vec<&'program checked_trees::typed_trees::name::Identifier>,
}

pub(super) fn boundary_policy_instances<'program>(
    typed: &'program TypedTrees,
    boundary: &'program checked_trees::typed_trees::trait_definition::TraitDefinition,
    policy_argument: TypeReferenceHandle,
) -> Result<Vec<BoundaryPolicySourceApplication<'program>>, String> {
    let parameters = typed.trait_type_parameters(boundary);
    if parameters.is_empty() && boundary.lifetime_parameters.is_empty() {
        return Ok(vec![BoundaryPolicySourceApplication {
            arguments: Vec::new(),
            policy: policy_argument,
            lexical_owner: boundary.symbol,
            lifetime_arguments: Vec::new(),
        }]);
    }
    let policy_parameter_index = named_parameter_index(typed, policy_argument, parameters);

    let mut instances = Vec::new();
    for conformance in typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.trait_symbol == boundary.symbol)
    {
        // An open implementation telescope is not a closed ABI application.
        // Its genuine instances enter through the produced-header inventory.
        if !typed.conformance_type_parameters(conformance).is_empty() {
            continue;
        }
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
        let concrete_policy = policy_parameter_index.map_or(policy_argument, |parameter_index| {
            arguments[parameter_index]
        });
        if conformance.trait_lifetime_arguments.len() != boundary.lifetime_parameters.len() {
            return Err(
                "calling application lost its original trait lifetime correspondence".to_owned(),
            );
        }
        let lifetime_arguments = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| {
                conformance
                    .lifetime_parameters
                    .get(*ordinal as usize)
                    .ok_or_else(|| {
                        "calling application lost its original lexical region".to_owned()
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        instances.push(BoundaryPolicySourceApplication {
            arguments,
            policy: concrete_policy,
            lexical_owner: conformance.symbol,
            lifetime_arguments,
        });
    }

    // The shared inventory also consumes genuine produced public headers;
    // no source conformance is required to demand such a closed application.
    Ok(instances)
}

fn named_parameter_index(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    parameters: &[checked_trees::typed_trees::data::TypeParameter],
) -> Option<usize> {
    loop {
        match typed.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                return parameters.iter().position(|parameter| {
                    parameter.symbol.is_valid() && parameter.symbol == *symbol
                });
            }
            _ => return None,
        }
    }
}

/// Close the selected implementation before selecting its exact requirement
/// row. A same-named attached machine is never an implementation selection.
pub(crate) fn selected_policy_machine<'program>(
    typed: &'program TypedTrees,
    policy_argument: TypeReferenceHandle,
    calling_policy: &checked_trees::typed_trees::trait_definition::TraitDefinition,
    bindings: &[StaticMachineBinding],
) -> Result<&'program checked_trees::typed_trees::machine::Machine, String> {
    let selected = policy_static_argument(typed, policy_argument, bindings)?;
    // Source admission already judged this complete static application.
    // Keep its original selection and consume the exact named map; Omega
    // neither repeats binder admission nor chooses a same-spelled declaration.
    let conformance = typed
        .conformance_by_symbol(selected.symbol)
        .ok_or_else(|| {
            format!(
                "static argument `{}` does not name a package-scoped conformance",
                selected.display_name()
            )
        })?;
    if conformance.trait_symbol != calling_policy.symbol || !conformance.arguments.is_empty() {
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
    if !typed.machine_type_parameters(machine).is_empty() || !machine.conformance_bounds.is_empty()
    {
        return Err("generic calling-policy realization requires an explicitly specialized machine before policy evaluation".to_owned());
    }
    if typed
        .machine_states(machine)
        .first()
        .is_none_or(|state| state.symbol != row.realization_state)
    {
        return Err(
            "calling policy row must select its realization machine's entry state".to_owned(),
        );
    }
    Ok(machine)
}

pub(super) fn policy_static_argument(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    bindings: &[StaticMachineBinding],
) -> Result<checked_trees::typed_trees::expression::StaticMachineArgument, String> {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        typed.type_reference_table.type_reference(type_reference)
    {
        type_reference = *base_type;
    }
    if let Some(symbol) = typed
        .type_reference_table
        .type_reference(type_reference)
        .as_named_symbol()
        && let Some(binding) = bindings
            .iter()
            .find(|binding| binding.parameter.is_valid() && binding.parameter == symbol)
    {
        match &binding.value {
            StaticMachineBindingValue::Type(actual) | StaticMachineBindingValue::Const(actual) => {
                type_reference = *actual;
            }
            StaticMachineBindingValue::Conformance(actual)
            | StaticMachineBindingValue::Machine(actual) => return Ok(actual.clone()),
            StaticMachineBindingValue::RuntimeValue { .. } => {
                return Err(
                    "calling-policy selection cannot consume a runtime value subject".to_owned(),
                );
            }
        }
    }
    while let TypeReferenceNode::Constrained { base_type, .. } =
        typed.type_reference_table.type_reference(type_reference)
    {
        type_reference = *base_type;
    }
    let (symbol, name, application) = match typed
        .type_reference_table
        .type_reference(type_reference)
    {
        TypeReferenceNode::Named { symbol, name } => (*symbol, name.clone(), None),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            // The static application can retain a substituted argument tree,
            // but a data-type identity still reads its original arena handle.
            // Do not claim that identity is closed when nested type substitution
            // would require constructing a new type-reference application.
            if !base_symbol.is_valid() {
                return Err(
                    "calling-policy generic application has no resolved declaration".to_owned(),
                );
            }
            if typed.symbols.get(*base_symbol).kind == symbols::SymbolKind::Data
                && policy_argument_needs_substitution(typed, type_reference, bindings)
            {
                return Err("nested generic data arguments in a calling-policy implementation require type application specialization before evaluation".to_owned());
            }
            let arguments = typed
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| policy_static_argument(typed, *argument, bindings))
                .collect::<Result<Vec<_>, _>>()?;
            (
                *base_symbol,
                base_name.clone(),
                Some(Box::new(
                    checked_trees::typed_trees::expression::StaticSymbolApplication {
                        lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                        arguments: arguments.into_boxed_slice(),
                    },
                )),
            )
        }
        _ => {
            return Err(
                "calling-policy static arguments require a resolved named or generic application"
                    .to_owned(),
            );
        }
    };
    if !symbol.is_valid()
        || matches!(
            typed.symbols.get(symbol).kind,
            symbols::SymbolKind::TypeParameter
                | symbols::SymbolKind::ConformanceParameter
                | symbols::SymbolKind::MachineParameter
        )
    {
        return Err(
            "calling-policy implementation application retains an unresolved static binder"
                .to_owned(),
        );
    }
    Ok(
        checked_trees::typed_trees::expression::StaticMachineArgument {
            authored_position: None,
            type_reference: if matches!(
                typed.symbols.get(symbol).kind,
                symbols::SymbolKind::Data | symbols::SymbolKind::BuiltinType
            ) {
                type_reference
            } else {
                TypeReferenceHandle::invalid()
            },
            path: Box::from([name]),
            application,
            const_literal: None,
            evidence_projection: None,
            symbol,
        },
    )
}

fn policy_argument_needs_substitution(
    typed: &TypedTrees,
    argument: TypeReferenceHandle,
    bindings: &[StaticMachineBinding],
) -> bool {
    match typed.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { symbol, .. } => bindings.iter().any(|binding| {
            binding.parameter == *symbol
                && !matches!(
                    &binding.value,
                    StaticMachineBindingValue::Type(actual)
                        | StaticMachineBindingValue::Const(actual) if *actual == argument
                )
        }),
        TypeReferenceNode::Generic { arguments, .. } => typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| policy_argument_needs_substitution(typed, *argument, bindings)),
        TypeReferenceNode::Constrained { base_type, .. } => {
            policy_argument_needs_substitution(typed, *base_type, bindings)
        }
        _ => false,
    }
}
