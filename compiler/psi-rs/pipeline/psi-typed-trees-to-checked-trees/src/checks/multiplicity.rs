use psi_arena::HandleSpan;
use psi_checked_trees::{
    CheckFacts, FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowClaimOutcomeSource,
    FlowPermissionEventFact,
};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use crate::flow::FlowOwnershipEventSource;

#[derive(Debug, Clone)]
struct LinearPlace {
    symbol: SymbolHandle,
    name: String,
    /// Canonical claim path below `symbol`. An empty path is one nominal claim;
    /// transparent records, active cases, and fixed arrays contribute one
    /// entry per contained linear claim instead of inventing an aggregate root.
    path: Vec<psi_facts::PlaceSegment>,
    multiplicity: Multiplicity,
    claim_identity: Option<PermissionClaimIdentity>,
    provenance: Option<PermissionProvenance>,
    live: bool,
    /// Parameters are established on entry. A local is established only by an
    /// explicit initializer/assignment; implicit zero-fill creates no debt.
    ever_established: bool,
    /// An affine sum can carry a linear payload only in selected cases. When
    /// false, `live` is unconditional for every established value; when true,
    /// `live` follows the active case.
    conditional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WrittenLinearTarget {
    root: psi_facts::PlaceRoot,
    destination_path: Vec<psi_facts::PlaceSegment>,
    place_index: usize,
    obligation_live: bool,
    claim_identity: Option<PermissionClaimIdentity>,
    provenance: Option<PermissionProvenance>,
}

#[derive(Debug, Clone)]
pub(super) struct LinearClaimTemplate {
    pub(super) path: Vec<psi_facts::PlaceSegment>,
    pub(super) type_reference: TypeReferenceHandle,
    pub(super) multiplicity: Multiplicity,
    pub(super) conditional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedClaimOutcomeMap {
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    entries: Vec<CheckedClaimOutcomeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedClaimOutcomeEntry {
    output_path: Vec<psi_facts::PlaceSegment>,
    source: CheckedClaimOutcomeSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckedClaimOutcomeSource {
    Input {
        parameter_symbol: SymbolHandle,
        path: Vec<psi_facts::PlaceSegment>,
    },
    Established {
        claim_identity: PermissionClaimIdentity,
        provenance: PermissionProvenance,
    },
}

#[derive(Debug, Default)]
struct ClaimIdentityAllocator {
    next_ordinal: u32,
}

impl ClaimIdentityAllocator {
    fn mint(
        &mut self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        source: PermissionEventSource,
    ) -> PermissionClaimIdentity {
        let ordinal = self.next_ordinal;
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .expect("permission claim identity ordinal overflow");
        PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source,
            ordinal,
        }
    }
}

pub(crate) fn check_linear_obligations(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    incoming_guards: &super::ranges::incoming_guards::IncomingGuardIndex,
) -> Result<(), Vec<Diagnostic>> {
    validate_nominal_drop_moves(program, facts)?;
    record_permission_events_with_incoming_guards(program, facts, incoming_guards);
    validate_linear_permission_events(program, facts)
}

/// A nominal cleanup machine is entitled to one whole valid value. Reject a
/// move below any prefix carrying that entitlement; structural records without
/// nominal cleanup remain decomposable, and moving the entitled value itself
/// remains legal.
fn validate_nominal_drop_moves(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut segments = psi_arena::Arena::default();
            let moves = crate::flow::discover_state_move_events(
                program,
                &facts.borrow,
                machine,
                state,
                &mut segments,
            );
            for event in moves {
                let path = segments.span_or_empty(event.segments);
                if path.is_empty() || move_event_is_production_target(program, state, &event, path)
                {
                    continue;
                }
                for prefix_len in 0..path.len() {
                    if prefix_len == 0 && event_is_owned_self_projection(program, state, &event) {
                        continue;
                    }
                    let prefix = crate::flow::CanonicalPlace {
                        root: event.root,
                        segments: path[..prefix_len].to_vec(),
                    };
                    let Some(type_name) = nominal_drop_place_name(
                        program,
                        state.symbol,
                        event_statement_index(event.source).unwrap_or(0),
                        &prefix,
                    ) else {
                        continue;
                    };
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot partially move a value of `{type_name}` because `{type_name}::drop` requires the whole value"
                    )));
                    break;
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn event_is_owned_self_projection(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    event: &crate::flow::DiscoveredMoveEvent,
) -> bool {
    let psi_facts::PlaceRoot::Symbol(event_root) = event.root else {
        return false;
    };
    if !program.machines().iter().any(|machine| {
        machine.symbol == event_root
            && program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
    }) {
        return false;
    }
    program.state_parameters(state).iter().any(|parameter| {
        parameter.is_self && !type_reference_is_reference(program, parameter.type_reference)
    })
}

fn type_reference_is_reference(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_reference(program, *base_type)
        }
        _ => false,
    }
}

fn nominal_drop_place_name<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
) -> Option<&'program str> {
    if place.segments.is_empty()
        && let psi_facts::PlaceRoot::Symbol(root) = place.root
        && let Some(attached) = program.machines().iter().find_map(|machine| {
            (machine.symbol == root
                && program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == state_symbol))
            .then(|| machine.attached_data.as_deref())
            .flatten()
        })
    {
        return data_name_with_nominal_drop(program, attached);
    }
    let type_reference =
        crate::flow::canonical_place_type_reference(program, state_symbol, statement_index, place)?;
    nominal_drop_type_name(program, type_reference)
}

fn data_name_with_nominal_drop<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    name: &str,
) -> Option<&'program str> {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)?;
    program
        .machines()
        .iter()
        .any(|machine| {
            machine.name.as_str().ends_with("::drop")
                && machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|attached| attached == &definition.name)
        })
        .then_some(definition.name.as_str())
}

fn move_event_is_production_target(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[psi_facts::PlaceSegment],
) -> bool {
    let Some(statement_index) = event_statement_index(event.source) else {
        return false;
    };
    let Some(statement) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return false;
    };
    let target = match statement {
        StatementNode::LocalData(local) => crate::flow::CanonicalPlace {
            root: psi_facts::PlaceRoot::Symbol(local.symbol),
            segments: Vec::new(),
        },
        StatementNode::Assignment(assignment) => {
            let Some(target) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                assignment.target,
            ) else {
                return false;
            };
            target
        }
        _ => return false,
    };
    crate::flow::normalized_event_place_root(program, target.root) == event.root
        && target.segments.as_slice() == path
}

fn nominal_drop_type_name(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&str> {
    let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => return nominal_drop_type_name(program, *base_type),
        TypeReferenceNode::Named { symbol, name } => (*symbol, name.as_str()),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => (*base_symbol, base_name.as_str()),
        _ => return None,
    };
    let definition = find_data_definition(program, symbol, name)?;
    data_name_with_nominal_drop(program, definition.name.as_str())
}

#[cfg(test)]
pub(crate) fn record_permission_events(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) {
    let call_frames = psi_validation::CallFrameResolver::new(program);
    let incoming_guards =
        super::ranges::incoming_guards::IncomingGuardIndex::build(program, call_frames.as_ref());
    record_permission_events_with_incoming_guards(program, facts, &incoming_guards);
}

fn record_permission_events_with_incoming_guards(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    incoming_guards: &super::ranges::incoming_guards::IncomingGuardIndex,
) {
    let mut permission_events = Vec::new();
    let mut claim_identities = ClaimIdentityAllocator::default();

    let state_flows = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state.clone())
        .collect::<Vec<_>>();
    for state_flow in state_flows {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
        else {
            continue;
        };
        let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);

        for place in places.iter_mut().filter(|place| place.ever_established) {
            let claim_identity = claim_identities.mint(
                state_flow.machine_symbol,
                state.symbol,
                PermissionEventSource::StateEntry,
            );
            place.claim_identity = Some(claim_identity);
            let segments = facts
                .flow
                .ownership
                .segments
                .insert_many(place.path.iter().copied());
            permission_events.push(FlowPermissionEventFact {
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state.symbol,
                source: PermissionEventSource::StateEntry,
                kind: PermissionEventKind::Establish,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity,
                provenance: place.provenance.expect("entry place has provenance"),
                root: psi_facts::PlaceRoot::Symbol(place.symbol),
                segments,
                obligation_live: true,
            });
        }

        let moves = crate::flow::discover_state_move_events(
            program,
            &facts.borrow,
            machine,
            state,
            &mut facts.flow.ownership.segments,
        );
        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        for (statement_index, statement) in statements[..prefix_end].iter().enumerate() {
            apply_statement_permission_production(
                program,
                facts,
                state_flow.machine_symbol,
                state.symbol,
                &moves,
                statement_index,
                statement,
                &mut places,
                &mut permission_events,
                &mut claim_identities,
            );
        }

        if let Some(first_transition) = first_transition {
            let entry = places.clone();
            let arm_indices = (first_transition..statements.len())
                .filter(|index| matches!(statements[*index], StatementNode::Transition(_)))
                .collect::<Vec<_>>();
            for statement_index in arm_indices.iter().copied() {
                let mut outcome = entry.clone();
                apply_statement_permission_production(
                    program,
                    facts,
                    state_flow.machine_symbol,
                    state.symbol,
                    &moves,
                    statement_index,
                    &statements[statement_index],
                    &mut outcome,
                    &mut permission_events,
                    &mut claim_identities,
                );
            }
        }

        append_affine_cleanup_permission_events(
            program,
            state,
            state_flow.machine_symbol,
            &places,
            &mut permission_events,
        );
    }

    append_borrow_permission_events(facts, &mut permission_events, &mut claim_identities);
    let claim_outcome_maps = reconcile_state_call_result_origins(
        program,
        &facts.flow.ownership.segments,
        &mut permission_events,
    );

    facts.flow.ownership.permissions = psi_arena::Arena::default();
    facts
        .flow
        .ownership
        .permissions
        .insert_many(permission_events);
    publish_claim_outcome_maps(facts, claim_outcome_maps);
    record_crash_frontier_lower_bounds(program, facts, incoming_guards);
}

#[derive(Debug)]
struct DerivedCrashFrontier {
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_ordinal: u32,
    claims: Vec<PermissionClaimIdentity>,
}

