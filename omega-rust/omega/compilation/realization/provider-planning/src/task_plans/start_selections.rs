//! Task start selections and activation targets.

use crate::{fnv1a::Fnv1a, task_plans::entry_report_fingerprint};
use checked_trees::typed_trees::static_applications::StaticMachineBindingValue;
use diagnostics::Diagnostic;
use symbols::JoinedPath;
use task_plans::TaskStartOperation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TaskStartSelection {
    pub(crate) requirement_owner: symbols::SymbolHandle,
    pub(crate) requirement: symbols::SymbolHandle,
    pub(crate) target_machine: symbols::SymbolHandle,
    pub(crate) target_entry: symbols::SymbolHandle,
    pub(crate) report_fingerprint: u64,
    pub(crate) operation: TaskStartOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskActivationTargetError {
    MissingEntry,
    AmbiguousEntry,
    AmbiguousMachine,
    MachineMismatch,
}

impl TaskActivationTargetError {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::MissingEntry => "target entry must name one exact typed state",
            Self::AmbiguousEntry => "target entry must resolve to exactly one typed state",
            Self::AmbiguousMachine => "target owner must resolve to exactly one typed machine",
            Self::MachineMismatch => "target entry must belong to its retained exact machine",
        }
    }
}

pub(crate) fn unique_task_activation_target<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    target_entry: symbols::SymbolHandle,
) -> Result<
    (
        &'program checked_trees::machine::Machine,
        &'program checked_trees::state::State,
    ),
    TaskActivationTargetError,
> {
    let mut matches = typed.machine_states_named(target_entry);
    let (machine, entry) = matches
        .next()
        .ok_or(TaskActivationTargetError::MissingEntry)?;
    if matches.next().is_some() {
        return Err(TaskActivationTargetError::AmbiguousEntry);
    }
    let mut owners = typed
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine.symbol);
    owners
        .next()
        .ok_or(TaskActivationTargetError::MissingEntry)?;
    if owners.next().is_some() {
        return Err(TaskActivationTargetError::AmbiguousMachine);
    }
    Ok((machine, entry))
}

pub(crate) fn exact_task_activation_target<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    target_machine: symbols::SymbolHandle,
    target_entry: symbols::SymbolHandle,
) -> Result<
    (
        &'program checked_trees::machine::Machine,
        &'program checked_trees::state::State,
    ),
    TaskActivationTargetError,
> {
    let (machine, entry) = unique_task_activation_target(typed, target_entry)?;
    if machine.symbol != target_machine {
        return Err(TaskActivationTargetError::MachineMismatch);
    }
    Ok((machine, entry))
}

pub(crate) fn task_start_selections(
    typed: &checked_trees::typed_trees::TypedTrees,
) -> Result<Vec<TaskStartSelection>, Vec<Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    for (_, expression) in typed.expression_table.iter_expressions() {
        let checked_trees::expression::ExpressionNode::Call(call) = expression else {
            continue;
        };
        append_task_start_selection(
            typed,
            call.target_symbol,
            call.target.as_str(),
            &mut selections,
            &mut diagnostics,
        );
    }
    for machine in typed.machines() {
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                let checked_trees::statement::StatementNode::Call(call) = statement else {
                    continue;
                };
                append_task_start_selection(
                    typed,
                    call.target_symbol,
                    call.target.as_str(),
                    &mut selections,
                    &mut diagnostics,
                );
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    selections.sort_by_key(|selection| selection.report_fingerprint);
    selections.dedup();
    Ok(selections)
}

fn append_task_start_selection(
    typed: &checked_trees::typed_trees::TypedTrees,
    requirement: symbols::SymbolHandle,
    target_name: &str,
    selections: &mut Vec<TaskStartSelection>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((definition, signature, operation)) = typed.traits().iter().find_map(|definition| {
        if !definition.is_boundary
            || definition.name.as_str() != "TaskRuntime"
            || !typed
                .symbol_toolchain_relative_path(definition.symbol)
                .is_some_and(|path| path == std::path::Path::new("task.omg"))
        {
            return None;
        }
        let signature = typed.trait_machine_signature_by_symbol(definition, requirement)?;
        let operation = match signature.name.as_str() {
            "start" => TaskStartOperation::Start,
            "try_start" => TaskStartOperation::TryStart,
            _ => return None,
        };
        Some((definition, signature, operation))
    }) else {
        return;
    };
    // The closed requirement retains the complete typed application even
    // after specialization erases its executable telescope. Machine bindings
    // select execution; type and value bindings keep their own identities.
    let Some(application) = typed.closed_requirement_application(requirement) else {
        diagnostics.push(Diagnostic::error(format!(
            "TaskRuntime requirement `{target_name}` has no retained checked static application"
        )));
        return;
    };
    let targets = typed
        .expression_table
        .static_machine_application_bindings(application.application)
        .iter()
        .filter_map(|binding| match &binding.value {
            StaticMachineBindingValue::Machine(target) => Some(target),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [target] = targets.as_slice() else {
        diagnostics.push(Diagnostic::error(format!(
            "TaskRuntime requirement `{target_name}` must select exactly one static target machine, got {}",
            targets.len()
        )));
        return;
    };
    let (target_machine, target_entry) = match unique_task_activation_target(typed, target.symbol) {
        Ok(target) => target,
        Err(TaskActivationTargetError::MissingEntry)
            if target.symbol.is_valid()
                && typed
                    .machines()
                    .iter()
                    .flat_map(|machine| typed.machine_type_parameters(machine))
                    .any(|parameter| parameter.symbol == target.symbol) =>
        {
            // A generic wrapper may forward its own machine parameter. Its
            // cloned concrete specialization contributes the eventual row.
            return;
        }
        Err(TaskActivationTargetError::MissingEntry) => {
            diagnostics.push(Diagnostic::error(format!(
                "TaskRuntime requirement `{target_name}` selects an unresolved static target `{}`",
                target.path.joined_path()
            )));
            return;
        }
        Err(error) => {
            diagnostics.push(Diagnostic::error(format!(
                "TaskRuntime requirement `{target_name}` selects an invalid static target coordinate: {}",
                error.message()
            )));
            return;
        }
    };
    let mut hash = Fnv1a::new();
    hash.string_terminated("task-runtime-requirement-specialization-v1");
    hash.string_terminated(
        typed
            .trait_requirement_callable_identity_text(definition, signature)
            .as_str(),
    );
    hash.u64(entry_report_fingerprint(
        typed,
        target_machine,
        target_entry,
    ));
    let selection = TaskStartSelection {
        requirement_owner: definition.symbol,
        requirement,
        target_machine: target_machine.symbol,
        target_entry: target_entry.symbol,
        report_fingerprint: hash.finish_or(1),
        operation,
    };
    if !selections.contains(&selection) {
        selections.push(selection);
    }
}
