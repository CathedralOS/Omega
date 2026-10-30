//! Proof-machine recursive-call decrease and citation validation.
//!
//! Runtime tail-position legality remains with the parent. This child owns the
//! structural decrease judgment, recursive-call collection, guard provenance,
//! and sub-state descent closure. Computed proof-data predecessors use the same
//! structural judge as contract entailment; a separate spelling-based citation
//! matcher could borrow guarantees from a different declaration and bypass the
//! judge's premise checks.

use super::is_current_entry_call;
use crate::value_custody::places::binding_observations::{
    binding_is_unchanged_value, immutable_value_origin,
};
use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

mod slices;

/// One independently checked recursive call, retained only for source progress
/// closure. Computed strict descent is not a structural-subterm IH witness and
/// supplies no exported proof-recursion certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSelfCallDescent {
    pub machine: symbols::SymbolHandle,
    pub entry: symbols::SymbolHandle,
    pub state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub rank_parameter: symbols::SymbolHandle,
    pub rank_argument: ExpressionHandle,
}

/// The arithmetic predecessor bridge for proof recursion over an integer
/// measure.  A call nested in the TAKEN value of `transition n > 0` may pass
/// `n - 1`: the guard proves subtraction cannot underflow and the result is
/// strictly below `n`.  Keep this association syntactic and local -- an
/// unrelated positive guard elsewhere in the state must not license the
/// call.
fn guarded_integer_predecessor_call(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    call_ordinal: usize,
    statement: &StatementNode,
    argument: ExpressionHandle,
    measure_symbol: symbols::SymbolHandle,
) -> bool {
    let has_builtin_meaning = |expression, spelling, left, right| {
        let operand_types = [left, right].map(|operand| {
            crate::expression_result_type_reference(program, machine, state, operand)
        });
        typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            state.symbol,
            expression,
            spelling,
            &operand_types,
        )
    };
    let Some(predecessor) = program.expression_table.binary_node(argument) else {
        return false;
    };
    if predecessor.operator != typed_trees::expression::BinaryOperator::Subtract
        || !has_builtin_meaning(
            argument,
            language_core::OperatorSpelling::Subtract,
            predecessor.left,
            predecessor.right,
        )
        || !expression_names_measure(program, predecessor.left, measure_symbol)
        || !matches!(program.expression_table.expression(predecessor.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(1))
    {
        return false;
    }
    let Some(transition) = statement.as_transition() else {
        return false;
    };
    if crate::transition_call_target(program, machine, state, statement_index, call_ordinal)
        != Some(transition.target)
    {
        return false;
    }
    let TransitionGuardNode::When(guard) = transition.guard else {
        return false;
    };
    let guard = match program.expression_table.expression(guard) {
        ExpressionNode::Binary(wrapper)
            if wrapper.operator == typed_trees::expression::BinaryOperator::Equal
                && has_builtin_meaning(
                    guard,
                    language_core::OperatorSpelling::Equal,
                    wrapper.left,
                    wrapper.right,
                )
                && matches!(
                    program.expression_table.expression(wrapper.right),
                    ExpressionNode::Boolean(true)
                ) =>
        {
            wrapper.left
        }
        _ => guard,
    };
    let Some(positive) = program.expression_table.binary_node(guard) else {
        return false;
    };
    positive.operator == typed_trees::expression::BinaryOperator::Greater
        && has_builtin_meaning(
            guard,
            language_core::OperatorSpelling::Greater,
            positive.left,
            positive.right,
        )
        && expression_names_measure(program, positive.left, measure_symbol)
        && matches!(program.expression_table.expression(positive.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(0))
}

fn expression_names_measure(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    measure_symbol: symbols::SymbolHandle,
) -> bool {
    let Some(path) = program.expression_table.name_path_node(expression) else {
        return false;
    };
    path.symbol.is_valid() && measure_symbol.is_valid() && path.symbol == measure_symbol
}

/// A direct self-application whose ranked operand is a strict constructor
/// subterm on every incoming state edge. This query deliberately excludes
/// cited decrease and contract normalization: an earlier call may use the
/// resulting IH to establish its own premises, so its guarantees cannot also
/// be the reason that induction is licensed.
/// `machine` is the origin of the current proof invocation; `state` owns the
/// exact recursive operand, possibly inside an explicitly invoked helper.
/// Every return to that origin must descend under the complete component.
pub(crate) fn proof_call_has_structural_descent(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    before: usize,
    expression: ExpressionHandle,
) -> bool {
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    call.target_symbol == entry.symbol
        && !call.receiver.is_valid()
        && crate::machine_calls::call_cycles::structural_return_descends(
            program, machine, state, before, expression,
        )
}

pub(crate) fn validate_proof_machine_recursion(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedSelfCallDescent> {
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    let entry_name = machine.name.as_str();
    let self_calls =
        crate::collect_call_sites_in_statement(program, machine, state, statement_index)
            .into_iter()
            .enumerate()
            .filter(|(_, site)| {
                if site.static_requirement_dispatch().is_some() {
                    return false;
                }
                match site {
                    crate::CallSite::Expression { call, .. } => {
                        is_current_entry_call(program, machine, call)
                    }
                    crate::CallSite::Statement(call) => {
                        call.target_symbol.is_valid()
                            && (call.target_symbol == machine.symbol
                                || call.target_symbol == entry.symbol)
                    }
                    crate::CallSite::TransitionNamed { path, .. } => {
                        path.symbol.is_valid()
                            && (path.symbol == machine.symbol || path.symbol == entry.symbol)
                    }
                }
            })
            .collect::<Vec<_>>();
    if self_calls.is_empty() {
        return Vec::new();
    }

    let Some(subjects) = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}` needs a single structural measure: declare \
             `terminates by <param>;` naming one proof-data parameter -- a \
             cycle without a measure is an unproven termination claim (measured \
             recursion, both strata)",
            machine.name,
        )));
        return Vec::new();
    };
    let [subject] = subjects.as_slice() else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}` needs a single structural measure: declare \
             `terminates by <param>;` naming one proof-data parameter -- a \
             cycle without a measure is an unproven termination claim (measured \
             recursion, both strata)",
            machine.name,
        )));
        return Vec::new();
    };
    let Some(measure_path) = program.expression_table.name_path_node(*subject) else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}`: the structural measure must be a bare \
             parameter name (rung 1); compound measures over proof data are not \
             proven yet",
            machine.name,
        )));
        return Vec::new();
    };
    let measure_symbol = measure_path.symbol;
    let measure_name = program
        .expression_table
        .name_path_members(measure_path.members)
        .first()
        .cloned();
    // The measure names an ENTRY parameter; its POSITION is where every
    // self-call's argument must descend.
    let Some(measure_position) = program.machine_states(machine).first().and_then(|entry| {
        program
            .state_parameters(entry)
            .iter()
            .position(|parameter| {
                parameter.symbol.is_valid()
                    && measure_symbol.is_valid()
                    && parameter.symbol == measure_symbol
            })
    }) else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}`: the measure must name an entry parameter",
            machine.name,
        )));
        return Vec::new();
    };

    // The slice-tail decrease probe scans the whole program once for its
    // bound index; share that scan across every self-call argument here.
    let mut bound_lookup = None;
    let diagnostics_before = diagnostics.len();
    let mut validated = Vec::new();
    for (call_ordinal, site) in self_calls {
        let arguments = crate::call_site_argument_expressions(program, &site);
        let argument = arguments.get(measure_position).copied();
        let descends = argument.is_some_and(|argument| {
            strict_subterm_of_measure(program, state, statement_index, argument, measure_symbol)
                || substate_parameter_descends(
                    program,
                    machine,
                    state,
                    statement_index,
                    argument,
                    measure_symbol,
                )
                || program.machine_states(machine).first().is_some_and(|entry| {
                    crate::proof_contracts::contract_entailment::proof_edge_strict_decrease_judged(
                        program,
                        machine,
                        state,
                        statement_index,
                        argument,
                        &program.state_parameters(entry)[measure_position],
                    )
                })
                || guarded_integer_predecessor_call(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    statement,
                    argument,
                    measure_symbol,
                )
                || slices::guarded_slice_tail_call(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    statement,
                    argument,
                    measure_position,
                    &mut bound_lookup,
                )
        });
        if let Some(argument) = argument.filter(|_| descends) {
            validated.push(ValidatedSelfCallDescent {
                machine: machine.symbol,
                entry: entry.symbol,
                state: state.symbol,
                statement_index,
                call_ordinal,
                rank_parameter: program.state_parameters(entry)[measure_position].symbol,
                rank_argument: argument,
            });
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "`{entry_name}(..)` cannot prove the measure `{}` structurally \
                 decreases at this self-call: the call does not prove a strict \
                 predecessor of ranking subject `{}`. Pass a case-payload subterm \
                 (`Nat::Succ {{ prev }} -> .. {entry_name}(prev)`) or, for an integer \
                 measure, `n - 1` in the taken value of its dominating `n > 0` arm; \
                 a slice-length measure admits `items[1..]` under its own nonempty guard",
                measure_name
                    .as_ref()
                    .map(|name| name.as_str())
                    .unwrap_or("<measure>"),
                measure_name
                    .as_ref()
                    .map(|name| name.as_str())
                    .unwrap_or("<measure>"),
            )));
        }
    }
    if diagnostics.len() == diagnostics_before {
        validated
    } else {
        Vec::new()
    }
}

