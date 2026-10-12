//! Static operand substitution owned by exact retained application bindings.
//!
//! Headers, results and executable bodies substitute the same original binder
//! rows. A simultaneous actual remains in its caller scope and is never visited
//! again by that telescope. Runtime Value subjects retain their independent
//! runtime route; their carrier is not a static const actual.

use crate::monomorphization::selection::state_by_symbol;
use crate::monomorphization::{
    StaticMachineArgument, SymbolHandle, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
};
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};

mod lexical_scope;
pub(super) use lexical_scope::bindings_are_in_scope;

pub(crate) fn substitute_machine_parameter_type_references(
    program: &mut TypedTrees,
    bindings: &[StaticMachineBinding],
    type_start: Option<usize>,
) {
    for row in bindings {
        let StaticMachineBindingValue::Machine(binding) = &row.value else {
            continue;
        };
        let parameter_symbol = &row.parameter;
        let name = binding
            .path
            .last()
            .cloned()
            .or_else(|| state_by_symbol(program, binding.symbol).map(|state| state.name.clone()))
            .expect("admitted static machine argument has an entry name");
        let occurrences: Vec<_> = program
            .type_reference_table
            .named_references()
            .filter(|(handle, symbol, _)| {
                type_start.is_none_or(|start| handle.arena_index() as usize >= start)
                    && symbol == parameter_symbol
            })
            .map(|(handle, _, _)| handle)
            .collect();
        for occurrence in occurrences {
            program.type_reference_table.substitute_node(
                occurrence,
                TypeReferenceNode::Named {
                    symbol: binding.symbol,
                    name: name.clone(),
                },
            );
        }
    }
}

pub(crate) fn substitute_forwarded_machine_arguments(
    arguments: &mut [StaticMachineArgument],
    static_rewrites: &[(SymbolHandle, StaticMachineArgument)],
    rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
) {
    for argument in arguments {
        if let Some((_, replacement)) = static_rewrites
            .iter()
            .find(|(parameter, _)| *parameter == argument.symbol)
        {
            let authored_position = argument.authored_position;
            *argument = replacement.clone();
            argument.clear_authored_positions();
            argument.authored_position = authored_position;
            // The actual is already in the caller's scope. Recursing into it
            // would apply the callee's telescope a second time.
            continue;
        } else if let Some((_, symbol, name)) = rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == argument.symbol)
        {
            argument.symbol = *symbol;
            argument.path = vec![name.clone()].into_boxed_slice();
        }
        if let Some(application) = &mut argument.application {
            substitute_forwarded_machine_arguments(
                &mut application.arguments,
                static_rewrites,
                rewrites,
            );
        }
    }
}

pub(crate) fn forwarded_static_argument_rewrites(
    program: &dyn ::validation::CheckingInput,
    bindings: &[StaticMachineBinding],
) -> Vec<(SymbolHandle, StaticMachineArgument)> {
    bindings
        .iter()
        .filter_map(|binding| {
            let actual = match &binding.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference) => program
                    .static_operand_from_type_reference(*reference)
                    .ok()?,
                StaticMachineBindingValue::Machine(actual)
                | StaticMachineBindingValue::Conformance(actual) => actual.clone(),
                // A runtime subject must be forwarded by the runtime-subject
                // producer, never substituted as its carrier's const value.
                StaticMachineBindingValue::RuntimeValue { .. } => return None,
            };
            Some((binding.parameter, actual))
        })
        .collect()
}

pub(crate) use typed_trees::static_applications::static_argument_from_type_reference;