/// Retain the machine-local claims that are definitely live at each explicit
/// crash. The result is an underapproximation by design: a conditional sum
/// payload is omitted until active-case proof can make its liveness definite.
/// Crash abandons these claims; it does not synthesize cleanup or consumption.
fn record_crash_frontier_lower_bounds(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    incoming_guards: &super::ranges::incoming_guards::IncomingGuardIndex,
) {
    let mut derived = Vec::new();
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
        else {
            continue;
        };
        let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let events = facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter_map(|(_, event)| {
                (event.machine_symbol == state_flow.machine_symbol
                    && event.state_symbol == state.symbol
                    && event.access == PermissionAccess::Owned)
                    .then_some(event)
            })
            .collect::<Vec<_>>();
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);
        apply_recorded_state_entry_events(&events, &facts.flow.ownership.segments, &mut places);
        let incoming = incoming_guards.for_machine(machine.symbol);
        let proven_conditional_claims =
            proven_conditional_entry_claims(program, state, incoming, &places);

        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        let mut ignored_diagnostics = Vec::new();
        for statement_index in 0..prefix_end {
            apply_recorded_statement_events(
                statement_index,
                &events,
                &facts.flow.ownership.segments,
                &mut places,
                &mut ignored_diagnostics,
            );
        }

        let Some(first_transition) = first_transition else {
            continue;
        };
        let entry = places;
        for statement_index in (first_transition..statements.len())
            .filter(|index| matches!(statements[*index], StatementNode::Transition(_)))
        {
            let StatementNode::Transition(transition) = &statements[statement_index] else {
                unreachable!("transition indices contain only transitions")
            };
            if !matches!(
                transition.exit,
                psi_typed_trees::statement::TransitionExit::Crash(_)
            ) {
                continue;
            }
            let mut outcome = entry.clone();
            apply_recorded_statement_events(
                statement_index,
                &events,
                &facts.flow.ownership.segments,
                &mut outcome,
                &mut ignored_diagnostics,
            );
            let claims = outcome
                .iter()
                .filter_map(|place| {
                    (place.live
                        && (!place.conditional
                            || place.claim_identity.is_some_and(|identity| {
                                proven_conditional_claims.contains(&identity)
                            })))
                    .then_some(place.claim_identity?)
                    .filter(|identity| *identity != PermissionClaimIdentity::Unknown)
                })
                .collect();
            derived.push(DerivedCrashFrontier {
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state.symbol,
                statement_ordinal: u32::try_from(statement_index)
                    .expect("state-local statement ordinal exceeds checked identity range"),
                claims,
            });
        }
    }

    for contract in &mut facts.contract_plans.machines {
        let checked_sites = contract
            .crash
            .checked_sites()
            .iter()
            .map(|site| {
                let location = site.location();
                let frontier = derived
                    .iter()
                    .find(|frontier| {
                        frontier.machine_symbol == contract.machine
                            && frontier.state_symbol == location.state()
                            && frontier.statement_ordinal == location.statement_ordinal()
                    })
                    .map(|frontier| frontier.claims.clone())
                    .unwrap_or_else(|| site.frontier_lower_bound().to_vec());
                site.clone().with_frontier_lower_bound(frontier)
            })
            .collect();
        contract.crash = contract
            .crash
            .clone()
            .with_checked_sites(checked_sites)
            .expect("derived crash frontiers retain valid checked-site identity");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProvenCaseMembership {
    parameter: SymbolHandle,
    /// Source-independent path below the final-state parameter at which this
    /// case tag was tested. The selected variant is stored separately.
    subject_path: Vec<psi_facts::PlaceSegment>,
    variant: SymbolHandle,
}

/// Conditional sum payloads are only a crash-frontier lower bound when the
/// path into this state proves every case on the payload path active.
/// Case-pattern dispatch is retained in typed trees as a symbol-stamped
/// `value == Type::Case` guard. Incoming-edge argument composition rebinds the
/// tested subject to a final-state parameter plus a canonical symbol path.
///
/// The proof is attached to the claim identity rather than the parameter
/// spelling. A whole-value transfer in the target state therefore preserves
/// the proof, while overwriting the parameter mints a different identity and
/// cannot accidentally inherit it. A nested claim enters the lower bound only
/// when membership evidence covers every case segment.
fn proven_conditional_entry_claims(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    incoming: &[super::ranges::incoming_guards::IncomingGuard],
    places: &[LinearPlace],
) -> Vec<PermissionClaimIdentity> {
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let mut memberships = Vec::new();

    for entry in incoming.iter().filter(|entry| entry.holds_at(state.symbol)) {
        let mut case_tests = Vec::new();
        collect_positive_case_tests(program, entry.guard(), &mut case_tests);
        for (subject, variant) in case_tests {
            let Some(subject) = source_independent_case_subject(program, subject) else {
                continue;
            };
            for parameter in &parameters {
                let Some(argument) = entry.argument_place_for_parameter(parameter.symbol) else {
                    continue;
                };
                if subject.root != argument.root
                    || !subject.segments.starts_with(&argument.segments)
                {
                    continue;
                }
                let membership = ProvenCaseMembership {
                    parameter: parameter.symbol,
                    subject_path: subject.segments[argument.segments.len()..].to_vec(),
                    variant,
                };
                if !memberships.contains(&membership) {
                    memberships.push(membership);
                }
            }
        }
    }

    let mut proven = Vec::new();
    for place in places.iter().filter(|place| place.conditional) {
        let mut subject_path = Vec::new();
        let all_cases_proven = place.path.iter().all(|segment| {
            let proven = match segment {
                psi_facts::PlaceSegment::Case { variant } => memberships.iter().any(|evidence| {
                    evidence.parameter == place.symbol
                        && evidence.subject_path == subject_path
                        && evidence.variant == *variant
                }),
                _ => true,
            };
            subject_path.push(*segment);
            proven
        });
        if all_cases_proven
            && let Some(identity) = place.claim_identity
            && identity != PermissionClaimIdentity::Unknown
            && !proven.contains(&identity)
        {
            proven.push(identity);
        }
    }
    proven
}

fn source_independent_case_subject(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<crate::flow::CanonicalPlace> {
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    if !matches!(place.root, psi_facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        || !place.segments.iter().all(|segment| match segment {
            psi_facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
            psi_facts::PlaceSegment::Case { variant } => variant.is_valid(),
            psi_facts::PlaceSegment::FixedIndex { .. } => true,
            psi_facts::PlaceSegment::Index { .. } => false,
        })
    {
        return None;
    }
    Some(place)
}

/// Extract positive case-membership conjuncts. Boolean-arm lowering may wrap
/// a predicate as `predicate == true`; unwrap that shell but deliberately do
/// not infer through negation or disjunction here.
fn collect_positive_case_tests(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    tests: &mut Vec<(psi_typed_trees::expression::ExpressionHandle, SymbolHandle)>,
) {
    use psi_typed_trees::expression::{BinaryOperator, ExpressionNode};

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return;
    };
    if binary.operator == BinaryOperator::And {
        collect_positive_case_tests(program, binary.left, tests);
        collect_positive_case_tests(program, binary.right, tests);
        return;
    }
    if binary.operator != BinaryOperator::Equal {
        return;
    }

    let is_true = |candidate| {
        matches!(
            program.expression_table.expression(candidate),
            ExpressionNode::Boolean(true)
        )
    };
    if is_true(binary.left) {
        collect_positive_case_tests(program, binary.right, tests);
        return;
    }
    if is_true(binary.right) {
        collect_positive_case_tests(program, binary.left, tests);
        return;
    }

    let case_variant = |candidate| match program.expression_table.expression(candidate) {
        ExpressionNode::Name(path)
            if program.data_definitions().iter().any(|definition| {
                program.data_members(definition).iter().any(|member| {
                    matches!(
                        member,
                        psi_typed_trees::data::DataMember::Variant(variant)
                            if variant.symbol == path.symbol
                    )
                })
            }) =>
        {
            Some(path.symbol)
        }
        _ => None,
    };
    if let Some(variant) = case_variant(binary.right) {
        tests.push((binary.left, variant));
    } else if let Some(variant) = case_variant(binary.left) {
        tests.push((binary.right, variant));
    }
}

fn exact_positive_case_test(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<(psi_typed_trees::expression::ExpressionHandle, SymbolHandle)> {
    use psi_typed_trees::expression::{BinaryOperator, ExpressionNode};

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }
    let is_true = |candidate| {
        matches!(
            program.expression_table.expression(candidate),
            ExpressionNode::Boolean(true)
        )
    };
    if is_true(binary.left) {
        return exact_positive_case_test(program, binary.right);
    }
    if is_true(binary.right) {
        return exact_positive_case_test(program, binary.left);
    }
    let mut tests = Vec::new();
    collect_positive_case_tests(program, expression, &mut tests);
    let [test] = tests.as_slice() else {
        return None;
    };
    Some(*test)
}

fn case_transition_run_is_exhaustive(
    program: &psi_typed_trees::TypedTrees,
    statements: &[StatementNode],
    arm_indices: &[usize],
) -> bool {
    let mut subject_label: Option<String> = None;
    let mut covered = Vec::new();
    for &statement_index in arm_indices {
        let Some(StatementNode::Transition(transition)) = statements.get(statement_index) else {
            return false;
        };
        let psi_typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
            return false;
        };
        let Some((subject, variant)) = exact_positive_case_test(program, guard) else {
            return false;
        };
        let label = program.expression_table.display_name(subject);
        if subject_label
            .as_ref()
            .is_some_and(|expected| *expected != label)
        {
            return false;
        }
        subject_label = Some(label);
        if !covered.contains(&variant) {
            covered.push(variant);
        }
    }
    let Some(first_variant) = covered.first() else {
        return false;
    };
    program.data_definitions().iter().any(|definition| {
        let variants = program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                psi_typed_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
                _ => None,
            })
            .collect::<Vec<_>>();
        variants.contains(first_variant)
            && variants.len() == covered.len()
            && variants.iter().all(|variant| covered.contains(variant))
    })
}

fn select_case_alternative_from_guard(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: psi_typed_trees::expression::ExpressionHandle,
    variant: SymbolHandle,
    places: &mut [LinearPlace],
) {
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        subject,
    ) else {
        return;
    };
    let psi_facts::PlaceRoot::Symbol(symbol) = place.root else {
        return;
    };
    let mut selected_path = place.segments;
    selected_path.push(psi_facts::PlaceSegment::Case { variant });
    select_static_case_alternative(symbol, &selected_path, places);
}

fn exclude_case_alternative(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: psi_typed_trees::expression::ExpressionHandle,
    variant: SymbolHandle,
    places: &mut [LinearPlace],
) {
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        subject,
    ) else {
        return;
    };
    let psi_facts::PlaceRoot::Symbol(symbol) = place.root else {
        return;
    };
    let case_index = place.segments.len();
    for candidate in places.iter_mut().filter(|candidate| {
        candidate.symbol == symbol
            && candidate.path.get(..case_index) == Some(place.segments.as_slice())
            && candidate.path.get(case_index) == Some(&psi_facts::PlaceSegment::Case { variant })
    }) {
        candidate.live = false;
    }
}

