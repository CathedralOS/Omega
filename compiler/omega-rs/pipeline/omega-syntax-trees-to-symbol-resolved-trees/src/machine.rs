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
    lowerer.current_machine_is_boundary = machine.boundary;
    let states = lower_machine_states(lowerer, syntax_trees, machine.states)?;
    lowerer.current_machine_is_boundary = false;
    let type_parameters = lower_type_parameters(lowerer, syntax_trees, machine.type_parameters)?;
    let satisfies = lower_machine_trait_conformances(lowerer, syntax_trees, machine.satisfies);
    let decreases = lower_machine_decreases(lowerer, syntax_trees, machine.decreases)?;
    let decrease_order =
        lower_machine_decrease_order(lowerer, syntax_trees, machine.decrease_order);
    // TPR3: argumented-view arguments lower exactly like the subjects.
    let decrease_view_arguments =
        lower_machine_decreases(lowerer, syntax_trees, machine.decrease_view_arguments)?;
    // TPR3 slice 3: the `in <range>` rank constraint lowers as an ordinary
    // expression; the CHECKER verifies it structurally (v1: floor 0 +
    // ceiling equal to the argumented view's own bound) and fails
    // compilation otherwise -- consumed, never silently dropped.
    let decrease_range = if machine.decrease_range.is_valid() {
        lower_expression_into_table(
            lowerer,
            syntax_trees,
            machine.decrease_range,
        )?
    } else {
        omega_symbol_resolved_trees::expression::ExpressionHandle::invalid()
    };
    let effects = lower_signature_effects(lowerer, syntax_trees, machine.effects);
    let contracts = lower_signature_contracts(lowerer, syntax_trees, machine.contracts)?;
    let machine_name = crate::name::lower_name(&machine.name);
    let attached_data = machine.attached_data.as_ref().map(crate::name::lower_name);
    let termination_plan = build_termination_plan(lowerer, syntax_trees, machine, states);

    // STR3: the supply mode's ONE population site. Requirement gains its
    // source when trait requirements reach this record; Accepted is the
    // bodyless `boundary machine` proof form (CH10 GR6d); a bodyless
    // NON-boundary machine with a `via` clause is PRV4's external leaf (the
    // item parser refuses every other bodyless shape). Computed before the
    // push so the interner borrow does not overlap the machines borrow.
    let supply_mode = {
        let via_rendering = syntax_trees
            .items
            .satisfies_clauses(machine.satisfies)
            .iter()
            .find_map(|clause| clause.via.as_ref())
            .map(|binding| binding.normalized_rendering());
        if machine.bodyless && machine.boundary {
            // A bodyless boundary declaration is ACCEPTED only when it
            // actually authors a fact. Claim-free symbols such as the
            // axiomatic Real package's operations assert nothing and need no
            // grant; they remain ordinary boundary supply.
            let authors_fact = syntax_trees
                .items
                .capability_contracts(machine.contracts)
                .iter()
                .any(|contract| {
                    matches!(
                        contract.kind,
                        omega_syntax_trees::item::CapabilityContractKind::Ensures
                    ) && !contract.facts.is_empty()
                });
            if authors_fact {
                omega_core::semantics::MachineSupplyMode::Accepted
            } else {
                omega_core::semantics::MachineSupplyMode::Boundary
            }
        } else if let (true, Some(rendering)) = (machine.bodyless, &via_rendering) {
            omega_core::semantics::MachineSupplyMode::ExternalRealization {
                binding: lowerer
                    .symbol_resolved_trees
                    .external_bindings
                    .intern(rendering),
            }
        } else if machine.boundary {
            omega_core::semantics::MachineSupplyMode::Boundary
        } else {
            omega_core::semantics::MachineSupplyMode::CheckedBody
        }
    };
    lowerer.symbol_resolved_trees.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        name: machine_name,
        attached_data,
        boundary: machine.boundary,
        supply_mode,
        // TPR2: the termination plan's ONE population site (see
        // build_termination_plan below).
        termination_plan,
        service_reach_row: omega_core::semantics::ServiceReachRowId::NULL,
        storage: MachineStorage {
            lifetime_parameters: machine
                .lifetime_parameters
                .iter()
                .map(crate::name::lower_name)
                .collect(),
            type_parameters,
            owned_data: HandleSpan::empty(),
            satisfies,
            terminates: machine.terminates,
            decreases,
            decrease_order,
            decrease_view_arguments,
            decrease_range,
            effects,
            suspends: machine.suspends,
            blocks: machine.blocks,
            contracts,
            states,
        },
    });
    Ok(())
}

