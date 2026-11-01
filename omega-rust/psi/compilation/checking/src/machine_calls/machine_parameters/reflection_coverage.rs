//! An open reflection subject retains a per-field coverage obligation. It is
//! not an ordinary promise that its callback accepts every type in the world.
//! Closed elaboration checks each concrete application as an ordinary call.
use super::ValidatedNominalMachineUseSite;
use typed_trees::expression::{ExpressionNode, StaticMachineArgument};
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

pub(super) fn is_open_field_query(
    program: &dyn crate::CheckingInput,
    site: Option<ValidatedNominalMachineUseSite>,
) -> bool {
    let Some(ValidatedNominalMachineUseSite::Statement(statement)) = site else {
        return false;
    };
    let Some(call) = program.statement_table.statement(statement).as_call() else {
        return false;
    };
    if !call.target_symbol.is_valid() {
        return false;
    }
    let symbol = program.symbols.get(call.target_symbol);
    let declaration = if symbol.kind == symbols::SymbolKind::State {
        symbol.parent
    } else {
        call.target_symbol
    };
    let Some(source) = program
        .symbols
        .symbol_source_span(declaration)
        .and_then(|span| program.symbols.source_file(span))
    else {
        return false;
    };
    if source.origin != source::SourceOrigin::Toolchain
        || source.path.strip_prefix(&source.package_root).ok()
            != Some(std::path::Path::new("reflect.omg"))
        || !matches!(
            program.symbols.name(declaration),
            "visit_runtime_fields"
                | "visit_runtime_fields_mut"
                | "inspect_record_fields"
                | "visit_active_fields"
                | "visit_active_fields_mut"
        )
    {
        return false;
    }
    let Some(subject) = call.machine_arguments.first() else {
        return false;
    };
    has_open_operand(program, subject)
}
fn has_open_operand(program: &dyn crate::CheckingInput, subject: &StaticMachineArgument) -> bool {
    let is_parameter = |symbol: symbols::SymbolHandle| {
        symbol.is_valid() && program.symbols.get(symbol).kind == symbols::SymbolKind::TypeParameter
    };
    let mut operands = vec![subject];
    let mut types = Vec::new();
    let mut expressions = Vec::new();
    while let Some(argument) = operands.pop() {
        if is_parameter(argument.symbol) {
            return true;
        }
        if argument.type_reference.is_valid() {
            types.push(argument.type_reference);
        }
        if let Some(application) = &argument.application {
            operands.extend(application.arguments.iter());
        }
    }
    let mut visited = Vec::new();
    while let Some(reference) = types.pop() {
        if visited.contains(&reference) {
            continue;
        }
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } if is_parameter(*symbol) => return true,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                if is_parameter(*base_symbol) {
                    return true;
                }
                types.extend_from_slice(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                );
            }
            TypeReferenceNode::Reference { referee, .. } => types.push(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => types.push(*base_type),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                types.push(*element_type);
                match length {
                    FixedArrayLength::ConstParameter { symbol, .. } if is_parameter(*symbol) => {
                        return true;
                    }
                    FixedArrayLength::Expression(reference) => types.push(*reference),
                    _ => {}
                }
            }
            TypeReferenceNode::Slice { element_type } => types.push(*element_type),
            TypeReferenceNode::ConstExpression(expression) => expressions.push(*expression),
            _ => {}
        }
    }
    let mut visited = Vec::new();
    while let Some(expression) = expressions.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        if matches!(node, ExpressionNode::Name(name) if is_parameter(name.symbol)) {
            return true;
        }
        crate::expression_traversal::executable_children(program, node, |child| {
            expressions.push(child)
        });
    }
    false
}