/// Join a state call's receiving establishment to the unique claim and
/// root-lineage provenance that the target state transferred through its
/// result.
///
/// Intra-state production can propagate provenance directly through source
/// expressions. A zero-argument state call has no caller-side source place,
/// though: without this join, binding a locally-created linear result in the
/// caller would mint a second identity and origin even when the target has one
/// unambiguous outgoing obligation. Direct aggregate construction and
/// path-aligned results publish structural output paths; opaque multi-output
/// calls remain conservative until they publish an explicit result mapping.
fn reconcile_state_call_result_origins(
    program: &psi_typed_trees::TypedTrees,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &mut [FlowPermissionEventFact],
) -> Vec<CheckedClaimOutcomeMap> {
    let iteration_limit = permission_events.len().saturating_add(1);
    for _ in 0..iteration_limit {
        let maps = derive_checked_claim_outcome_maps(program, segments, permission_events);
        let rewrites = call_result_origin_rewrites(program, segments, permission_events, &maps);
        let origins_changed = apply_claim_origin_rewrites(permission_events, &rewrites);
        let liveness_changed =
            apply_statically_inactive_call_results(program, segments, permission_events, &maps);
        if !origins_changed && !liveness_changed {
            return derive_checked_claim_outcome_maps(program, segments, permission_events);
        }
    }
    derive_checked_claim_outcome_maps(program, segments, permission_events)
}

fn apply_statically_inactive_call_results(
    program: &psi_typed_trees::TypedTrees,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &mut [FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
) -> bool {
    let inactive_identities = permission_events
        .iter()
        .filter_map(|event| {
            if event.kind != PermissionEventKind::Establish
                || event.access != PermissionAccess::Owned
                || !event.obligation_live
            {
                return None;
            }
            let PermissionEventSource::Statement { statement_index } = event.source else {
                return None;
            };
            let state = crate::find_state(program, event.state_symbol)?;
            let statement = program
                .statement_table
                .statements(state.statement_nodes)
                .get(statement_index)?;
            let expression = match statement {
                StatementNode::LocalData(local) => local.initial_value,
                StatementNode::Assignment(assignment) => assignment.value,
                _ => return None,
            };
            let psi_typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(expression)
            else {
                return None;
            };
            let target = crate::find_state(program, call.target_symbol)?;
            let map = maps.iter().find(|map| map.state_symbol == target.symbol)?;
            let receiving_path = segments.span_or_empty(event.segments);
            (!map
                .entries
                .iter()
                .any(|entry| entry.output_path == receiving_path))
            .then_some(event.claim_identity)
        })
        .filter(|identity| *identity != PermissionClaimIdentity::Unknown)
        .collect::<Vec<_>>();
    if inactive_identities.is_empty() {
        return false;
    }
    let mut changed = false;
    for event in permission_events.iter_mut().filter(|event| {
        event.access == PermissionAccess::Owned
            && inactive_identities.contains(&event.claim_identity)
    }) {
        changed |=
            event.obligation_live || event.claim_identity != PermissionClaimIdentity::Unknown;
        event.obligation_live = false;
        event.claim_identity = PermissionClaimIdentity::Unknown;
    }
    changed
}

fn derive_checked_claim_outcome_maps(
    program: &psi_typed_trees::TypedTrees,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> Vec<CheckedClaimOutcomeMap> {
    let state_count = program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum::<usize>();
    let mut maps = Vec::new();
    for _ in 0..=state_count {
        let next = program
            .machines()
            .iter()
            .flat_map(|machine| {
                program.machine_states(machine).iter().filter_map(|state| {
                    derive_checked_claim_outcome_map(
                        program,
                        machine.symbol,
                        state,
                        segments,
                        permission_events,
                        &maps,
                    )
                })
            })
            .collect::<Vec<_>>();
        if next == maps {
            return maps;
        }
        maps = next;
    }
    maps
}

#[allow(clippy::too_many_arguments)]
fn derive_checked_claim_outcome_map(
    program: &psi_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state: &psi_typed_trees::state::State,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
    known_maps: &[CheckedClaimOutcomeMap],
) -> Option<CheckedClaimOutcomeMap> {
    let expected_paths = linear_claim_frontier(program, state.return_type)
        .into_iter()
        .map(|claim| claim.path)
        .collect::<Vec<_>>();
    if expected_paths.is_empty() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let result_expressions = state_result_expressions(program, statements);
    let named_transitions = state_result_named_transitions(program, statements);
    let mut entries = result_expressions
        .iter()
        .copied()
        .into_iter()
        .flat_map(|(statement_index, expression)| {
            claim_outcomes_for_expression(
                program,
                state,
                statement_index,
                expression,
                segments,
                permission_events,
                known_maps,
                &[],
            )
        })
        .chain(named_transitions.iter().copied().flat_map(
            |(statement_index, target_symbol, arguments)| {
                claim_outcomes_for_named_transition(
                    program,
                    state,
                    statement_index,
                    target_symbol,
                    arguments,
                    segments,
                    permission_events,
                    known_maps,
                )
            },
        ))
        .fold(Vec::new(), |mut entries, entry| {
            if !entries.contains(&entry) {
                entries.push(entry);
            }
            entries
        });
    entries.retain(|entry| expected_paths.contains(&entry.output_path));
    for path in &expected_paths {
        let sources = entries
            .iter()
            .filter(|entry| entry.output_path == *path)
            .map(|entry| &entry.source)
            .fold(Vec::new(), |mut sources, source| {
                if !sources.contains(&source) {
                    sources.push(source);
                }
                sources
            });
        if sources.len() != 1 {
            if sources.is_empty()
                && claim_path_is_statically_inactive(
                    program,
                    path,
                    &result_expressions,
                    &named_transitions,
                    known_maps,
                )
            {
                continue;
            }
            return None;
        }
    }
    entries.retain(|entry| {
        expected_paths
            .iter()
            .filter(|path| **path == entry.output_path)
            .count()
            == 1
    });
    entries.sort_by_key(|entry| {
        expected_paths
            .iter()
            .position(|path| *path == entry.output_path)
            .unwrap_or(usize::MAX)
    });
    for (index, entry) in entries.iter().enumerate() {
        if entries[index + 1..].iter().any(|candidate| {
            candidate.source == entry.source
                && !claim_paths_are_case_alternatives(&entry.output_path, &candidate.output_path)
        }) {
            return None;
        }
    }
    Some(CheckedClaimOutcomeMap {
        machine_symbol,
        state_symbol: state.symbol,
        entries,
    })
}

fn claim_paths_are_case_alternatives(
    left: &[psi_facts::PlaceSegment],
    right: &[psi_facts::PlaceSegment],
) -> bool {
    left.iter()
        .zip(right)
        .find_map(|(left, right)| {
            if left == right {
                return None;
            }
            Some(matches!(
                (left, right),
                (
                    psi_facts::PlaceSegment::Case {
                        variant: left_variant
                    },
                    psi_facts::PlaceSegment::Case {
                        variant: right_variant
                    }
                ) if left_variant != right_variant
            ))
        })
        .unwrap_or(false)
}

fn claim_path_is_statically_inactive(
    program: &psi_typed_trees::TypedTrees,
    path: &[psi_facts::PlaceSegment],
    result_expressions: &[(usize, psi_typed_trees::expression::ExpressionHandle)],
    named_transitions: &[(
        usize,
        SymbolHandle,
        HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
    )],
    known_maps: &[CheckedClaimOutcomeMap],
) -> bool {
    if result_expressions.is_empty() && named_transitions.is_empty() {
        return false;
    }
    result_expressions.iter().all(|(_, expression)| {
        expression_statically_excludes_claim_path(program, *expression, path, known_maps)
    }) && named_transitions.iter().all(|(_, target_symbol, _)| {
        crate::find_state(program, *target_symbol)
            .and_then(|target| {
                known_maps
                    .iter()
                    .find(|map| map.state_symbol == target.symbol)
            })
            .is_some_and(|map| !map.entries.iter().any(|entry| entry.output_path == path))
    })
}

fn expression_statically_excludes_claim_path(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    path: &[psi_facts::PlaceSegment],
    known_maps: &[CheckedClaimOutcomeMap],
) -> bool {
    match program.expression_table.expression(expression) {
        psi_typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
            let mut remaining = path;
            if literal.case_name.is_some() {
                let Some(psi_facts::PlaceSegment::Case { variant }) = remaining.first() else {
                    return false;
                };
                if literal_variant(program, literal).map(|candidate| candidate.symbol)
                    != Some(*variant)
                {
                    return true;
                }
                remaining = &remaining[1..];
            }
            let Some(psi_facts::PlaceSegment::Field { symbol }) = remaining.first() else {
                return false;
            };
            let Some(field_name) = data_field_name(program, *symbol) else {
                return false;
            };
            program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .find(|field| field.name.as_str() == field_name)
                .is_some_and(|field| {
                    expression_statically_excludes_claim_path(
                        program,
                        field.value,
                        &remaining[1..],
                        known_maps,
                    )
                })
        }
        psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            let Some(psi_facts::PlaceSegment::FixedIndex { index }) = path.first() else {
                return false;
            };
            program
                .expression_table
                .expression_handles(*values)
                .get(*index)
                .is_some_and(|value| {
                    expression_statically_excludes_claim_path(
                        program,
                        *value,
                        &path[1..],
                        known_maps,
                    )
                })
        }
        psi_typed_trees::expression::ExpressionNode::Call(call) => {
            crate::find_state(program, call.target_symbol)
                .and_then(|target| {
                    known_maps
                        .iter()
                        .find(|map| map.state_symbol == target.symbol)
                })
                .is_some_and(|map| !map.entries.iter().any(|entry| entry.output_path == path))
        }
        _ => false,
    }
}

