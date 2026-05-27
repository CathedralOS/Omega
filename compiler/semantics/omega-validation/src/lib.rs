mod symbols;

use crate::symbols::{MachineSymbols, TopLevelSymbols};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::{SymbolHandle, SymbolKind};
use omega_facts::{FactPlan, ProgramPoint};
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataMember, DataShapeKind};
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::signature::{SignatureContract, StateParameter, StateSignature};
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableCall, TransitionTargetHandle, TransitionTargetNode,
};
use omega_typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::fmt;

pub fn validate_program(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    let fact_plan = omega_facts::build_definition_fact_plan(program);

    validate_domain_definitions(program, &symbols, &fact_plan, &mut diagnostics);
    validate_invariant_definitions(program, &fact_plan, &mut diagnostics);
    validate_callable_state_signatures(program, &symbols, &mut diagnostics);
    validate_trait_requirements(program, &mut diagnostics);
    validate_data_field_types(program, &symbols, &mut diagnostics);
    validate_operator_declarations(program, &mut diagnostics);
    validate_entry_point(program, &mut diagnostics);

    for machine in program.machines() {
        let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);

        validate_contained_types(program, machine, &symbols, &mut diagnostics);
        validate_owned_data(program, machine, &symbols, &mut diagnostics);
        validate_machine_effects(program, machine, &mut diagnostics);
        validate_machine_contracts(program, machine, &mut diagnostics);
        validate_machine_trait_conformances(program, machine, &mut diagnostics);

        for state in program.machine_states(machine) {
            validate_local_data_names(
                program.statement_table.statements(state.statement_nodes),
                &machine_symbols,
                program.state_parameters(state),
                machine.name.as_str(),
                state.name.as_str(),
                &mut diagnostics,
            );
            let writable_roots = WritableRoots {
                machine_symbols: &machine_symbols,
                statements: program.statement_table.statements(state.statement_nodes),
                parameters: program.state_parameters(state),
            };

            for statement in program.statement_table.statements(state.statement_nodes) {
                validate_state_statement_node(
                    program,
                    machine,
                    &state.name,
                    &machine_symbols,
                    &symbols,
                    &writable_roots,
                    statement,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub fn validate_effect_plan(
    program: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for machine_effects in effect_plan.machines() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_effects.symbol)
        else {
            continue;
        };

        let declared_effects = declared_machine_effect_set(program, machine);
        if declared_effects.is_empty() {
            continue;
        }

        if !declared_effects.contains_all(machine_effects.transitive) {
            let missing = machine_effects.transitive.difference(declared_effects);
            let mut message = format!(
                "machine `{}` declares effects `{}` but reaches undeclared effects `{}`",
                machine.name,
                format_effect_set(declared_effects),
                format_effect_set(missing)
            );
            append_effect_paths(
                program,
                effect_plan,
                machine_effects.symbol,
                missing,
                &mut message,
            );
            diagnostics.push(Diagnostic::error(message));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn append_effect_paths(
    program: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
    root_machine_symbol: SymbolHandle,
    missing: omega_effects::EffectSet,
    message: &mut String,
) {
    for effect_name in missing.names() {
        let Some(effect) = omega_effects::EffectSet::from_name(effect_name) else {
            continue;
        };
        let Some(path) = effect_plan.find_effect_path(root_machine_symbol, effect) else {
            continue;
        };

        message.push_str("\n\ncall path for `");
        message.push_str(effect_name);
        message.push_str("`:");
        append_effect_path_source(program, &path.source, message, 1);
    }
}

fn append_effect_path_source(
    program: &TypedTrees,
    source: &omega_effects::EffectPathSource,
    message: &mut String,
    depth: usize,
) {
    match source {
        omega_effects::EffectPathSource::MachineDirect { machine_symbol } => {
            message.push('\n');
            push_indent(message, depth);
            message.push_str("source: machine `");
            message.push_str(&machine_label(program, *machine_symbol));
            message.push_str("` directly declares the effect");
        }
        omega_effects::EffectPathSource::StateDirect {
            machine_symbol,
            state_symbol,
        } => {
            message.push('\n');
            push_indent(message, depth);
            message.push_str("source: state `");
            message.push_str(&callable_state_label(
                program,
                *machine_symbol,
                *state_symbol,
            ));
            message.push_str("` directly declares the effect");
        }
        omega_effects::EffectPathSource::CallDirect {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index,
            call_ordinal,
            target_machine_symbol,
            target_state_symbol,
        } => {
            append_effect_call_step(
                program,
                message,
                depth,
                *caller_machine_symbol,
                *caller_state_symbol,
                *statement_index,
                *call_ordinal,
                *target_machine_symbol,
                *target_state_symbol,
            );
            message.push('\n');
            push_indent(message, depth + 1);
            message.push_str("source: target signature declares the effect");
        }
        omega_effects::EffectPathSource::ThroughCall {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index,
            call_ordinal,
            target_machine_symbol,
            target_state_symbol,
            target_source,
        } => {
            append_effect_call_step(
                program,
                message,
                depth,
                *caller_machine_symbol,
                *caller_state_symbol,
                *statement_index,
                *call_ordinal,
                *target_machine_symbol,
                *target_state_symbol,
            );
            append_effect_path_source(program, target_source, message, depth + 1);
        }
    }
}

fn append_effect_call_step(
    program: &TypedTrees,
    message: &mut String,
    depth: usize,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
) {
    message.push('\n');
    push_indent(message, depth);
    message.push_str(&callable_state_label(
        program,
        caller_machine_symbol,
        caller_state_symbol,
    ));
    message.push_str(" statement ");
    message.push_str(&statement_index.to_string());
    message.push_str(" call ");
    message.push_str(&call_ordinal.to_string());
    message.push_str(" -> ");
    message.push_str(&call_target_label(
        program,
        target_machine_symbol,
        target_state_symbol,
    ));
}

fn push_indent(message: &mut String, depth: usize) {
    for _ in 0..depth {
        message.push_str("  ");
    }
}

fn machine_label(program: &TypedTrees, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|| symbol_label(symbol))
}

fn state_label(program: &TypedTrees, symbol: SymbolHandle) -> String {
    find_machine_state(program, symbol)
        .map(|state| state.name.to_string())
        .or_else(|| find_signature_name(program, symbol))
        .unwrap_or_else(|| symbol_label(symbol))
}

fn call_target_label(
    program: &TypedTrees,
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
) -> String {
    if target_machine_symbol.is_valid() {
        return callable_state_label(program, target_machine_symbol, target_state_symbol);
    }

    state_label(program, target_state_symbol)
}

fn callable_state_label(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> String {
    let machine = machine_label(program, machine_symbol);
    let state = state_label(program, state_symbol);

    if machine
        .rsplit_once("::")
        .is_some_and(|(_, entry_name)| entry_name == state)
    {
        machine
    } else {
        format!("{machine}::{state}")
    }
}

fn find_machine_state(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::state::State> {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == symbol)
}

fn find_signature_name(program: &TypedTrees, symbol: SymbolHandle) -> Option<String> {
    for platform in program.platforms() {
        if let Some(signature) = program
            .platform_state_signatures(platform)
            .iter()
            .find(|signature| signature.symbol == symbol)
        {
            return Some(signature.name.to_string());
        }
    }

    for trait_definition in program.traits() {
        if let Some(signature) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == symbol)
        {
            return Some(format!("{}::{}", trait_definition.name, signature.name));
        }
    }

    None
}

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("{symbol:?}")
    } else {
        "<unresolved>".to_owned()
    }
}

fn validate_invariant_definitions(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for invariant in program.invariant_definitions() {
        let constraint_fact_count = fact_plan
            .contexts_at_point(ProgramPoint::Definition {
                symbol: invariant.symbol,
            })
            .flat_map(|context| context.type_constraints())
            .count();

        if constraint_fact_count != invariant.constraints.len() {
            diagnostics.push(Diagnostic::error(format!(
                "invariant `{}` references invalid constraint storage",
                invariant.name
            )));
            continue;
        }
    }
}

fn validate_domain_definitions(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    fact_plan: &FactPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for domain in program.domain_definitions() {
        validate_type_reference_handle(
            program,
            domain.target_type,
            symbols,
            diagnostics,
            TypeReferenceOwner::DomainTarget {
                domain: domain.name.as_str(),
                generic_depth: 0,
            },
        );
        validate_domain_fact_payloads(
            program,
            fact_plan,
            domain.symbol,
            diagnostics,
            ProofFactOwner::Domain(domain.name.as_str()),
        );
        validate_domain_membership_targets(program, fact_plan, domain, diagnostics);
    }

    validate_domain_membership_cycles(program, fact_plan, diagnostics);
}

fn validate_domain_fact_payloads(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    symbol: SymbolHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ProofFactOwner<'_>,
) {
    for fact in fact_plan.boolean_facts_for_symbol(symbol) {
        if !is_boolean_fact_expression(program, fact.expression) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} proof fact `{}` is not boolean-shaped",
                program.expression_table.display_name(fact.expression)
            )));
        }
    }

    for membership in fact_plan.domain_memberships_for_symbol(symbol) {
        if membership.domain_symbol.is_valid() {
            continue;
        }

        diagnostics.push(Diagnostic::error(format!(
            "{owner} references unknown domain `{}`",
            domain_path_label(program, membership.domain)
        )));
    }
}