/// A STRICT subterm of the measure: a Member chain of one or more hops whose
/// root Name is the resolved measure parameter. One hop
/// = one constructor consumed (`n.prev`); depth composes for free.
/// N3 rung 2 -- descent THROUGH a sub-state parameter: a self-call inside a
/// per-arm sub-proof passes the sub-state's own parameter (`step_case(prev,
/// b)` bound it at the entry arm from the measure's case payload, so inside
/// `step_case` the recursion argument is the bare name `prev`). The
/// parameter counts as strictly descending iff EVERY Named transition into
/// its state passes either a strict-subterm Member read of the measure or a
/// previously proven descending sub-state parameter. The latter closure is
/// what preserves provenance through proof choreography (`prev` forwarded
/// through two case-analysis states before the recursive call). Provenance is
/// still over ALL binding sites, so a single non-descending entry poisons the
/// parameter and cycles without a direct strict-subterm seed prove nothing.
///
/// Only resolved parameter and transition identities carry provenance.
/// A same-spelled local or parameter cannot stand in for the ranked binder.
fn substate_parameter_descends(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    source: &typed_trees::state::State,
    before: usize,
    argument: ExpressionHandle,
    measure_symbol: symbols::SymbolHandle,
) -> bool {
    let Some((_, argument)) = immutable_value_origin(program, source, before, argument) else {
        return false;
    };
    let Some(path) = program.expression_table.name_path_node(argument) else {
        return false;
    };
    if !path.symbol.is_valid() {
        return false;
    }
    let states = program.machine_states(machine);
    let candidates = states
        .iter()
        .skip(1)
        .flat_map(|state| {
            program
                .state_parameters(state)
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    parameter.symbol.is_valid() && parameter.symbol == path.symbol
                })
                .map(move |(position, _)| (state, position))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return false;
    }
    let parameters: Vec<(&typed_trees::state::State, usize, symbols::SymbolHandle)> = states[1..]
        .iter()
        .flat_map(|state| {
            program
                .state_parameters(state)
                .iter()
                .enumerate()
                .map(move |(position, parameter)| (state, position, parameter.symbol))
        })
        .collect();
    let mut descending: Vec<symbols::SymbolHandle> = Vec::new();
    loop {
        let mut gained = Vec::new();
        for (sub_state, position, parameter_symbol) in &parameters {
            if !parameter_symbol.is_valid()
                || descending.iter().any(|known| known == parameter_symbol)
            {
                continue;
            }
            let mut binding_sites = 0usize;
            let mut all_descend = true;
            for source in states {
                for (statement_index, statement) in program
                    .statement_table
                    .indexed_statements(source.statement_nodes)
                {
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    for target_handle in transition.targets() {
                        let TransitionTargetNode::Named {
                            path, arguments, ..
                        } = program.statement_table.transition_target(target_handle)
                        else {
                            continue;
                        };
                        if !path.symbol.is_valid() || path.symbol != sub_state.symbol {
                            continue;
                        }
                        binding_sites += 1;
                        let handles = program.statement_table.expression_handles(*arguments);
                        let Some(bound) = handles.get(*position) else {
                            all_descend = false;
                            continue;
                        };
                        let forwarded =
                            immutable_value_origin(program, source, statement_index, *bound)
                                .and_then(|(_, origin)| {
                                    program.expression_table.name_path_node(origin)
                                })
                                .is_some_and(|path| {
                                    path.symbol.is_valid() && descending.contains(&path.symbol)
                                });
                        if !forwarded
                            && !strict_subterm_of_measure(
                                program,
                                source,
                                statement_index,
                                *bound,
                                measure_symbol,
                            )
                        {
                            all_descend = false;
                        }
                    }
                }
            }
            if binding_sites > 0 && all_descend {
                gained.push(*parameter_symbol);
            }
        }
        if gained.is_empty() {
            break;
        }
        descending.extend(gained);
    }
    candidates.iter().all(|(state, position)| {
        program
            .state_parameters(state)
            .get(*position)
            .is_some_and(|parameter| {
                parameter.symbol.is_valid() && descending.contains(&parameter.symbol)
            })
    })
}

