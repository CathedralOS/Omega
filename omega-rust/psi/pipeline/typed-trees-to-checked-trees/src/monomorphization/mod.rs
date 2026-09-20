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

use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use sha2::Sha256;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::signature::StateSignature;
use typed_trees::statement::{StatementHandle, StatementNode};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

mod attached_methods;
pub(crate) use attached_methods::validate_selected_attached_method_bounds;
mod candidate;
mod selection;
pub(crate) use selection::collect_expression_tree;
use selection::*;
mod body_rewriting;
pub(crate) use body_rewriting::collect_statement_expression_trees;
use body_rewriting::*;
mod identities;
use identities::*;
pub(crate) use identities::{
    bind_specialization_contract_identities, canonical_state_signature_bytes,
};
pub use identities::{
    generic_machine_template_commitment, generic_machine_template_report_fingerprint,
    recompute_machine_specialization_commitment,
};
mod body_cloning;
use body_cloning::*;
mod const_arguments;
mod dynamic_families;
pub(crate) use dynamic_families::generate_dynamic_family_specializations;
mod const_values;
#[cfg(test)]
mod membership_tests;
mod range_arguments;
mod result_locals;
#[cfg(test)]
mod runtime_value_tests;
mod saved_calls;
#[cfg(test)]
mod static_call_contract_tests;
#[cfg(test)]
mod storage_tests;

/// Authored metadata shared by all candidate applications in a round.
#[derive(Clone)]
struct CandidateTemplate {
    machine_index: usize,
    template_symbol: SymbolHandle,
    template_name: String,
    state_symbols: Vec<SymbolHandle>,
    type_parameters: Vec<(SymbolHandle, String)>,
    parameter_bounds: Vec<Vec<validation::DeclaredPropertyRequirement>>,
    conformance_bounds: Vec<typed_trees::machine::GenericConformanceBound>,
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
    type_bindings: Vec<Option<TypeReferenceHandle>>,
    const_bindings: Vec<Option<TypeReferenceHandle>>,
    /// A runtime Value slot binds its carrier here, while the exact call-site
    /// subject is retained separately and appended as an ordinary argument.
    runtime_value_bindings: Vec<Option<StaticMachineArgument>>,
    machine_bindings: Vec<Option<StaticMachineArgument>>,
    evidence_bindings: Vec<Option<StaticMachineArgument>>,
    inferred_conformance_arguments: Vec<SymbolHandle>,
    selected_bound_applications: Vec<typed_trees::typed_trees::ClosedConformanceApplication>,
    conflicted: bool,
}

impl Candidate<'_> {
    fn borrowed(&self) -> Candidate<'_> {
        Candidate {
            template: std::borrow::Cow::Borrowed(&self.template),
            type_bindings: self.type_bindings.clone(),
            const_bindings: self.const_bindings.clone(),
            runtime_value_bindings: self.runtime_value_bindings.clone(),
            machine_bindings: self.machine_bindings.clone(),
            evidence_bindings: self.evidence_bindings.clone(),
            inferred_conformance_arguments: self.inferred_conformance_arguments.clone(),
            selected_bound_applications: self.selected_bound_applications.clone(),
            conflicted: self.conflicted,
        }
    }
}