fn validate_proof_facts(
    program: &TypedTrees,
    facts: &[ProofFact],
    diagnostics: &mut Vec<Diagnostic>,
    owner: ProofFactOwner<'_>,
) {
    for fact in facts {
        match fact {
            ProofFact::Expression(expression) => {
                if !is_boolean_fact_expression(program, *expression) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} proof fact `{}` is not boolean-shaped",
                        program.expression_table.display_name(*expression)
                    )));
                }
            }
            ProofFact::Membership(membership) => {
                if membership.domain_symbol.is_valid() {
                    continue;
                }

                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown domain `{}`",
                    domain_path_label(program, membership.domain)
                )));
            }
        }
    }
}

fn is_boolean_fact_expression(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::And
            | BinaryOperator::Equal
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::NotEqual
            | BinaryOperator::Or => true,
            BinaryOperator::Add
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Multiply
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::Subtract => false,
        },
        ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_) => true,
        ExpressionNode::Range(_) => false,
        ExpressionNode::Mutable(inner) => is_boolean_fact_expression(program, *inner),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => false,
    }
}

fn domain_path_label(
    program: &TypedTrees,
    domain: omega_core::arena::HandleSpan<Identifier>,
) -> String {
    let path = program.domain_path_members(domain);
    if path.is_empty() {
        return "<unknown>".to_owned();
    }

    path.iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

fn validate_domain_membership_targets(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    domain: &omega_typed_trees::domain::DomainDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for membership in domain_membership_facts(fact_plan, domain.symbol) {
        let Some(referenced_domain) =
            domain_definition_by_symbol(program, membership.domain_symbol)
        else {
            continue;
        };

        if type_references_match(program, domain.target_type, referenced_domain.target_type) {
            continue;
        }

        diagnostics.push(Diagnostic::error(format!(
            "domain `{}` imports `{}` but they classify different types: `{}` vs `{}`",
            domain.name,
            referenced_domain.name,
            type_reference_label(program, domain.target_type),
            type_reference_label(program, referenced_domain.target_type)
        )));
    }
}

fn validate_domain_membership_cycles(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reported = Vec::new();
    for domain in program.domain_definitions() {
        let mut path = Vec::new();
        validate_domain_membership_cycle_from(
            program,
            fact_plan,
            domain.symbol,
            &mut path,
            &mut reported,
            diagnostics,
        );
    }
}

fn validate_domain_membership_cycle_from(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    domain_symbol: SymbolHandle,
    path: &mut Vec<SymbolHandle>,
    reported: &mut Vec<SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !domain_symbol.is_valid() || reported.contains(&domain_symbol) {
        return;
    }

    if let Some(cycle_start) = path.iter().position(|symbol| *symbol == domain_symbol) {
        reported.push(domain_symbol);
        let cycle = path[cycle_start..]
            .iter()
            .copied()
            .chain(std::iter::once(domain_symbol))
            .filter_map(|symbol| domain_definition_by_symbol(program, symbol))
            .map(|domain| domain.name.to_string())
            .collect::<Vec<_>>()
            .join(" -> ");
        diagnostics.push(Diagnostic::error(format!(
            "domain membership cycle: {cycle}"
        )));
        return;
    }

    let Some(domain) = domain_definition_by_symbol(program, domain_symbol) else {
        return;
    };

    path.push(domain_symbol);
    for membership in domain_membership_facts(fact_plan, domain.symbol) {
        validate_domain_membership_cycle_from(
            program,
            fact_plan,
            membership.domain_symbol,
            path,
            reported,
            diagnostics,
        );
    }
    path.pop();
}

fn domain_membership_facts(
    fact_plan: &FactPlan,
    symbol: SymbolHandle,
) -> impl Iterator<Item = omega_facts::DomainMembershipFact> + '_ {
    fact_plan.domain_memberships_for_symbol(symbol)
}

struct WritableRoots<'program, 'state> {
    machine_symbols: &'state MachineSymbols<'program>,
    statements: &'state [StatementNode],
    parameters: &'state [StateParameter],
}

