//! Which callback inputs can an expression use? Compute this on open typed
//! applications once per typing pass, not by exploring closed call graphs for
//! every instance. Concrete declaration dependencies are already in the node graph.
use super::PreparationDependencies;
use crate::declarations::resolved_source_file::ParameterSlot;
use crate::{CallUse, InvocationTarget, MachineApplication, ObligationKind, TypedCall, TypedStage, ValueType};
use omega_resolved::{DeclarationId, TreeNodeId};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Copy)]
enum Application<'a> {
    Call(&'a TypedCall),
    Selection(&'a MachineApplication),
}

impl<'a> Application<'a> {
    fn machine(self) -> &'a DeclarationId {
        match self {
            Self::Call(call) => &call.machine,
            Self::Selection(selection) => &selection.machine,
        }
    }

    fn argument(
        self,
        (owner, index): ParameterSlot<'_>,
    ) -> Option<&'a ValueType> {
        match self {
            Self::Call(call) => call.generic_argument(owner, index),
            Self::Selection(selection) if owner == selection.machine.as_ref() => selection.arguments.get(index),
            Self::Selection(selection) => selection
                .enclosing_arguments
                .iter()
                .find(|(id, _)| id == owner)?
                .1
                .get(index),
        }
    }
}

#[derive(Clone, Copy)]
enum User<'a> {
    Machine(&'a DeclarationId),
    Expression(TreeNodeId),
}

#[derive(Clone, Copy)]
enum Input<'a> {
    Slot(ParameterSlot<'a>),
    Application(Application<'a>),
}

impl<'a> Input<'a> {
    fn entry(target: &'a InvocationTarget) -> Option<Self> {
        match target {
            InvocationTarget::StaticParameter { owner, index } => Some(Self::Slot((owner, *index))),
            InvocationTarget::AppliedMachine(selection) => Some(Self::Application(Application::Selection(selection))),
            _ => None,
        }
    }

    fn argument(value: &'a ValueType) -> Option<Self> {
        match value {
            ValueType::Parameter { owner, index } => Some(Self::Slot((owner, *index))),
            ValueType::MachineSelection(selection) => Some(Self::Application(Application::Selection(selection))),
            _ => None,
        }
    }
}

#[derive(Default)]
pub(super) struct SelectedInputs<'a> {
    machines: HashMap<&'a DeclarationId, HashSet<ParameterSlot<'a>>>,
    expressions: BTreeMap<TreeNodeId, HashSet<ParameterSlot<'a>>>,
}

impl<'a> SelectedInputs<'a> {
    pub(super) fn collect(output: &'a TypedStage) -> Self {
        let mut pending = Vec::new();
        // Include declared and composed invocation rows (including imported
        // machines). Fact calls in contracts are separate from execution rows.
        for row in &output.machine_effects {
            let user = User::Machine(&row.machine);
            pending.extend(
                row.invokes
                    .iter()
                    .filter_map(Input::entry)
                    .map(|input| (user, input)),
            );
            pending.extend(
                row.callback_pause
                    .keys()
                    .map(|&i| (user, Input::Slot((&row.machine, i)))),
            );
        }
        collect_calls(output, true, &mut pending);
        if let Some(library) = output.library {
            collect_calls(library, false, &mut pending);
        }

        let mut used = Self::default();
        let mut callers = HashMap::<_, Vec<(User, Application)>>::new();
        let project = |user, app: Application<'a>, slot| Some((user, Input::argument(app.argument(slot)?)?));
        // Propagate each newly used formal back to its actuals. An actual may
        // itself be a selected application; only then subscribe to its formals.
        // A recursive body adds no unbounded closed applications: all edges
        // refer to finite templates, and a machine slot is announced once.
        while let Some((user, input)) = pending.pop() {
            match input {
                Input::Slot(slot) => match user {
                    User::Expression(node) => {
                        used.expressions.entry(node).or_default().insert(slot);
                    }
                    User::Machine(machine) => {
                        if used.machines.entry(machine).or_default().insert(slot)
                            && let Some(users) = callers.get(machine)
                        {
                            pending.extend(users.iter().filter_map(|&(user, app)| project(user, app, slot)));
                        }
                    }
                },
                Input::Application(app) => {
                    let machine = app.machine();
                    callers.entry(machine).or_default().push((user, app));
                    if let Some(slots) = used.machines.get(machine) {
                        pending.extend(slots.iter().filter_map(|&slot| project(user, app, slot)));
                    }
                }
            }
        }
        used
    }

    pub(super) fn expression_slots(
        &self,
        node: TreeNodeId,
        end: usize,
    ) -> impl Iterator<Item = ParameterSlot<'a>> + '_ {
        self.expressions
            .range(node..TreeNodeId { node: end, ..node })
            .flat_map(|(_, slots)| slots.iter().copied())
    }

    pub(super) fn ready(
        &self,
        value: &ValueType,
        dependencies: &PreparationDependencies<'_>,
    ) -> bool {
        let mut pending = vec![value];
        while let Some(value) = pending.pop() {
            if let ValueType::MachineSelection(selection) = value {
                if !dependencies.declaration_is_ready(&selection.machine) {
                    return false;
                }
                if let Some(slots) = self.machines.get(selection.machine.as_ref()) {
                    pending.extend(
                        slots
                            .iter()
                            .filter_map(|&slot| Application::Selection(selection).argument(slot)),
                    );
                }
            }
        }
        true
    }
}

fn collect_calls<'a>(
    stage: &'a TypedStage,
    local: bool,
    pending: &mut Vec<(User<'a>, Input<'a>)>,
) {
    let user = |node, caller: Option<&'a DeclarationId>, usage| match usage {
        // Library node numbers belong to a different forest.
        CallUse::StaticArgument => local.then_some(User::Expression(node)),
        CallUse::Execution | CallUse::Fact => caller.map(User::Machine),
    };
    for call in &stage.indirect_calls {
        if let Some(user) = user(call.node, call.caller.as_ref(), call.use_site)
            && let Some(input) = Input::entry(&call.entry)
        {
            pending.push((user, input));
        }
    }
    // Admitting a concrete callback refinement also needs the selected
    // application, even if the recipient doesn't invoke that formal.
    let mut refinements = HashMap::<_, Vec<_>>::new();
    for obligation in &stage.obligations {
        if let ObligationKind::RefineCallable {
            selection,
            formal: Some((owner, _)),
            ..
        } = &obligation.kind
        {
            refinements
                .entry((obligation.node, owner))
                .or_default()
                .push(selection);
        }
    }
    for call in &stage.calls {
        if let Some(user) = user(call.node, call.caller.as_ref(), call.use_site)
            && !(call.type_arguments.is_empty() && call.enclosing_arguments.is_empty())
        {
            pending.push((user, Input::Application(Application::Call(call))));
            if let Some(selections) = refinements.get(&(call.node, &call.machine)) {
                pending.extend(
                    selections
                        .iter()
                        .map(|selection| (user, Input::Application(Application::Selection(selection)))),
                );
            }
        }
    }
}
