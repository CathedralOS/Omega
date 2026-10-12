//! Compile-time specialization of generic machines.
//!
//! Type parameters, canonical const parameters, and static machine parameters
//! are one specialization tuple. Every concrete tuple receives a private
//! deep-copied body with fresh lexical symbols. Calls are rewritten to their
//! selected concrete state,
//! calls through `F(...)` become direct calls to the selected entry, and each
//! tuple records a deterministic cache identity. The authored declaration stays
//! generic: its exported contract must not depend on the current callers. This
//! also keeps symbolic calls in generic wrappers available for later selection,
//! without a second template snapshot or a separate row-inference authority.
//! The retained body is checked against its declared requirements as well as
//! checking each selected instance. Checking only observed tuples could hide
//! an undeclared range or conformance requirement in the generic interface.
//! Incomplete tuples remain generic and are fenced by validation; no runtime
//! const/callable value or dictionary is introduced.
//!
//! Executable rewrites retain the original binder separately: structural reach
//! and both kinds' operational envelopes remain requirement-owned, while nominal
//! reach uses the selected public contract. The parameter arena survives closing
//! the live generic span. Template commitments protect its stable contract axes;
//! application commitments also protect each exact retained-call binding.

use crate::monomorphization::body_cloning::clone_specialized_machine;
use crate::monomorphization::body_rewriting::rewrite_selected_call;
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::signature::StateSignature;
use typed_trees::statement::{StatementHandle, StatementNode};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

pub(crate) mod application_arguments;
mod application_calls;
mod application_evidence;
pub(crate) mod applications;
mod candidate;
mod contextual_calls;
mod selection;
pub(crate) use selection::infer_static_bindings;

mod body_rewriting;
pub(crate) use application_evidence::collect_evidence_requirement_closure;
pub(crate) use body_rewriting::insert_subject_name;
mod body_cloning;
pub(crate) use body_cloning::copy_type_reference;
pub(crate) mod captured_lexical_scopes;
mod const_arguments;
pub(crate) mod demanded_applications;
mod dynamic_families;
pub(crate) mod requirement_applications;
mod requirement_calls;
mod requirement_scopes;
mod requirement_signatures;
pub(crate) use requirement_signatures::retain_requirement_operand_application;
mod signature_cloning;
mod trait_provider_applications;
pub(crate) use dynamic_families::generate_dynamic_family_specializations;
mod dynamic_slots;
pub(crate) use dynamic_slots::instantiate_dynamic_slots;
pub(crate) use requirement_applications::instantiate_boundary_requirement_families;
pub(crate) use type_reference_walk::{for_each_type_reference, type_reference_endpoints};
pub(crate) mod call_results;
mod const_values;
mod range_arguments;
mod saved_calls;
mod target_families;
mod type_reference_walk;

/// Authored metadata shared by all candidate applications in a round.
#[derive(Clone)]
struct CandidateTemplate {
    machine_index: usize,
    template_symbol: SymbolHandle,
    template_name: String,
    state_symbols: Vec<SymbolHandle>,
    type_parameters: Vec<(SymbolHandle, String)>,
    const_parameters: Vec<(SymbolHandle, String, TypeReferenceHandle)>,
    /// Const-slot ordinals declared as runtime-capable Value binders.
    value_const_parameters: Vec<usize>,
    machine_parameters: Vec<(SymbolHandle, String, StateSignature)>,
    evidence_parameters: Vec<typed_trees::machine::GenericConformanceBound>,
}

/// Discovery owns a template; each selected application borrows that immutable
/// metadata and owns only its argument bindings and closed conformance results.
#[derive(Clone)]
struct Candidate<'template> {
    template: std::borrow::Cow<'template, CandidateTemplate>,
    binding_application: typed_trees::static_applications::StaticMachineApplicationHandle,
    type_bindings: Vec<Option<TypeReferenceHandle>>,
    const_bindings: Vec<Option<TypeReferenceHandle>>,
    /// A runtime Value slot binds its carrier here, while the exact call-site
    /// subject is retained separately and appended as an ordinary argument.
    runtime_value_bindings: Vec<Option<StaticMachineArgument>>,
    machine_bindings: Vec<Option<StaticMachineArgument>>,
    evidence_bindings: Vec<Option<StaticMachineArgument>>,
    conflicted: bool,
}

