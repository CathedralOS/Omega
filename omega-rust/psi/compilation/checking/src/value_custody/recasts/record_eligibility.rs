//! Fact-free value eligibility follows exact closed data applications.

use crate::value_custody::places::unique_data_definition_by_symbol;
use crate::value_custody::recasts::representation_types::exact_primitive_type;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

const MAX_RECAST_RECORD_GRAPH_NODES: usize = 4096;
const MAX_RECAST_RECORD_GRAPH_EDGES: usize = 16384;
pub(crate) const MAX_RECAST_PHANTOM_LIFETIME_SHELL_DEPTH: usize = 2;

/// A representation changes storage, never the semantic value it must establish.
/// Closed application views retain substituted field types; erased policy
/// evidence contributes no value fields or additional authority. Qualification
/// wrappers stay visible and cannot be discharged by arbitrary byte contents.
pub(crate) fn closed_fact_free_record_type_is_eligible(
    program: &dyn crate::CheckingInput,
    root: TypeReferenceHandle,
) -> bool {
    enum Work {
        Enter(TypeReferenceHandle),
        Exit(typed_trees::type_identity::TypeIdentityId),
    }
    let mut pending = vec![Work::Enter(root)];
    // One row per producer-owned identity: unseen, active, complete.
    let mut states = Vec::<u8>::new();
    let mut nodes = 0usize;
    let mut edges = 0usize;
    while let Some(work) = pending.pop() {
        let reference = match work {
            Work::Exit(identity) => {
                states[identity.storage_index().expect("entered identity")] = 2;
                continue;
            }
            Work::Enter(reference) => reference,
        };
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return false;
        }
        if matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Constrained { .. }
        ) {
            return false;
        }
        let identity = program.normalized_type_identity(reference);
        if !identity.is_valid() {
            return false;
        }
        let Some(index) = identity.storage_index() else {
            return false;
        };
        if states.len() <= index {
            states.resize(index + 1, 0);
        }
        match states[index] {
            2 => continue,
            1 => return false,
            _ => {}
        }
        nodes += 1;
        if nodes > MAX_RECAST_RECORD_GRAPH_NODES {
            return false;
        }
        states[index] = 1;
        pending.push(Work::Exit(identity));
        if let Some(storage) = program.layout_storage_arguments(reference) {
            pending.push(Work::Enter(storage.semantic_type));
            edges += 1;
        } else if let Some(primitive) = exact_primitive_type(program, reference) {
            if primitive == PrimitiveType::Bool || primitive.scalar_byte_size().is_none() {
                return false;
            }
        } else {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(_),
                } => {
                    pending.push(Work::Enter(*element_type));
                    edges += 1;
                }
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                    if program
                        .type_reference_table
                        .type_reference(reference)
                        .as_named_symbol()
                        .is_some_and(|symbol| program.symbols.builtin_type_atom(symbol).is_some())
                    {
                        return false;
                    }
                    let Some(data) = program.closed_data_definition(reference) else {
                        return false;
                    };
                    let members = program.data_members(data);
                    if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                        || data.quotient.is_some()
                        || !data.where_facts.is_empty()
                        || data.zero_gated
                        || typed_trees::data::DataDefinition::shape_kind_from_members(members)
                            != typed_trees::data::DataShapeKind::Record
                    {
                        return false;
                    }
                    for member in members.iter().rev() {
                        let Some(field) = member.as_field() else {
                            return false;
                        };
                        if field.relevance.is_erased() {
                            return false;
                        }
                        pending.push(Work::Enter(field.type_reference));
                        edges += 1;
                    }
                }
                _ => return false,
            }
        }
        if edges > MAX_RECAST_RECORD_GRAPH_EDGES {
            return false;
        }
    }
    true
}

