use crate::data::lower_type_parameters;
use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::state::{
    lower_service_reach_names, lower_signature_contracts, lower_signature_invokes, lower_state_node,
};
use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::machine::{
    GenericConformanceBound, Machine, MachineStorage, TraitConformance,
};
use psi_symbol_resolved_trees::state::State;
use psi_symbols::SymbolHandle;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<(), Diagnostic> {
    lowerer.current_machine_is_boundary = machine.boundary;
    lowerer.current_machine_root_index = Some(lowerer.symbol_resolved_trees.machines.len());
    lowerer.current_machine_name = Some(machine.name.as_str().to_owned());
    lowerer.current_evidence_term_names = syntax_trees
        .items
        .capability_contracts(machine.contracts)
        .iter()
        .filter_map(|contract| contract.binding.as_ref())
        .map(|binding| binding.as_str().to_owned())
        .collect();
    let states = lower_machine_states(lowerer, syntax_trees, machine.states)?;
    lowerer.current_machine_is_boundary = false;
    lowerer.current_machine_root_index = None;
    lowerer.current_machine_name = None;
    lowerer.current_state_name = None;
    lowerer.current_evidence_term_names.clear();
    let type_parameters = lower_type_parameters(lowerer, syntax_trees, machine.type_parameters)?;
    let satisfies = lower_machine_trait_conformances(lowerer, syntax_trees, machine.satisfies)?;
    let conformance_bounds =
        lower_generic_conformance_bounds(lowerer, syntax_trees, &machine.conformance_bounds)?;
    let ranking_subjects =
        lower_ranking_expressions(lowerer, syntax_trees, machine.ranking_subjects)?;
    let ranking_view = lower_machine_ranking_view(lowerer, syntax_trees, machine.ranking_view);
    // TPR3: argumented-view arguments lower exactly like the subjects.
    let ranking_view_arguments =
        lower_ranking_expressions(lowerer, syntax_trees, machine.ranking_view_arguments)?;
    // TPR3 slice 3: the `in <range>` rank constraint lowers as an ordinary
    // expression; the CHECKER verifies it structurally (v1: floor 0 +
    // ceiling equal to the argumented view's own bound) and fails
    // compilation otherwise -- consumed, never silently dropped.
    let ranking_range = if machine.ranking_range.is_valid() {
        lower_expression_into_table(lowerer, syntax_trees, machine.ranking_range)?
    } else {
        psi_symbol_resolved_trees::expression::ExpressionHandle::invalid()
    };
    let service_reaches = lower_service_reach_names(syntax_trees, machine.service_reaches);
    let invokes = lower_signature_invokes(lowerer, syntax_trees, machine.invokes);
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
        let via_binding = syntax_trees
            .items
            .satisfies_clauses(machine.satisfies)
            .iter()
            .find_map(|clause| clause.via.as_ref())
            .map(external_binding_identity);
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
                        psi_syntax_trees::item::CapabilityContractKind::Ensures
                    ) && !contract.facts.is_empty()
                });
            if authors_fact {
                psi_language_semantics::MachineSupplyMode::Accepted
            } else {
                psi_language_semantics::MachineSupplyMode::Boundary
            }
        } else if let (true, Some(identity)) = (machine.bodyless, via_binding) {
            let mechanism = identity.mechanism();
            psi_language_semantics::MachineSupplyMode::ExternalRealization {
                binding: lowerer
                    .symbol_resolved_trees
                    .external_bindings
                    .intern(identity),
                mechanism,
            }
        } else if machine.boundary {
            psi_language_semantics::MachineSupplyMode::Boundary
        } else {
            psi_language_semantics::MachineSupplyMode::CheckedBody
        }
    };
    lowerer
        .pending_machine_service_reaches
        .push(service_reaches);
    lowerer.symbol_resolved_trees.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        name: machine_name,
        attached_data,
        attached_data_symbol: SymbolHandle::invalid(),
        is_public: machine.is_public,
        supply_mode,
        body_is_present: !machine.bodyless,
        // TPR2: the termination plan's ONE population site (see
        // build_termination_plan below).
        termination_plan,
        service_reach_row: psi_language_semantics::ServiceReachRowId::NULL,
        service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
        storage: MachineStorage {
            lifetime_parameters: machine
                .lifetime_parameters
                .iter()
                .map(crate::name::lower_name)
                .collect(),
            type_parameters,
            owned_data: HandleSpan::empty(),
            satisfies,
            conformance_bounds,
            ranking_subjects,
            ranking_view,
            ranking_view_arguments,
            ranking_range,
            invokes,
            suspends: machine.suspends,
            blocks: machine.blocks,
            contracts,
            states,
        },
    });
    Ok(())
}

pub(crate) fn lower_generic_conformance_bounds(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    bounds: &[syntax::item::GenericConformanceBound],
) -> Result<Vec<GenericConformanceBound>, Diagnostic> {
    bounds
        .iter()
        .map(|bound| {
            Ok(GenericConformanceBound {
                binder: bound.binder.as_ref().map(|_| SymbolHandle::invalid()),
                binder_name: bound.binder.as_ref().map(crate::name::lower_name),
                subject: SymbolHandle::invalid(),
                subject_name: crate::name::lower_name(&bound.subject),
                carrier: SymbolHandle::invalid(),
                carrier_name: crate::name::lower_name(&bound.carrier),
                arguments: crate::type_reference::lower_child_type_references(
                    lowerer,
                    syntax_trees,
                    bound.arguments,
                )?,
                conformance: None,
                conformance_name: bound.conformance.as_ref().map(crate::name::lower_name),
            })
        })
        .collect()
}