impl Candidate<'_> {
    fn borrowed(&self) -> Candidate<'_> {
        Candidate {
            template: std::borrow::Cow::Borrowed(&self.template),
            binding_application: self.binding_application,
            type_bindings: self.type_bindings.clone(),
            const_bindings: self.const_bindings.clone(),
            runtime_value_bindings: self.runtime_value_bindings.clone(),
            machine_bindings: self.machine_bindings.clone(),
            evidence_bindings: self.evidence_bindings.clone(),
            conflicted: self.conflicted,
        }
    }
}

struct CalleeState {
    symbol: SymbolHandle,
    candidate_index: usize,
    return_type: TypeReferenceHandle,
    parameter_types: Vec<TypeReferenceHandle>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallSite {
    Statement(StatementHandle),
    Expression(ExpressionHandle),
}

#[derive(Clone)]
struct CallSelection {
    site: CallSite,
    lexical_owner: SymbolHandle,
    enclosing_data_applications: Vec<TypeReferenceHandle>,
    receiving_data_parameter: SymbolHandle,
    callee_symbol: SymbolHandle,
    candidate_index: usize,
    caller_is_generic: bool,
    unresolved_type_parameters: bool,
    unresolved_machine_parameters: bool,
    unresolved_evidence_parameters: bool,
    unresolved_const_parameters: bool,
    type_bindings: Vec<Option<TypeReferenceHandle>>,
    const_bindings: Vec<Option<TypeReferenceHandle>>,
    /// See `Candidate::runtime_value_bindings`.
    runtime_value_bindings: Vec<Option<StaticMachineArgument>>,
    machine_bindings: Vec<Option<StaticMachineArgument>>,
    evidence_bindings: Vec<Option<StaticMachineArgument>>,
    conflicted: bool,
    /// An authored argument exceeded its binder-kind capacity before inference.
    explicit_argument_overflow: bool,
}

impl CallSelection {
    fn is_complete(&self) -> bool {
        self.has_bound_arguments()
            && !self.unresolved_type_parameters
            && !self.unresolved_machine_parameters
            && !self.unresolved_evidence_parameters
            && !self.unresolved_const_parameters
    }