impl WritableRoots<'_, '_> {
    fn contains(&self, root_name: &str) -> bool {
        self.machine_symbols.has_owned_data(root_name)
            || self.statements.iter().any(|statement| {
                let StatementNode::LocalData(local_data) = statement else {
                    return false;
                };

                local_data.name.as_str() == root_name
            })
            || self
                .parameters
                .iter()
                .any(|parameter| parameter.is_mutable && parameter.name.as_str() == root_name)
    }
}

fn validate_local_data_names(
    statements: &[StatementNode],
    machine_symbols: &MachineSymbols<'_>,
    parameters: &[StateParameter],
    machine_name: &str,
    state_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (statement_index, statement) in statements.iter().enumerate() {
        let StatementNode::LocalData(local_data) = statement else {
            continue;
        };

        if machine_symbols.has_member(local_data.name.as_str())
            || parameters
                .iter()
                .any(|parameter| parameter.name == local_data.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{machine_name}` state `{state_name}` local data `{}` conflicts with an existing name",
                local_data.name
            )));
            continue;
        }

        if statements[..statement_index].iter().any(|previous| {
            matches!(
                previous,
                StatementNode::LocalData(previous) if previous.name == local_data.name
            )
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{machine_name}` state `{state_name}` has duplicate local data `{}`",
                local_data.name
            )));
        }
    }
}

fn validate_state_statement_node(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assignment(assignment) => validate_assignment_target_handle(
            program,
            assignment.target,
            writable_roots,
            diagnostics,
            machine.name.as_str(),
            state_name,
        ),
        StatementNode::Call(call) => validate_call_node(
            program,
            call,
            machine,
            machine_symbols,
            symbols,
            writable_roots,
            diagnostics,
        ),
        StatementNode::Expression(expression) => {
            let Some(state) = machine_symbols.state(state_name) else {
                return;
            };

            if !state.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` has a terminal expression but no return type",
                    machine.name
                )));
                return;
            }

            validate_expression_type_handle(
                program,
                *expression,
                state.return_type,
                diagnostics,
                ExpressionTypeOwner::StateTerminalExpression {
                    machine: machine.name.as_str(),
                    state: state_name,
                },
            );
        }
        StatementNode::LocalData(local_data) => validate_type_reference_handle(
            program,
            local_data.type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::StateLocalData {
                machine: machine.name.as_str(),
                state: state_name,
                local: local_data.name.as_str(),
                generic_depth: 0,
            },
        ),
        StatementNode::Transition(transition) => {
            validate_transition_target_node(
                program,
                transition.target,
                machine_symbols,
                symbols,
                writable_roots,
                diagnostics,
            );

            if transition.continuation.is_valid() {
                validate_transition_target_node(
                    program,
                    transition.continuation,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_assignment_target_handle(
    program: &TypedTrees,
    target: ExpressionHandle,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
    machine_name: &str,
    state_name: &str,
) {
    if !is_mutable_place_handle(program, target) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment target must be a named place"
        )));
        return;
    }

    let Some(root_name) = expression_root_name_handle(program, target) else {
        return;
    };

    if !writable_roots.contains(root_name) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment cannot write `{root_name}` because it is not mutable in this state"
        )));
    }
}

fn validate_callable_state_signatures(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        validate_state_signature_types(
            program
                .machine_states(machine)
                .iter()
                .map(|state| StateSignatureView {
                    name: state.name.as_str(),
                    parameters: program.state_parameters(state),
                    return_type: state.return_type,
                    effects: &[],
                    contracts: &[],
                }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Machine(machine.name.as_str()),
        );
    }

    for platform in program.platforms() {
        let platform_states = program.platform_state_signatures(platform);
        validate_platform_state_names(platform, platform_states, diagnostics);
        validate_state_signature_types(
            platform_states.iter().map(|state| StateSignatureView {
                name: state.name.as_str(),
                parameters: program.state_signature_parameters(state),
                return_type: state.return_type,
                effects: program.state_signature_effects(state),
                contracts: program.state_signature_contracts(state),
            }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Platform(platform.name.as_str()),
        );
    }

    for trait_definition in program.traits() {
        validate_state_signature_types(
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .map(|machine| StateSignatureView {
                    name: machine.name.as_str(),
                    parameters: program.state_signature_parameters(machine),
                    return_type: machine.return_type,
                    effects: program.state_signature_effects(machine),
                    contracts: program.state_signature_contracts(machine),
                }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::Trait(trait_definition.name.as_str()),
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct StateSignatureView<'program> {
    name: &'program str,
    parameters: &'program [StateParameter],
    return_type: TypeReferenceHandle,
    effects: &'program [Identifier],
    contracts: &'program [SignatureContract],
}

#[derive(Debug, Clone, Copy)]
enum StateSignatureOwner<'program> {
    Machine(&'program str),
    Platform(&'program str),
    Trait(&'program str),
}

impl fmt::Display for StateSignatureOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Machine(machine) => write!(formatter, "machine `{machine}`"),
            Self::Platform(platform) => write!(formatter, "platform `{platform}`"),
            Self::Trait(trait_definition) => write!(formatter, "trait `{trait_definition}`"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TypeReferenceOwner<'program> {
    DomainTarget {
        domain: &'program str,
        generic_depth: usize,
    },
    DataField {
        data: &'program str,
        field: &'program str,
        generic_depth: usize,
    },
    MachineOwnedData {
        machine: &'program str,
        data: &'program str,
        generic_depth: usize,
    },
    StateLocalData {
        machine: &'program str,
        state: &'program str,
        local: &'program str,
        generic_depth: usize,
    },
    StateParameter {
        owner: StateSignatureOwner<'program>,
        state: &'program str,
        parameter: &'program str,
        generic_depth: usize,
    },
    StateReturn {
        owner: StateSignatureOwner<'program>,
        state: &'program str,
        generic_depth: usize,
    },
}

#[derive(Debug, Clone, Copy)]
enum InitialValueOwner<'program> {
    MachineOwnedData {
        machine: &'program str,
        data: &'program str,
    },
}

impl fmt::Display for InitialValueOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MachineOwnedData { machine, data } => {
                write!(formatter, "machine `{machine}` owned data `{data}`")
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ExpressionTypeOwner<'program> {
    StateTerminalExpression {
        machine: &'program str,
        state: &'program str,
    },
}

#[derive(Debug, Clone, Copy)]
enum ProofFactOwner<'program> {
    Domain(&'program str),
    MachineContract {
        machine: &'program str,
        kind: &'static str,
    },
    StateSignatureContract {
        owner: StateSignatureOwner<'program>,
        state: &'program str,
        kind: &'static str,
    },
}

impl fmt::Display for ProofFactOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(domain) => write!(formatter, "domain `{domain}`"),
            Self::MachineContract { machine, kind } => {
                write!(formatter, "machine `{machine}` {kind} contract")
            }
            Self::StateSignatureContract { owner, state, kind } => {
                write!(formatter, "{owner} state `{state}` {kind} contract")
            }
        }
    }
}

impl fmt::Display for ExpressionTypeOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateTerminalExpression { machine, state } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` terminal expression"
                )
            }
        }
    }
}