/// TPR2 (decision 23): populate the normalized `MachineTerminationPlan` --
/// the plan's ONE population site; every later stage copies, never
/// re-derives.
///
/// - Bare `terminates;` authors the PUBLIC eventual-terminal guarantee
///   (premises land with TPR4's pinned progress profiles). `terminates by
///   ...` alone publishes NOTHING: it supplies the private witness for a
///   checked body or an inherited claim (the brief's firewall).
/// - `checked_summary` stays `NoGuarantee` here: it is the CHECKER's to
///   establish (TPR3), never claimed at lowering.
/// - The witness records the ranked subjects and the EXPLICIT ranking view.
///   Canonical defaults elaborate immediately where this stage determines
///   them (the two-subject builtin, `.len` subjects, and root-state
///   parameters of unsigned/slice type -- mirroring the checker's inference
///   EXACTLY); the remaining single-subject short forms keep an empty
///   `view_path` until TPR3 folds elaboration into the migrated checker.
fn build_termination_plan(
    lowerer: &Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    states: HandleSpan<Handle<State>>,
) -> omega_core::semantics::MachineTerminationPlan {
    use omega_core::semantics::{MachineTerminationPlan, RankingWitness, TerminationGuarantee};

    let published = machine
        .terminates_guarantee
        .then(|| TerminationGuarantee::EventualTerminal {
            premises: Vec::new(),
        });

    let subjects = syntax_trees
        .expressions
        .expression_handles(machine.decreases);
    let implementation_witness = (!subjects.is_empty()).then(|| {
        let (ranking_view, view_path) =
            elaborate_ranking_view(lowerer, syntax_trees, machine, states, subjects);
        RankingWitness {
            subjects: subjects
                .iter()
                .map(|subject| render_ranked_subject(syntax_trees, *subject))
                .collect(),
            ranking_view,
            view_path,
            // TPR3: an argumented view's arguments, rendered source-like in
            // order (`Nat::IncreasingTo(limit)` carries `["limit"]`).
            view_arguments: syntax_trees
                .expressions
                .expression_handles(machine.decrease_view_arguments)
                .iter()
                .map(|argument| render_ranked_subject(syntax_trees, *argument))
                .collect(),
            // TPR3 slice 3: the authored rank-range fact, rendered
            // source-like; the checker verifies it structurally and fails
            // compilation otherwise.
            rank_range: machine.decrease_range.is_valid().then(|| {
                let syntax::expression::ExpressionNode::Range(range) =
                    syntax_trees.expressions.expression(machine.decrease_range)
                else {
                    return omega_core::semantics::RankRange::default();
                };
                omega_core::semantics::RankRange {
                    floor: render_ranked_subject(syntax_trees, range.start),
                    ceiling: render_ranked_subject(syntax_trees, range.end),
                    ceiling_inclusive: range.end_inclusive,
                }
            }),
        }
    });

    MachineTerminationPlan {
        published,
        checked_summary: TerminationGuarantee::NoGuarantee,
        implementation_witness,
    }
}

/// Render one ranked subject as source-like text for the private witness
/// (mirrors the checker's `decreasing_value_text`).
fn render_ranked_subject(
    syntax_trees: &SyntaxTrees,
    subject: syntax::expression::ExpressionHandle,
) -> String {
    match syntax_trees.expressions.expression(subject) {
        // Range endpoints are commonly literals (`in 0..=capacity`).
        syntax::expression::ExpressionNode::Integer(literal) => literal.text().to_string(),
        syntax::expression::ExpressionNode::Name(path) => syntax_trees
            .expressions
            .identifier_path_members(*path)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("."),
        syntax::expression::ExpressionNode::Member(member) => format!(
            "{}.{}",
            render_ranked_subject(syntax_trees, member.receiver),
            member.member.as_str()
        ),
        // A subtraction is not an admissible ranking subject under decision
        // 23, but retaining its exact normalized spelling lets the checked
        // stage issue the directed tuple-view migration diagnostic without
        // consulting the legacy expression-span projection.
        syntax::expression::ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                syntax::expression::BinaryOperator::Subtract
            ) =>
        {
            format!(
                "{} - {}",
                render_ranked_subject(syntax_trees, binary.left),
                render_ranked_subject(syntax_trees, binary.right)
            )
        }
        _ => "value".to_string(),
    }
}

