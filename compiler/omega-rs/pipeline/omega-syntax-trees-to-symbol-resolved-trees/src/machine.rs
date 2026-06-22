use crate::data::lower_type_parameters;
use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::state::{lower_signature_contracts, lower_signature_effects, lower_state_node};
use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::machine::{Machine, MachineStorage, TraitConformance};
use omega_symbol_resolved_trees::state::State;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<(), Diagnostic> {
    let states = lower_machine_states(lowerer, syntax_trees, machine.states)?;
    let type_parameters = lower_type_parameters(lowerer, syntax_trees, machine.type_parameters)?;
    let satisfies = lower_machine_trait_conformances(lowerer, syntax_trees, machine.satisfies);
    let decreases = lower_machine_decreases(lowerer, syntax_trees, machine.decreases)?;
    let decrease_order =
        lower_machine_decrease_order(lowerer, syntax_trees, machine.decrease_order);
    let effects = lower_signature_effects(lowerer, syntax_trees, machine.effects);
    let contracts = lower_signature_contracts(lowerer, syntax_trees, machine.contracts)?;
    let machine_name = crate::name::lower_name(&machine.name);
    let attached_data = machine.attached_data.as_ref().map(crate::name::lower_name);

    lowerer.symbol_resolved_trees.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        name: machine_name,
        attached_data,
        abi: machine.abi.clone(),
        storage: MachineStorage {
            type_parameters,
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies,
            terminates: machine.terminates,
            decreases,
            decrease_order,
            effects,
            contracts,
            states,
        },
    });
    Ok(())
}

fn lower_machine_decrease_order(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    order: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<omega_symbol_resolved_trees::name::DiagnosticName> {
    let mut lowered = HandleSpan::empty();

    for member in syntax_trees.items.identifier_path_members(order) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .signature_effects
            .append_to_span(&mut lowered, crate::name::lower_name(member));
    }

    lowered
}

fn lower_machine_decreases(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    decreases: HandleSpan<syntax::expression::ExpressionHandle>,
) -> Result<HandleSpan<omega_symbol_resolved_trees::expression::ExpressionHandle>, Diagnostic> {
    let mut expressions = Vec::new();

    for expression in syntax_trees.expressions.expression_handles(decreases) {
        let expression = lower_expression_into_table(
            syntax_trees,
            &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
            *expression,
        )?;
        expressions.push(expression);
    }

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert_expression_handles(expressions))
}

fn lower_machine_trait_conformances(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    satisfies: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<TraitConformance> {
    let mut span = HandleSpan::empty();

    for trait_name in syntax_trees.items.identifier_path_members(satisfies) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_trait_conformances
            .append_to_span(
                &mut span,
                TraitConformance {
                    symbol: SymbolHandle::invalid(),
                    name: crate::name::lower_name(trait_name),
                },
            );
    }

    span
}

fn lower_machine_states(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    states: HandleSpan<syntax::item::StateHandle>,
) -> Result<HandleSpan<Handle<State>>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for state in syntax_trees.items.state_handles(states) {
        let state = lower_state_node(lowerer, syntax_trees, syntax_trees.items.state(*state))?;
        let state = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_states
            .append(state);
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_state_handles
            .append_to_span(&mut span, state);
    }

    Ok(span)
}