impl TypeReferenceOwner<'_> {
    fn generic_argument(self) -> Self {
        match self {
            Self::DomainTarget {
                domain,
                generic_depth,
            } => Self::DomainTarget {
                domain,
                generic_depth: generic_depth + 1,
            },
            Self::DataField {
                data,
                field,
                generic_depth,
            } => Self::DataField {
                data,
                field,
                generic_depth: generic_depth + 1,
            },
            Self::MachineOwnedData {
                machine,
                data,
                generic_depth,
            } => Self::MachineOwnedData {
                machine,
                data,
                generic_depth: generic_depth + 1,
            },
            Self::StateLocalData {
                machine,
                state,
                local,
                generic_depth,
            } => Self::StateLocalData {
                machine,
                state,
                local,
                generic_depth: generic_depth + 1,
            },
            Self::StateParameter {
                owner,
                state,
                parameter,
                generic_depth,
            } => Self::StateParameter {
                owner,
                state,
                parameter,
                generic_depth: generic_depth + 1,
            },
            Self::StateReturn {
                owner,
                state,
                generic_depth,
            } => Self::StateReturn {
                owner,
                state,
                generic_depth: generic_depth + 1,
            },
        }
    }
}

impl fmt::Display for TypeReferenceOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generic_depth = match self {
            Self::DomainTarget {
                domain,
                generic_depth,
            } => {
                write!(formatter, "domain `{domain}` target type")?;
                *generic_depth
            }
            Self::DataField {
                data,
                field,
                generic_depth,
            } => {
                write!(formatter, "data `{data}` field `{field}`")?;
                *generic_depth
            }
            Self::MachineOwnedData {
                machine,
                data,
                generic_depth,
            } => {
                write!(formatter, "machine `{machine}` owned data `{data}`")?;
                *generic_depth
            }
            Self::StateLocalData {
                machine,
                state,
                local,
                generic_depth,
            } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` local data `{local}`"
                )?;
                *generic_depth
            }
            Self::StateParameter {
                owner,
                state,
                parameter,
                generic_depth,
            } => {
                write!(formatter, "{owner} state `{state}` parameter `{parameter}`")?;
                *generic_depth
            }
            Self::StateReturn {
                owner,
                state,
                generic_depth,
            } => {
                write!(formatter, "{owner} state `{state}` return type")?;
                *generic_depth
            }
        };

        for _ in 0..generic_depth {
            formatter.write_str(" generic argument")?;
        }

        Ok(())
    }
}

fn validate_state_signature_types<'program>(
    signatures: impl Iterator<Item = StateSignatureView<'program>>,
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: StateSignatureOwner<'program>,
) {
    for signature in signatures {
        validate_state_parameter_names(signature, owner, diagnostics);
        validate_state_signature_effects(signature, owner, diagnostics);
        validate_state_signature_contracts(program, signature, owner, diagnostics);

        for parameter in signature.parameters {
            if parameter.is_self {
                continue;
            }

            validate_type_reference_handle(
                program,
                parameter.type_reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateParameter {
                    owner,
                    state: signature.name,
                    parameter: parameter.name.as_str(),
                    generic_depth: 0,
                },
            );
        }

        if signature.return_type.is_valid() {
            validate_type_reference_handle(
                program,
                signature.return_type,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateReturn {
                    owner,
                    state: signature.name,
                    generic_depth: 0,
                },
            );
        }
    }
}

fn validate_state_signature_effects(
    signature: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for effect in signature.effects {
        if !omega_effects::is_standard_effect_name(effect.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` declares unknown effect `{}`",
                signature.name, effect
            )));
        }
    }
}

fn validate_state_signature_contracts(
    program: &TypedTrees,
    signature: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in signature.contracts {
        validate_proof_facts(
            program,
            program.proof_facts.span_or_empty(contract.facts),
            diagnostics,
            ProofFactOwner::StateSignatureContract {
                owner,
                state: signature.name,
                kind: contract_kind_label(contract.kind),
            },
        );
    }
}

fn validate_machine_effects(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for effect in program.machine_effects(machine) {
        if !omega_effects::is_standard_effect_name(effect.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` declares unknown effect `{}`",
                machine.name, effect
            )));
        }
    }
}

fn validate_machine_contracts(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in program.machine_contracts(machine) {
        validate_proof_facts(
            program,
            program.proof_facts.span_or_empty(contract.facts),
            diagnostics,
            ProofFactOwner::MachineContract {
                machine: machine.name.as_str(),
                kind: contract_kind_label(contract.kind),
            },
        );
    }
}

fn contract_kind_label(kind: omega_typed_trees::signature::SignatureContractKind) -> &'static str {
    match kind {
        omega_typed_trees::signature::SignatureContractKind::Requires => "requires",
        omega_typed_trees::signature::SignatureContractKind::Ensures => "ensures",
        omega_typed_trees::signature::SignatureContractKind::Trusted => "trusted",
    }
}

fn declared_machine_effect_set(
    program: &TypedTrees,
    machine: &Machine,
) -> omega_effects::EffectSet {
    let mut effects = omega_effects::EffectSet::empty();
    for effect in program.machine_effects(machine) {
        effects.insert_name(effect.as_str());
    }
    effects
}

fn format_effect_set(effects: omega_effects::EffectSet) -> String {
    if effects.is_empty() {
        return "<none>".to_owned();
    }

    effects.names().collect::<Vec<_>>().join(", ")
}

fn validate_platform_state_names(
    platform: &omega_typed_trees::platform::Platform,
    platform_states: &[omega_typed_trees::signature::StateSignature],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (state_index, state) in platform_states.iter().enumerate() {
        if platform_states[..state_index]
            .iter()
            .any(|previous| previous.name == state.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has duplicate state `{}`",
                platform.name, state.name
            )));
        }
    }
}

fn validate_state_parameter_names(
    state: StateSignatureView<'_>,
    owner: StateSignatureOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (parameter_index, parameter) in state.parameters.iter().enumerate() {
        if state.parameters[..parameter_index]
            .iter()
            .any(|previous| previous.name == parameter.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` has duplicate parameter `{}`",
                state.name, parameter.name
            )));
        }
    }
}

fn validate_data_field_types(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in program.data_definitions() {
        let data_members = program.data_members(data_definition);
        validate_data_member_names(data_definition, data_members, diagnostics);
        validate_data_shape(data_definition, data_members, diagnostics);

        for member in data_members {
            let DataMember::Field(field) = member else {
                continue;
            };

            validate_type_reference_handle(
                program,
                field.type_reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::DataField {
                    data: data_definition.name.as_str(),
                    field: field.name.as_str(),
                    generic_depth: 0,
                },
            );
        }
    }
}