struct CalleeState {
    symbol: SymbolHandle,
    name: String,
    candidate_index: usize,
    return_type: TypeReferenceHandle,
    parameter_types: Vec<TypeReferenceHandle>,
    /// An attached method's `self` formal's position in `parameter_types`
    /// (`Named { machine, "Self" }`). Receiver-syntax calls exclude it from
    /// the argument list, so its binding evidence arrives through the
    /// receiver place instead.
    self_index: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallSite {
    Statement(StatementHandle),
    Expression(ExpressionHandle),
}

#[derive(Clone)]
struct CallSelection {
    site: CallSite,
    callee_symbol: SymbolHandle,
    candidate_index: usize,
    caller_is_generic: bool,
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
        !self.conflicted
            && !self.explicit_argument_overflow
            && !self.unresolved_machine_parameters
            && !self.unresolved_evidence_parameters
            && !self.unresolved_const_parameters
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

/// `enforce_complete_concrete_selections` distinguishes the authoritative
/// checking pass from speculative specialization runs. Build-program
/// preparation and `build.omg` interpretation specialize a private typed copy
/// while const endpoint folds are still outstanding; an underivable tuple
/// there is interim evidence, not a rejected program. Only the checking pass
/// that emits the program may reject an incomplete concrete selection, and
/// only once no pending endpoint fold can still supply the missing binding.
pub(crate) fn monomorphize_generic_machine_value_calls_with_selections(
    program: &mut TypedTrees,
    retained: &mut validation::ValidatedStaticMachineSelections,
    enforce_complete_concrete_selections: bool,
) -> Result<(), Vec<Diagnostic>> {
    loop {
        // Provider clones may restore old inferred types even when this round
        // discovers no further ordinary specialization.
        result_locals::refresh(program);
        materialize_static_argument_types(program);
        let candidates = candidate::collect(program);
        let callee_states = candidate::callees(program, &candidates);
        const_arguments::validate_authored(program, &candidates, &callee_states)?;
        if candidates.is_empty() {
            return Ok(());
        }
        let contract_expressions = contract_expression_handles(program);
        let selections = selection::collect_call_selections(
            program,
            &candidates,
            &callee_states,
            &contract_expressions,
        );
        if let Some(selection) = selections
            .iter()
            .find(|selection| selection.explicit_argument_overflow)
        {
            return Err(vec![Diagnostic::error(format!(
                "generic machine `{}` has excess explicit static arguments for a parameter kind",
                candidates[selection.candidate_index].template.template_name
            ))]);
        }
        // A retained generic body still checks each known application argument,
        // including a discarded call with no inferred destination to refresh.
        // The tuple's bounds do not depend on whether we emit a private instance.
        for selection in selections
            .iter()
            .filter(|selection| selection.caller_is_generic)
        {
            let mut candidate =
                candidate_for_selection(&candidates[selection.candidate_index], selection);
            if approved_type_bounds(program, std::slice::from_ref(&candidate)) != [true] {
                return Err(vec![Diagnostic::error(format!(
                    "generic machine `{}` has a call tuple that does not satisfy its authored type bounds",
                    candidate.template.template_name,
                ))]);
            }
            if selection.is_complete() {
                validate_candidate_conformance_bounds(program, &mut candidate)?;
            }
            const_arguments::validate_bindings(program, &candidate).map_err(|error| vec![error])?;
        }
        result_locals::refresh_generic_call_results(program, &candidates, &selections)?;
        // All templates in this round share the complete original call graph.
        // Per-instance inference would mix capture with newly selected clones.
        let operational = validation::infer_operational_may(program);
        let service_reaches = validation::infer_service_reaches(program, &operational);
        let mut diagnostics = Vec::new();
        let mut applied_any = false;
        for (candidate_index, candidate) in candidates.iter().enumerate() {
            // Open callers keep their symbolic applications. Concrete applications
            // of the same template are independent tuples, never evidence with
            // which to consume the declaration or specialize another caller.
            //
            // An incomplete selection that already holds const, machine, or
            // conformance evidence rejects in every pass: partial static
            // evidence can never complete to a single tuple.
            //
            // A zero-binding incomplete selection is NOT rejected here. The
            // specialization loop is a fixed point: a later round may still
            // land a receiver, provider, or endpoint binding that completes
            // the tuple, so this pass must wait for `applied_any` to go quiet
            // before calling a statement call underivable.
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
                continue;
            }
            match apply_call_specializations(
                program,
                candidate,
                &selections,
                candidate_index,
                &service_reaches,
            ) {
                Ok(changed) => applied_any |= changed,
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        if !applied_any {
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
            return Ok(());
        }
        refresh_closed_domain_instance_identities(program).map_err(|error| vec![error])?;
        retained.extend(validation::validate_static_machine_selections_with_facts(
            program,
        )?);
    }
}

fn materialize_static_argument_types(program: &mut TypedTrees) {
    fn collect(
        program: &TypedTrees,
        arguments: &[StaticMachineArgument],
        literals: &mut Vec<String>,
        types: &mut Vec<(SymbolHandle, typed_trees::name::Identifier)>,
    ) {
        for argument in arguments {
            if let Some(literal) = const_arguments::spelling(program, argument)
                && !literals.contains(&literal)
            {
                literals.push(literal);
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
                collect(program, &application.arguments, literals, types);
            }
        }
    }

    let mut literals = Vec::new();
    let mut types = Vec::new();
    for (_, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            collect(program, &call.machine_arguments, &mut literals, &mut types);
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement {
                    collect(program, &call.machine_arguments, &mut literals, &mut types);
                }
            }
        }
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
        let exists = program
            .type_reference_table
            .named_references()
            .any(|(_, symbol, name)| !symbol.is_valid() && name == literal);
        if !exists {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: typed_trees::name::Identifier::generated(literal),
                });
        }
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
            ExpressionNode::Integer(literal) => {
                literal.landing().map(|landing| match landing.landed_type {
                    numerics::literals::LandedIntegerType::I8 => symbols::BuiltinTypeAtom::I8,
                    numerics::literals::LandedIntegerType::I16 => symbols::BuiltinTypeAtom::I16,
                    numerics::literals::LandedIntegerType::I32 => symbols::BuiltinTypeAtom::I32,
                    numerics::literals::LandedIntegerType::I64 => symbols::BuiltinTypeAtom::I64,
                    numerics::literals::LandedIntegerType::U8 => symbols::BuiltinTypeAtom::U8,
                    numerics::literals::LandedIntegerType::U16 => symbols::BuiltinTypeAtom::U16,
                    numerics::literals::LandedIntegerType::U32 => symbols::BuiltinTypeAtom::U32,
                    numerics::literals::LandedIntegerType::U64 => symbols::BuiltinTypeAtom::U64,
                    numerics::literals::LandedIntegerType::Addr => {
                        symbols::BuiltinTypeAtom::Address
                    }
                })
            }
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
mod selected_operator_providers;
use crate::monomorphization::selection::{
    approved_type_bounds, contract_expression_handles, unique_complete_selections,
    validate_candidate_conformance_bounds,
};
pub(crate) use selected_operator_providers::{
    SelectedProviderTemplates, specialize_selected_generic_operator_providers,
};

fn apply_call_specializations(
    program: &mut TypedTrees,
    template: &Candidate,
    selections: &[CallSelection],
    candidate_index: usize,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
) -> Result<bool, Vec<Diagnostic>> {
    let groups = unique_complete_selections(program, selections, candidate_index);
    if groups.is_empty() {
        return Ok(false);
    }
    let mut concrete_candidates: Vec<Candidate> = groups
        .iter()
        .map(|(_, members)| candidate_for_selection(template, &selections[members[0]]))
        .collect();
    if approved_type_bounds(program, &concrete_candidates)
        .iter()
        .any(|approved| !approved)
    {
        return Err(vec![Diagnostic::error(format!(
            "generic machine `{}` has a concrete specialization tuple that does not satisfy its authored type bounds",
            template.template.template_name
        ))]);
    }
    let mut conformance_diagnostics = Vec::new();
    for candidate in &mut concrete_candidates {
        if let Err(mut errors) = validate_candidate_conformance_bounds(program, candidate) {
            conformance_diagnostics.append(&mut errors);
        }
    }
    if !conformance_diagnostics.is_empty() {
        return Err(conformance_diagnostics);
    }

    let canonical_template_contract_bytes = canonical_template_contract_bytes(
        program,
        template.template.machine_index,
        service_reaches,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let template_contract_report_fingerprint =
        fnv1a_report_fingerprint(&canonical_template_contract_bytes);
    let template_contract_commitment =
        machine_template_commitment(&canonical_template_contract_bytes);
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
            let Some(machine) = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == existing)
            else {
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
            let ordinal = program
                .machine_specializations
                .iter()
                .filter(|instance| instance.template == template.template.template_symbol)
                .count();
            clone_specialized_machine(
                None,
                program,
                candidate,
                ordinal,
                template_contract_report_fingerprint,
                template_contract_commitment,
                canonical_template_contract_bytes.clone(),
                normalized_template_identity.clone(),
                accepted_template_commitment.clone(),
            )
            .map_err(|error| vec![error])?
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
            // Each site keeps its own exact runtime subjects: the shared
            // carrier-keyed instance receives them as appended arguments.
            let subjects = selection
                .runtime_value_bindings
                .iter()
                .flatten()
                .filter_map(|argument| Some((argument.path.first()?.clone(), argument.symbol)))
                .collect::<Vec<_>>();
            selected_call_rewrites.push((selection.site, *concrete_state, subjects));
        }
    }
    // Delay call rewrites until every tuple has copied its authored body.
    for (site, concrete_state, subjects) in selected_call_rewrites {
        rewrite_selected_call(program, site, concrete_state, &subjects);
    }
    Ok(true)
}