    /// Known operands can still belong to an open caller's telescope. They
    /// instantiate its result type but must not create a closed executable
    /// specialization. Preserve that distinction in the original proposal,
    /// rather than reinterpreting the call in a result-only inference pass.
    fn has_bound_arguments(&self) -> bool {
        !self.conflicted
            && !self.explicit_argument_overflow
            && self.type_bindings.iter().all(Option::is_some)
            && self.const_bindings.iter().all(Option::is_some)
            && self.machine_bindings.iter().all(Option::is_some)
            && self.evidence_bindings.iter().all(Option::is_some)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct SpecializationKey {
    type_arguments: Vec<String>,
    const_arguments: Vec<String>,
    machine_arguments: Vec<SymbolHandle>,
    evidence_arguments: Vec<u64>,
}

/// Return the lexical roots whose calls or inferred results changed. The
/// enclosing elaboration round binds those outputs only after this fixed point
/// settles, alongside every newly cloned body; unchanged callers stay untouched.
///
/// `enforce_complete_concrete_selections` distinguishes the authoritative
/// checking pass from speculative specialization runs. Build-program
/// preparation and `build.omg` interpretation specialize a private typed copy
/// while const endpoint folds are still outstanding; an underivable tuple
/// there is interim evidence, not a rejected program. Only the checking pass
/// that emits the program may reject an incomplete concrete selection, and
/// only once no pending endpoint fold can still supply the missing binding.
pub(crate) fn monomorphize_generic_machine_value_calls(
    program: &mut TypedTrees,
    enforce_complete_concrete_selections: bool,
) -> Result<typed_trees::references::DeclarationSelection, Vec<Diagnostic>> {
    let mut changed_owners = Vec::new();
    let mut result_derivations = call_results::ResultDerivations::default();
    loop {
        materialize_static_argument_types(program);
        // One postorder publishes calls, borrows and constructions before
        // original lifetime inference and inferred-local consumers.
        let publication =
            crate::preparation::retain_value_application_results(program, &mut result_derivations)?;
        let mut result_progress = publication.changed;
        changed_owners.extend(publication.changed_owners);
        let local_owners =
            call_results::refresh_inferred_local_results(program, &publication.unavailable)?;
        result_progress |= !local_owners.is_empty();
        changed_owners.extend(local_owners);
        let observations = call_results::materialize_reference_observations(program)?;
        if !observations.is_empty() {
            changed_owners.extend(observations);
            continue;
        }
        crate::conformance::conformance_application_lifetimes::resolve_elided_conformance_lifetimes(program)?;
        crate::conformance::explicit_requirement_calls::route_explicit_requirement_calls(program)?;
        changed_owners.extend(contextual_calls::close(program)?);
        let requirement_owners = requirement_calls::bind(program)?;
        let requirement_progress = !requirement_owners.is_empty();
        changed_owners.extend(requirement_owners);
        // Requirement headers can complete another child. Reuse the same
        // producer derivations and traversal, rather than a second copier.
        let publication =
            crate::preparation::retain_value_application_results(program, &mut result_derivations)?;
        result_progress |= publication.changed;
        changed_owners.extend(publication.changed_owners);
        let local_owners =
            call_results::refresh_inferred_local_results(program, &publication.unavailable)?;
        result_progress |= !local_owners.is_empty();
        changed_owners.extend(local_owners);
        crate::conformance::conformance_application_lifetimes::resolve_elided_conformance_lifetimes(program)?;
        // Reference results are published before their scalar observations.
        // Insertion replaces a statement span, so recollect all call selections
        // next round. This point also serves programs with no generic candidates.
        let observations = call_results::materialize_reference_observations(program)?;
        if !observations.is_empty() {
            changed_owners.extend(observations);
            continue;
        }
        crate::preparation::selections::retain_receiver_contexts(program);
        let candidates = candidate::collect(program);
        let callee_states = candidate::callees(program, &candidates);
        if candidates.is_empty() {
            // A public result can supply another public call's operand even
            // when there is no ordinary executable specialization template.
            if requirement_progress || result_progress {
                continue;
            }
            return Ok(typed_trees::references::DeclarationSelection::from_symbols(
                changed_owners,
            ));
        }
        let contract_expressions = contract_expression_handles(program);
        let mut selections = selection::collect_call_selections(
            program,
            &candidates,
            &callee_states,
            &contract_expressions,
        );
        // Public result/header copying can introduce const graphs after the
        // round-entry type scan. Complete each exact selected const operand
        // before its application, result or body consumes the proposal. The
        // same normalizer preserves landed carriers and leaves unknown/open
        // graphs untouched; immutable checking still admits every bound.
        for selection in &selections {
            for (ordinal, binding) in selection.const_bindings.iter().enumerate() {
                if selection.runtime_value_bindings[ordinal].is_none()
                    && let Some(binding) = binding
                {
                    application_arguments::materialize_const_type_argument(program, *binding);
                }
            }
        }
        // An overflowing proposal cannot form an executable tuple. Preserve
        // its authored call for source admission instead of masking its binder
        // kind with a derivative grouping error.
        selections.retain(|selection| !selection.explicit_argument_overflow);
        // A marked bound is an obligation, not a closed type argument. In
        // particular an unused type binder must not disappear into an instance
        // before the bound is evaluated and validated. Keep the original call
        // until the owning const-evaluation continuation closes the tuple.
        selections.retain(|selection| {
            !const_arguments::has_pending_type_arguments(program, &selection.type_bindings)
        });
        for selection in &selections {
            let candidate =
                candidate_for_selection(program, &candidates[selection.candidate_index], selection);
            applications::retain_call(program, &candidate, selection);
        }
        let result_owners = call_results::refresh_generic_call_results(
            program,
            &candidates,
            &selections,
            &mut result_derivations,
        )?;
        result_progress |= !result_owners.is_empty();
        changed_owners.extend(result_owners);
        let mut diagnostics = Vec::new();
        let mut applied_any = requirement_progress || result_progress;
        for (candidate_index, candidate) in candidates.iter().enumerate() {
            // Open callers retain their symbolic tuples. Ordinary and public
            // result progress may complete another call later in this same
            // fixed point; incomplete selections are judged after it settles.
            match apply_call_specializations(
                program,
                candidate,
                &selections,
                candidate_index,
                &mut changed_owners,
            ) {
                Ok(changed) => applied_any |= changed,
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        if !applied_any {
            // Retain the original partial-static refusal, but only after both
            // public headers and ordinary specializations stop making progress.
            for (candidate_index, candidate) in candidates.iter().enumerate() {
                if selections.iter().any(|selection| {
                    selection.candidate_index == candidate_index
                        && !selection.caller_is_generic
                        && !selection.is_complete()
                        && (selection.const_bindings.iter().any(Option::is_some)
                            || selection.machine_bindings.iter().any(Option::is_some)
                            || selection.evidence_bindings.iter().any(Option::is_some))
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                        "generic machine `{}` has a static selection, but its complete type/const/machine/conformance specialization tuple cannot be derived",
                        candidate.template.template_name
                    )));
                }
            }
            if !diagnostics.is_empty() {
                return Err(diagnostics);
            }

            // Fixed point: no later specialization round can still land a
            // receiver, provider, or endpoint binding, so a concrete
            // selection still incomplete here is honestly underivable. The
            // authoritative checking pass then rejects every incomplete
            // statement-position selection: a statement call has no result
            // slot for the value-position validation fence to inspect, so a
            // fully underivable or type-only-partial tuple would otherwise
            // emit unspecialized. This is the last honest stop for
            // `pick(7);` and `pair<i32>(v);` alike.
            //
            // Speculative passes (build-program preparation, `build.omg`
            // interpretation) never enforce this gate, and neither does a
            // settled pass that still carries pending endpoint folds: both
            // may see a tuple complete only once deferred const evaluation
            // lands. Incomplete expression calls keep their value-position
            // fence diagnostic.
            if enforce_complete_concrete_selections
                && program.pending_const_range_endpoints.is_empty()
            {
                for (candidate_index, candidate) in candidates.iter().enumerate() {
                    if selections.iter().any(|selection| {
                        selection.candidate_index == candidate_index
                            && !selection.caller_is_generic
                            && !selection.is_complete()
                            && matches!(selection.site, CallSite::Statement(_))
                    }) {
                        diagnostics.push(Diagnostic::error(format!(
                            "generic machine `{}` has a static selection, but its complete type/const/machine/conformance specialization tuple cannot be derived",
                            candidate.template.template_name
                        )));
                    }
                }
                if !diagnostics.is_empty() {
                    return Err(diagnostics);
                }
            }
            return Ok(typed_trees::references::DeclarationSelection::from_symbols(
                changed_owners,
            ));
        }
    }
}

fn materialize_static_argument_types(program: &mut TypedTrees) {
    application_arguments::materialize_const_type_arguments(program);
    fn collect(
        program: &dyn ::validation::CheckingInput,
        arguments: &[StaticMachineArgument],
        types: &mut Vec<(SymbolHandle, typed_trees::name::Identifier)>,
        meanings: &mut Vec<(String, typed_trees::types::ConstIntegerMeaning)>,
    ) {
        for argument in arguments {
            if let Some(literal) = const_arguments::spelling(program, argument) {
                let meaning = program
                    .typed_program()
                    .static_const_integer_meaning(argument);
                if !meanings.contains(&(literal.clone(), meaning)) {
                    meanings.push((literal, meaning));
                }
            }
            // Builtin-type arguments intern the same named reference an
            // authored `-> u64` would: an explicit `ident<u64>` is complete
            // static evidence even when no other mention of the carrier
            // exists in the program.
            if argument.application.is_none()
                && argument.symbol.is_valid()
                && (matches!(
                    program.symbols.get(argument.symbol).kind,
                    SymbolKind::Data | SymbolKind::BuiltinType
                ) || const_arguments::forwarded_type(program, argument).is_valid())
                && let Some(name) = argument.path.last()
                && !types.iter().any(|(symbol, _)| *symbol == argument.symbol)
            {
                types.push((argument.symbol, name.clone()));
            }
            if let Some(application) = &argument.application {
                collect(program, &application.arguments, types, meanings);
            }
        }
    }

    let mut literals = Vec::new();
    let mut types = Vec::new();
    let mut meanings = Vec::new();
    for (_, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            collect(program, &call.machine_arguments, &mut types, &mut meanings);
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(call) = statement.as_call() {
                    collect(program, &call.machine_arguments, &mut types, &mut meanings);
                }
            }
        }
    }
    for (literal, meaning) in meanings {
        application_arguments::intern_closed_const_with_meaning(program, &literal, meaning);
    }
    literals.extend(
        program
            .type_reference_table
            .fixed_array_lengths()
            .filter_map(|(_, length)| match length {
                typed_trees::types::FixedArrayLength::Literal(value) => Some(value.to_string()),
                typed_trees::types::FixedArrayLength::ConstParameter { .. }
                | typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
            }),
    );
    // Open array extents also participate in inference. Retain their binder
    // identity so another occurrence cannot prematurely close the same slot.
    for (_, length) in program.type_reference_table.fixed_array_lengths() {
        if let typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } = length {
            types.push((*symbol, name.clone()));
        }
    }
    range_arguments::collect_literals(program, &mut literals);
    // Open declared-range endpoints keep their exact binder identity the same
    // way open array extents do: a forwarded const binder occurring as an
    // endpoint gets a named reference so range inference can propose the
    // structural equation rather than a value leaf.
    range_arguments::collect_binders(program, &mut types);
    literals.sort();
    literals.dedup();
    for literal in literals {
        application_arguments::intern_closed_const_with_meaning(
            program,
            &literal,
            typed_trees::types::ConstIntegerMeaning::Anonymous,
        );
    }
    for (symbol, name) in types {
        if program
            .type_reference_table
            .find_named_type_reference(symbol)
            .is_none()
        {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named { symbol, name });
        }
    }
    // A literal carrying a source-owned landing is static inference evidence
    // even when no place declaration or authored spelling names its carrier.
    // Typed lowering retains integer landings; intern the remaining literal
    // carriers here so every literal family is proposable on every route.
    let mut atoms = Vec::new();
    for (_, expression) in program.expression_table.iter_expressions() {
        let atom = match expression {
            ExpressionNode::Integer(literal) => literal
                .landing()
                .map(|landing| typed_trees::types::builtin_type_atom(landing.landed_type)),
            ExpressionNode::Float(literal) => literal.landing().map(|format| match format {
                numerics::literals::FloatFormat::F32 => symbols::BuiltinTypeAtom::F32,
                numerics::literals::FloatFormat::F64 => symbols::BuiltinTypeAtom::F64,
            }),
            ExpressionNode::Boolean(_) => Some(symbols::BuiltinTypeAtom::Bool),
            _ => None,
        };
        if let Some(atom) = atom
            && !atoms.contains(&atom)
        {
            atoms.push(atom);
        }
    }
    for atom in atoms {
        let Some(symbol) = program
            .symbols
            .child_handles(program.symbols.root())
            .and_then(|mut children| {
                children.find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))
            })
        else {
            continue;
        };
        if program
            .type_reference_table
            .find_named_type_reference(symbol)
            .is_none()
        {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: typed_trees::name::Identifier::generated_static(atom.symbol_name()),
                });
        }
    }
}