/// Earlier immutable values retain identity, but never manufacture descent.
/// At least one actual constructor-member hop remains mandatory. Unknown or
/// reference-backed projections and changed storage cannot stand for the
/// ranking entry value when a captured alias rejoins it.
fn strict_subterm_of_measure(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    mut before: usize,
    mut expression: ExpressionHandle,
    measure_symbol: symbols::SymbolHandle,
) -> bool {
    let mut receivers = Vec::new();
    let mut seen = Vec::new();
    let mut followed_alias = false;
    loop {
        if !program.expression_table.expression_is_valid(expression) || seen.contains(&expression) {
            return false;
        }
        seen.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                receivers.push(member.receiver);
                expression = member.receiver;
            }
            ExpressionNode::Name(path) if path.symbol.is_valid() => {
                if path.symbol == measure_symbol {
                    if receivers.is_empty() {
                        return false;
                    }
                    if !followed_alias {
                        return true;
                    }
                    let Some(parameter) = program.state_parameter_by_symbol(state, measure_symbol)
                    else {
                        return false;
                    };
                    if parameter.is_mutable
                        || crate::reference_shell(program, parameter.type_reference).is_some()
                        || !binding_is_unchanged_value(program, state, measure_symbol)
                    {
                        return false;
                    }
                    let Some(machine) = program.machine_holding_state(state.symbol) else {
                        return false;
                    };
                    return receivers.iter().all(|receiver| {
                        crate::expression_result_type_reference(program, machine, state, *receiver)
                            .is_some_and(|reference| {
                                crate::reference_shell(program, reference).is_none()
                            })
                    });
                }
                let Some((position, origin)) =
                    immutable_value_origin(program, state, before, expression)
                else {
                    return false;
                };
                if origin == expression {
                    return false;
                }
                followed_alias = true;
                before = position;
                expression = origin;
            }
            // A call's guarantee or a freshly constructed wrapper is not a
            // strict syntactic subterm and cannot establish its own IH.
            _ => return false,
        }
    }
}
