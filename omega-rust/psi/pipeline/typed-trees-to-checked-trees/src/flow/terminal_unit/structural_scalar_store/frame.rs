//! Reconcile assignments and instantiated call writes with the complete frame.

use super::*;

pub(super) fn matches(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameter: SymbolHandle,
    is_self: bool,
    mutation_root: &str,
    frame: &facts::NormalizedWriteFrame,
) -> bool {
    let Some(paths) = frame.complete_paths() else {
        return false;
    };
    let Some(resolver) = validation::CallFrameResolver::new(program) else {
        return false;
    };
    let Some(source) = program
        .state_parameters(state)
        .iter()
        .find(|source| source.symbol == parameter)
    else {
        return false;
    };
    let source_root = if is_self {
        "self"
    } else {
        source.name.as_str()
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut call_paths = Vec::new();
    for statement in statements {
        let writes = match statement {
            StatementNode::Call(call) => resolver.may_write_frame(machine, call),
            StatementNode::Expression(expression) => {
                resolver.expression_write_frame(machine, *expression)
            }
            _ => continue,
        };
        let Some(writes) = writes.complete_paths() else {
            return false;
        };
        for written in writes {
            let Some(suffix) = written.strip_prefix(source_root) else {
                return false;
            };
            if !suffix.is_empty() && !suffix.starts_with('.') && !suffix.starts_with('[') {
                return false;
            }
            let path = format!("{mutation_root}{suffix}");
            if !paths.contains(&path) {
                return false;
            }
            call_paths.push(path);
        }
    }
    // Shared destinations may be written by both a call and an assignment.
    // Compare their union, retaining every authored assignment in its schedule.
    assignment_frame_matches(program, state, parameter, mutation_root, frame, &call_paths)
}