/// Materialize exact private specializations for generic checked bodies chosen
/// by Omega's boundary-operator ProviderPlans. The authored generic machine is
/// never rewritten: it remains the package API and every closed application
/// is cloned from that stable template.
mod operator_candidates;
use crate::monomorphization::selection::{contract_expression_handles, unique_complete_selections};
pub(crate) use operator_candidates::{OperatorCandidateTemplates, specialize_operator_candidates};

fn apply_call_specializations(
    program: &mut TypedTrees,
    template: &Candidate,
    selections: &[CallSelection],
    candidate_index: usize,
    changed_owners: &mut Vec<SymbolHandle>,
) -> Result<bool, Vec<Diagnostic>> {
    // A complete telescope is not yet an admitted application. Malformed
    // source arguments must retain their ordinary diagnostic during grouping.
    let groups =
        unique_complete_selections(program, selections, candidate_index, &template.template)
            .map_err(|diagnostic| vec![diagnostic])?;
    if groups.is_empty() {
        return Ok(false);
    }
    let concrete_candidates: Vec<Candidate> = groups
        .iter()
        .map(|(_, members)| candidate_for_selection(program, template, &selections[members[0]]))
        .collect();

    let normalized_template_identity = normalized_machine_identity(
        program,
        &program.machines()[template.template.machine_index],
    )
    .expect("generic template must retain a normalized callable identity");
    let accepted_template_commitment =
        accepted_template_commitment(program, template.template.machine_index);

    let mut selected_call_rewrites = Vec::new();
    for ((_, members), candidate) in groups.iter().zip(&concrete_candidates) {
        let selection = &selections[members[0]];
        // A later closed caller can select a tuple already materialized by an
        // earlier fixed-point round. Rejoin its complete retained arguments,
        // including conformance records, rather than allocating another body.
        let existing = saved_calls::selected_instance(program, program, template, selection)
            .map(|instance| instance.instance);
        let state_symbols = if let Some(existing) = existing {
            let Some(machine) = program.machine_by_symbol(existing) else {
                return Err(vec![Diagnostic::error(
                    "specialization receipt has no live instance",
                )]);
            };
            let states = program.machine_states(machine);
            if states.len() != template.template.state_symbols.len() {
                return Err(vec![Diagnostic::error(
                    "specialization instance lost its template state correspondence",
                )]);
            }
            template
                .template
                .state_symbols
                .iter()
                .copied()
                .zip(states.iter().map(|state| state.symbol))
                .collect()
        } else {
            clone_specialized_machine(
                None,
                program,
                candidate,
                normalized_template_identity.clone(),
                accepted_template_commitment.clone(),
            )?
            .state_symbols
        };
        for selection_index in members {
            let selection = &selections[*selection_index];
            let Some((_, concrete_state)) = state_symbols
                .iter()
                .find(|(template_state, _)| *template_state == selection.callee_symbol)
            else {
                return Err(vec![Diagnostic::error(
                    "selected call has no exact specialization state",
                )]);
            };
            let original =
                candidate_for_selection(program, template, selection).binding_application;
            let instance = program
                .machine_holding_state(*concrete_state)
                .ok_or_else(|| vec![Diagnostic::error("selected state lost its instance owner")])?
                .symbol;
            program.retain_machine_conformance_application(instance, original);
            // Each site keeps its own exact runtime subjects: the shared
            // carrier-keyed instance receives them as appended arguments.
            let subjects = selection
                .runtime_value_bindings
                .iter()
                .flatten()
                .filter_map(|argument| Some((argument.path.first()?.clone(), argument.symbol)))
                .collect::<Vec<_>>();
            selected_call_rewrites.push((
                selection.site,
                selection.lexical_owner,
                *concrete_state,
                subjects,
            ));
        }
    }
    // Delay call rewrites until every tuple has copied its authored body.
    for (site, owner, concrete_state, subjects) in selected_call_rewrites {
        rewrite_selected_call(program, site, concrete_state, &subjects);
        // A call can be an operator operand without an inferred local between
        // them. Its new result type invalidates this lexical owner's selection
        // inputs even when result-local elaboration reports no change.
        changed_owners.push(owner);
    }
    Ok(true)
}