/// TPR2 (decision 23): populate the normalized `MachineTerminationPlan` --
/// the plan's ONE population site; every later stage copies, never
/// re-derives.
///
/// - Bare `terminates;` authors the PUBLIC termination guarantee
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
) -> psi_language_semantics::MachineTerminationPlan {
    use psi_language_semantics::{
        MachineTerminationPlan, RankingWitness, TerminationGuarantee, TerminationInterface,
    };

    let interface = if machine.terminates_guarantee {
        TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new(),
        })
    } else if machine.boundary || machine.bodyless {
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee)
    } else {
        TerminationInterface::InternalDerived
    };

    let subjects = syntax_trees
        .expressions
        .expression_handles(machine.ranking_subjects);
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
                .expression_handles(machine.ranking_view_arguments)
                .iter()
                .map(|argument| render_ranked_subject(syntax_trees, *argument))
                .collect(),
            // TPR3 slice 3: the authored rank-range fact, rendered
            // source-like; the checker verifies it structurally and fails
            // compilation otherwise.
            rank_range: machine.ranking_range.is_valid().then(|| {
                let syntax::expression::ExpressionNode::Range(range) =
                    syntax_trees.expressions.expression(machine.ranking_range)
                else {
                    return psi_language_semantics::RankRange::default();
                };
                psi_language_semantics::RankRange {
                    floor: render_ranked_subject(syntax_trees, range.start),
                    ceiling: render_ranked_subject(syntax_trees, range.end),
                    ceiling_inclusive: range.end_inclusive,
                }
            }),
        }
    });

    MachineTerminationPlan {
        interface,
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
) -> (psi_language_semantics::RankingViewId, String) {
    use psi_language_semantics::RankingViewId;

    let order = syntax_trees
        .items
        .identifier_path_members(machine.ranking_view);
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
    id: psi_language_semantics::RankingViewId,
) -> (psi_language_semantics::RankingViewId, String) {
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
) -> (psi_language_semantics::RankingViewId, String) {
    use psi_language_semantics::RankingViewId;
    use psi_symbol_resolved_trees::types::TypeReference;

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

fn lower_machine_ranking_view(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    order: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<psi_symbol_resolved_trees::name::DiagnosticName> {
    let mut lowered = HandleSpan::empty();

    for member in syntax_trees.items.identifier_path_members(order) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .ranking_views
            .append_to_span(&mut lowered, crate::name::lower_name(member));
    }

    lowered
}

fn lower_ranking_expressions(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    source_expressions: HandleSpan<syntax::expression::ExpressionHandle>,
) -> Result<HandleSpan<psi_symbol_resolved_trees::expression::ExpressionHandle>, Diagnostic> {
    let mut lowered = Vec::new();

    for expression in syntax_trees
        .expressions
        .expression_handles(source_expressions)
    {
        let expression = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
        lowered.push(expression);
    }

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .insert_expression_handles(lowered))
}

fn lower_machine_trait_conformances(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    satisfies: HandleSpan<syntax::item::SatisfiesClause>,
) -> Result<HandleSpan<TraitConformance>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for clause in syntax_trees.items.satisfies_clauses(satisfies) {
        let arguments = crate::type_reference::lower_child_type_references(
            lowerer,
            syntax_trees,
            clause.arguments,
        )?;
        let external_binding = clause.via.as_ref().map(|binding| {
            lowerer
                .symbol_resolved_trees
                .external_bindings
                .intern(external_binding_identity(binding))
        });
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
                    arguments,
                    requirement: clause.requirement.as_ref().map(crate::name::lower_name),
                    alias: clause.alias.as_ref().map(crate::name::lower_name),
                    external_binding,
                },
            );
    }

    Ok(span)
}

fn external_binding_identity(
    binding: &syntax::item::ExternalBinding,
) -> psi_language_semantics::ExternalBindingIdentity {
    use psi_language_semantics::ExternalBindingIdentity;

    match binding {
        syntax::item::ExternalBinding::Syscall { number } => {
            ExternalBindingIdentity::Syscall { number: *number }
        }
        syntax::item::ExternalBinding::DllImport { module, symbol } => {
            ExternalBindingIdentity::Import {
                library: module.clone(),
                symbol: symbol.clone(),
            }
        }
        syntax::item::ExternalBinding::CompilerIntrinsic => {
            ExternalBindingIdentity::CompilerIntrinsic
        }
        syntax::item::ExternalBinding::VtableSlot { index } => {
            ExternalBindingIdentity::VtableSlot { index: *index }
        }
        syntax::item::ExternalBinding::VtableField { field } => {
            ExternalBindingIdentity::VtableField {
                field: field.as_str().to_owned(),
            }
        }
        syntax::item::ExternalBinding::TableFunction { field } => {
            ExternalBindingIdentity::TableFunction {
                field: field.as_str().to_owned(),
            }
        }
    }
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

    let synthesized = std::mem::take(&mut lowerer.pending_synthesized_transition_argument_states);
    for arm in synthesized {
        let state = crate::state::build_synthesized_transition_argument_state(lowerer, arm);
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