fn validate_operator_declarations(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    validate_duplicate_operator_names(program, "root", program.operators(), diagnostics);

    for domain in program.domain_definitions() {
        validate_duplicate_operator_names(
            program,
            domain.name.as_str(),
            program.domain_operators(domain),
            diagnostics,
        );
    }
}

fn validate_duplicate_operator_names(
    program: &TypedTrees,
    owner: &str,
    operators: &[omega_typed_trees::operator::OperatorDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (operator_index, operator) in operators.iter().enumerate() {
        let signature = operator_signature_key(program, operator);
        if operators[..operator_index]
            .iter()
            .any(|previous| operator_signature_key(program, previous) == signature)
        {
            let name = operator_name(program, operator);
            if owner == "root" {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate operator declaration `{name}`"
                )));
            } else {
                diagnostics.push(Diagnostic::error(format!(
                    "domain `{owner}` has duplicate operator `{name}`"
                )));
            }
        }
    }
}

fn operator_signature_key(
    program: &TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
) -> String {
    let parameter_types = program
        .operator_parameters(operator)
        .iter()
        .map(|parameter| program.display_type_reference(parameter.type_reference))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}({parameter_types})", operator_name(program, operator))
}

fn operator_name(
    program: &TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
) -> String {
    program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

fn validate_data_shape(
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match omega_typed_trees::data::DataDefinition::shape_kind_from_members(data_members) {
        DataShapeKind::Empty => {}
        DataShapeKind::Mixed => diagnostics.push(Diagnostic::error(format!(
            "data `{}` mixes fields and variants; split record data from enum-like data",
            data_definition.name
        ))),
        DataShapeKind::Enum | DataShapeKind::Record => {}
    }
}

fn validate_data_member_names(
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (member_index, member) in data_members.iter().enumerate() {
        let member_name = match member {
            DataMember::Field(field) => field.name.as_str(),
            DataMember::Variant(variant) => variant.name.as_str(),
        };

        if data_members[..member_index]
            .iter()
            .any(|previous| data_member_name(previous) == member_name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has duplicate member `{member_name}`",
                data_definition.name
            )));
        }
    }
}

fn data_member_name(member: &DataMember) -> &str {
    match member {
        DataMember::Field(field) => field.name.as_str(),
        DataMember::Variant(variant) => variant.name.as_str(),
    }
}

fn validate_type_reference_handle(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            validate_type_reference_handle(program, *referee, symbols, diagnostics, owner);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            validate_type_reference_handle(program, *base_type, symbols, diagnostics, owner);
            validate_type_constraints_node(program, *base_type, *constraints, diagnostics, owner);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            validate_type_reference_handle(program, *element_type, symbols, diagnostics, owner);
        }
        TypeReferenceNode::Slice { element_type } => {
            validate_type_reference_handle(program, *element_type, symbols, diagnostics, owner);
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            if !symbols.has_type(base_name) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown generic type `{base_name}`"
                )));
            }

            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                validate_type_reference_handle(
                    program,
                    *argument,
                    symbols,
                    diagnostics,
                    owner.generic_argument(),
                );
            }
        }
        TypeReferenceNode::Named { name, .. } => {
            if !symbols.has_type(name) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown data type `{name}`"
                )));
            }
        }
        TypeReferenceNode::Unit => {}
    }
}

fn validate_type_constraints_node(
    program: &TypedTrees,
    base_type: TypeReferenceHandle,
    constraints: omega_core::arena::HandleSpan<TypeConstraintNode>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
) {
    let primitive_type = program.type_reference_table.primitive_type(base_type);

    for constraint in program.type_reference_table.constraints(constraints) {
        match constraint {
            TypeConstraintNode::Named(name) if name.as_str() == "finite" => {
                let Some(primitive_type) = primitive_type else {
                    continue;
                };

                if !primitive_type.accepts_finite_constraint() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `finite` on `{}`, but `finite` is only valid on floats",
                        primitive_type.name()
                    )));
                }
            }
            TypeConstraintNode::Named(_) => {}
            TypeConstraintNode::Range { .. } => {
                let Some(primitive_type) = primitive_type else {
                    continue;
                };

                if !primitive_type.accepts_range_constraint() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `range` on `{}`, but `range` is only valid on numeric types",
                        primitive_type.name()
                    )));
                }
            }
        }
    }
}

fn validate_entry_point(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    if has_entry_point(program, "Main::main", "main") || has_entry_point(program, "main", "entry") {
        return;
    }

    diagnostics.push(Diagnostic::error(
        "missing runtime entry point `Main::main`",
    ));
}

fn has_entry_point(program: &TypedTrees, machine_name: &str, state_name: &str) -> bool {
    let machine_symbol = program.symbols.find_child_by_name_and_kind(
        program.symbols.root(),
        machine_name,
        SymbolKind::Machine,
    );
    let Some(machine) = machine_symbol.and_then(|machine_symbol| {
        program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
    }) else {
        return false;
    };

    let state_symbol =
        program
            .symbols
            .find_child_by_name_and_kind(machine.symbol, state_name, SymbolKind::State);

    program
        .machine_states(machine)
        .iter()
        .any(|state| Some(state.symbol) == state_symbol)
}

#[cfg(test)]
mod tests {
    use super::{validate_effect_plan, validate_program};
    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn validates_main_entry_surface_from_source_pipeline() {
        let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        assert_eq!(typed.machines().len(), 1);
        assert_eq!(typed.machines()[0].name.as_str(), "Main::main");
        assert_eq!(typed.machine_states(&typed.machines()[0]).len(), 1);
        assert_eq!(
            typed.machine_states(&typed.machines()[0])[0].name.as_str(),
            "main"
        );
        validate_program(&typed).expect("validation should succeed");
    }

    #[test]
    fn validates_local_state_call_arguments_from_source_pipeline() {
        let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {
            take_non_negative(0);

            state take_non_negative(
                &mut self,
                value: u32[exact, non_negative]
            ) {
            }
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        let entry = typed
            .machine_states(&typed.machines()[0])
            .iter()
            .find(|state| state.name.as_str() == "main")
            .expect("entry state");
        let call_argument_count = typed
            .statement_table
            .statements(entry.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                omega_typed_trees::statement::StatementNode::Call(call) => {
                    Some(call.arguments.len())
                }
                omega_typed_trees::statement::StatementNode::Expression(expression) => {
                    let omega_typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    else {
                        return None;
                    };
                    Some(call.arguments.len())
                }
                _ => None,
            })
            .expect("expected call statement");
        assert_eq!(call_argument_count, 1);
        validate_program(&typed).expect("validation should succeed");
    }