fn candidate_for_selection<'template>(
    program: &dyn validation::CheckingInput,
    template: &'template Candidate<'_>,
    selection: &CallSelection,
) -> Candidate<'template> {
    Candidate {
        template: std::borrow::Cow::Borrowed(&template.template),
        binding_application: match selection.site {
            CallSite::Expression(handle) => program.expression_table.call_node(handle).map_or(
                typed_trees::static_applications::StaticMachineApplicationHandle::invalid(),
                |call| call.authored_machine_application,
            ),
            CallSite::Statement(handle) => {
                program.statement_table.statement(handle).as_call().map_or(
                    typed_trees::static_applications::StaticMachineApplicationHandle::invalid(),
                    |call| call.authored_machine_application,
                )
            }
        },
        type_bindings: selection.type_bindings.clone(),
        const_bindings: selection.const_bindings.clone(),
        runtime_value_bindings: selection.runtime_value_bindings.clone(),
        machine_bindings: selection.machine_bindings.clone(),
        evidence_bindings: selection.evidence_bindings.clone(),
        conflicted: selection.conflicted,
    }
}

fn candidate_conformance_fingerprint_arguments(candidate: &Candidate) -> Vec<String> {
    candidate
        .evidence_bindings
        .iter()
        .map(|binding| {
            binding
                .as_ref()
                .expect("complete specialization")
                .display_name()
        })
        .collect()
}

