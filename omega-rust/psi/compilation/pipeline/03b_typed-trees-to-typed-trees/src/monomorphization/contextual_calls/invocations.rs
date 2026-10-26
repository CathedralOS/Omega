//! Obtain the expected invocation telescope and its known caller operands.
//! These proposals authorize no selection: providers must close separately,
//! and ordinary checking still admits every carrier, bound and contract.

use super::super::{
    CallSite, ExpressionNode, StaticMachineArgument, SymbolHandle, TypeReferenceHandle, TypedTrees,
};
use symbols::SymbolKind;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::signature::StateSignature;
mod operands;

pub(super) struct Invocation {
    pub site: CallSite,
    pub owner: SymbolHandle,
    pub target: SymbolHandle,
    pub machine_arguments: Vec<StaticMachineArgument>,
    pub requirements: Vec<MachineDemand>,
    pub types: Vec<(SymbolHandle, TypeReferenceHandle)>,
    pub use_site: InvocationUse,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum InvocationUse {
    Executable,
    RetainedEvaluation,
}

pub(super) enum InvocationScope {
    State {
        machine: typed_trees::machine::Machine,
        state: typed_trees::state::State,
    },
    Declaration(SymbolHandle),
}

impl InvocationScope {
    fn owner(&self) -> SymbolHandle {
        match self {
            Self::State { machine, .. } => machine.symbol,
            Self::Declaration(owner) => *owner,
        }
    }

    fn use_site(&self) -> InvocationUse {
        match self {
            Self::State { .. } => InvocationUse::Executable,
            Self::Declaration(_) => InvocationUse::RetainedEvaluation,
        }
    }

    pub(super) fn result_type(
        &self,
        program: &dyn validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        match self {
            Self::State { machine, state } => {
                validation::expression_result_type_reference(program, machine, state, expression)
            }
            Self::Declaration(owner) => validation::parameter_expression_result_type_reference(
                program,
                *owner,
                &[],
                expression,
            ),
        }
    }

    pub(super) fn has_open_parameters(&self, program: &TypedTrees) -> bool {
        let mut owner = self.owner();
        while owner.is_valid() {
            if let Some(machine) = program.machine_by_symbol(owner) {
                return program.callable_body_has_open_parameters(machine);
            }
            owner = program.symbols.get(owner).parent;
        }
        false
    }
}

pub(super) struct MachineDemand {
    pub parameter: SymbolHandle,
    pub signature: StateSignature,
}

pub(super) fn machine_demands(
    program: &TypedTrees,
    parameters: &[TypeParameter],
) -> Vec<MachineDemand> {
    parameters
        .iter()
        .filter_map(|parameter| {
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                return None;
            };
            program
                .machine_parameter_contract_view(contract)
                .map(|view| MachineDemand {
                    parameter: parameter.symbol,
                    signature: view.signature().clone(),
                })
        })
        .collect()
}

pub(super) fn is_machine(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::Machine | SymbolKind::State | SymbolKind::MachineParameter
        )
}

pub(super) fn collect(program: &mut TypedTrees) -> Result<Vec<Invocation>, Vec<super::Diagnostic>> {
    let mut pending = Vec::new();
    let mut scopes = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let scope = scopes.len();
            scopes.push(InvocationScope::State {
                machine: machine.clone(),
                state: state.clone(),
            });
            for (statement_handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                if let Some(call) = statement.as_call() {
                    pending.push((
                        scope,
                        CallSite::Statement(statement_handle),
                        call.target_symbol,
                        call.machine_arguments.to_vec(),
                        program
                            .statement_table
                            .expression_handles(call.arguments)
                            .to_vec(),
                    ));
                }
                let mut expressions = Vec::new();
                validation::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for expression in expressions {
                    let site = CallSite::Expression(expression);
                    let ExpressionNode::Call(call) =
                        program.expression_table.expression(expression)
                    else {
                        continue;
                    };
                    pending.push((
                        scope,
                        site,
                        call.target_symbol,
                        call.machine_arguments.to_vec(),
                        program
                            .expression_table
                            .expression_handles(call.arguments)
                            .to_vec(),
                    ));
                }
            }
        }
    }
    // Constant evaluation has already consumed these recipes in its private
    // executable graph. Final checking still owns every source application,
    // including an explicitly applied provider. Retain its checking selection
    // without rewriting the detached arguments needed by independent replay.
    for declaration in program.const_declarations() {
        let scope = scopes.len();
        scopes.push(InvocationScope::Declaration(declaration.symbol));
        let mut expressions = Vec::new();
        for root in [
            declaration.authored_initializer,
            declaration.materialized_initializer,
        ] {
            if program.expression_table.expression_is_valid(root) {
                validation::collect_expression_tree(program, root, &mut expressions);
            }
        }
        for expression in expressions {
            let Some(call) = program.expression_table.call_node(expression) else {
                continue;
            };
            pending.push((
                scope,
                CallSite::Expression(expression),
                call.target_symbol,
                call.machine_arguments.to_vec(),
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec(),
            ));
        }
    }
    let mut invocations = Vec::new();
    let mut diagnostics = Vec::new();
    for (scope, site, target, selected, arguments) in pending {
        if invocations
            .iter()
            .any(|invocation: &Invocation| invocation.site == site)
        {
            continue;
        }
        if let Some(invocation) = proposal(
            program,
            &scopes[scope],
            site,
            target,
            &selected,
            &arguments,
            &mut diagnostics,
        ) {
            invocations.push(invocation);
        }
    }
    if diagnostics.is_empty() {
        Ok(invocations)
    } else {
        Err(diagnostics)
    }
}