fn state_result_expressions(
    program: &psi_typed_trees::TypedTrees,
    statements: &[StatementNode],
) -> Vec<(usize, psi_typed_trees::expression::ExpressionHandle)> {
    statements
        .iter()
        .enumerate()
        .flat_map(|(statement_index, statement)| match statement {
            StatementNode::Expression(expression) if statement_index + 1 == statements.len() => {
                vec![(statement_index, *expression)]
            }
            StatementNode::Transition(transition) => [transition.target, transition.continuation]
                .into_iter()
                .filter(|handle| handle.is_valid())
                .filter_map(|handle| {
                    let psi_typed_trees::statement::TransitionTargetNode::Value(expression) =
                        program.statement_table.transition_target(handle)
                    else {
                        return None;
                    };
                    Some((statement_index, *expression))
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn state_result_named_transitions(
    program: &psi_typed_trees::TypedTrees,
    statements: &[StatementNode],
) -> Vec<(
    usize,
    SymbolHandle,
    HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
)> {
    statements
        .iter()
        .enumerate()
        .flat_map(|(statement_index, statement)| {
            let StatementNode::Transition(transition) = statement else {
                return Vec::new();
            };
            [transition.target, transition.continuation]
                .into_iter()
                .filter(|handle| handle.is_valid())
                .filter_map(|handle| {
                    let psi_typed_trees::statement::TransitionTargetNode::Named {
                        path,
                        arguments,
                        ..
                    } = program.statement_table.transition_target(handle)
                    else {
                        return None;
                    };
                    Some((statement_index, path.symbol, *arguments))
                })
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn claim_outcomes_for_named_transition(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    target_symbol: SymbolHandle,
    arguments: HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
    known_maps: &[CheckedClaimOutcomeMap],
) -> Vec<CheckedClaimOutcomeEntry> {
    let Some(target_state) = crate::find_state(program, target_symbol) else {
        return Vec::new();
    };
    let Some(target_map) = known_maps
        .iter()
        .find(|map| map.state_symbol == target_state.symbol)
    else {
        return Vec::new();
    };
    let arguments = program.statement_table.expression_handles(arguments);
    target_map
        .entries
        .iter()
        .filter_map(|entry| {
            let source = bind_claim_outcome_source_at_arguments(
                program,
                state,
                statement_index,
                arguments,
                None,
                target_state,
                &entry.source,
                segments,
                permission_events,
            )?;
            Some(CheckedClaimOutcomeEntry {
                output_path: entry.output_path.clone(),
                source,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn claim_outcomes_for_expression(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    expression: psi_typed_trees::expression::ExpressionHandle,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
    known_maps: &[CheckedClaimOutcomeMap],
    output_prefix: &[psi_facts::PlaceSegment],
) -> Vec<CheckedClaimOutcomeEntry> {
    if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    ) {
        let found = permission_events
            .iter()
            .filter(|event| {
                event.state_symbol == state.symbol
                    && permission_event_statement_index(event.source) == Some(statement_index)
                    && event.kind == PermissionEventKind::Transfer
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.root == place.root
            })
            .filter_map(|event| {
                let event_path = segments.span_or_empty(event.segments);
                if !event_path.starts_with(place.segments.as_slice()) {
                    return None;
                }
                let mut output_path = output_prefix.to_vec();
                output_path.extend_from_slice(&event_path[place.segments.len()..]);
                Some(CheckedClaimOutcomeEntry {
                    output_path,
                    source: claim_outcome_source_for_event(
                        program,
                        state,
                        event,
                        segments,
                        permission_events,
                    ),
                })
            })
            .collect::<Vec<_>>();
        if !found.is_empty() {
            return found;
        }
    }

    match program.expression_table.expression(expression) {
        psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .enumerate()
            .flat_map(|(index, value)| {
                let mut element_prefix = output_prefix.to_vec();
                element_prefix.push(psi_facts::PlaceSegment::FixedIndex { index });
                claim_outcomes_for_expression(
                    program,
                    state,
                    statement_index,
                    *value,
                    segments,
                    permission_events,
                    known_maps,
                    &element_prefix,
                )
            })
            .collect(),
        psi_typed_trees::expression::ExpressionNode::StructLiteral(literal)
            if literal.case_name.is_some() =>
        {
            let Some(variant) = literal_variant(program, literal) else {
                return Vec::new();
            };
            let mut case_prefix = output_prefix.to_vec();
            case_prefix.push(psi_facts::PlaceSegment::Case {
                variant: variant.symbol,
            });
            program
                .data_payload_fields(variant)
                .iter()
                .filter_map(|field| {
                    let value = program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .find(|literal_field| literal_field.name == field.name)?
                        .value;
                    let mut field_prefix = case_prefix.clone();
                    field_prefix.push(psi_facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    Some(claim_outcomes_for_expression(
                        program,
                        state,
                        statement_index,
                        value,
                        segments,
                        permission_events,
                        known_maps,
                        &field_prefix,
                    ))
                })
                .flatten()
                .collect()
        }
        psi_typed_trees::expression::ExpressionNode::StructLiteral(literal)
            if literal.case_name.is_none() =>
        {
            let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == literal.type_name.as_str())
            else {
                return Vec::new();
            };
            program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .flat_map(|literal_field| {
                    let Some(field) =
                        program
                            .data_members(definition)
                            .iter()
                            .find_map(|member| match member {
                                psi_typed_trees::data::DataMember::Field(field)
                                    if field.name.as_str() == literal_field.name.as_str() =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            })
                    else {
                        return Vec::new();
                    };
                    let mut field_prefix = output_prefix.to_vec();
                    field_prefix.push(psi_facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    claim_outcomes_for_expression(
                        program,
                        state,
                        statement_index,
                        literal_field.value,
                        segments,
                        permission_events,
                        known_maps,
                        &field_prefix,
                    )
                })
                .collect()
        }
        psi_typed_trees::expression::ExpressionNode::Call(call) => {
            let Some(target_state) = crate::find_state(program, call.target_symbol) else {
                return Vec::new();
            };
            let Some(target_map) = known_maps
                .iter()
                .find(|map| map.state_symbol == target_state.symbol)
            else {
                return Vec::new();
            };
            target_map
                .entries
                .iter()
                .filter_map(|entry| {
                    let source = bind_claim_outcome_source_at_call(
                        program,
                        state,
                        statement_index,
                        call,
                        target_state,
                        &entry.source,
                        segments,
                        permission_events,
                    )?;
                    let mut output_path = output_prefix.to_vec();
                    output_path.extend_from_slice(&entry.output_path);
                    Some(CheckedClaimOutcomeEntry {
                        output_path,
                        source,
                    })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn claim_outcome_source_for_event(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    event: &FlowPermissionEventFact,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> CheckedClaimOutcomeSource {
    if let Some(entry) = permission_events.iter().find(|candidate| {
        candidate.state_symbol == state.symbol
            && candidate.source == PermissionEventSource::StateEntry
            && candidate.kind == PermissionEventKind::Establish
            && candidate.access == PermissionAccess::Owned
            && candidate.claim_identity == event.claim_identity
            && candidate.provenance == event.provenance
    }) && let psi_facts::PlaceRoot::Symbol(parameter_symbol) = entry.root
        && program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == parameter_symbol)
    {
        return CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path: segments.span_or_empty(entry.segments).to_vec(),
        };
    }
    CheckedClaimOutcomeSource::Established {
        claim_identity: event.claim_identity,
        provenance: event.provenance,
    }
}

fn claim_origin_for_source(
    state: &psi_typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> Option<(PermissionProvenance, PermissionClaimIdentity)> {
    match source {
        CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => Some((*provenance, *claim_identity)),
        CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path,
        } => {
            let origins = permission_events
                .iter()
                .filter(|event| {
                    event.state_symbol == state.symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.root == psi_facts::PlaceRoot::Symbol(*parameter_symbol)
                        && segments.span_or_empty(event.segments) == path
                        && event.claim_identity != PermissionClaimIdentity::Unknown
                        && event.provenance != PermissionProvenance::Unknown
                })
                .map(|event| (event.provenance, event.claim_identity))
                .fold(Vec::new(), |mut origins, origin| {
                    if !origins.contains(&origin) {
                        origins.push(origin);
                    }
                    origins
                });
            let [origin] = origins.as_slice() else {
                return None;
            };
            Some(*origin)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn bind_claim_outcome_source_at_call(
    program: &psi_typed_trees::TypedTrees,
    caller_state: &psi_typed_trees::state::State,
    statement_index: usize,
    call: &psi_typed_trees::expression::TableCallExpression,
    target_state: &psi_typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> Option<CheckedClaimOutcomeSource> {
    bind_claim_outcome_source_at_arguments(
        program,
        caller_state,
        statement_index,
        program.expression_table.expression_handles(call.arguments),
        call.receiver.is_valid().then_some(call.receiver),
        target_state,
        source,
        segments,
        permission_events,
    )
}

#[allow(clippy::too_many_arguments)]
fn bind_claim_outcome_source_at_arguments(
    program: &psi_typed_trees::TypedTrees,
    caller_state: &psi_typed_trees::state::State,
    statement_index: usize,
    arguments: &[psi_typed_trees::expression::ExpressionHandle],
    receiver: Option<psi_typed_trees::expression::ExpressionHandle>,
    target_state: &psi_typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> Option<CheckedClaimOutcomeSource> {
    let CheckedClaimOutcomeSource::Input {
        parameter_symbol,
        path,
    } = source
    else {
        return Some(source.clone());
    };
    let argument = argument_for_parameter(
        program,
        arguments,
        receiver,
        target_state,
        *parameter_symbol,
    )?;
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        caller_state.symbol,
        statement_index,
        argument,
    )?;
    place.segments.extend_from_slice(path);
    let sources = permission_events
        .iter()
        .filter(|event| {
            event.state_symbol == caller_state.symbol
                && permission_event_statement_index(event.source) == Some(statement_index)
                && event.kind == PermissionEventKind::Transfer
                && event.access == PermissionAccess::Owned
                && event.obligation_live
                && event.root == place.root
                && segments.span_or_empty(event.segments) == place.segments
        })
        .map(|event| {
            claim_outcome_source_for_event(
                program,
                caller_state,
                event,
                segments,
                permission_events,
            )
        })
        .fold(Vec::new(), |mut sources, source| {
            if !sources.contains(&source) {
                sources.push(source);
            }
            sources
        });
    let [source] = sources.as_slice() else {
        return None;
    };
    Some(source.clone())
}

fn argument_for_parameter(
    program: &psi_typed_trees::TypedTrees,
    arguments: &[psi_typed_trees::expression::ExpressionHandle],
    receiver: Option<psi_typed_trees::expression::ExpressionHandle>,
    target_state: &psi_typed_trees::state::State,
    parameter_symbol: SymbolHandle,
) -> Option<psi_typed_trees::expression::ExpressionHandle> {
    let parameters = program.state_parameters(target_state);
    let includes_explicit_self =
        parameters.iter().any(|parameter| parameter.is_self) && arguments.len() == parameters.len();
    let mut argument_index = 0usize;
    for parameter in parameters {
        let argument = if parameter.is_self && !includes_explicit_self {
            receiver
        } else {
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            argument
        };
        if parameter.symbol == parameter_symbol {
            return argument;
        }
    }
    None
}

fn call_result_origin_rewrites(
    program: &psi_typed_trees::TypedTrees,
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
) -> Vec<(
    PermissionProvenance,
    PermissionClaimIdentity,
    PermissionProvenance,
    PermissionClaimIdentity,
)> {
    let mut rewrites = Vec::new();
    for event in permission_events.iter() {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
        {
            continue;
        }
        let PermissionEventSource::Statement { statement_index } = event.source else {
            continue;
        };
        let locally_minted =
            established_provenance(event.machine_symbol, event.state_symbol, event.source);
        if event.provenance != locally_minted
            || event.claim_identity == PermissionClaimIdentity::Unknown
        {
            continue;
        }
        let Some(state) = crate::find_state(program, event.state_symbol) else {
            continue;
        };
        let Some(statement) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index)
        else {
            continue;
        };
        let result_expression = match statement {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Assignment(assignment) => assignment.value,
            _ => continue,
        };
        let psi_typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression(result_expression)
        else {
            continue;
        };
        let Some(target_state) = crate::find_state(program, call.target_symbol) else {
            continue;
        };
        let Some(map) = maps
            .iter()
            .find(|map| map.state_symbol == target_state.symbol)
        else {
            continue;
        };
        let receiving_path = segments.span_or_empty(event.segments);
        let matching = map
            .entries
            .iter()
            .filter(|entry| entry.output_path == receiving_path)
            .filter_map(|entry| {
                bind_claim_outcome_source_at_call(
                    program,
                    state,
                    statement_index,
                    call,
                    target_state,
                    &entry.source,
                    segments,
                    permission_events,
                )
            })
            .fold(Vec::new(), |mut sources, source| {
                if !sources.contains(&source) {
                    sources.push(source);
                }
                sources
            });
        let [source] = matching.as_slice() else {
            continue;
        };
        let Some((provenance, claim_identity)) =
            claim_origin_for_source(state, source, segments, permission_events)
        else {
            continue;
        };
        rewrites.push((
            locally_minted,
            event.claim_identity,
            provenance,
            claim_identity,
        ));
    }
    rewrites
}

fn apply_claim_origin_rewrites(
    permission_events: &mut [FlowPermissionEventFact],
    rewrites: &[(
        PermissionProvenance,
        PermissionClaimIdentity,
        PermissionProvenance,
        PermissionClaimIdentity,
    )],
) -> bool {
    let mut changed = false;
    for event in permission_events
        .iter_mut()
        .filter(|event| event.access == PermissionAccess::Owned)
    {
        let mut provenance = event.provenance;
        let mut claim_identity = event.claim_identity;
        for _ in 0..rewrites.len() {
            let Some((_, _, replacement_provenance, replacement_identity)) =
                rewrites
                    .iter()
                    .find(|(source_provenance, source_identity, _, _)| {
                        *source_provenance == provenance && *source_identity == claim_identity
                    })
            else {
                break;
            };
            if *replacement_provenance == provenance && *replacement_identity == claim_identity {
                break;
            }
            provenance = *replacement_provenance;
            claim_identity = *replacement_identity;
        }
        changed |= event.provenance != provenance || event.claim_identity != claim_identity;
        event.provenance = provenance;
        event.claim_identity = claim_identity;
    }
    changed
}

fn publish_claim_outcome_maps(facts: &mut CheckFacts, maps: Vec<CheckedClaimOutcomeMap>) {
    facts.flow.ownership.claim_outcome_entries = psi_arena::Arena::default();
    facts.flow.ownership.claim_outcome_maps = psi_arena::Arena::default();
    for map in maps {
        let mut entries = Vec::new();
        for entry in map.entries {
            let output_segments = facts.flow.ownership.segments.insert_many(entry.output_path);
            let source = match entry.source {
                CheckedClaimOutcomeSource::Input {
                    parameter_symbol,
                    path,
                } => FlowClaimOutcomeSource::Input {
                    parameter_symbol,
                    segments: facts.flow.ownership.segments.insert_many(path),
                },
                CheckedClaimOutcomeSource::Established {
                    claim_identity,
                    provenance,
                } => FlowClaimOutcomeSource::Established {
                    claim_identity,
                    provenance,
                },
            };
            entries.push(FlowClaimOutcomeEntryFact {
                output_segments,
                source,
            });
        }
        let entries = facts
            .flow
            .ownership
            .claim_outcome_entries
            .insert_many(entries);
        facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(FlowClaimOutcomeMapFact {
                machine_symbol: map.machine_symbol,
                state_symbol: map.state_symbol,
                entries,
            });
    }
}

pub(crate) fn validate_linear_permission_events(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);
        let events = facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter_map(|(_, event)| {
                (event.machine_symbol == state_flow.machine_symbol
                    && event.state_symbol == state.symbol
                    && event.access == PermissionAccess::Owned)
                    .then_some(event)
            })
            .collect::<Vec<_>>();
        append_unresolved_state_result_mapping_diagnostics(
            program,
            state,
            &events,
            &facts.flow.ownership,
            &mut diagnostics,
        );

        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        for statement_index in 0..prefix_end {
            apply_recorded_statement_events(
                statement_index,
                &events,
                &facts.flow.ownership.segments,
                &mut places,
                &mut diagnostics,
            );
        }

        let mut mixed_places = Vec::new();
        let mut has_ordinary_exit = first_transition.is_none();
        if let Some(first_transition) = first_transition {
            let entry = places.clone();
            let arm_indices = (first_transition..statements.len())
                .filter(|index| matches!(statements[*index], StatementNode::Transition(_)))
                .collect::<Vec<_>>();
            let mut outcomes = Vec::new();
            let mut excluded_case_tests = Vec::new();
            for statement_index in arm_indices.iter().copied() {
                let mut outcome = entry.clone();
                for &(subject, variant) in &excluded_case_tests {
                    exclude_case_alternative(
                        program,
                        state.symbol,
                        statement_index,
                        subject,
                        variant,
                        &mut outcome,
                    );
                }
                let StatementNode::Transition(transition) = &statements[statement_index] else {
                    unreachable!("transition indices contain only transitions")
                };
                if let psi_typed_trees::statement::TransitionGuardNode::When(guard) =
                    transition.guard
                {
                    let mut selected = Vec::new();
                    collect_positive_case_tests(program, guard, &mut selected);
                    for (subject, variant) in selected {
                        select_case_alternative_from_guard(
                            program,
                            state.symbol,
                            statement_index,
                            subject,
                            variant,
                            &mut outcome,
                        );
                    }
                }
                apply_recorded_statement_events(
                    statement_index,
                    &events,
                    &facts.flow.ownership.segments,
                    &mut outcome,
                    &mut diagnostics,
                );
                if transition.exit == psi_typed_trees::statement::TransitionExit::Ordinary {
                    outcomes.push(outcome);
                }
                if let psi_typed_trees::statement::TransitionGuardNode::When(guard) =
                    transition.guard
                    && let Some(case_test) = exact_positive_case_test(program, guard)
                {
                    excluded_case_tests.push(case_test);
                }
            }
            let exhaustive =
                arm_indices.last().is_some_and(|index| {
                    matches!(
                        statements[*index],
                        StatementNode::Transition(psi_typed_trees::statement::TableTransition {
                            guard: psi_typed_trees::statement::TransitionGuardNode::Always,
                            ..
                        })
                    )
                }) || case_transition_run_is_exhaustive(program, statements, &arm_indices);
            if !exhaustive {
                outcomes.push(entry);
            }
            has_ordinary_exit = !outcomes.is_empty();

            if let Some(first) = outcomes.first() {
                for place_index in 0..places.len() {
                    let live = first[place_index].live;
                    if outcomes
                        .iter()
                        .skip(1)
                        .any(|outcome| outcome[place_index].live != live)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "linear value `{}` has inconsistent treatment across transition arms; every path must consume/transfer it or every path must preserve the same live obligation",
                            places[place_index].name
                        )));
                        mixed_places
                            .push((places[place_index].symbol, places[place_index].path.clone()));
                    } else {
                        places[place_index].live = live;
                        places[place_index].ever_established = outcomes
                            .iter()
                            .any(|outcome| outcome[place_index].ever_established);
                    }
                }
            }
        }

        if has_ordinary_exit {
            for place in places.iter().filter(|place| {
                place.live
                    && !mixed_places
                        .iter()
                        .any(|(symbol, path)| *symbol == place.symbol && *path == place.path)
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "linear value `{}` reaches scope exit without being consumed or transferred",
                    place.name
                )));
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn append_unresolved_state_result_mapping_diagnostics(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    events: &[&FlowPermissionEventFact],
    ownership: &psi_checked_trees::FlowOwnershipFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut unresolved_statements = Vec::new();
    for event in events {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
        {
            continue;
        }
        let PermissionEventSource::Statement { statement_index } = event.source else {
            continue;
        };
        let Some(statement) = statements.get(statement_index) else {
            continue;
        };
        let result_expression = match statement {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Assignment(assignment) => assignment.value,
            _ => continue,
        };
        let psi_typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression(result_expression)
        else {
            continue;
        };
        let Some(target_state) = crate::find_state(program, call.target_symbol) else {
            continue;
        };
        if program
            .statement_table
            .statements(target_state.statement_nodes)
            .is_empty()
            || !type_carries_linear_obligation(program, target_state.return_type)
        {
            continue;
        }
        let mapped_from_call = ownership.permissions.iter().any(|(_, candidate)| {
            candidate.kind == PermissionEventKind::Transfer
                && candidate.access == PermissionAccess::Owned
                && candidate.obligation_live
                && candidate.claim_identity != PermissionClaimIdentity::Unknown
                && candidate.claim_identity == event.claim_identity
                && (candidate.state_symbol == target_state.symbol
                    || (candidate.state_symbol == state.symbol
                        && permission_event_statement_index(candidate.source)
                            == Some(statement_index)))
        });
        let receiving_path = ownership.segments.span_or_empty(event.segments);
        let mapped_by_checked_outcome = ownership
            .claim_outcome_maps
            .iter()
            .filter(|(_, map)| map.state_symbol == target_state.symbol)
            .flat_map(|(_, map)| ownership.claim_outcome_entries.span_or_empty(map.entries))
            .any(|entry| {
                ownership.segments.span_or_empty(entry.output_segments) == receiving_path
                    && matches!(
                        entry.source,
                        FlowClaimOutcomeSource::Established {
                            claim_identity,
                            provenance,
                        } if claim_identity == event.claim_identity
                            && provenance == event.provenance
                    )
            });
        if mapped_from_call || mapped_by_checked_outcome {
            continue;
        }
        if !unresolved_statements.contains(&statement_index) {
            unresolved_statements.push(statement_index);
        }
    }

    for statement_index in unresolved_statements {
        diagnostics.push(Diagnostic::error(format!(
            "linear state-call result at statement {statement_index} has no unique conserved claim mapping; return a path-aligned source place or publish an explicit outcome mapping"
        )));
    }
}

fn initial_linear_places(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Vec<LinearPlace> {
    let mut places = Vec::new();
    for parameter in program.state_parameters(state) {
        // A by-value `self` parameter is the language's terminal-consumer
        // form. The caller owns the consumption judgment.
        if parameter.is_self {
            continue;
        }
        for claim in linear_claim_frontier(program, parameter.type_reference) {
            places.push(LinearPlace {
                symbol: parameter.symbol,
                name: claim_place_name(program, parameter.name.as_str(), &claim.path),
                path: claim.path,
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: Some(established_provenance(
                    machine_symbol,
                    state_symbol,
                    PermissionEventSource::StateEntry,
                )),
                live: true,
                ever_established: true,
                conditional: claim.conditional,
            });
        }
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        for claim in linear_claim_frontier(program, local.type_reference) {
            places.push(LinearPlace {
                symbol: local.symbol,
                name: claim_place_name(program, local.name.as_str(), &claim.path),
                path: claim.path,
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                conditional: claim.conditional,
            });
        }
    }
    places
}

pub(super) fn linear_claim_frontier(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<LinearClaimTemplate> {
    let mut claims = Vec::new();
    append_linear_claim_frontier(
        program,
        type_reference,
        &[],
        &[],
        &mut Vec::new(),
        &mut claims,
    );
    claims
}

fn append_linear_claim_frontier(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[psi_facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<LinearClaimTemplate>,
) {
    if !type_reference.is_valid() {
        return;
    }
    let multiplicity = type_multiplicity_with_substitutions(program, type_reference, substitutions);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            if multiplicity == Multiplicity::Linear {
                claims.push(LinearClaimTemplate {
                    path: path.to_vec(),
                    type_reference,
                    multiplicity,
                    conditional: path
                        .iter()
                        .any(|segment| matches!(segment, psi_facts::PlaceSegment::Case { .. })),
                });
                return;
            }
            append_linear_claim_frontier(
                program,
                *base_type,
                substitutions,
                path,
                visiting,
                claims,
            );
            return;
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            for index in 0..*length {
                let mut element_path = path.to_vec();
                element_path.push(psi_facts::PlaceSegment::FixedIndex { index });
                append_linear_claim_frontier(
                    program,
                    *element_type,
                    substitutions,
                    &element_path,
                    visiting,
                    claims,
                );
            }
            return;
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some(replacement) =
                substitutions
                    .iter()
                    .rev()
                    .find_map(|(parameter, replacement)| {
                        (*parameter == *symbol).then_some(*replacement)
                    })
                && replacement != type_reference
            {
                append_linear_claim_frontier(
                    program,
                    replacement,
                    substitutions,
                    path,
                    visiting,
                    claims,
                );
                return;
            }
        }
        _ => {}
    }

    if multiplicity == Multiplicity::Linear {
        claims.push(LinearClaimTemplate {
            path: path.to_vec(),
            type_reference,
            multiplicity,
            conditional: path
                .iter()
                .any(|segment| matches!(segment, psi_facts::PlaceSegment::Case { .. })),
        });
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { .. } => unreachable!("handled before multiplicity"),
        TypeReferenceNode::Named { symbol, name } => {
            let Some(definition) = find_data_definition(program, *symbol, name.as_str()) else {
                return;
            };
            append_data_linear_claim_frontier(
                program,
                definition,
                substitutions,
                path,
                visiting,
                claims,
            );
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            let Some(definition) = find_data_definition(program, *base_symbol, base_name.as_str())
            else {
                return;
            };
            let mut instantiated = substitutions.to_vec();
            instantiated.extend(
                program
                    .data_type_parameters(definition)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments),
                    )
                    .filter_map(|(parameter, argument)| {
                        matches!(
                            parameter.kind,
                            psi_typed_trees::data::TypeParameterKind::Type
                        )
                        .then_some((parameter.symbol, *argument))
                    }),
            );
            append_data_linear_claim_frontier(
                program,
                definition,
                &instantiated,
                path,
                visiting,
                claims,
            );
        }
        TypeReferenceNode::FixedArray { .. } => {
            // Const-parameter lengths must become literal before this stage
            // can enumerate the complete fixed-index ownership frontier.
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn append_data_linear_claim_frontier(
    program: &psi_typed_trees::TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[psi_facts::PlaceSegment],
    visiting: &mut Vec<SymbolHandle>,
    claims: &mut Vec<LinearClaimTemplate>,
) {
    if visiting.contains(&definition.symbol) {
        return;
    }
    visiting.push(definition.symbol);
    for member in program.data_members(definition) {
        match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                let mut field_path = path.to_vec();
                field_path.push(psi_facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                append_linear_claim_frontier(
                    program,
                    field.type_reference,
                    substitutions,
                    &field_path,
                    visiting,
                    claims,
                );
            }
            psi_typed_trees::data::DataMember::Variant(variant) => {
                let mut case_path = path.to_vec();
                case_path.push(psi_facts::PlaceSegment::Case {
                    variant: variant.symbol,
                });
                for field in program.data_payload_fields(variant) {
                    let mut field_path = case_path.clone();
                    field_path.push(psi_facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    append_linear_claim_frontier(
                        program,
                        field.type_reference,
                        substitutions,
                        &field_path,
                        visiting,
                        claims,
                    );
                }
            }
        }
    }
    visiting.pop();
}

fn claim_place_name(
    program: &psi_typed_trees::TypedTrees,
    root: &str,
    path: &[psi_facts::PlaceSegment],
) -> String {
    let mut name = root.to_owned();
    for segment in path {
        match segment {
            psi_facts::PlaceSegment::Case { variant } => {
                let case = program.data_definitions().iter().find_map(|definition| {
                    program.data_members(definition).iter().find_map(|member| {
                        let psi_typed_trees::data::DataMember::Variant(candidate) = member else {
                            return None;
                        };
                        (candidate.symbol == *variant).then_some(candidate.name.as_str())
                    })
                });
                name.push_str("::");
                name.push_str(case.unwrap_or("<case>"));
            }
            psi_facts::PlaceSegment::Field { symbol } => {
                let field = data_field_name(program, *symbol);
                name.push('.');
                name.push_str(field.unwrap_or("<field>"));
            }
            psi_facts::PlaceSegment::FixedIndex { index } => {
                name.push('[');
                name.push_str(&index.to_string());
                name.push(']');
            }
            psi_facts::PlaceSegment::Index { .. } => name.push_str("[<index>]"),
        }
    }
    name
}

fn apply_recorded_state_entry_events(
    events: &[&FlowPermissionEventFact],
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    places: &mut [LinearPlace],
) {
    for event in events.iter().copied().filter(|event| {
        event.source == PermissionEventSource::StateEntry
            && event.kind == PermissionEventKind::Establish
    }) {
        let psi_facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let event_path = segments.span_or_empty(event.segments);
        let Some(place) = places
            .iter_mut()
            .find(|place| place.symbol == symbol && place.path.as_slice() == event_path)
        else {
            continue;
        };
        place.live = event.obligation_live;
        place.ever_established = true;
        place.claim_identity = Some(event.claim_identity);
        place.provenance = Some(event.provenance);
    }
}

fn apply_recorded_statement_events(
    statement_index: usize,
    events: &[&FlowPermissionEventFact],
    segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    places: &mut [LinearPlace],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for event in events.iter().copied().filter(|event| {
        permission_event_statement_index(event.source) == Some(statement_index)
            && event.kind != PermissionEventKind::AffineDrop
    }) {
        let psi_facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let event_path = segments.span_or_empty(event.segments);
        let Some(place) = places
            .iter_mut()
            .find(|place| place.symbol == symbol && place.path.as_slice() == event_path)
        else {
            continue;
        };
        match event.kind {
            PermissionEventKind::Transfer | PermissionEventKind::Consume => {
                if !place.ever_established {
                    diagnostics.push(Diagnostic::error(format!(
                        "linear value `{}` has not been established (implicit zero-fill creates no linear obligation); it cannot be moved here",
                        place.name
                    )));
                } else if !place.live {
                    diagnostics.push(Diagnostic::error(format!(
                        "linear value `{}` was already transferred or consumed; it cannot be moved here",
                        place.name
                    )));
                } else {
                    place.live = false;
                }
            }
            PermissionEventKind::Establish => {
                if place.live {
                    diagnostics.push(Diagnostic::error(format!(
                        "assignment would overwrite live linear value `{}`; consume or transfer the existing obligation first",
                        place.name
                    )));
                }
                place.live = event.obligation_live;
                place.ever_established = true;
                place.claim_identity = Some(event.claim_identity);
                place.provenance = Some(event.provenance);
            }
            PermissionEventKind::AffineDrop => {}
        }
    }
}

fn permission_event_statement_index(source: PermissionEventSource) -> Option<usize> {
    match source {
        PermissionEventSource::Statement { statement_index }
        | PermissionEventSource::Call {
            statement_index, ..
        } => Some(statement_index),
        PermissionEventSource::StateEntry | PermissionEventSource::StateExit => None,
    }
}

fn append_borrow_permission_events(
    facts: &mut CheckFacts,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    claim_identities: &mut ClaimIdentityAllocator,
) {
    // Clone only the small state/span index so the ownership-segment arena can
    // be extended while the already-built borrow facts remain immutable.
    let states = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| {
            (
                state.machine_symbol,
                state.state_symbol,
                state.borrow_activations,
                state.borrow_weakenings,
            )
        })
        .collect::<Vec<_>>();

    let mut loan_claim_identities = Vec::new();
    for (machine_symbol, state_symbol, activations, weakenings) in states {
        for activation in facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(activations)
            .to_vec()
        {
            let claim_identity = claim_identities.mint(
                machine_symbol,
                state_symbol,
                permission_source_from_invalidation(activation.source),
            );
            loan_claim_identities.push((activation.loan, claim_identity));
            append_borrow_permission_event(
                facts,
                permission_events,
                machine_symbol,
                state_symbol,
                activation.loan,
                permission_source_from_invalidation(activation.source),
                PermissionEventKind::Establish,
                claim_identity,
            );
        }
        for weakening in facts
            .flow
            .borrow_lifetimes
            .weakenings
            .span_or_empty(weakenings)
            .to_vec()
        {
            let source =
                if weakening.reason == psi_checked_trees::FlowBorrowWeakeningReason::StateExit {
                    PermissionEventSource::StateExit
                } else {
                    permission_source_from_invalidation(weakening.source)
                };
            let claim_identity = loan_claim_identities
                .iter()
                .rev()
                .find_map(|(loan, identity)| (*loan == weakening.loan).then_some(*identity))
                .unwrap_or(PermissionClaimIdentity::Unknown);
            append_borrow_permission_event(
                facts,
                permission_events,
                machine_symbol,
                state_symbol,
                weakening.loan,
                source,
                PermissionEventKind::Consume,
                claim_identity,
            );
        }
    }
}

fn append_borrow_permission_event(
    facts: &mut CheckFacts,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    loan_handle: psi_arena::Handle<psi_checked_trees::BorrowLoanFact>,
    source: PermissionEventSource,
    kind: PermissionEventKind,
    claim_identity: PermissionClaimIdentity,
) {
    let loan = facts.borrow.loans.get(loan_handle).clone();
    let segments = facts
        .flow
        .ownership
        .segments
        .insert_many(facts.borrow.loan_segments(&loan).iter().copied());
    let (multiplicity, access) = match loan.kind {
        psi_checked_trees::BorrowAccessKind::Read => {
            (Multiplicity::Unrestricted, PermissionAccess::Shared)
        }
        psi_checked_trees::BorrowAccessKind::Mutable => {
            (Multiplicity::Affine, PermissionAccess::Exclusive)
        }
    };
    permission_events.push(FlowPermissionEventFact {
        machine_symbol,
        state_symbol,
        source,
        kind,
        multiplicity,
        access,
        claim_identity,
        provenance: established_provenance(
            machine_symbol,
            state_symbol,
            PermissionEventSource::Statement {
                statement_index: loan.statement_index,
            },
        ),
        root: psi_facts::PlaceRoot::Symbol(loan.root_symbol),
        segments,
        obligation_live: false,
    });
}

fn permission_source_from_invalidation(
    source: psi_checked_trees::FlowInvalidationSource,
) -> PermissionEventSource {
    match source {
        psi_checked_trees::FlowInvalidationSource::Statement { statement_index } => {
            PermissionEventSource::Statement { statement_index }
        }
        psi_checked_trees::FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_statement_permission_production(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    moves: &[crate::flow::DiscoveredMoveEvent],
    statement_index: usize,
    statement: &StatementNode,
    places: &mut [LinearPlace],
    permission_events: &mut Vec<FlowPermissionEventFact>,
    claim_identities: &mut ClaimIdentityAllocator,
) {
    // Destructure coverage markers are proof-only reads synthesized by the
    // parser; they neither transfer a value nor establish user storage.
    if matches!(statement, StatementNode::LocalData(local) if local.name.as_str().starts_with("__arm_destructure#"))
    {
        return;
    }

    let written_targets =
        written_linear_targets(program, state_symbol, statement_index, statement, places);

    // Moves out of initializer/assignment sources happen before the
    // destination becomes established. The old move-only summary also
    // contains a production event *at* the destination; exclude that
    // compatibility event here rather than mistaking creation for use.
    for event in moves
        .iter()
        .filter(|event| event_statement_index(event.source) == Some(statement_index))
    {
        let psi_facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let event_path = facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .to_vec();
        select_static_case_alternative(symbol, &event_path, places);
        if written_targets.iter().any(|target| {
            target.root == event.root && target.destination_path.as_slice() == event_path
        }) {
            continue;
        }
        let matching = places
            .iter()
            .enumerate()
            .filter_map(|(index, place)| {
                (place.symbol == symbol && move_selects_claim(&event_path, place)).then_some(index)
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            continue;
        }
        let kind = permission_kind_for_move(program, facts, machine_symbol, state_symbol, event);
        for index in matching {
            let claim_path = places[index].path.clone();
            if places[index].ever_established
                && !places[index].live
                && places[index].conditional
                && !event_path
                    .iter()
                    .any(|segment| matches!(segment, psi_facts::PlaceSegment::Case { .. }))
            {
                continue;
            }
            let segments = facts.flow.ownership.segments.insert_many(claim_path);
            let place = &mut places[index];
            let obligation_live = place.live;
            permission_events.push(FlowPermissionEventFact {
                machine_symbol,
                state_symbol,
                source: permission_source(event.source),
                kind,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity: place
                    .claim_identity
                    .unwrap_or(PermissionClaimIdentity::Unknown),
                provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                root: event.root,
                segments,
                obligation_live,
            });
            place.live = false;
        }
    }

    for target in written_targets {
        let psi_facts::PlaceRoot::Symbol(symbol) = target.root else {
            continue;
        };
        let place_index = target.place_index;
        let obligation_live = target.obligation_live;
        let claim_identity = target.claim_identity.unwrap_or_else(|| {
            obligation_live
                .then(|| {
                    claim_identities.mint(
                        machine_symbol,
                        state_symbol,
                        PermissionEventSource::Statement { statement_index },
                    )
                })
                .unwrap_or(PermissionClaimIdentity::Unknown)
        });
        let provenance = target.provenance;
        let claim_path = places[place_index].path.clone();
        let segments = facts.flow.ownership.segments.insert_many(claim_path);
        let place = &mut places[place_index];
        debug_assert_eq!(place.symbol, symbol);
        place.live = obligation_live;
        place.ever_established = true;
        place.claim_identity = Some(claim_identity);
        place.provenance = Some(provenance.unwrap_or_else(|| {
            established_provenance(
                machine_symbol,
                state_symbol,
                PermissionEventSource::Statement { statement_index },
            )
        }));
        permission_events.push(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index },
            kind: PermissionEventKind::Establish,
            multiplicity: place.multiplicity,
            access: PermissionAccess::Owned,
            claim_identity,
            provenance: place
                .provenance
                .expect("an established place has explicit provenance"),
            root: psi_facts::PlaceRoot::Symbol(symbol),
            segments,
            obligation_live,
        });
    }
}

fn move_selects_claim(event_path: &[psi_facts::PlaceSegment], claim: &LinearPlace) -> bool {
    claim.path.starts_with(event_path)
        || (claim.conditional && event_path.starts_with(claim.path.as_slice()))
}

fn select_static_case_alternative(
    symbol: SymbolHandle,
    event_path: &[psi_facts::PlaceSegment],
    places: &mut [LinearPlace],
) {
    let Some((case_index, selected_variant)) =
        event_path
            .iter()
            .enumerate()
            .find_map(|(index, segment)| match segment {
                psi_facts::PlaceSegment::Case { variant } => Some((index, *variant)),
                _ => None,
            })
    else {
        return;
    };
    let prefix = &event_path[..case_index];
    for place in places {
        if place.symbol != symbol
            || place.path.get(..case_index) != Some(prefix)
            || place.path.get(case_index)
                == Some(&psi_facts::PlaceSegment::Case {
                    variant: selected_variant,
                })
        {
            continue;
        }
        if matches!(
            place.path.get(case_index),
            Some(psi_facts::PlaceSegment::Case { .. })
        ) {
            place.live = false;
        }
    }
}

fn established_provenance(
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    source: PermissionEventSource,
) -> PermissionProvenance {
    PermissionProvenance::Established {
        machine_symbol,
        state_symbol,
        source,
    }
}

fn event_statement_index(source: FlowOwnershipEventSource) -> Option<usize> {
    match source {
        FlowOwnershipEventSource::Statement { statement_index }
        | FlowOwnershipEventSource::Call {
            statement_index, ..
        } => Some(statement_index),
    }
}

fn permission_source(source: FlowOwnershipEventSource) -> PermissionEventSource {
    match source {
        FlowOwnershipEventSource::Statement { statement_index } => {
            PermissionEventSource::Statement { statement_index }
        }
        FlowOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
    }
}

fn permission_kind_for_move(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
) -> PermissionEventKind {
    let FlowOwnershipEventSource::Call {
        statement_index,
        call_ordinal,
        target_symbol,
    } = event.source
    else {
        return PermissionEventKind::Transfer;
    };
    let Some(call_site) = crate::find_call_site(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    ) else {
        return PermissionEventKind::Transfer;
    };
    let Some(target_state) = crate::find_state(program, target_symbol) else {
        return PermissionEventKind::Transfer;
    };
    let arguments = crate::call_site_argument_expressions(program, &call_site);
    let parameters = program.state_parameters(target_state);
    let event_segments = facts.flow.ownership.segments.span_or_empty(event.segments);
    if arguments.len() == parameters.len() {
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if !parameter.is_self {
                continue;
            }
            let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                *argument,
            ) else {
                continue;
            };
            if place.root == event.root && place.segments.as_slice() == event_segments {
                return if type_carries_linear_obligation(program, target_state.return_type) {
                    PermissionEventKind::Transfer
                } else {
                    PermissionEventKind::Consume
                };
            }
        }
    } else if let Some(place) = crate::flow::owned_method_receiver_place(
        program,
        state_symbol,
        statement_index,
        &call_site,
        parameters,
        SymbolHandle::invalid(),
    ) {
        if place.root == event.root && place.segments.as_slice() == event_segments {
            return if type_carries_linear_obligation(program, target_state.return_type) {
                PermissionEventKind::Transfer
            } else {
                PermissionEventKind::Consume
            };
        }
    }
    PermissionEventKind::Transfer
}

fn written_linear_targets(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    places: &[LinearPlace],
) -> Vec<WrittenLinearTarget> {
    let (target, value) = match statement {
        StatementNode::LocalData(local) => {
            if !local.initial_value.is_valid() {
                return Vec::new();
            }
            (
                crate::flow::CanonicalPlace {
                    root: psi_facts::PlaceRoot::Symbol(local.symbol),
                    segments: Vec::new(),
                },
                local.initial_value,
            )
        }
        StatementNode::Assignment(assignment) => {
            let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            ) else {
                return Vec::new();
            };
            (place, assignment.value)
        }
        _ => return Vec::new(),
    };
    let psi_facts::PlaceRoot::Symbol(symbol) = target.root else {
        return Vec::new();
    };

    places
        .iter()
        .enumerate()
        .filter_map(|(place_index, tracked)| {
            if tracked.symbol != symbol || !tracked.path.starts_with(target.segments.as_slice()) {
                return None;
            }
            let relative_path = &tracked.path[target.segments.len()..];
            Some(WrittenLinearTarget {
                root: target.root,
                destination_path: target.segments.clone(),
                place_index,
                obligation_live: expression_establishes_obligation(
                    program,
                    state_symbol,
                    statement_index,
                    value,
                    relative_path,
                    places,
                ),
                claim_identity: expression_permission_claim_identity_for_claim(
                    program,
                    state_symbol,
                    statement_index,
                    value,
                    relative_path,
                    places,
                ),
                provenance: expression_permission_provenance_for_claim(
                    program,
                    state_symbol,
                    statement_index,
                    value,
                    relative_path,
                    places,
                ),
            })
        })
        .collect()
}

fn expression_permission_claim_identity_for_claim(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: psi_typed_trees::expression::ExpressionHandle,
    relative_path: &[psi_facts::PlaceSegment],
    places: &[LinearPlace],
) -> Option<PermissionClaimIdentity> {
    if relative_path.is_empty() {
        match program.expression_table.expression(expression) {
            psi_typed_trees::expression::ExpressionNode::Call(call) => {
                let mut candidates = Vec::new();
                if call.receiver.is_valid() {
                    candidates.push(call.receiver);
                }
                candidates
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
                return common_permission_claim_identity(candidates.into_iter().filter_map(
                    |candidate| {
                        expression_permission_claim_identity_for_claim(
                            program,
                            state_symbol,
                            statement_index,
                            candidate,
                            &[],
                            places,
                        )
                    },
                ));
            }
            psi_typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                return common_permission_claim_identity(
                    program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .filter_map(|field| {
                            expression_permission_claim_identity_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                field.value,
                                &[],
                                places,
                            )
                        }),
                );
            }
            psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
                return common_permission_claim_identity(
                    program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .filter_map(|value| {
                            expression_permission_claim_identity_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                *value,
                                &[],
                                places,
                            )
                        }),
                );
            }
            _ => {}
        }
    }

    if let psi_typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(psi_facts::PlaceSegment::Case { variant }) = relative_path.first()
    {
        if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant) {
            return None;
        }
        return expression_permission_claim_identity_for_claim(
            program,
            state_symbol,
            statement_index,
            expression,
            &relative_path[1..],
            places,
        );
    }

    if let psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
        program.expression_table.expression(expression)
        && let Some(psi_facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
    {
        let value = *program
            .expression_table
            .expression_handles(*values)
            .get(*index)?;
        return expression_permission_claim_identity_for_claim(
            program,
            state_symbol,
            statement_index,
            value,
            &relative_path[1..],
            places,
        );
    }

    if let psi_typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(psi_facts::PlaceSegment::Field { symbol }) = relative_path.first()
    {
        let field_name = data_field_name(program, *symbol)?;
        let field = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name.as_str() == field_name)?;
        return expression_permission_claim_identity_for_claim(
            program,
            state_symbol,
            statement_index,
            field.value,
            &relative_path[1..],
            places,
        );
    }

    if !relative_path.is_empty()
        && matches!(
            program.expression_table.expression(expression),
            psi_typed_trees::expression::ExpressionNode::Call(_)
        )
    {
        return None;
    }

    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    let psi_facts::PlaceRoot::Symbol(symbol) = source.root else {
        return None;
    };
    let mut source_path = source.segments;
    source_path.extend_from_slice(relative_path);
    let matches = places
        .iter()
        .filter(|place| {
            place.symbol == symbol
                && place.live
                && (place.path == source_path
                    || (place.conditional && source_path.starts_with(place.path.as_slice())))
        })
        .filter_map(|place| place.claim_identity);
    common_permission_claim_identity(matches)
}

fn expression_permission_provenance_for_claim(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: psi_typed_trees::expression::ExpressionHandle,
    relative_path: &[psi_facts::PlaceSegment],
    places: &[LinearPlace],
) -> Option<PermissionProvenance> {
    if relative_path.is_empty() {
        match program.expression_table.expression(expression) {
            psi_typed_trees::expression::ExpressionNode::Call(call) => {
                let mut candidates = Vec::new();
                if call.receiver.is_valid() {
                    candidates.push(call.receiver);
                }
                candidates
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
                return common_permission_provenance(candidates.into_iter().filter_map(
                    |candidate| {
                        expression_permission_provenance_for_claim(
                            program,
                            state_symbol,
                            statement_index,
                            candidate,
                            &[],
                            places,
                        )
                    },
                ));
            }
            psi_typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                return common_permission_provenance(
                    program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .filter_map(|field| {
                            expression_permission_provenance_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                field.value,
                                &[],
                                places,
                            )
                        }),
                );
            }
            psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
                return common_permission_provenance(
                    program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .filter_map(|value| {
                            expression_permission_provenance_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                *value,
                                &[],
                                places,
                            )
                        }),
                );
            }
            _ => {}
        }
    }

    if let psi_typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(psi_facts::PlaceSegment::Case { variant }) = relative_path.first()
    {
        if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant) {
            return None;
        }
        return expression_permission_provenance_for_claim(
            program,
            state_symbol,
            statement_index,
            expression,
            &relative_path[1..],
            places,
        );
    }

    if let psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
        program.expression_table.expression(expression)
        && let Some(psi_facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
    {
        let value = *program
            .expression_table
            .expression_handles(*values)
            .get(*index)?;
        return expression_permission_provenance_for_claim(
            program,
            state_symbol,
            statement_index,
            value,
            &relative_path[1..],
            places,
        );
    }

    if let psi_typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
        && let Some(psi_facts::PlaceSegment::Field { symbol }) = relative_path.first()
    {
        let field_name = data_field_name(program, *symbol)?;
        let field = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name.as_str() == field_name)?;
        return expression_permission_provenance_for_claim(
            program,
            state_symbol,
            statement_index,
            field.value,
            &relative_path[1..],
            places,
        );
    }

    if !relative_path.is_empty()
        && matches!(
            program.expression_table.expression(expression),
            psi_typed_trees::expression::ExpressionNode::Call(_)
        )
    {
        // Multi-output call mappings need the explicit P1c outcome map. Do
        // not guess a field origin from argument order.
        return None;
    }

    let source = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    let psi_facts::PlaceRoot::Symbol(symbol) = source.root else {
        return None;
    };
    let mut source_path = source.segments;
    source_path.extend_from_slice(relative_path);
    let matches = places
        .iter()
        .filter(|place| {
            place.symbol == symbol
                && place.live
                && (place.path == source_path
                    || (place.conditional && source_path.starts_with(place.path.as_slice())))
        })
        .filter_map(|place| place.provenance);
    common_permission_provenance(matches)
}

/// Discover ordinary affine cleanup directly from typed ownership. Locals drop in
/// reverse declaration order, followed by owned by-value parameters in reverse
/// declaration order, exactly matching the language's cleanup order. Linear
/// and conditional roots are excluded because their path-sensitive settlement
/// is represented by the permission events produced above.
fn append_affine_cleanup_permission_events(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    machine_symbol: SymbolHandle,
    tracked_places: &[LinearPlace],
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    let mut append = |symbol: SymbolHandle, type_reference: TypeReferenceHandle| {
        if tracked_places.iter().any(|place| place.symbol == symbol)
            || type_multiplicity(program, type_reference) == Multiplicity::Unrestricted
        {
            return;
        }
        permission_events.push(FlowPermissionEventFact {
            machine_symbol,
            state_symbol: state.symbol,
            source: PermissionEventSource::StateExit,
            kind: PermissionEventKind::AffineDrop,
            multiplicity: Multiplicity::Affine,
            access: PermissionAccess::Owned,
            claim_identity: PermissionClaimIdentity::Unknown,
            provenance: PermissionProvenance::Unknown,
            root: psi_facts::PlaceRoot::Symbol(symbol),
            segments: HandleSpan::empty(),
            obligation_live: false,
        });
    };

    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .rev()
    {
        if let StatementNode::LocalData(local) = statement {
            append(local.symbol, local.type_reference);
        }
    }
    for parameter in program.state_parameters(state).iter().rev() {
        if !parameter.is_self {
            append(parameter.symbol, parameter.type_reference);
        }
    }
}

fn common_permission_provenance(
    mut origins: impl Iterator<Item = PermissionProvenance>,
) -> Option<PermissionProvenance> {
    let first = origins.next()?;
    origins.all(|origin| origin == first).then_some(first)
}

fn common_permission_claim_identity(
    mut identities: impl Iterator<Item = PermissionClaimIdentity>,
) -> Option<PermissionClaimIdentity> {
    let first = identities.next()?;
    identities
        .all(|identity| identity == first)
        .then_some(first)
}

pub(crate) fn type_carries_linear_obligation(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    !linear_claim_frontier(program, type_reference).is_empty()
}

fn expression_establishes_obligation(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: psi_typed_trees::expression::ExpressionHandle,
    relative_path: &[psi_facts::PlaceSegment],
    places: &[LinearPlace],
) -> bool {
    if let Some(source) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    ) && let psi_facts::PlaceRoot::Symbol(symbol) = source.root
    {
        let mut source_path = source.segments;
        source_path.extend_from_slice(relative_path);
        if let Some(place) = places
            .iter()
            .find(|place| place.symbol == symbol && place.path == source_path)
        {
            return place.live;
        }
    }

    if let psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
        program.expression_table.expression(expression)
        && let Some(psi_facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
    {
        return program
            .expression_table
            .expression_handles(*values)
            .get(*index)
            .is_some_and(|value| {
                expression_establishes_obligation(
                    program,
                    state_symbol,
                    statement_index,
                    *value,
                    &relative_path[1..],
                    places,
                )
            });
    }

    if let psi_typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
    {
        let mut remaining = relative_path;
        if literal.case_name.is_some() {
            let Some(psi_facts::PlaceSegment::Case { variant }) = remaining.first() else {
                return remaining.is_empty();
            };
            if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant)
            {
                return false;
            }
            remaining = &remaining[1..];
        }
        if let Some(psi_facts::PlaceSegment::Field { symbol }) = remaining.first() {
            let Some(field_name) = data_field_name(program, *symbol) else {
                return false;
            };
            let Some(field) = program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .find(|field| field.name.as_str() == field_name)
            else {
                return false;
            };
            return expression_establishes_obligation(
                program,
                state_symbol,
                statement_index,
                field.value,
                &remaining[1..],
                places,
            );
        }
        return remaining.is_empty();
    }

    if matches!(
        program.expression_table.expression(expression),
        psi_typed_trees::expression::ExpressionNode::Name(path)
            if program.data_definitions().iter().any(|definition| {
                program.data_members(definition).iter().any(|member| {
                    matches!(
                        member,
                        psi_typed_trees::data::DataMember::Variant(variant)
                            if variant.symbol == path.symbol
                    )
                })
            })
    ) {
        // A payloadless case has no obligation for a conditional payload
        // frontier, but constructing a data type declared `[linear]` still
        // establishes that type's root obligation. The latter arrives with an
        // empty relative path; case/payload claims retain at least one segment.
        return relative_path.is_empty();
    }

    // Calls and boundary results have unknown active cases. Every case path is
    // retained as a possible obligation until a static case projection selects
    // one alternative.
    true
}

pub(crate) fn type_multiplicity(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Multiplicity {
    program.type_multiplicity(type_reference)
}

fn type_multiplicity_with_substitutions(
    program: &psi_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_multiplicity_with_substitutions(program, *base_type, substitutions)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_multiplicity_with_substitutions(program, *element_type, substitutions)
        }
        TypeReferenceNode::Named { symbol, .. } => substitutions
            .iter()
            .rev()
            .find_map(|(parameter, replacement)| {
                (*parameter == *symbol && *replacement != type_reference).then_some(*replacement)
            })
            .map(|replacement| {
                type_multiplicity_with_substitutions(program, replacement, substitutions)
            })
            .unwrap_or_else(|| type_multiplicity(program, type_reference)),
        _ => type_multiplicity(program, type_reference),
    }
}

fn find_data_definition<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program psi_typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name.as_str() == name
    })
}