/// Const substitution changes an indexed-domain instance from binder identity
/// (`Quantity<To>`) to the selected canonical value (`Quantity<METER>`). The
/// semantic ID is derived data. Finalize each producer's owned graph before
/// downstream selection can observe it, including constraints, casts and
/// membership facts. Declaration selection excludes unchanged graphs while
/// retaining shared arenas for semantic lookup; it grants no checking authority.
/// The authored normalization pass alone selects the whole program.
pub(crate) fn finalize_type_domain_instance_identities(
    program: &mut TypedTrees,
    references: impl IntoIterator<Item = TypeReferenceHandle>,
) -> Result<(), Diagnostic> {
    let mut constraint_updates = Vec::new();
    for reference in references {
        let TypeReferenceNode::Constrained { constraints, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            continue;
        };
        let constraints = *constraints;
        for (offset, constraint) in program
            .type_reference_table
            .constraints(constraints)
            .iter()
            .enumerate()
        {
            let TypeConstraintNode::Domain(constraint) = constraint else {
                continue;
            };
            if constraint.arguments.is_empty() || !constraint.symbol.is_valid() {
                continue;
            }
            let Some(domain) = program.domain_definition(constraint.symbol).cloned() else {
                continue;
            };
            let index_parameters = typed_trees::domain::index_parameters(program, &domain);
            let identity = typed_trees::domain::indexed_domain_instance_name(
                program,
                &domain,
                index_parameters,
                &constraint.arguments,
            )?;
            constraint_updates.push((constraints, offset, identity, domain.semantic_roles));
        }
    }

    for (constraints, offset, identity, roles) in constraint_updates {
        let semantic_id = program.semantic_domains.intern(&identity);
        let TypeConstraintNode::Domain(domain) =
            &mut program.type_reference_table.constraints_mut(constraints)[offset]
        else {
            unreachable!("collected domain constraint changed kind")
        };
        domain.semantic_id = semantic_id;
        domain.semantic_roles = language_semantics::DomainSemanticRoles {
            denotation_dimension: roles.denotation_dimension.map(|_| semantic_id),
            arithmetic_policy: roles.arithmetic_policy.map(|_| semantic_id),
        };
    }
    Ok(())
}