/// Retain one already resolved static operand in the shared type/index
/// application representation. The caller owns payload relocation; category
/// and bound admission continue to use the original declaration telescope.
pub(crate) fn static_argument_type_reference(
    owner: &mut TypedTrees,
    argument: &StaticMachineArgument,
) -> Option<TypeReferenceHandle> {
    if argument.type_reference.is_valid() {
        return Some(argument.type_reference);
    }
    if argument.evidence_projection.is_some() {
        return None;
    }
    if argument.symbol.is_valid()
        && matches!(
            owner.symbols.get(argument.symbol).kind,
            symbols::SymbolKind::Data
                | symbols::SymbolKind::BuiltinType
                | symbols::SymbolKind::TypeParameter
                | symbols::SymbolKind::Machine
                | symbols::SymbolKind::State
                | symbols::SymbolKind::MachineParameter
                | symbols::SymbolKind::Conformance
                | symbols::SymbolKind::ConformanceParameter
                | symbols::SymbolKind::Trait
        )
    {
        if let Some(application) = &argument.application {
            let arguments = application
                .arguments
                .iter()
                .map(|actual| static_argument_type_reference(owner, actual))
                .collect::<Option<Vec<_>>>()?;
            let arguments = owner
                .type_reference_table
                .insert_type_reference_handles(arguments);
            return Some(
                owner
                    .type_reference_table
                    .insert(TypeReferenceNode::Generic {
                        base_symbol: argument.symbol,
                        base_name: argument.path.last().cloned().unwrap_or_default(),
                        lifetime_arguments: application.lifetime_arguments.to_vec(),
                        arguments,
                    }),
            );
        }
        return Some(
            owner
                .type_reference_table
                .find_named_type_reference(argument.symbol)
                .unwrap_or_else(|| {
                    owner.type_reference_table.insert(TypeReferenceNode::Named {
                        symbol: argument.symbol,
                        name: argument.path.last().cloned().unwrap_or_default(),
                    })
                }),
        );
    }
    // Type-role const children no longer retain a proposal alongside their
    // normalized type row. Preserve any prior scalar landing/carrier in the
    // existing canonical atom instead of erasing it into an anonymous integer.
    use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
    let spelling = if let Some(literal) = &argument.const_literal {
        if let Some(landing) = literal.landing() {
            let value = literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))?;
            let identity = CanonicalConstIdentity::integer(landing.landed_type.name(), value);
            CanonicalConstValue::new(identity.type_name, identity.encoding, value.to_string())
                .atom()
        } else {
            validation::static_arguments::spelling(owner, argument)?
        }
    } else if argument.symbol.is_valid() {
        let declaration = owner.const_declaration_by_symbol(argument.symbol)?;
        // Structured const closure needs its declaration-owned carrier join;
        // do not invent that receipt from a source spelling here.
        let primitive = owner
            .type_reference_table
            .primitive_type(declaration.declared_type)?;
        CanonicalConstValue::new(
            if primitive.accepts_integer_literal() {
                primitive.name().to_owned()
            } else {
                owner.display_type_reference(declaration.declared_type)
            },
            declaration.evaluated_value.as_ref()?.encoding()?.to_owned(),
            argument.display_name(),
        )
        .atom()
    } else {
        validation::static_arguments::spelling(owner, argument)?
    };
    let meaning = owner.static_const_integer_meaning(argument);
    Some(intern_closed_const_with_meaning(owner, &spelling, meaning))
}

pub(crate) fn intern_closed_const_with_meaning(
    owner: &mut TypedTrees,
    spelling: &str,
    meaning: typed_trees::types::ConstIntegerMeaning,
) -> TypeReferenceHandle {
    if let Some((handle, _, _)) =
        owner
            .type_reference_table
            .named_references()
            .find(|(handle, symbol, name)| {
                !symbol.is_valid()
                    && *name == spelling
                    && owner.type_reference_table.const_integer_meaning(*handle) == meaning
            })
    {
        return handle;
    }
    let reference = owner.type_reference_table.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::invalid(),
        name: typed_trees::name::Identifier::generated(spelling),
    });
    owner
        .type_reference_table
        .retain_const_integer_meaning(reference, meaning);
    reference
}

/// Every declaration application owns argument handles, including trait edges
/// whose arguments are not children of a Generic type node. Normalize closed
/// Const/Value slots through their original telescope before inference or a
/// retained schema reads them. Open values keep their existing continuation;
/// normalization neither admits a carrier nor invents a runtime subject.
pub(super) fn materialize_const_type_arguments(program: &mut TypedTrees) {
    let mut sites = Vec::new();
    for (_, reference) in program.type_reference_table.type_reference_rows() {
        let (base_symbol, arguments) = match reference {
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            }
            | TypeReferenceNode::DynamicTrait {
                symbol: base_symbol,
                arguments,
                ..
            } => (*base_symbol, *arguments),
            _ => continue,
        };
        collect_const_type_argument_sites(
            program,
            base_symbol,
            program
                .type_reference_table
                .type_reference_handles(arguments),
            &mut sites,
        );
    }
    for definition in program.traits() {
        for relationship in program.trait_requirements(definition) {
            collect_const_type_argument_sites(
                program,
                relationship.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(relationship.arguments),
                &mut sites,
            );
        }
        if let Some(base) = &definition.refines {
            collect_const_type_argument_sites(
                program,
                base.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(base.arguments),
                &mut sites,
            );
        }
        for bound in &definition.conformance_bounds {
            collect_const_type_argument_sites(
                program,
                bound.trait_symbol,
                &bound.arguments,
                &mut sites,
            );
        }
    }
    for implementation in program.conformances() {
        collect_const_type_argument_sites(
            program,
            implementation.trait_symbol,
            program
                .type_reference_table
                .type_reference_handles(implementation.arguments),
            &mut sites,
        );
    }
    for machine in program.machines() {
        for edge in program.machine_trait_conformances(machine) {
            collect_const_type_argument_sites(
                program,
                edge.symbol,
                program
                    .type_reference_table
                    .type_reference_handles(edge.arguments),
                &mut sites,
            );
        }
        for bound in &machine.conformance_bounds {
            collect_const_type_argument_sites(
                program,
                bound.trait_symbol,
                &bound.arguments,
                &mut sites,
            );
        }
    }
    for (_, parameter) in program.data_type_parameters.iter() {
        if let typed_trees::data::TypeParameterKind::Machine {
            contract:
                typed_trees::data::MachineParameterContract::Nominal {
                    trait_definition,
                    trait_arguments,
                    ..
                },
        } = &parameter.kind
        {
            collect_const_type_argument_sites(
                program,
                *trait_definition,
                trait_arguments,
                &mut sites,
            );
        }
    }
    for definition in program.data_definitions() {
        if let Some(equivalence) = definition
            .quotient
            .as_ref()
            .and_then(|quotient| quotient.equivalence.as_ref())
        {
            collect_const_type_argument_sites(
                program,
                equivalence.trait_symbol,
                program
                    .type_reference_table
                    .type_reference_handles(equivalence.trait_arguments),
                &mut sites,
            );
        }
    }
    // Shared argument nodes still belong to one issuing table. Materialize
    // each full generational handle once, not once per reaching declaration.
    sites.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    sites.dedup();
    for argument in sites {
        materialize_const_type_argument(program, argument);
    }
}