fn candidate_for_selection<'template>(
    template: &'template Candidate<'_>,
    selection: &CallSelection,
) -> Candidate<'template> {
    Candidate {
        template: std::borrow::Cow::Borrowed(&template.template),
        type_bindings: selection.type_bindings.clone(),
        const_bindings: selection.const_bindings.clone(),
        runtime_value_bindings: selection.runtime_value_bindings.clone(),
        machine_bindings: selection.machine_bindings.clone(),
        evidence_bindings: selection.evidence_bindings.clone(),
        inferred_conformance_arguments: template.inferred_conformance_arguments.clone(),
        selected_bound_applications: template.selected_bound_applications.clone(),
        conflicted: selection.conflicted,
    }
}

fn candidate_conformance_fingerprint_arguments(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<String> {
    let mut arguments = candidate
        .evidence_bindings
        .iter()
        .map(|binding| {
            binding
                .as_ref()
                .expect("complete specialization")
                .display_name()
        })
        .collect::<Vec<_>>();
    arguments.extend(
        candidate
            .inferred_conformance_arguments
            .iter()
            .map(|symbol| conformance_symbol_identity(program, *symbol)),
    );
    arguments.extend(
        candidate
            .selected_bound_applications
            .iter()
            .map(|application| {
                format!(
                    "{}#{:016x}",
                    conformance_symbol_identity(program, application.declaration),
                    application.report_fingerprint
                )
            }),
    );
    arguments
}

fn conformance_symbol_identity(program: &TypedTrees, symbol: SymbolHandle) -> String {
    let path = program.symbols.display_path(symbol, "::");
    let Some(package) = program.symbols.symbol_package_identity(symbol) else {
        return format!("unmanaged::{path}");
    };
    let digest = package.digest();
    let mut owner = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        owner.push(char::from(HEX[usize::from(byte >> 4)]));
        owner.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    format!("package:{owner}::{path}")
}

fn normalized_machine_identity(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<String> {
    let declaration = conformance_symbol_identity(program, machine.symbol);
    let overload = program
        .normalized_machine_overload_identity(machine)?
        .identity();
    Some(format!("{declaration}|{overload}"))
}

/// Const substitution changes an indexed-domain instance from binder identity
/// (`Quantity<To>`) to the selected canonical value (`Quantity<METER>`). The
/// semantic ID is derived data, so refresh every affected constraint and cast
/// before validation or checked-fact construction observes the specialized
/// graph.
pub fn refresh_closed_domain_instance_identities(
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    let mut constraint_updates = Vec::new();
    for (_, _, constraints) in program
        .type_reference_table
        .constrained_type_reference_sites()
    {
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
            let Some(domain) = program
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == constraint.symbol)
                .cloned()
            else {
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

    let cast_sites = program
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| {
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
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
            .cloned()
        else {
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
    for (handle, fact) in program.proof_facts.iter() {
        let typed_trees::domain::ProofFact::Membership(membership) = fact else {
            continue;
        };
        if !membership.domain_symbol.is_valid() {
            // Compiler carry permissions and unresolved authored facts have
            // their own validation; they are not declared-domain instances.
            continue;
        }
        let domain = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)
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
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
) -> Result<Vec<typed_trees::operator::ClosedOperatorRealizationApplication>, Diagnostic> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
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
    program: &TypedTrees,
    candidate: &Candidate,
    machine: &typed_trees::machine::Machine,
) -> Option<(typed_trees::name::Identifier, SymbolHandle)> {
    let attached = machine.attached_data.as_ref()?;
    let parameter_index = candidate
        .template
        .type_parameters
        .iter()
        .position(|(_, name)| name == attached.as_str());
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

fn resolve_specialized_receiver_calls(
    program: &mut TypedTrees,
    machine: &typed_trees::machine::Machine,
) {
    let states = program.machine_states(machine).to_vec();
    let mut statement_updates = Vec::new();
    let mut expression_updates = Vec::new();
    for state in &states {
        let mut expression_handles = Vec::new();
        for (index, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            collect_statement_expression_trees(program, statement, &mut expression_handles);
            let StatementNode::Call(call) = statement else {
                continue;
            };
            let target = crate::lookup::resolve_state_call_target(
                program,
                machine,
                state,
                call.receiver_symbol,
                call.target_symbol,
                crate::lookup::statement_call_receiver_members(program, call),
                &call.target,
            );
            if target.is_valid() {
                statement_updates.push((state.statement_nodes, index, target));
            }
        }
        for handle in expression_handles {
            let ExpressionNode::Call(call) = program.expression_table.expression(handle) else {
                continue;
            };
            let (receiver_symbol, receiver_path) =
                crate::lookup::call_receiver_parts(program, call.receiver);
            let target = crate::lookup::resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            );
            if target.is_valid() {
                expression_updates.push((handle, target));
            }
        }
    }
    for (span, index, target) in statement_updates {
        let StatementNode::Call(call) = &mut program.statement_table.statements_mut(span)[index]
        else {
            continue;
        };
        call.target_symbol = target;
    }

    expression_updates.sort_unstable_by_key(|(handle, _)| handle.arena_index());
    expression_updates.dedup_by_key(|(handle, _)| handle.arena_index());
    for (handle, target) in expression_updates {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        call.target_symbol = target;
    }
}