/// Finalize one producer's complete declaration graphs. Applied field views
/// use the same type operation without inventing declaration roots or replaying
/// the unchanged authored graph.
pub(crate) fn finalize_domain_instance_identities(
    program: &mut TypedTrees,
    declarations: &typed_trees::references::DeclarationSelection,
) -> Result<(), Diagnostic> {
    let references =
        typed_trees::references::collect_selected_program_references(program, declarations);
    finalize_type_domain_instance_identities(program, references.type_references)?;
    let mut expressions = references.expressions.into_iter().collect::<Vec<_>>();
    expressions.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    let cast_sites = expressions
        .into_iter()
        .filter_map(|handle| {
            let expression = program.expression_table.expression(handle);
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            (cast.semantic_domain_symbol.is_valid() && !cast.semantic_domain_arguments.is_empty())
                .then_some((
                    handle,
                    cast.semantic_domain_symbol,
                    cast.semantic_domain_arguments,
                ))
        })
        .collect::<Vec<_>>();
    let mut cast_updates = Vec::new();
    for (handle, symbol, arguments) in cast_sites {
        let Some(domain) = program.domain_definition(symbol).cloned() else {
            continue;
        };
        let index_parameters = typed_trees::domain::index_parameters(program, &domain);
        let arguments = program
            .type_reference_table
            .type_reference_handles(arguments);
        let identity = typed_trees::domain::indexed_domain_instance_name(
            program,
            &domain,
            index_parameters,
            arguments,
        )?;
        cast_updates.push((handle, identity));
    }
    for (handle, identity) in cast_updates {
        let semantic_id = program.semantic_domains.intern(&identity);
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle) else {
            unreachable!("collected cast changed kind")
        };
        cast.semantic_domain_id = semantic_id;
    }

    let mut membership_updates = Vec::new();
    for handle in references.proof_facts {
        let fact = program.proof_facts.get(handle);
        let Some(membership) = fact.as_membership() else {
            continue;
        };
        if !membership.domain_symbol.is_valid() {
            // Compiler carry permissions and unresolved authored facts have
            // their own validation; they are not declared-domain instances.
            continue;
        }
        let domain = program
            .domain_definition(membership.domain_symbol)
            .ok_or_else(|| Diagnostic::error("membership instance has no declared domain"))?;
        let arguments = program
            .type_reference_table
            .type_reference_handles(membership.domain_arguments);
        let parameters = typed_trees::domain::index_parameters(program, domain);
        if arguments.len() != membership.domain_arguments.len()
            || arguments.len() != parameters.len()
            || arguments.iter().any(|argument| {
                !program
                    .type_reference_table
                    .contains_type_reference(*argument)
            })
        {
            return Err(Diagnostic::error(
                "membership instance has missing or invalid domain arguments",
            ));
        }
        let identity = typed_trees::domain::indexed_domain_instance_name(
            program, domain, parameters, arguments,
        )?;
        membership_updates.push((handle, identity));
    }
    for (handle, identity) in membership_updates {
        let semantic_domain = program.semantic_domains.intern(&identity);
        if let typed_trees::domain::ProofFact::Membership(membership) =
            program.proof_facts.get_mut(handle)
        {
            membership.semantic_domain = semantic_domain;
        }
    }
    Ok(())
}