fn collect_const_type_argument_sites(
    program: &TypedTrees,
    declaration: SymbolHandle,
    arguments: &[TypeReferenceHandle],
    sites: &mut Vec<TypeReferenceHandle>,
) {
    use typed_trees::data::TypeParameterKind;
    let parameters = if let Some(data) = program.data_definition_by_symbol(declaration) {
        program.data_type_parameters(data)
    } else if let Some(machine) = program.machine(declaration) {
        program.machine_type_parameters(machine)
    } else if let Some(definition) = program.trait_definition_by_symbol(declaration) {
        program.trait_type_parameters(definition)
    } else if let Some(domain) = program.domain_definition(declaration) {
        program.domain_type_parameters(domain)
    } else if let Some(implementation) = program.conformance_by_symbol(declaration) {
        program.conformance_type_parameters(implementation)
    } else {
        return;
    };
    if parameters.len() != arguments.len() {
        return; // Ordinary declaration admission owns incomplete argument packs.
    }
    sites.extend(
        parameters
            .iter()
            .zip(arguments)
            .filter_map(|(parameter, argument)| {
                matches!(
                    parameter.kind,
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                )
                .then_some(*argument)
            }),
    );
}

/// An inferred application can introduce a const graph after the ordinary
/// type-argument scan. Normalize that same graph before retaining its tuple;
/// declaration admission still owns its carrier and range obligations.
pub(super) fn materialize_const_type_argument(
    program: &mut TypedTrees,
    argument: TypeReferenceHandle,
) {
    if let Some((value, meaning)) = normalized_const_type_argument_with_meaning(program, argument) {
        program
            .type_reference_table
            .substitute_node(argument, value);
        program
            .type_reference_table
            .retain_const_integer_meaning(argument, meaning);
    }
}

/// Read value bytes and original integer meaning from the same issuing graph.
/// Neither normalization nor value materialization borrows a destination policy.
/// A materializer can run after a specialization retained its issuing tuple;
/// changing that binding in place would invalidate the correspondence.
pub(super) fn normalized_const_type_argument_with_meaning(
    program: &TypedTrees,
    argument: TypeReferenceHandle,
) -> Option<(TypeReferenceNode, typed_trees::types::ConstIntegerMeaning)> {
    if let Some(value) = program.completed_const_argument_atom(argument) {
        return Some((
            TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: typed_trees::name::Identifier::generated(value.atom()),
            },
            typed_trees::types::ConstIntegerMeaning::Unavailable,
        ));
    }
    let value = program.closed_const_integer_argument(argument)?;
    let integer = value
        .value
        .to_i64()
        .map(i128::from)
        .or_else(|| value.value.to_u64().map(i128::from))?;
    // An anonymous integer has no independently selected carrier to retain.
    // Keep its ordinary const identity; the declared slot owns its landing.
    // A typed operation must preserve its carrier in the canonical atom.
    let meaning = typed_trees::types::ConstIntegerMeaning::from_closed_value(
        &value,
        &program.type_reference_table,
    );
    let atom = if let Some(primitive) = value.primitive {
        let identity = language_semantics::const_value::CanonicalConstIdentity::integer(
            primitive.name(),
            integer,
        );
        language_semantics::const_value::CanonicalConstValue::new(
            identity.type_name,
            identity.encoding,
            integer.to_string(),
        )
        .atom()
    } else {
        integer.to_string()
    };
    // This is value normalization, not destination admission. The ordinary
    // const-slot judgment still checks its declared carrier and bounds.
    Some((
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: typed_trees::name::Identifier::generated(atom),
        },
        meaning,
    ))
}

