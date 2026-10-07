//! Diagnostic labels: the domain, point, carrier, and family names
//! index-compatibility diagnostics print.

use super::instance_collection::IndexedInstance;
use facts::ProgramPoint;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn carrier_label(program: &TypedTrees, target_type: TypeReferenceHandle) -> String {
    program.display_type_reference(target_type)
}

/// The family spelling inside an instance label: `Resident<SlotPlacement, Slot>`
/// names family `Resident`, so a diagnostic can name the family whose
/// establishment is missing beside the exact instance that was declared.
pub(super) fn family_label(instance_label: &str) -> &str {
    instance_label
        .split_once('<')
        .map_or(instance_label, |(family, _)| family)
}

/// Whether the declared family carries no predicate body. An unresolved symbol
pub(super) fn compatibility_name(
    program: &TypedTrees,
    actual: &IndexedInstance,
    expected: &IndexedInstance,
    point: ProgramPoint,
) -> String {
    format!(
        "index-equality:{}:{}:{}=={}",
        program.symbols.display_path(actual.family, "::"),
        point_label(program, point),
        actual.label,
        expected.label,
    )
}

pub(super) fn domain_label(
    program: &TypedTrees,
    name: &str,
    arguments: &[TypeReferenceHandle],
) -> String {
    if arguments.is_empty() {
        return name.to_owned();
    }
    let arguments = arguments
        .iter()
        .map(|argument| index_argument_label(program, *argument))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}

pub(super) fn qualification_label(
    program: &TypedTrees,
    cast: &typed_trees::expression::TableCastExpression,
) -> String {
    let name = program
        .expression_table
        .name_path_members(cast.semantic_domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    domain_label(
        program,
        &name,
        program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments),
    )
}

pub(super) fn index_argument_label(program: &TypedTrees, argument: TypeReferenceHandle) -> String {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { name, .. } => {
            language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                .map_or_else(|| name.to_string(), |value| value.display)
        }
        TypeReferenceNode::ConstExpression(expression) => {
            program.expression_table.display_name(*expression)
        }
        _ => program.display_type_reference(argument),
    }
}

pub(super) fn point_label(program: &TypedTrees, point: ProgramPoint) -> String {
    let symbol = |symbol| {
        let path = program.symbols.display_path(symbol, "::");
        if path.is_empty() {
            format!("symbol-{}", symbol.arena_index())
        } else {
            path
        }
    };
    match point {
        ProgramPoint::Global => "global".to_owned(),
        ProgramPoint::Definition { symbol: definition } => symbol(definition),
        ProgramPoint::Machine { machine_symbol } => symbol(machine_symbol),
        ProgramPoint::State { state_symbol, .. } => symbol(state_symbol),
        ProgramPoint::Call {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallRequires {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-requires-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallEnsures {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-ensures-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::Exit {
            state_symbol,
            statement_index,
            ..
        } => format!("{}:exit-{statement_index}", symbol(state_symbol)),
        ProgramPoint::TransitionArm {
            state_symbol,
            statement_index,
            transition_target,
            ..
        } => format!(
            "{}:arm-{statement_index}-{}-{}",
            symbol(state_symbol),
            transition_target.arena_index(),
            transition_target.generation(),
        ),
        ProgramPoint::Statement {
            state_symbol,
            statement_index,
            ..
        } => format!("{}:statement-{statement_index}", symbol(state_symbol)),
    }
}