    #[test]
    fn rejects_unknown_trait_machine_effects() {
        let source = r#"
        boundary trait Console {
            machine write_line(text: String)
            effects
                stdoutish;
        }

        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        let diagnostics = validate_program(&typed).expect_err("validation should reject effect");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unknown effect `stdoutish`")),
            "expected unknown effect diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn rejects_unknown_domain_membership_in_domain_body() {
        let source = r#"
        data Player {
        }

        domain Player::Alive {
            self in Player::Valid
        }

        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("domain `Player::Alive` references unknown domain `Player::Valid`")),
            "expected unknown domain diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn rejects_unknown_domain_membership_in_contract() {
        let source = r#"
        data Player {
        }

        boundary trait Renderer {
            machine draw(player: Player)
            requires
                player in Player::Drawable;
        }

        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        let diagnostics = validate_program(&typed).expect_err("validation should reject domain");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
                "trait `Renderer` state `draw` requires contract references unknown domain `Player::Drawable`"
            )),
            "expected unknown contract domain diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn rejects_non_boolean_shaped_proof_fact() {
        let source = r#"
        data Player {
        }

        domain Player::Weird {
            1 + 2
        }

        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        let diagnostics = validate_program(&typed).expect_err("validation should reject fact");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("domain `Player::Weird` proof fact `1 + 2` is not boolean-shaped")),
            "expected non-boolean proof fact diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn rejects_domain_import_with_different_target_type() {
        let source = r#"
        data Player {
        }

        data Enemy {
        }

        domain Enemy::Valid {
            true
        }

        domain Player::Alive {
            self in Enemy::Valid
        }

        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        let diagnostics = validate_program(&typed).expect_err("validation should reject import");
        let has_target_mismatch = diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "domain `Player::Alive` imports `Enemy::Valid` but they classify different types",
            )
        });
        assert!(
            has_target_mismatch,
            "expected domain target mismatch diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn rejects_domain_import_cycles() {
        let source = r#"
        data Player {
        }

        domain Player::Alive {
            self in Player::Valid
        }

        domain Player::Valid {
            self in Player::Alive
        }

        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        let diagnostics = validate_program(&typed).expect_err("validation should reject cycle");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(
                    "domain membership cycle: Player::Alive -> Player::Valid -> Player::Alive",
                )
            }),
            "expected domain cycle diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn rejects_machine_effects_outside_trait_ceiling() {
        let source = r#"
        boundary trait Console {
            machine write_line(text: String)
            effects
                stdout_io;
        }

        data ConsoleImpl {
        }

        machine ConsoleImpl::write_line(text: String) satisfies Console
        effects
            stdout_io, filesystem_io
        {
        }

        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        let diagnostics =
            validate_program(&typed).expect_err("validation should reject extra effect");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("effect `filesystem_io` is not allowed by the trait requirement")),
            "expected effect ceiling diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn accepts_machine_effects_within_trait_ceiling() {
        let source = r#"
        boundary trait Console {
            machine write_line(text: String)
            effects
                stdout_io;
        }

        data ConsoleImpl {
        }

        machine ConsoleImpl::write_line(text: String) satisfies Console
        effects
            stdout_io
        {
        }

        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        validate_program(&typed).expect("validation should succeed");
    }

    #[test]
    fn accepts_machine_effects_below_trait_ceiling() {
        let source = r#"
        boundary trait Console {
            machine write_line(text: String)
            effects
                stdout_io;
        }

        data TestConsole {
        }

        machine TestConsole::write_line(text: String) satisfies Console {
        }

        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        validate_program(&typed).expect("validation should succeed");
    }

    #[test]
    fn rejects_declared_machine_effects_below_reached_effects() {
        let source = r#"
        boundary trait Console {
            machine read_line(out: &mut String)
            effects
                stdin_io;
        }

        data ConsoleImpl {
        }

        machine ConsoleImpl::read_line(out: &mut String) satisfies Console
        effects
            stdin_io
        {
        }

        data Main {
            console: ConsoleImpl;
        }

        machine Main::main(&mut self)
        effects
            stdout_io
        {
            let line: String;
            self.console.read_line(&mut line);
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typed lowering should succeed");

        validate_program(&typed).expect("direct effect validation should pass");
        let effect_plan = omega_effects::infer_effects(&typed);
        let diagnostics =
            validate_effect_plan(&typed, &effect_plan).expect_err("effect ceiling should fail");

        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("reaches undeclared effects `stdin_io`")
                && diagnostic.message.contains("call path for `stdin_io`")
                && diagnostic.message.contains("Main::main statement")
                && diagnostic.message.contains(
                    "source: machine `ConsoleImpl::read_line` directly declares the effect"
                )),
            "expected transitive effect ceiling diagnostic, got {diagnostics:#?}"
        );
    }
}

fn validate_contained_types(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contained_object in program.machine_contained_objects(machine) {
        if !symbols.is_callable_receiver_type(&contained_object.type_name) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` contains `{}` with unknown type `{}`",
                machine.name, contained_object.name, contained_object.type_name
            )));
        }
    }
}

fn validate_owned_data(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for owned_data in program.machine_owned_data(machine) {
        validate_type_reference_handle(
            program,
            owned_data.type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::MachineOwnedData {
                machine: machine.name.as_str(),
                data: owned_data.name.as_str(),
                generic_depth: 0,
            },
        );

        if owned_data.initial_value.is_valid() {
            validate_initial_value_handle(
                program,
                owned_data.type_reference,
                owned_data.initial_value,
                diagnostics,
                InitialValueOwner::MachineOwnedData {
                    machine: machine.name.as_str(),
                    data: owned_data.name.as_str(),
                },
            );
        }
    }
}

fn validate_machine_trait_conformances(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies unknown trait `{}`",
                machine.name, conformance.name
            )));
            continue;
        };

        let mut visited_traits = Vec::new();
        validate_machine_satisfies_trait(
            program,
            machine,
            trait_definition,
            diagnostics,
            &mut visited_traits,
        );
    }
}

fn validate_trait_requirements(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for trait_definition in program.traits() {
        for requirement in program.trait_requirements(trait_definition) {
            if trait_definition_by_symbol(program, requirement.symbol).is_none() {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` requires unknown trait `{}`",
                    trait_definition.name, requirement.name
                )));
            }
        }
    }

    let mut reported_cycle_symbols = Vec::new();
    for trait_definition in program.traits() {
        let mut path = Vec::new();
        validate_trait_requirement_cycles(
            program,
            trait_definition,
            &mut path,
            &mut reported_cycle_symbols,
            diagnostics,
        );
    }
}