fn remapped_symbol(symbol: SymbolHandle, symbols: &[(SymbolHandle, SymbolHandle)]) -> SymbolHandle {
    symbols
        .iter()
        .find_map(|(before, after)| (*before == symbol).then_some(*after))
        .unwrap_or(symbol)
}

fn closed_operator_realizations_for_machine(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
) -> Result<Vec<typed_trees::operator::ClosedOperatorRealizationApplication>, Diagnostic> {
    let machine = program
        .machine_by_symbol(machine_symbol)
        .expect("specialized machine must remain in the typed program");
    program
        .machine_trait_conformances(machine)
        .iter()
        .filter_map(|conformance| {
            typed_trees::operator::declaration_by_symbol(
                program,
                conformance.requirement_symbol,
            )
            .map(|operator| (conformance, operator))
        })
        .map(|(conformance, operator)| {
            typed_trees::operator::closed_operator_realization_application(
                program, machine, operator,
            )
            .filter(|application| application.requirement_symbol == conformance.requirement_symbol)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "specialized machine `{}` does not reconstruct one exact closed application of operator requirement `{}::{}`",
                    machine.name,
                    conformance.name,
                    conformance
                        .requirement
                        .as_ref()
                        .map_or("<missing>", |requirement| requirement.as_str()),
                ))
            })
        })
        .collect()
}

fn specialized_attached_data(
    program: &dyn ::validation::CheckingInput,
    candidate: &Candidate,
    machine: &typed_trees::machine::Machine,
) -> Option<(typed_trees::name::Identifier, SymbolHandle)> {
    let attached = machine.attached_data.as_ref()?;
    let parameter_index = candidate
        .template
        .type_parameters
        .iter()
        .position(|(parameter, _)| *parameter == machine.attached_data_symbol);
    let Some(parameter_index) = parameter_index else {
        return Some((attached.clone(), machine.attached_data_symbol));
    };
    let binding = candidate.type_bindings[parameter_index]?;
    let symbol = program.type_reference_symbol(binding);
    match program.type_reference_table.type_reference(binding) {
        TypeReferenceNode::Named { name, .. } => Some((name.clone(), symbol)),
        TypeReferenceNode::Generic { base_name, .. } => Some((base_name.clone(), symbol)),
        _ => Some((attached.clone(), machine.attached_data_symbol)),
    }
}