/// The witness's explicit view: the authored `-> View` verbatim (canonical
/// builtins get their FIXED ids; a user-declared measure carries `NULL`
/// until TPR3 assigns normalized measure identity), or the canonical
/// default the short form elaborates to.
fn elaborate_ranking_view(
    lowerer: &Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    states: HandleSpan<Handle<State>>,
    subjects: &[syntax::expression::ExpressionHandle],
) -> (omega_core::semantics::RankingViewId, String) {
    use omega_core::semantics::RankingViewId;

    let order = syntax_trees
        .items
        .identifier_path_members(machine.decrease_order);
    if !order.is_empty() {
        let path = order
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let id = RankingViewId::canonical(&path).unwrap_or(RankingViewId::NULL);
        return (id, path);
    }

    match subjects {
        // The only builtin two-subject view, and therefore the two-subject
        // short-form default (the checker's exact rule).
        [_, _] => canonical_view(RankingViewId::NAT_BOUNDED_DISTANCE),
        [single] => elaborate_single_subject_default(lowerer, syntax_trees, states, *single),
        _ => (RankingViewId::NULL, String::new()),
    }
}

fn canonical_view(
    id: omega_core::semantics::RankingViewId,
) -> (omega_core::semantics::RankingViewId, String) {
    (id, id.canonical_path().unwrap_or_default().to_string())
}

/// The single-subject short form's canonical default, mirroring the
/// checker's `decreasing_value_kind` EXACTLY: a `.len` member is nat-like;
/// a plain name ranking a root-state parameter elaborates from the
/// parameter's DECLARED type with constraint shells stripped (ranges and
/// arithmetic domains never change WHAT descends) -- unsigned/bounded
/// scalars descend through the naturals, slices by their length. Anything
/// else stays PENDING (empty path): the checker diagnoses ambiguity as
/// today, and TPR3 folds elaboration into the migrated checker.
fn elaborate_single_subject_default(
    lowerer: &Lowerer,
    syntax_trees: &SyntaxTrees,
    states: HandleSpan<Handle<State>>,
    subject: syntax::expression::ExpressionHandle,
) -> (omega_core::semantics::RankingViewId, String) {
    use omega_core::semantics::RankingViewId;
    use omega_symbol_resolved_trees::types::TypeReference;

    if let syntax::expression::ExpressionNode::Member(member) =
        syntax_trees.expressions.expression(subject)
        && member.member.as_str() == "len"
    {
        return canonical_view(RankingViewId::NAT_DESCENDING);
    }

    let syntax::expression::ExpressionNode::Name(path) =
        syntax_trees.expressions.expression(subject)
    else {
        return (RankingViewId::NULL, String::new());
    };
    let trees = &lowerer.symbol_resolved_trees;
    let pending = (RankingViewId::NULL, String::new());
    let Some(subject_name) = syntax_trees
        .expressions
        .identifier_path_members(*path)
        .last()
    else {
        return pending;
    };
    let Some(root_state) = trees.machine_state_handles(states).first() else {
        return pending;
    };
    let root_state = trees.machine_state(*root_state);
    let Some(parameter) = trees
        .state_parameters(root_state.parameters)
        .iter()
        .find(|parameter| !parameter.is_self && parameter.name.as_str() == subject_name.as_str())
    else {
        return pending;
    };

    let mut base = &parameter.type_reference;
    loop {
        match base {
            TypeReference::Constrained(constrained) => {
                base = trees.child_type_reference(constrained.base_type);
            }
            TypeReference::Reference(reference) => {
                base = trees.child_type_reference(reference.referee);
            }
            _ => break,
        }
    }
    match base {
        TypeReference::Slice(_) | TypeReference::FixedArray(_) => {
            canonical_view(RankingViewId::SLICE_LENGTH)
        }
        TypeReference::Named { name, .. }
            if matches!(name.as_str(), "u8" | "u16" | "u32" | "u64" | "nat") =>
        {
            canonical_view(RankingViewId::NAT_DESCENDING)
        }
        _ => pending,
    }
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
            lowerer,
            syntax_trees,
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
                    via: clause
                        .via
                        .as_ref()
                        .map(|binding| binding.normalized_rendering()),
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

    // GUARDED-ARM DEEP FIX (task #45): append the continuation states the
    // value-call rewrite synthesized while lowering this machine's arms --
    // they join the state list BEFORE symbol assignment, so they mint
    // symbols exactly like authored states.
    let synthesized = std::mem::take(&mut lowerer.pending_synthesized_states);
    for arm in synthesized {
        let state = crate::state::build_synthesized_arm_state(lowerer, arm);
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