fn validate_trait_requirement_cycles(
    program: &TypedTrees,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    path: &mut Vec<SymbolHandle>,
    reported_cycle_symbols: &mut Vec<SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if reported_cycle_symbols
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return;
    }

    if let Some(cycle_start) = path
        .iter()
        .position(|symbol| *symbol == trait_definition.symbol)
    {
        let cycle_symbols = path[cycle_start..]
            .iter()
            .copied()
            .chain(std::iter::once(trait_definition.symbol))
            .collect::<Vec<_>>();
        let mut cycle = path[cycle_start..]
            .iter()
            .filter_map(|symbol| trait_definition_by_symbol(program, *symbol))
            .map(|trait_definition| trait_definition.name.to_string())
            .collect::<Vec<_>>();
        cycle.push(trait_definition.name.to_string());

        diagnostics.push(Diagnostic::error(format!(
            "trait requirement cycle detected: {}",
            cycle.join(" -> ")
        )));
        reported_cycle_symbols.extend(cycle_symbols);
        return;
    }

    path.push(trait_definition.symbol);
    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };

        validate_trait_requirement_cycles(
            program,
            required_trait,
            path,
            reported_cycle_symbols,
            diagnostics,
        );
    }
    path.pop();
}

fn validate_machine_satisfies_trait(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    diagnostics: &mut Vec<Diagnostic>,
    visited_traits: &mut Vec<SymbolHandle>,
) {
    if visited_traits
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return;
    }

    visited_traits.push(trait_definition.symbol);

    for requirement in program.trait_machine_signatures(trait_definition) {
        let Some((state_machine, state)) = trait_requirement_state(program, machine, requirement)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies trait `{}` but is missing machine `{}`",
                machine.name, trait_definition.name, requirement.name
            )));
            continue;
        };

        validate_machine_state_satisfies_trait_signature(
            program,
            state_machine,
            state,
            trait_definition.name.as_str(),
            requirement,
            diagnostics,
        );
    }

    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };

        validate_machine_satisfies_trait(
            program,
            machine,
            required_trait,
            diagnostics,
            visited_traits,
        );
    }

    visited_traits.pop();
}

fn trait_requirement_state<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    requirement: &StateSignature,
) -> Option<(&'program Machine, &'program State)> {
    trait_conformance_candidate_machines(program, machine)
        .into_iter()
        .find_map(|candidate| {
            program
                .machine_states(candidate)
                .iter()
                .find(|state| state.name == requirement.name)
                .map(|state| (candidate, state))
        })
}

fn trait_conformance_candidate_machines<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
) -> Vec<&'program Machine> {
    let Some(attached_data) = machine.attached_data.as_ref() else {
        return vec![machine];
    };

    let mut candidates = Vec::new();
    candidates.push(machine);
    candidates.extend(program.machines().iter().filter(|candidate| {
        !std::ptr::eq(*candidate, machine)
            && candidate.attached_data.as_ref() == Some(attached_data)
    }));
    candidates
}

fn trait_definition_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::trait_definition::TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
}

fn domain_definition_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::domain::DomainDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == symbol)
}

fn validate_machine_state_satisfies_trait_signature(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let actual_parameters = program.state_parameters(state);
    let required_parameters = program.state_signature_parameters(requirement);
    if actual_parameters.len() != required_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected {} parameter(s), got {}",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            required_parameters.len(),
            actual_parameters.len()
        )));
        return;
    }

    for (index, (actual, required)) in actual_parameters
        .iter()
        .zip(required_parameters.iter())
        .enumerate()
    {
        validate_trait_parameter_match(
            program,
            machine,
            state,
            trait_name,
            requirement,
            index,
            actual,
            required,
            diagnostics,
        );
    }

    if !type_references_match(program, state.return_type, requirement.return_type) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected return `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            type_reference_label(program, requirement.return_type),
            type_reference_label(program, state.return_type)
        )));
    }

    validate_trait_effect_ceiling(
        program,
        machine,
        state,
        trait_name,
        requirement,
        diagnostics,
    );
}

fn validate_trait_effect_ceiling(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let allowed_effects = program.state_signature_effects(requirement);

    for effect in program.machine_effects(machine) {
        if !allowed_effects
            .iter()
            .any(|allowed| allowed.as_str() == effect.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: effect `{}` is not allowed by the trait requirement",
                machine.name,
                state.name,
                trait_name,
                requirement.name,
                effect
            )));
        }
    }
}

fn validate_trait_parameter_match(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    index: usize,
    actual: &StateParameter,
    required: &StateParameter,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if actual.is_self != required.is_self || actual.is_mutable != required.is_mutable {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter {}: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            index,
            parameter_shape_label(program, required),
            parameter_shape_label(program, actual)
        )));
        return;
    }

    if !type_references_match(program, actual.type_reference, required.type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter `{}`: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            required.name,
            type_reference_label(program, required.type_reference),
            type_reference_label(program, actual.type_reference)
        )));
    }
}

fn parameter_shape_label(program: &TypedTrees, parameter: &StateParameter) -> String {
    let qualifier = if parameter.is_mutable { "mut " } else { "" };
    if parameter.is_self {
        format!("&{qualifier}self")
    } else {
        format!(
            "{}: {}",
            parameter.name,
            type_reference_label(program, parameter.type_reference)
        )
    }
}

fn type_references_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }

    program.display_type_reference_with_constraints(actual)
        == program.display_type_reference_with_constraints(required)
}

fn type_reference_label(program: &TypedTrees, type_reference: TypeReferenceHandle) -> String {
    if type_reference.is_valid() {
        program.display_type_reference_with_constraints(type_reference)
    } else {
        "()".to_owned()
    }
}

fn validate_initial_value_handle(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    initial_value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: InitialValueOwner<'_>,
) {
    if !argument_matches_type_reference_handle(program, initial_value, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} initializer expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, initial_value)
        )));
    }
}

fn validate_call_node(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let arguments = program.statement_table.expression_handles(call.arguments);

    if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver.as_str() == "self")
    {
        if let Some(state) = machine_symbols.state(&call.target) {
            validate_call_arguments_handles(
                program,
                arguments,
                state.name.as_str(),
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            return;
        }

        let Some((_, state)) = current_machine
            .attached_data
            .as_ref()
            .and_then(|attached_data| {
                symbols.attached_machine_state(
                    program,
                    attached_data.as_str(),
                    call.target.as_str(),
                )
            })
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local state `{}`",
                current_machine.name, call.target
            )));
            return;
        };

        validate_call_arguments_handles(
            program,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let receiver = receiver_members
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let receiver_type = machine_symbols.contained_type(receiver);

    if let Some(platform) = receiver_type.and_then(|type_name| symbols.platform(type_name)) {
        let Some(state_signature) = program
            .platform_state_signatures(platform)
            .iter()
            .find(|state| state.name == call.target)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has no state `{}`",
                platform.name, call.target
            )));
            return;
        };

        validate_call_arguments_handles(
            program,
            arguments,
            &state_signature.name,
            program.state_signature_parameters(state_signature),
            writable_roots,
            diagnostics,
        );
        return;
    }

    if let Some(machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver))
    {
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name == call.target)
        {
            validate_call_arguments_handles(
                program,
                arguments,
                &state.name,
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            return;
        };

        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` has no state `{}`",
            machine.name, call.target
        )));
        return;
    }

    if let Some((_, state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        validate_call_arguments_handles(
            program,
            arguments,
            &state.name,
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let _ = diagnostics;
}

fn validate_call_arguments_handles(
    program: &TypedTrees,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let callable_parameter_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();

    if arguments.len() != callable_parameter_count {
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` expects {} argument(s), got {}",
            target_name,
            callable_parameter_count,
            arguments.len()
        )));
        return;
    }

    for (argument, parameter) in arguments
        .iter()
        .zip(parameters.iter().filter(|parameter| !parameter.is_self))
    {
        let is_mutable = matches!(
            program.expression_table.expression(*argument),
            ExpressionNode::Mutable(_)
        );

        if parameter.is_mutable && !is_mutable {
            continue;
        }

        if !parameter.is_mutable && is_mutable {
            continue;
        }

        let expected_type =
            program.display_type_reference_with_constraints(parameter.type_reference);

        if !argument_matches_type_reference_handle(program, *argument, parameter.type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` expects `{}`, got `{}`",
                parameter.name,
                target_name,
                expected_type,
                expression_type_name_handle(program, *argument)
            )));
        }
    }

    let _ = (writable_roots, diagnostics);
}
fn is_mutable_place_handle(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => is_mutable_place_handle(program, indexed.collection),
        ExpressionNode::Member(member) => is_mutable_place_handle(program, member.receiver),
        ExpressionNode::Name(_) => true,
        _ => false,
    }
}