/// Resolve one structurally valid erased-lifetime shell to its exact
/// synthesized runtime record symbol.
///
/// Generic-instance synthesis has already absorbed every Type/Const argument
/// into `base_symbol`; the remaining `Generic` node carries only checked
/// lifetime identity. Graph eligibility, nesting depth, cycles, and physical
/// non-emptiness are checked separately by the bounded recast traversal.
pub(crate) fn phantom_lifetime_record_symbol_shape(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    if !base_symbol.is_valid()
        || lifetime_arguments.is_empty()
        || !program
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty()
    {
        return None;
    }

    let instance = unique_data_definition_by_symbol(program, *base_symbol)?;
    let origin = instance.generic_instance?;
    let TypeReferenceNode::Generic {
        base_symbol: origin_base_symbol,
        lifetime_arguments: origin_lifetime_arguments,
        ..
    } = program.type_reference_table.type_reference(origin)
    else {
        return None;
    };
    let base = unique_data_definition_by_symbol(program, *origin_base_symbol)?;
    if !origin_lifetime_arguments.is_empty()
        || instance.lifetime_parameters.is_empty()
        || instance.lifetime_parameters.len() != lifetime_arguments.len()
        || instance.lifetime_parameters != base.lifetime_parameters
        || !program.data_type_parameters(instance).is_empty()
        || !closed_record_generic_origin_is_eligible(program, instance)
        || instance.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || instance.quotient.is_some()
        || !instance.where_facts.is_empty()
        || instance.zero_gated
    {
        return None;
    }

    let members = program.data_members(instance);
    if typed_trees::data::DataDefinition::shape_kind_from_members(members)
        != typed_trees::data::DataShapeKind::Record
    {
        return None;
    }
    Some(instance.symbol)
}

/// Resolve the direct erased-lifetime root admitted by the indexed-loan rung.
/// The graph walk may admit one further record shell below this root, but
/// arrays and a third lifetime shell remain fail closed.
pub(crate) fn direct_phantom_lifetime_record_symbol(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    let symbol = phantom_lifetime_record_symbol_shape(program, type_reference)?;
    closed_fact_free_record_type_is_eligible(program, type_reference).then_some(symbol)
}

pub(crate) fn direct_record_view_type_is_fact_free(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    closed_fact_free_record_type_is_eligible(program, type_reference)
}

fn closed_record_generic_origin_is_eligible(
    program: &dyn crate::CheckingInput,
    instance: &typed_trees::data::DataDefinition,
) -> bool {
    let Some(origin) = instance.generic_instance else {
        return true;
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(origin)
    else {
        return false;
    };
    if !base_symbol.is_valid() || *base_symbol == instance.symbol || !lifetime_arguments.is_empty()
    {
        return false;
    }
    let Some(base) = unique_data_definition_by_symbol(program, *base_symbol) else {
        return false;
    };
    let parameters = program.data_type_parameters(base);
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    base.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && base.generic_instance.is_none()
        && base.quotient.is_none()
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(base))
            == typed_trees::data::DataShapeKind::Record
        && !parameters.is_empty()
        && parameters.len() == arguments.len()
        && parameters
            .iter()
            .zip(arguments)
            .all(|(parameter, argument)| match parameter.kind {
                typed_trees::data::TypeParameterKind::Type => {
                    closed_raw_generic_type_argument_is_eligible(program, *argument)
                }
                typed_trees::data::TypeParameterKind::Const { type_reference }
                | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                    closed_raw_generic_const_argument_is_eligible(
                        program,
                        type_reference,
                        *argument,
                    )
                }
                typed_trees::data::TypeParameterKind::Machine { .. }
                | typed_trees::data::TypeParameterKind::Implementation
                | typed_trees::data::TypeParameterKind::Proposition { .. } => false,
            })
}

fn closed_raw_generic_const_argument_is_eligible(
    program: &dyn crate::CheckingInput,
    parameter_type: TypeReferenceHandle,
    argument: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        return false;
    };
    if symbol.is_valid() {
        return false;
    }
    if let Some(value) =
        language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
    {
        return crate::value_custody::type_references::validate_exact_typed_structured_const_argument(
            program,
            parameter_type,
            &value,
        )
        .is_ok();
    }
    let Some(primitive) = exact_primitive_type(program, parameter_type)
        .filter(|primitive| primitive.accepts_integer_literal())
    else {
        return false;
    };
    let Ok(value) = name.as_str().parse::<i128>() else {
        return false;
    };
    name.as_str() == value.to_string()
        && crate::value_custody::type_references::const_integer_value_fits_primitive(
            primitive, value,
        )
}

fn closed_raw_generic_type_argument_is_eligible(
    program: &dyn crate::CheckingInput,
    argument: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { symbol, .. } => {
            exact_primitive_type(program, argument).is_some()
                || (symbol.is_valid()
                    && program.data_definitions().iter().any(|data| {
                        data.symbol == *symbol
                            && data.lifetime_parameters.is_empty()
                            && program.data_type_parameters(data).is_empty()
                    }))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => *length > 0 && closed_raw_generic_type_argument_is_eligible(program, *element_type),
        _ => false,
    }
}
