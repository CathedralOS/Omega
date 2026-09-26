//! Fact-free record graph eligibility for recasts.

use crate::validation::value_custody::recasts::raw_byte_region::record_view_type_is_fact_free;
use crate::validation::value_custody::recasts::representation_types::exact_primitive_type;
use std::collections::HashSet;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::{
    PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};

pub(crate) fn closed_fact_free_record_symbol_is_eligible(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> bool {
    fact_free_record_graph_is_eligible(program, symbol, false)
}

const MAX_RECAST_RECORD_GRAPH_NODES: usize = 4096;

const MAX_RECAST_RECORD_GRAPH_EDGES: usize = 16384;

type RecordEligibilityContext = (u32, u32, usize, bool, bool);

#[derive(Clone, Copy)]
enum RecordEligibilityWork {
    Enter {
        symbol: symbols::SymbolHandle,
        lifetime_shell_depth: usize,
        entered_through_lifetime_shell: bool,
        allow_lifetime_shell: bool,
    },
    Type {
        type_reference: TypeReferenceHandle,
        lifetime_shell_depth: usize,
        allow_lifetime_shell: bool,
    },
    Exit {
        symbol: symbols::SymbolHandle,
        context: RecordEligibilityContext,
    },
}

pub(crate) const MAX_RECAST_PHANTOM_LIFETIME_SHELL_DEPTH: usize = 2;

/// Bounded, memoized exact-symbol walk for recursively fact-free records.
/// Explicit exit markers detect cycles without host recursion, while completed
/// symbols make shared diamonds linear in the distinct schema graph.
fn fact_free_record_graph_is_eligible(
    program: &TypedTrees,
    root: symbols::SymbolHandle,
    permit_root_lifetime_parameters: bool,
) -> bool {
    if !root.is_valid() {
        return false;
    }

    let Some(work_capacity) =
        MAX_RECAST_RECORD_GRAPH_EDGES.checked_add(MAX_RECAST_RECORD_GRAPH_NODES)
    else {
        return false;
    };
    let mut work = Vec::new();
    if work.try_reserve_exact(work_capacity).is_err() {
        return false;
    }
    let mut visiting = HashSet::new();
    if visiting.try_reserve(MAX_RECAST_RECORD_GRAPH_NODES).is_err() {
        return false;
    }
    let mut complete = HashSet::new();
    if complete.try_reserve(MAX_RECAST_RECORD_GRAPH_NODES).is_err() {
        return false;
    }
    work.push(RecordEligibilityWork::Enter {
        symbol: root,
        lifetime_shell_depth: usize::from(permit_root_lifetime_parameters),
        entered_through_lifetime_shell: permit_root_lifetime_parameters,
        allow_lifetime_shell: true,
    });
    let mut node_count = 0usize;
    let mut edge_count = 0usize;

    while let Some(next) = work.pop() {
        match next {
            RecordEligibilityWork::Enter {
                symbol,
                lifetime_shell_depth,
                entered_through_lifetime_shell,
                allow_lifetime_shell,
            } => {
                if !symbol.is_valid() {
                    return false;
                }
                let identity = (symbol.arena_index(), symbol.generation());
                let context = (
                    identity.0,
                    identity.1,
                    lifetime_shell_depth,
                    entered_through_lifetime_shell,
                    allow_lifetime_shell,
                );
                if complete.contains(&context) {
                    continue;
                }
                if !visiting.insert(identity) {
                    return false;
                }
                node_count = match node_count.checked_add(1) {
                    Some(count) if count <= MAX_RECAST_RECORD_GRAPH_NODES => count,
                    _ => return false,
                };
                let Some(data) = unique_data_definition_by_symbol(program, symbol) else {
                    return false;
                };
                let members = program.data_members(data);
                if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                    || (!entered_through_lifetime_shell && !data.lifetime_parameters.is_empty())
                    || !program.data_type_parameters(data).is_empty()
                    || !closed_record_generic_origin_is_eligible(program, data)
                    || data.quotient.is_some()
                    || !data.where_facts.is_empty()
                    || data.zero_gated
                    || symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition::shape_kind_from_members(members)
                        != symbol_resolved_trees_to_typed_trees::typed_trees::data::DataShapeKind::Record
                {
                    return false;
                }
                work.push(RecordEligibilityWork::Exit { symbol, context });
                for member in members.iter().rev() {
                    let symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Field(
                        field,
                    ) = member
                    else {
                        return false;
                    };
                    if field.relevance.is_erased() {
                        return false;
                    }
                    edge_count = match edge_count.checked_add(1) {
                        Some(count) if count <= MAX_RECAST_RECORD_GRAPH_EDGES => count,
                        _ => return false,
                    };
                    work.push(RecordEligibilityWork::Type {
                        type_reference: field.type_reference,
                        lifetime_shell_depth,
                        allow_lifetime_shell,
                    });
                }
            }
            RecordEligibilityWork::Type {
                type_reference,
                lifetime_shell_depth,
                allow_lifetime_shell,
            } => {
                if let Some(primitive) = exact_primitive_type(program, type_reference) {
                    if primitive == PrimitiveType::Bool || primitive.scalar_byte_size().is_none() {
                        return false;
                    }
                    continue;
                }
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Named { symbol, .. } => {
                        edge_count = match edge_count.checked_add(1) {
                            Some(count) if count <= MAX_RECAST_RECORD_GRAPH_EDGES => count,
                            _ => return false,
                        };
                        work.push(RecordEligibilityWork::Enter {
                            symbol: *symbol,
                            lifetime_shell_depth,
                            entered_through_lifetime_shell: false,
                            allow_lifetime_shell,
                        });
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: symbol_resolved_trees_to_typed_trees::typed_trees::types::FixedArrayLength::Literal(_),
                    } => {
                        edge_count = match edge_count.checked_add(1) {
                            Some(count) if count <= MAX_RECAST_RECORD_GRAPH_EDGES => count,
                            _ => return false,
                        };
                        work.push(RecordEligibilityWork::Type {
                            type_reference: *element_type,
                            lifetime_shell_depth,
                            allow_lifetime_shell: false,
                        });
                    }
                    TypeReferenceNode::Generic { .. }
                        if allow_lifetime_shell
                            && lifetime_shell_depth > 0
                            && lifetime_shell_depth < MAX_RECAST_PHANTOM_LIFETIME_SHELL_DEPTH =>
                    {
                        let Some(symbol) =
                            phantom_lifetime_record_symbol_shape(program, type_reference)
                        else {
                            return false;
                        };
                        edge_count = match edge_count.checked_add(1) {
                            Some(count) if count <= MAX_RECAST_RECORD_GRAPH_EDGES => count,
                            _ => return false,
                        };
                        work.push(RecordEligibilityWork::Enter {
                            symbol,
                            lifetime_shell_depth: lifetime_shell_depth + 1,
                            entered_through_lifetime_shell: true,
                            allow_lifetime_shell,
                        });
                    }
                    _ => return false,
                }
            }
            RecordEligibilityWork::Exit { symbol, context } => {
                let identity = (symbol.arena_index(), symbol.generation());
                if !visiting.remove(&identity) || !complete.insert(context) {
                    return false;
                }
            }
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
    program: &TypedTrees,
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
    if symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition::shape_kind_from_members(members)
        != symbol_resolved_trees_to_typed_trees::typed_trees::data::DataShapeKind::Record
    {
        return None;
    }
    Some(instance.symbol)
}

/// Resolve the direct erased-lifetime root admitted by the indexed-loan rung.
/// The graph walk may admit one further record shell below this root, but
/// arrays and a third lifetime shell remain fail closed.
pub(crate) fn direct_phantom_lifetime_record_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    let symbol = phantom_lifetime_record_symbol_shape(program, type_reference)?;
    fact_free_record_graph_is_eligible(program, symbol, true).then_some(symbol)
}

pub(crate) fn direct_record_view_type_is_fact_free(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    direct_phantom_lifetime_record_symbol(program, type_reference).is_some()
        || record_view_type_is_fact_free(program, type_reference, &mut HashSet::new())
}

fn unique_data_definition_by_symbol(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<&symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition> {
    if !symbol.is_valid() {
        return None;
    }
    let mut matches = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == symbol);
    let definition = matches.next()?;
    matches.next().is_none().then_some(definition)
}

fn closed_record_generic_origin_is_eligible(
    program: &TypedTrees,
    instance: &symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition,
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
        && symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(base))
            == symbol_resolved_trees_to_typed_trees::typed_trees::data::DataShapeKind::Record
        && !parameters.is_empty()
        && parameters.len() == arguments.len()
        && parameters
            .iter()
            .zip(arguments)
            .all(|(parameter, argument)| match parameter.kind {
                symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind::Type => {
                    closed_raw_generic_type_argument_is_eligible(program, *argument)
                }
                symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind::Const { type_reference }
                | symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind::Value { type_reference } => {
                    closed_raw_generic_const_argument_is_eligible(
                        program,
                        type_reference,
                        *argument,
                    )
                }
                symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind::Machine { .. }
                | symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind::Proposition { .. } => false,
            })
}

fn closed_raw_generic_const_argument_is_eligible(
    program: &TypedTrees,
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
        return crate::validation::value_custody::type_references::validate_exact_typed_structured_const_argument(
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
        && crate::validation::value_custody::type_references::const_integer_value_fits_primitive(
            primitive, value,
        )
}

fn closed_raw_generic_type_argument_is_eligible(
    program: &TypedTrees,
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
            length:
                symbol_resolved_trees_to_typed_trees::typed_trees::types::FixedArrayLength::Literal(
                    length,
                ),
        } => *length > 0 && closed_raw_generic_type_argument_is_eligible(program, *element_type),
        _ => false,
    }
}