fn proposal(
    program: &mut TypedTrees,
    scope: &InvocationScope,
    site: CallSite,
    selected_target: SymbolHandle,
    selected: &[StaticMachineArgument],
    arguments: &[typed_trees::expression::ExpressionHandle],
    diagnostics: &mut Vec<super::Diagnostic>,
) -> Option<Invocation> {
    // Executable retargeting can name a provider's private telescope and erase
    // its static roster. Checking still admits the occurrence's authored
    // requirement. Derive its views from that same retained declaration and
    // operands, while reusing the occurrence's current selected entries.
    let application = site.authored_machine_application(program)?;
    let (target, mut selected, selected_machines) = if application.is_valid() {
        (
            program
                .expression_table
                .static_machine_application(application)
                .registration_operation,
            program
                .expression_table
                .static_machine_application_arguments(application)
                .to_vec(),
            program
                .expression_table
                .static_machine_application_selected_machines(application)
                .to_vec(),
        )
    } else {
        (selected_target, selected.to_vec(), Vec::new())
    };
    let retained_owner = if application.is_valid() {
        program
            .expression_table
            .static_machine_application(application)
            .lexical_owner
    } else {
        SymbolHandle::invalid()
    };
    let owner = if retained_owner.is_valid() {
        retained_owner
    } else {
        scope.owner()
    };
    if let Err(message) =
        super::super::body_rewriting::normalize_const_arguments(program, owner, &mut selected)
    {
        diagnostics.push(super::Diagnostic::error(message));
        return None;
    }
    if !selected
        .iter()
        .any(|argument| is_machine(program, argument.symbol))
    {
        return None;
    }
    let (parameters, runtime): (&[TypeParameter], Vec<TypeReferenceHandle>) = if let Some(machine) =
        program
            .machines()
            .iter()
            .find(|machine| program.machine_or_state_symbol(machine, target))
    {
        let entry = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target)
            .or_else(|| {
                (machine.symbol == target)
                    .then(|| program.machine_states(machine).first())
                    .flatten()
            })?;
        (
            program.machine_type_parameters(machine),
            program
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect(),
        )
    } else {
        // Registration retains the authored template. Admission retains the
        // exact closed contract selected by this occurrence; its copied binders
        // can have the same source symbols and different outer substitutions.
        let signature =
            if let Some(application) = program.closed_requirement_application(selected_target) {
                if application.template != target {
                    return None;
                }
                application.signature
            } else {
                program
                    .machine_parameter_signature(target)
                    .map(|(_, signature)| signature)
                    .or_else(|| {
                        program
                            .traits()
                            .iter()
                            .flat_map(|definition| program.trait_machine_signatures(definition))
                            .find(|signature| signature.symbol == target)
                    })?
            };
        (
            program.requirement_invocation_contract_parameters(signature),
            program
                .state_signature_parameters(signature)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect(),
        )
    };
    let machine_arguments = if selected_machines.is_empty() {
        selected
            .iter()
            .filter(|argument| is_machine(program, argument.symbol))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        selected_machines
    };
    let parameters = parameters.to_vec();
    let requirements = machine_demands(program, &parameters);
    if requirements.len() != machine_arguments.len() {
        return None;
    }
    let types = operands::derive(
        program,
        scope,
        site,
        &parameters,
        &selected,
        &runtime,
        arguments,
    )?;
    Some(Invocation {
        site,
        owner: scope.owner(),
        target,
        machine_arguments,
        requirements,
        types,
        use_site: scope.use_site(),
    })
}
