use crate::data::lower_type_parameters;
use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::state::{lower_signature_contracts, lower_signature_effects, lower_state_node};
use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::machine::{
    Machine, MachineStorage, RankingRange, RankingWitness, TraitConformance,
};
use omega_symbol_resolved_trees::state::State;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<(), Diagnostic> {
    lowerer.current_machine_is_boundary = machine.boundary;
    let states = lower_machine_states(lowerer, syntax_trees, machine.states)?;
    lowerer.current_machine_is_boundary = false;
    let type_parameters = lower_type_parameters(lowerer, syntax_trees, machine.type_parameters)?;
    let satisfies = lower_machine_trait_conformances(lowerer, syntax_trees, machine.satisfies);
    let ranking_witness = lower_ranking_witness(lowerer, syntax_trees, machine.ranking_witness)?;
    let effects = lower_signature_effects(lowerer, syntax_trees, machine.effects);
    let contracts = lower_signature_contracts(lowerer, syntax_trees, machine.contracts)?;
    let machine_name = crate::name::lower_name(&machine.name);
    let attached_data = machine.attached_data.as_ref().map(crate::name::lower_name);

    lowerer.symbol_resolved_trees.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        name: machine_name,
        attached_data,
        boundary: machine.boundary,
        storage: MachineStorage {
            type_parameters,
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies,
            termination_guarantee: machine.termination_guarantee.into(),
            ranking_witness,
            effects,
            contracts,
            states,
        },
    });
    Ok(())
}

fn lower_ranking_view(
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

fn lower_ranking_expressions(
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

pub(crate) fn lower_ranking_witness(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    witness: syntax::item::RankingWitnessSyntax,
) -> Result<RankingWitness, Diagnostic> {
    let subjects = lower_ranking_expressions(lowerer, syntax_trees, witness.subjects)?;
    let view = lower_ranking_view(lowerer, syntax_trees, witness.view);
    let view_arguments = lower_ranking_expressions(lowerer, syntax_trees, witness.view_arguments)?;
    let range = if witness.range.is_present() {
        RankingRange {
            start: lower_expression_into_table(
                syntax_trees,
                &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
                witness.range.start,
            )?,
            end: lower_expression_into_table(
                syntax_trees,
                &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
                witness.range.end,
            )?,
            end_inclusive: witness.range.end_inclusive,
        }
    } else {
        RankingRange::default()
    };
    Ok(RankingWitness {
        subjects,
        view,
        view_arguments,
        range,
    })
}

fn lower_machine_trait_conformances(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    satisfies: HandleSpan<syntax::item::SatisfiesClause>,
) -> HandleSpan<TraitConformance> {
    let mut span = HandleSpan::empty();

    for clause in syntax_trees.items.satisfies_clauses(satisfies) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_trait_conformances
            .append_to_span(
                &mut span,
                TraitConformance {
                    symbol: SymbolHandle::invalid(),
                    name: crate::name::lower_name(&clause.trait_name),
                    requirement: clause.requirement.as_ref().map(crate::name::lower_name),
                    alias: clause.alias.as_ref().map(crate::name::lower_name),
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