fn expression_root_name_handle(program: &TypedTrees, expression: ExpressionHandle) -> Option<&str> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_name_handle(program, indexed.collection)
        }
        ExpressionNode::Member(member) => {
            match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path)
                    if path.members.count() == 1
                        && program
                            .expression_table
                            .name_path_members(path.members)
                            .first()
                            .is_some_and(|name| name.as_str() == "self") =>
                {
                    Some(member.member.as_str())
                }
                _ => expression_root_name_handle(program, member.receiver),
            }
        }
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .map(|name| name.as_str()),
        _ => None,
    }
}

fn argument_matches_type_reference_handle(
    program: &TypedTrees,
    argument: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> bool {
    if let ExpressionNode::Mutable(inner_expression) = program.expression_table.expression(argument)
    {
        return argument_matches_type_reference_handle(program, *inner_expression, type_reference);
    }

    let argument_node = program.expression_table.expression(argument);

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            argument_matches_type_reference_handle(program, argument, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            argument_matches_type_reference_handle(program, argument, *base_type)
        }
        TypeReferenceNode::FixedArray { .. } => matches!(
            argument_node,
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Slice { .. } => matches!(
            argument_node,
            ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Generic { .. } => matches!(
            argument_node,
            ExpressionNode::Binary(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
                | ExpressionNode::StructLiteral(_)
        ),
        TypeReferenceNode::Named {
            name: type_name, ..
        } => {
            if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
                return matches!(argument_node, ExpressionNode::Boolean(_))
                    && primitive_type == PrimitiveType::Bool
                    || matches!(argument_node, ExpressionNode::String(_))
                        && primitive_type == PrimitiveType::String
                    || matches!(argument_node, ExpressionNode::Float(_))
                        && primitive_type.accepts_float_literal()
                    || matches!(argument_node, ExpressionNode::Integer(_))
                        && primitive_type.accepts_integer_literal()
                    || matches!(
                        argument_node,
                        ExpressionNode::Binary(_)
                            | ExpressionNode::Call(_)
                            | ExpressionNode::Cast(_)
                            | ExpressionNode::Indexed(_)
                            | ExpressionNode::Member(_)
                            | ExpressionNode::Name(_)
                            | ExpressionNode::StructLiteral(_)
                    );
            }

            matches!(
                argument_node,
                ExpressionNode::Binary(_)
                    | ExpressionNode::Call(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
                    | ExpressionNode::StructLiteral(_)
            )
        }
        TypeReferenceNode::Unit => false,
    }
}

fn validate_expression_type_handle(
    program: &TypedTrees,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ExpressionTypeOwner<'_>,
) {
    if !argument_matches_type_reference_handle(program, expression, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, expression)
        )));
    }
}

fn expression_type_name_handle(program: &TypedTrees, argument: ExpressionHandle) -> &'static str {
    match program.expression_table.expression(argument) {
        ExpressionNode::ArrayLiteral(_) => "array literal",
        ExpressionNode::Binary(_) => "binary expression",
        ExpressionNode::Boolean(_) => "bool",
        ExpressionNode::Call(_) => "call expression",
        ExpressionNode::Cast(_) => "cast expression",
        ExpressionNode::Float(_) => "float literal",
        ExpressionNode::Indexed(_) => "indexed value",
        ExpressionNode::Integer(_) => "integer literal",
        ExpressionNode::Member(_) => "member access",
        ExpressionNode::Mutable(inner_expression) => {
            expression_type_name_handle(program, *inner_expression)
        }
        ExpressionNode::Name(_) => "named value",
        ExpressionNode::Range(_) => "range expression",
        ExpressionNode::StructLiteral(_) => "struct literal",
        ExpressionNode::String(_) => "String",
    }
}

fn validate_transition_target_node(
    program: &TypedTrees,
    target: TransitionTargetHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TransitionTargetNode::Named { path, arguments } =
        program.statement_table.transition_target(target)
    else {
        return;
    };

    let path = program.statement_table.name_path_members(path.members);
    let arguments = program.statement_table.expression_handles(*arguments);

    if path.len() == 1 {
        let Some(state) = machine_symbols.state(path[0].as_str()) else {
            return;
        };

        validate_transition_arguments_handles(
            program,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );

        return;
    }

    if path.len() == 2 && path[0].as_str() == "self" {
        let Some(state) = machine_symbols.state(path[1].as_str()) else {
            return;
        };

        validate_transition_arguments_handles(
            program,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let Some(receiver_type) = machine_symbols.contained_type(path[0].as_str()) else {
        return;
    };

    if path.len() == 2 {
        let Some(machine) = symbols.machine(receiver_type) else {
            return;
        };

        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name == path[1])
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no state `{}`",
                machine.name, path[1]
            )));
            return;
        };

        validate_transition_arguments_handles(
            program,
            arguments,
            &state.name,
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
    }
}

fn validate_transition_arguments_handles(
    program: &TypedTrees,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_call_arguments_handles(
        program,
        arguments,
        target_name,
        parameters,
        writable_roots,
        diagnostics,
    );
}