/// Identity may describe a complete symbolic tuple. Executable production
/// additionally requires every static operand to be closed; nested forwarded
/// type, const, machine and conformance binders keep their original caller.
pub(super) fn static_application_is_closed(
    program: &TypedTrees,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
) -> bool {
    use typed_trees::static_applications::StaticMachineBindingValue;
    fn open_symbol(program: &TypedTrees, symbol: SymbolHandle) -> bool {
        symbol.is_valid()
            && matches!(
                program.symbols.get(symbol).kind,
                symbols::SymbolKind::TypeParameter
                    | symbols::SymbolKind::MachineParameter
                    | symbols::SymbolKind::ConformanceParameter
            )
    }
    fn reference_is_closed(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
        reference.is_valid()
            && !super::for_each_type_reference(
                program,
                &[reference],
                |_, node| match node {
                    TypeReferenceNode::Named { symbol, .. } => open_symbol(program, *symbol),
                    TypeReferenceNode::Generic { base_symbol, .. }
                    | TypeReferenceNode::DynamicTrait {
                        symbol: base_symbol,
                        ..
                    } => open_symbol(program, *base_symbol),
                    TypeReferenceNode::ConstExpression(expression) => program
                        .closed_integer_expression_value(*expression)
                        .is_none(),
                    TypeReferenceNode::FixedArray { length, .. } => {
                        !matches!(length, typed_trees::types::FixedArrayLength::Literal(_))
                    }
                    _ => false,
                },
                |_, minimum, maximum| {
                    [minimum, maximum].into_iter().any(|expression| {
                        let mut expressions = Vec::new();
                        validation::collect_expression_tree(program, expression, &mut expressions);
                        expressions.into_iter().any(|expression| {
                            matches!(program.expression_table.expression(expression),
                        typed_trees::expression::ExpressionNode::Name(path)
                            if open_symbol(program, path.symbol))
                        })
                    })
                },
            )
    }
    fn argument_is_closed(program: &TypedTrees, argument: &StaticMachineArgument) -> bool {
        !open_symbol(program, argument.symbol)
            && (!argument.type_reference.is_valid()
                || reference_is_closed(program, argument.type_reference))
            && argument.evidence_projection.is_none()
            && argument.application.as_ref().is_none_or(|application| {
                application
                    .arguments
                    .iter()
                    .all(|argument| argument_is_closed(program, argument))
            })
    }
    program
        .expression_table
        .static_machine_application_bindings(application)
        .iter()
        .all(|binding| match &binding.value {
            StaticMachineBindingValue::Type(reference)
            | StaticMachineBindingValue::Const(reference) => {
                reference_is_closed(program, *reference)
            }
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => {
                argument_is_closed(program, argument)
            }
            StaticMachineBindingValue::RuntimeValue { type_reference, .. } => {
                reference_is_closed(program, *type_reference)
            }
        })
}

/// Const specialization keys follow the original template's declared slots.
/// Runtime Value bindings keep their carrier identity; static anonymous values
/// land contextually and typed values retain their original meaning.
pub(super) fn const_binding_identities(
    program: &dyn validation::CheckingInput,
    template: &super::CandidateTemplate,
    types: &[Option<TypeReferenceHandle>],
    bindings: &[Option<TypeReferenceHandle>],
    runtime_values: &[Option<StaticMachineArgument>],
) -> Option<Vec<String>> {
    if template.type_parameters.len() != types.len()
        || template.const_parameters.len() != bindings.len()
        || bindings.len() != runtime_values.len()
    {
        return None;
    }
    let substitutions = template
        .type_parameters
        .iter()
        .zip(types)
        .map(|((symbol, _), actual)| Some((*symbol, (*actual)?)))
        .collect::<Option<Vec<_>>>()?;
    template
        .const_parameters
        .iter()
        .zip(bindings)
        .zip(runtime_values)
        .map(|(((_, _, carrier), actual), runtime)| {
            let actual = (*actual)?;
            if runtime.is_some() {
                return Some(program.normalized_type_identity_text(actual));
            }
            typed_trees::type_identity::declared_const_argument_identity(
                program.typed_program(),
                typed_trees::type_identity::IndexIdentityArgument::TypeReference(actual),
                *carrier,
                &substitutions,
            )
        })
        .collect()
}
