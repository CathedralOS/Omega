//! Settle the subject of a value-form `align_of(place)` query.
//!
//! Typing keeps the authored operand because a place's declared type is known
//! only once locals and parameters resolve in their state. The query observes
//! that place's storage location: a reference observes its referent, never
//! the reference descriptor. Every Psi and Omega consumer then answers the
//! same portable selected-layout alignment request the type form uses, so the
//! location's guaranteed alignment is the validated minimum of the storage
//! type under the default storage rule.
//!
//! The operand is observed once without being read or moved: it leaves the
//! executable call here, so no load, loan or initialization is generated.
//! A value leaf (a call result, literal or arithmetic) owns no location and
//! rejects. A borrowed slice observes its first element's location, which
//! exists only for a nonempty view. The settled request carries a proof-only
//! `view[0]` index that range checking discharges with the same length facts
//! as an ordinary index; it is never evaluated, so the query creates no
//! implicit `&view[0]` and reads no element.

use crate::tables::{expressions_of, expressions_of_mut};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_core::data_layout_queries::DataLayoutQueryKind;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::expression_traversal::collect_statement_expression_trees;

pub(super) fn settle(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut settled = Vec::new();
    let mut diagnostics = Vec::new();
    let mut rejected = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut handles = Vec::new();
                collect_statement_expression_trees(program, statement, &mut handles);
                for query in handles {
                    let Some(place) = value_query_operand(program, query) else {
                        continue;
                    };
                    if settled.iter().any(|(handle, _, _)| *handle == query) {
                        continue;
                    }
                    match observed_storage_type(program, state.symbol, statement_index, place) {
                        Ok(subject) => settled.push((query, place, subject)),
                        Err(message) => {
                            rejected.push(query);
                            diagnostics.push(
                                Diagnostic::error(message)
                                    .with_source_span(expressions_of(program).source_span(query)),
                            );
                        }
                    }
                }
            }
        }
    }
    for (query, place, subject) in settled {
        let nonempty_view = match subject {
            ObservedStorage::Place(_) => ExpressionHandle::invalid(),
            ObservedStorage::FirstElement(_) => {
                let span = expressions_of(program).source_span(query);
                let table = expressions_of_mut(program);
                let zero = table.insert(ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(0),
                ));
                table.set_source_span(zero, span);
                let first = table.insert(ExpressionNode::Indexed(
                    typed_trees::expression::TableIndexedExpression {
                        collection: place,
                        index: zero,
                    },
                ));
                table.set_source_span(first, span);
                first
            }
        };
        if let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(query)
            && let Some(request) = call.layout_query.as_mut()
        {
            request.operand.type_reference = subject.storage_type();
            request.nonempty_view = nonempty_view;
            call.arguments = HandleSpan::empty();
        }
    }
    // A query in a `requires` contract runs in no statement. Contracts bind
    // only parameters, so its place must be rooted at one and takes that
    // parameter's declared type, projected through any field path. `ensures` and trait requirement contracts
    // are not yet answered and reject. Anything else (a constant
    // initializer, a data predicate) names no storage place and rejects.
    let roots: Vec<_> = expressions_of(program)
        .iter_expressions()
        .map(|(root, _)| root)
        .collect();
    let mut unsettled = Vec::new();
    for root in roots {
        let mut handles = Vec::new();
        validation::expression_traversal::collect_expression_tree(program, root, &mut handles);
        unsettled.extend(handles.into_iter().filter(|query| {
            !rejected.contains(query) && value_query_operand(program, *query).is_some()
        }));
    }
    unsettled.sort_by_key(|query| (query.arena_index(), query.generation()));
    unsettled.dedup();
    // A value-form `ensures` is admitted by settling but cannot yet be proved
    // at exit, so a true clause would fail as unprovable. Reject it plainly.
    let mut ensures_handles = Vec::new();
    let mut collect_ensures = |contract: &typed_trees::signature::SignatureContract| {
        if contract.kind == typed_trees::signature::SignatureContractKind::Ensures {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let Some(expression) = fact.as_expression() {
                    validation::expression_traversal::collect_expression_tree(
                        program,
                        *expression,
                        &mut ensures_handles,
                    );
                }
            }
        }
    };
    for machine in program.machines() {
        program
            .signature_contracts
            .span_or_empty(machine.contracts)
            .iter()
            .for_each(&mut collect_ensures);
        for state in program.machine_states(machine) {
            program
                .state_contracts(state)
                .iter()
                .for_each(&mut collect_ensures);
        }
    }
    for signature in program.prepared_callable_signatures() {
        program
            .state_signature_contracts(signature)
            .iter()
            .for_each(&mut collect_ensures);
    }
    let mut contract_settled = Vec::new();
    for query in unsettled {
        if ensures_handles.contains(&query) {
            diagnostics.push(
                Diagnostic::error(
                    "`align_of(value)` in an `ensures` contract is not yet supported; state the alignment in `requires` or check it in the body",
                )
                .with_source_span(expressions_of(program).source_span(query)),
            );
            continue;
        }
        let Some(place) = value_query_operand(program, query) else {
            continue;
        };
        match contract_storage_type(program, place) {
            Ok(subject) => contract_settled.push((query, subject)),
            Err(message) => diagnostics.push(
                Diagnostic::error(message)
                    .with_source_span(expressions_of(program).source_span(query)),
            ),
        }
    }
    for (query, subject) in contract_settled {
        if let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(query)
            && let Some(request) = call.layout_query.as_mut()
        {
            request.operand.type_reference = subject;
            call.arguments = HandleSpan::empty();
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// A value-form query keeps exactly one authored operand until settled here.
fn value_query_operand(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Call(call) = expressions_of(program).expression(expression) else {
        return None;
    };
    let request = call.layout_query.as_ref()?;
    if request.kind != DataLayoutQueryKind::Alignment || request.operand.type_reference.is_valid() {
        return None;
    }
    match expressions_of(program).expression_handles(call.arguments) {
        [operand] => Some(*operand),
        _ => None,
    }
}

/// A `requires` query's subject: a parameter of an ordinary machine, a
/// reference parameter's referent, or a field path below either. Contracts
/// carry no proof context for a slice's nonempty view, so slices stay
/// statement-only, and trait requirement contracts are not yet answered.
fn contract_storage_type(
    program: &TypedTrees,
    place: ExpressionHandle,
) -> Result<TypeReferenceHandle, &'static str> {
    const NO_PLACE: &str = "`align_of(value)` observes a storage place; outside a machine body only a parameter of the contracted machine names one";
    let canonical = validation::canonical_place_from_expression(program, place).ok_or(NO_PLACE)?;
    let root = canonical.root.as_symbol().ok_or(NO_PLACE)?;
    let declared = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .flat_map(|state| program.state_parameters(state).iter())
        .find(|parameter| parameter.symbol == root)
        .map(|parameter| parameter.type_reference);
    let Some(declared) = declared else {
        let trait_parameter = program
            .traits()
            .iter()
            .flat_map(|definition| program.trait_machine_signatures(definition).iter())
            .flat_map(|signature| program.state_signature_parameters(signature).iter())
            .any(|parameter| parameter.symbol == root);
        return Err(if trait_parameter {
            "`align_of(value)` in a trait requirement's contract is not yet supported; its implementations cannot carry the clause"
        } else {
            NO_PLACE
        });
    };
    // A field path projects from the parameter's referent, exactly as the
    // body form does: a field inside an explicit representation has no
    // default-placement guarantee, and a reference field observes its referent.
    let mut subject = referent(program, declared);
    for segment in &canonical.segments {
        if !matches!(segment, facts::PlaceSegment::Field { .. }) {
            return Err(
                "`align_of(value)` in a contract observes a parameter, its referent or a field path; an element or other projection is not yet supported there",
            );
        }
        if program.layout_storage_arguments(subject).is_some() {
            return Err(
                "`align_of(value)` on a field of an explicit layout representation needs that policy's placement, which is not yet observed",
            );
        }
        subject = validation::project_type_reference_from_segments(
            program,
            subject,
            std::slice::from_ref(segment),
        )
        .map(|field| referent(program, field))
        .ok_or("`align_of(value)` requires an operand whose storage type is known")?;
    }
    if matches!(
        program.type_reference_table.type_reference(subject),
        TypeReferenceNode::Slice { .. }
    ) {
        return Err(
            "`align_of(view)` on a slice in a contract needs a nonempty-view proof the contract cannot carry; query the slice in the body after its guard",
        );
    }
    Ok(subject)
}

/// The storage type whose selected layout bounds the observed location.
fn observed_storage_type(
    program: &TypedTrees,
    state: symbols::SymbolHandle,
    statement_index: usize,
    place: ExpressionHandle,
) -> Result<ObservedStorage, &'static str> {
    // Only a local or parameter names storage; a constant is a value and a
    // value leaf (call result, literal, arithmetic) owns no location.
    let canonical =
        validation::canonical_place_from_expression_in_state(program, state, statement_index, place)
            .filter(|canonical| {
                canonical.root.as_symbol().is_some_and(|root| {
                    matches!(
                        program.symbols.get(root).kind,
                        symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
                    )
                })
            })
            .ok_or(
                "`align_of(value)` observes an addressable storage place; this operand is a value with no storage location",
            )?;
    // A case payload field is selected by its name, not by a proof that the
    // case is active; its storage type would be the first matching case's.
    // An ordinary read owes that case proof, but this query never reads.
    if canonical
        .segments
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
    {
        return Err(
            "`align_of(value)` on a case payload field needs a proof that the case is active, which the query does not yet carry; query a field outside the case",
        );
    }
    // A field inside an explicit representation (`LayoutStorage<T, Policy>`)
    // sits wherever that policy placed it, so the field type's default
    // minimum is no guarantee for it. Reject rather than overstate it.
    for prefix in 0..canonical.segments.len() {
        let enclosing = validation::CanonicalPlace {
            root: canonical.root,
            segments: canonical.segments[..prefix].to_vec(),
        };
        let through_representation =
            validation::canonical_place_type_reference(program, state, statement_index, &enclosing)
                .map(|enclosing| referent(program, enclosing))
                .is_some_and(|enclosing| program.layout_storage_arguments(enclosing).is_some());
        if through_representation {
            return Err(
                "`align_of(value)` on a field of an explicit layout representation needs that policy's placement, which is not yet observed",
            );
        }
    }
    let declared =
        validation::expression_type_reference_in_state(program, state, statement_index, place)
            .ok_or("`align_of(value)` requires an operand whose storage type is known")?;
    // A reference observes its referent's location, not its descriptor.
    let subject = referent(program, declared);
    if let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(subject)
    {
        return Ok(ObservedStorage::FirstElement(unconstrained(
            program,
            *element_type,
        )));
    }
    Ok(ObservedStorage::Place(subject))
}

/// The storage a query observes: the place itself (or a reference's
/// referent), or the first element of a borrowed slice, which owes a proved
/// nonempty view.
#[derive(Clone, Copy)]
enum ObservedStorage {
    Place(TypeReferenceHandle),
    FirstElement(TypeReferenceHandle),
}

impl ObservedStorage {
    fn storage_type(self) -> TypeReferenceHandle {
        match self {
            Self::Place(storage) | Self::FirstElement(storage) => storage,
        }
    }
}

fn referent(program: &TypedTrees, reference: TypeReferenceHandle) -> TypeReferenceHandle {
    let reference = unconstrained(program, reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, .. } => unconstrained(program, *referee),
        _ => reference,
    }
}

fn unconstrained(program: &TypedTrees, mut reference: TypeReferenceHandle) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    reference
}
