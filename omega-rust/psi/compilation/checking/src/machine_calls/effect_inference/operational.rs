//! Recursive suspension/blocking inference plus normalized call topology.
//!
//! Service reach is deliberately absent. Calls retain stable
//! `(state, statement, ordinal)` identity in grouped arenas so service reach,
//! capability approval, carry checks, and reports can join the same topology
//! without rebuilding leaf `Vec`s or reintroducing a global service catalog.
//! Exact typed named-operator targets remain distinct from machine targets so
//! early execution can validate their declaration authority without inventing
//! a call-closure machine edge.
//! Static argument bindings belong to the same call topology: retaining their
//! exact parameter-to-selection join lets generic summaries substitute rows
//! without recovering arguments from source spelling or a second body walk.

//! Operational summaries share borrow/semantic call-site coordinates. Every
//! named transfer reserves its parent ordinal before argument calls, including
//! internal state transfers that add no machine-call edge. Exact machine-entry
//! and requirement targets contribute to the ordinary recursive summaries.

use arena::HandleSpan;
use symbols::JoinedPath;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use typed_trees::state::State;
use typed_trees::statement::{TableCall, TableNamePath};

use super::call_sites::{CallSiteVisitor, visit_state_calls};

use flow_effects::{CallOperational, MachineOperational, OperationalPlan, StateOperational};

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachineWork {
    symbol: SymbolHandle,
    family_members: Vec<SymbolHandle>,
    parameters: Vec<SymbolHandle>,
    uses_published_contract: bool,
    published_may_suspend: bool,
    published_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
    body_may_suspend: bool,
    body_may_block: bool,
    states: Vec<StateWork>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateWork {
    symbol: SymbolHandle,
    direct_may_suspend: bool,
    direct_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
    calls: Vec<CallWork>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallWork {
    statement_index: usize,
    call_ordinal: usize,
    target_name: String,
    target_state_symbol: SymbolHandle,
    static_machine_parameter: SymbolHandle,
    static_machine_arguments: Box<[typed_trees::expression::StaticMachineArgument]>,
    target_machine_symbol: SymbolHandle,

    target_operator_symbol: SymbolHandle,
    direct_may_suspend: bool,
    direct_may_block: bool,
    transitive_may_suspend: bool,
    transitive_may_block: bool,
    acknowledgement: language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DirectCallOperational {
    pub(crate) may_suspend: bool,
    pub(crate) may_block: bool,
}

pub fn infer_operational_may(program: &dyn crate::CheckingInput) -> OperationalPlan {
    super::prepared_effects::operational_plan(program)
}

pub(crate) fn infer_operational_may_uncached(
    program: &dyn crate::CheckingInput,
) -> OperationalPlan {
    infer_operational_may_for_dependencies(
        program,
        &arena::Arena::default(),
        &flow_effects::CapturedSelectorCalls::default(),
    )
}

/// Settle the same operational graph with ownership-selected edge invocations.
/// The caller supplies exact custody selection, never inferred source calls.
pub fn infer_operational_may_for_dependencies(
    program: &dyn crate::CheckingInput,
    dependencies: &arena::Arena<flow_effects::CleanupDependency>,
    captured: &flow_effects::CapturedSelectorCalls,
) -> OperationalPlan {
    let signature_effects = DirectSignatureEffects::of(program);
    let mut machines = build_machine_work(&signature_effects, program);
    propagate_operational_may(program, &mut machines, dependencies, captured);
    let mut plan = build_plan(program, machines);
    plan.cleanup_dependencies = dependencies.clone();
    plan.captured_selector_calls = captured.clone();
    plan
}

fn build_machine_work(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
) -> Vec<MachineWork> {
    let mut machines = Vec::with_capacity(program.machines().len());

    for machine in program.machines() {
        let family_members = super::family_interface_members(program, machine)
            .iter()
            .map(|member| member.machine)
            .collect::<Vec<_>>();
        let uses_published_contract = family_members.is_empty()
            && (machine.is_public
                || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody);
        let states = program
            .machine_states(machine)
            .iter()
            .map(|state| StateWork {
                symbol: state.symbol,
                direct_may_suspend: false,
                direct_may_block: false,
                transitive_may_suspend: false,
                transitive_may_block: false,
                calls: collect_state_calls(signature_effects, program, machine.symbol, state),
            })
            .collect();

        let published = machine_operational(program, machine);
        machines.push(MachineWork {
            symbol: machine.symbol,
            family_members,
            parameters: program
                .machine_states(machine)
                .first()
                .map(|entry| {
                    program
                        .state_parameters(entry)
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .collect()
                })
                .unwrap_or_default(),
            uses_published_contract,
            published_may_suspend: published.may_suspend,
            published_may_block: published.may_block,
            transitive_may_suspend: uses_published_contract && published.may_suspend,
            transitive_may_block: uses_published_contract && published.may_block,
            body_may_suspend: false,
            body_may_block: false,
            states,
        });
    }

    machines
}

fn collect_state_calls(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    state: &State,
) -> Vec<CallWork> {
    let mut calls = Vec::new();

    let mut sites = OperationalCallSites {
        signature_effects,
        program,
        calls: &mut calls,
    };
    visit_state_calls(program, machine, state, &mut sites);

    // Evidence-child targets are lexical coordinates for public requirement
    // signatures, not executable states. Borrow/flow already retain that
    // public identity. Resolve it here while its declaring machine is known,
    // so effect attachment joins the same call rather than losing both its
    // acknowledgement and the requirement's suspension/blocking envelope.
    // Invalid evidence applications remain for ordinary source diagnostics;
    // this association grants no implementation or conformance authority.
    if let Some(owner) = program.machine_by_symbol(machine) {
        for call in &mut calls {
            if let Ok(Some(requirement)) = crate::named_conformance_target_requirement(
                program,
                owner,
                call.target_state_symbol,
            ) {
                call.target_state_symbol = requirement.signature.symbol;
                let direct = signature_effects.for_symbol(requirement.signature.symbol);
                call.direct_may_suspend = direct.may_suspend;
                call.direct_may_block = direct.may_block;
            }
        }
    }

    calls
}

/// Records the operational may-ceilings of one machine. The walk itself
/// belongs to `call_sites`; this pass supplies only what it records per site.
struct OperationalCallSites<'a> {
    signature_effects: &'a DirectSignatureEffects,
    program: &'a dyn crate::CheckingInput,
    calls: &'a mut Vec<CallWork>,
}

impl CallSiteVisitor for OperationalCallSites<'_> {
    fn statement_call(&mut self, call: &TableCall, statement_index: usize, ordinal: usize) {
        let target_operator_symbol = {
            let candidates =
                typed_trees::operator::named_statement_call_candidates(self.program, call);
            match candidates.as_slice() {
                [operator] => operator.symbol,
                _ => SymbolHandle::invalid(),
            }
        };
        push_call(
            self.signature_effects,
            self.program,
            call.target.as_str(),
            call.target_symbol,
            call.static_requirement_dispatch
                .as_ref()
                .map_or(SymbolHandle::invalid(), |dispatch| {
                    dispatch.contract_symbol()
                }),
            call.static_machine_parameter,
            &call.machine_arguments,
            target_operator_symbol,
            call.operational_acknowledgement,
            statement_index,
            ordinal,
            self.calls,
        );
    }

    fn expression_call(
        &mut self,
        call: &TableCallExpression,
        statement_index: usize,
        ordinal: usize,
    ) {
        if crate::proof_contracts::proof_embeddings::is_exact_embed_call(self.program, call) {
            // The proof-embedding owner validates this exact builtin's carrier
            // and proof-only occurrence. It forms a value term, not an
            // unresolved operational edge. The walk reserved its lexical
            // coordinate and still traverses every operand, so an effectful
            // operand cannot disappear with the term.
            return;
        }
        let named_operator =
            typed_trees::operator::resolve_named_expression_call(self.program, call);
        let target_operator_symbol = named_operator
            .map(|operator| operator.symbol)
            .unwrap_or_else(SymbolHandle::invalid);
        // A named boundary use carries no leaf target symbol; borrow and flow
        // record the same resolved operator identity, so the call summary must
        // key on it for the reach/operational attach to join the occurrence.
        let target_state_symbol = named_operator
            .filter(|operator| operator.is_boundary)
            .map_or(call.target_symbol, |operator| operator.symbol);
        push_call(
            self.signature_effects,
            self.program,
            call.target.as_str(),
            target_state_symbol,
            call.static_requirement_dispatch
                .as_ref()
                .map_or(SymbolHandle::invalid(), |dispatch| {
                    dispatch.contract_symbol()
                }),
            call.static_machine_parameter,
            &call.machine_arguments,
            target_operator_symbol,
            call.operational_acknowledgement,
            statement_index,
            ordinal,
            self.calls,
        );
    }

    fn named_transfer(
        &mut self,
        target: SymbolHandle,
        path: &TableNamePath,
        _arguments: HandleSpan<ExpressionHandle>,
        static_machine_parameter: SymbolHandle,
        acknowledgement: language_semantics::CallOperationalAcknowledgement,
        statement_index: usize,
        ordinal: usize,
    ) {
        let target_name = self
            .program
            .statement_table
            .name_path_members(path.members)
            .joined_path();
        push_call(
            self.signature_effects,
            self.program,
            &target_name,
            target,
            SymbolHandle::invalid(),
            static_machine_parameter,
            &[],
            SymbolHandle::invalid(),
            acknowledgement,
            statement_index,
            ordinal,
            self.calls,
        );
    }
}

/// Record one reached call site into `calls`. The walk owns the ordinal
/// counter, so this pass reads its position instead of advancing it.
fn push_call(
    signature_effects: &DirectSignatureEffects,
    program: &dyn crate::CheckingInput,
    target_name: &str,
    target_state_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    static_machine_parameter: SymbolHandle,
    static_machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    target_operator_symbol: SymbolHandle,
    acknowledgement: language_semantics::CallOperationalAcknowledgement,
    statement_index: usize,
    call_ordinal: usize,
    calls: &mut Vec<CallWork>,
) {
    let target_machine_symbol = machine_symbol_for_state(program, target_state_symbol);
    // Static selection changes execution, not the requirement's independent
    // acknowledgement envelope. Both nominal and structural binders retain it.
    let direct = program
        .retained_static_machine_contract(static_machine_parameter)
        .map(|contract| signature_operational(program, contract.signature()))
        .unwrap_or_else(|| {
            signature_effects.for_symbol(if requirement_symbol.is_valid() {
                requirement_symbol
            } else {
                target_state_symbol
            })
        });
    calls.push(CallWork {
        statement_index,
        call_ordinal,
        target_name: target_name.to_owned(),
        target_state_symbol,
        static_machine_parameter,
        static_machine_arguments: static_machine_arguments.into(),
        target_machine_symbol,

        target_operator_symbol,
        direct_may_suspend: direct.may_suspend,
        direct_may_block: direct.may_block,
        transitive_may_suspend: false,
        transitive_may_block: false,
        acknowledgement,
    });
}

fn machine_symbol_for_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
) -> SymbolHandle {
    program
        .machine_holding_state(state_symbol)
        .map(|machine| machine.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

/// Signature effects are declaration data for this inference pass. Collection
/// borrows these rows explicitly; nested inference owns another table and
/// cannot clear or replace its caller's answers.
struct DirectSignatureEffects {
    rows: Vec<(SymbolHandle, DirectCallOperational)>,
}

impl DirectSignatureEffects {
    fn of(program: &dyn crate::CheckingInput) -> Self {
        let mut symbols: Vec<_> = program
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter.symbol)
            .chain(
                program
                    .trait_machine_signatures
                    .iter()
                    .map(|(_, signature)| signature.symbol),
            )
            .chain(
                program
                    .closed_requirement_applications()
                    .map(|application| application.signature.symbol),
            )
            .chain(program.dynamic_slots.iter().map(|slot| slot.symbol))
            .filter(|symbol| symbol.is_valid())
            .collect();
        symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        symbols.dedup();
        let rows = symbols
            .into_iter()
            .map(|symbol| {
                (
                    symbol,
                    direct_operational_for_signature_symbol_uncached(program, symbol),
                )
            })
            .collect();
        Self { rows }
    }

    fn for_symbol(&self, symbol: SymbolHandle) -> DirectCallOperational {
        self.rows
            .binary_search_by_key(
                &(symbol.arena_index(), symbol.generation()),
                |(symbol, _)| (symbol.arena_index(), symbol.generation()),
            )
            .ok()
            .map(|position| self.rows[position].1)
            .unwrap_or_default()
    }
}

fn direct_operational_for_signature_symbol_uncached(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> DirectCallOperational {
    if let Some(requirement) = program
        .dynamic_slot_by_symbol(symbol)
        .and_then(|slot| program.dynamic_slot_requirement(slot))
    {
        return signature_operational(program, requirement);
    }
    if let Some((_, signature)) = program.machine_parameter_signature(symbol) {
        return signature_operational(program, signature);
    }

    if let Some((_, signature)) = program.trait_machine_signature(symbol) {
        return signature_operational(program, signature);
    }

    DirectCallOperational::default()
}

pub(crate) fn signature_operational(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> DirectCallOperational {
    let mut operational = DirectCallOperational {
        may_suspend: signature.suspends,
        may_block: signature.blocks,
    };
    super::callback_requirements::visit(
        program,
        super::callback_requirements::opaque_signature_parameters(program, signature),
        &super::declared_signature_invocations(program, signature),
        |invoked| {
            operational.may_suspend |= invoked.suspends;
            operational.may_block |= invoked.blocks;
        },
    );
    operational
}

/// Opaque top-level requirements publish the same exact callback envelope as
/// trait requirements. Checked machines derive calls from their body instead;
/// an invocation permission alone must not manufacture a body effect.
pub(crate) fn machine_operational(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> DirectCallOperational {
    let mut operational = DirectCallOperational {
        may_suspend: machine.suspends,
        may_block: machine.blocks,
    };
    if machine.supply_mode.is_boundary_declaration() && machine.has_bodyless_signature() {
        super::callback_requirements::visit(
            program,
            super::callback_requirements::opaque_machine_parameters(program, machine),
            &super::declared_machine_invocations(program, machine),
            |invoked| {
                operational.may_suspend |= invoked.suspends;
                operational.may_block |= invoked.blocks;
            },
        );
    }
    operational
}

/// A selector parameter's envelope is substituted per call, without cloning code.
/// Unknown inputs retain their declared ceilings. Only reached contexts are queued.
#[derive(Clone, PartialEq, Eq)]
struct OperationalContext {
    machine: usize,
    selectors: Vec<Option<Vec<SymbolHandle>>>,
    body: DirectCallOperational,
    summary: DirectCallOperational,
}

fn context_index(
    machines: &[MachineWork],
    contexts: &mut Vec<OperationalContext>,
    machine: usize,
    mut selectors: Vec<Option<Vec<SymbolHandle>>>,
) -> usize {
    selectors.resize(machines[machine].parameters.len(), None);
    if let Some(position) = contexts
        .iter()
        .position(|context| context.machine == machine && context.selectors == selectors)
    {
        return position;
    }
    let owner = &machines[machine];
    let position = contexts.len();
    contexts.push(OperationalContext {
        machine,
        selectors,
        body: DirectCallOperational::default(),
        summary: DirectCallOperational {
            may_suspend: owner.uses_published_contract && owner.published_may_suspend,
            may_block: owner.uses_published_contract && owner.published_may_block,
        },
    });
    position
}

fn selector_actual(
    captured: &flow_effects::CapturedSelectorCalls,
    call: Option<&flow_effects::CapturedSelectorCall>,
    position: usize,
    owner: &MachineWork,
    selectors: &[Option<Vec<SymbolHandle>>],
) -> Option<Vec<SymbolHandle>> {
    let argument = captured
        .arguments
        .span_or_empty(call?.arguments)
        .iter()
        .find(|argument| argument.source_position == position)?;
    let mut implementations = Vec::new();
    for origin in captured.origins.span_or_empty(argument.origins) {
        match origin {
            flow_effects::CapturedSelectorOrigin::Unresolved => return None,
            flow_effects::CapturedSelectorOrigin::Implementation(implementation) => {
                implementations.push(*implementation)
            }
            flow_effects::CapturedSelectorOrigin::Parameter(parameter) => {
                let position = owner
                    .parameters
                    .iter()
                    .position(|symbol| symbol == parameter)?;
                implementations.extend_from_slice(selectors.get(position)?.as_ref()?);
            }
        }
    }
    implementations.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    implementations.dedup();
    (!implementations.is_empty()).then_some(implementations)
}

#[allow(clippy::too_many_arguments)]
fn call_envelope(
    program: &dyn crate::CheckingInput,
    machines: &[MachineWork],
    captured: &flow_effects::CapturedSelectorCalls,
    contexts: &mut Vec<OperationalContext>,
    owner: &MachineWork,
    selectors: &[Option<Vec<SymbolHandle>>],
    state: SymbolHandle,
    call: &CallWork,
) -> DirectCallOperational {
    let mut records = captured.calls.iter().filter_map(|(_, record)| {
        (record.caller_state == state
            && record.statement_index == call.statement_index
            && record.call_ordinal == call.call_ordinal
            && record.target == call.target_state_symbol)
            .then_some(record)
    });
    let record = records.next().filter(|_| records.next().is_none());
    let mut envelope = DirectCallOperational {
        may_suspend: call.direct_may_suspend,
        may_block: call.direct_may_block,
    };
    let (targets, offset) = if let Some(slot) =
        program.dynamic_slot_by_symbol(call.target_state_symbol)
    {
        let selected =
            selector_actual(captured, record, 0, owner, selectors).and_then(|implementations| {
                implementations
                    .into_iter()
                    .map(|implementation| {
                        let implementation = program.conformance_by_symbol(implementation)?;
                        implementation
                            .rows
                            .iter()
                            .find(|row| {
                                row.declaring_trait == slot.declaring_trait
                                    && row.requirement == slot.requirement
                            })
                            .map(|row| row.realization_machine)
                    })
                    .collect::<Option<Vec<_>>>()
            });
        match selected.filter(|targets| {
            !targets.is_empty()
                && targets
                    .iter()
                    .all(|target| machines.iter().any(|machine| machine.symbol == *target))
        }) {
            Some(targets) => {
                envelope = DirectCallOperational::default();
                (targets, 1)
            }
            None => (Vec::new(), 1),
        }
    } else {
        (vec![call.target_machine_symbol], 0)
    };
    for target in targets {
        let Some(machine) = machines.iter().position(|machine| machine.symbol == target) else {
            continue;
        };
        let arguments = (0..machines[machine].parameters.len())
            .map(|position| selector_actual(captured, record, position + offset, owner, selectors))
            .collect();
        let target = context_index(machines, contexts, machine, arguments);
        envelope.may_suspend |= contexts[target].summary.may_suspend;
        envelope.may_block |= contexts[target].summary.may_block;
    }
    envelope
}

fn propagate_operational_may(
    program: &dyn crate::CheckingInput,
    machines: &mut [MachineWork],
    dependencies: &arena::Arena<flow_effects::CleanupDependency>,
    captured: &flow_effects::CapturedSelectorCalls,
) {
    let mut contexts = Vec::new();
    let roots = (0..machines.len())
        .map(|machine| context_index(machines, &mut contexts, machine, Vec::new()))
        .collect::<Vec<_>>();
    loop {
        let previous = contexts.clone();
        for position in 0..previous.len() {
            let owner = &machines[contexts[position].machine];
            let selectors = contexts[position].selectors.clone();
            let mut body = DirectCallOperational::default();
            for state in &owner.states {
                body.may_suspend |= state.direct_may_suspend;
                body.may_block |= state.direct_may_block;
                for call in &state.calls {
                    let envelope = call_envelope(
                        program,
                        machines,
                        captured,
                        &mut contexts,
                        owner,
                        &selectors,
                        state.symbol,
                        call,
                    );
                    body.may_suspend |= envelope.may_suspend;
                    body.may_block |= envelope.may_block;
                }
            }
            for target in dependencies.iter().filter_map(|(_, dependency)| {
                (dependency.caller_machine == owner.symbol).then_some(dependency.target_machine)
            }) {
                if let Some(machine) = machines.iter().position(|machine| machine.symbol == target)
                {
                    let target = context_index(machines, &mut contexts, machine, Vec::new());
                    body.may_suspend |= contexts[target].summary.may_suspend;
                    body.may_block |= contexts[target].summary.may_block;
                }
            }
            // A target family owns an envelope, not a source body to validate.
            let mut family = DirectCallOperational::default();
            for target in &owner.family_members {
                if let Some(machine) = machines
                    .iter()
                    .position(|machine| machine.symbol == *target)
                {
                    let target = context_index(machines, &mut contexts, machine, selectors.clone());
                    family.may_suspend |= contexts[target].summary.may_suspend;
                    family.may_block |= contexts[target].summary.may_block;
                }
            }
            contexts[position].body = body;
            contexts[position].summary = if owner.uses_published_contract {
                DirectCallOperational {
                    may_suspend: owner.published_may_suspend,
                    may_block: owner.published_may_block,
                }
            } else {
                DirectCallOperational {
                    may_suspend: body.may_suspend || family.may_suspend,
                    may_block: body.may_block || family.may_block,
                }
            };
        }
        if contexts == previous {
            break;
        }
    }
    let declaration_summaries = machines
        .iter()
        .enumerate()
        .map(|(position, machine)| (machine.symbol, contexts[roots[position]].summary))
        .collect::<Vec<_>>();
    // Publish each declaration under unknown fitting inputs. Its call sites can
    // independently consume a narrower context without changing the body or markers.
    for machine_index in 0..machines.len() {
        let root = roots[machine_index];
        let summary = contexts[root].clone();
        let mut call_summaries = Vec::new();
        for state in &machines[machine_index].states {
            for call in &state.calls {
                call_summaries.push(call_envelope(
                    program,
                    machines,
                    captured,
                    &mut contexts,
                    &machines[machine_index],
                    &summary.selectors,
                    state.symbol,
                    call,
                ));
            }
        }
        let machine = &mut machines[machine_index];
        machine.body_may_suspend = summary.body.may_suspend;
        machine.body_may_block = summary.body.may_block;
        machine.transitive_may_suspend = summary.summary.may_suspend;
        machine.transitive_may_block = summary.summary.may_block;
        let mut summaries = call_summaries.into_iter();
        for state in &mut machine.states {
            state.transitive_may_suspend = state.direct_may_suspend;
            state.transitive_may_block = state.direct_may_block;
            for call in &mut state.calls {
                if let Some(envelope) = summaries.next() {
                    // The selected row replaces the slot ceiling for every consumer.
                    call.direct_may_suspend &= envelope.may_suspend;
                    call.direct_may_block &= envelope.may_block;
                    call.transitive_may_suspend = envelope.may_suspend;
                    call.transitive_may_block = envelope.may_block;
                }
                state.transitive_may_suspend |= call.transitive_may_suspend;
                state.transitive_may_block |= call.transitive_may_block;
            }
            for (_, dependency) in dependencies.iter().filter(|(_, dependency)| {
                dependency.caller_machine == machine.symbol
                    && dependency.caller_state == state.symbol
            }) {
                if let Some(target) = declaration_summaries.iter().find_map(|(symbol, summary)| {
                    (*symbol == dependency.target_machine).then_some(*summary)
                }) {
                    state.transitive_may_suspend |= target.may_suspend;
                    state.transitive_may_block |= target.may_block;
                }
            }
        }
    }
}

fn retain_static_machine_bindings(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
    arguments: &[typed_trees::expression::StaticMachineArgument],
    bindings: &mut arena::Arena<flow_effects::StaticMachineCallBinding>,
) -> HandleSpan<flow_effects::StaticMachineCallBinding> {
    let mut result = HandleSpan::empty();
    let Some((machine, _)) =
        crate::declarations::transitions::resolved_transition_target_state(program, target)
    else {
        return result;
    };
    let parameters = program.machine_kind_type_parameters(machine);
    // Static arguments occupy category-specific slots, matching specialization:
    // type and const arguments do not shift a machine parameter's ordinal.
    let selected = arguments.iter().filter(|argument| {
        argument.symbol.is_valid()
            && matches!(
                program.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::State | symbols::SymbolKind::MachineParameter
            )
    });
    for (parameter, selected) in parameters.zip(selected) {
        let nested = selected
            .application
            .as_ref()
            .map_or(&[][..], |application| application.arguments.as_ref());
        let arguments = retain_static_machine_bindings(program, selected.symbol, nested, bindings);
        bindings.append_to_span(
            &mut result,
            flow_effects::StaticMachineCallBinding {
                parameter: parameter.symbol,
                selected: selected.symbol,
                arguments,
            },
        );
    }
    result
}

fn build_plan(program: &dyn crate::CheckingInput, machines: Vec<MachineWork>) -> OperationalPlan {
    let mut plan = OperationalPlan::default();

    for machine in machines {
        let mut states = HandleSpan::empty();
        for state in machine.states {
            let mut calls = HandleSpan::empty();
            for call in state.calls {
                let static_machine_bindings = retain_static_machine_bindings(
                    program,
                    call.target_state_symbol,
                    &call.static_machine_arguments,
                    &mut plan.static_machine_bindings,
                );
                let acknowledgement = if call.acknowledgement.origin
                    == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
                {
                    language_semantics::CallOperationalAcknowledgement {
                        acknowledges_suspend: call.transitive_may_suspend,
                        acknowledges_block: call.transitive_may_block,
                        ..call.acknowledgement
                    }
                } else {
                    call.acknowledgement
                };
                plan.calls.append_to_span(
                    &mut calls,
                    CallOperational {
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_name: call.target_name,
                        target_state_symbol: call.target_state_symbol,
                        static_machine_parameter: call.static_machine_parameter,
                        static_machine_bindings,
                        target_machine_symbol: call.target_machine_symbol,
                        target_operator_symbol: call.target_operator_symbol,
                        direct_may_suspend: call.direct_may_suspend,
                        direct_may_block: call.direct_may_block,
                        transitive_may_suspend: call.transitive_may_suspend,
                        transitive_may_block: call.transitive_may_block,
                        acknowledgement,
                    },
                );
            }
            plan.states.append_to_span(
                &mut states,
                StateOperational {
                    symbol: state.symbol,
                    direct_may_suspend: state.direct_may_suspend,
                    direct_may_block: state.direct_may_block,
                    transitive_may_suspend: state.transitive_may_suspend,
                    transitive_may_block: state.transitive_may_block,
                    calls,
                },
            );
        }
        plan.machines.append_to_span(
            &mut plan.root_machines,
            MachineOperational {
                symbol: machine.symbol,
                published_may_suspend: machine.published_may_suspend,
                published_may_block: machine.published_may_block,
                transitive_may_suspend: machine.transitive_may_suspend,
                transitive_may_block: machine.transitive_may_block,
                body_may_suspend: machine.body_may_suspend,
                body_may_block: machine.body_may_block,
                states,
            },
        );
    }

    plan
}