fn literal_variant<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    literal: &psi_typed_trees::expression::TableStructLiteral,
) -> Option<&'program psi_typed_trees::data::DataVariant> {
    let case_name = literal.case_name.as_ref()?;
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == literal.type_name)?;
    program.data_members(definition).iter().find_map(|member| {
        let psi_typed_trees::data::DataMember::Variant(variant) = member else {
            return None;
        };
        (variant.name == *case_name).then_some(variant)
    })
}

fn data_field_name(
    program: &psi_typed_trees::TypedTrees,
    field_symbol: SymbolHandle,
) -> Option<&str> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) => {
                    (field.symbol == field_symbol).then_some(field.name.as_str())
                }
                psi_typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find_map(|field| {
                        (field.symbol == field_symbol).then_some(field.name.as_str())
                    }),
            })
    })
}

#[cfg(test)]
mod generic_substitution_tests {
    use super::*;
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn linear_generic_bound_classifies_the_parameter_type() {
        let source = r#"
            data Main {}
            machine Main::identity<T [linear]>(value: T) -> T {
                value
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::identity")
            .expect("generic identity machine");
        let state = typed
            .machine_states(machine)
            .first()
            .expect("generic identity state");
        let parameter = typed
            .state_parameters(state)
            .iter()
            .find(|parameter| !parameter.is_self)
            .expect("linear generic value parameter");
        assert_eq!(
            type_multiplicity(&typed, parameter.type_reference),
            Multiplicity::Linear
        );
    }
}
