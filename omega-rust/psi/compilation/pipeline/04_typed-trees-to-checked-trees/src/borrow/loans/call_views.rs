//! call views slice of loans.

use super::super::accesses::{self, borrow_access_place};
use super::super::tracker::StateLoanTracker;
use super::RebasedBorrowPlace;
use super::owner_paths::owner_path_matches;
use crate::borrow::view_link::{ViewReturnSource, resolve_signature_view_return_source};

use arena::Handle;
use checked_trees::expression::ExpressionHandle;
use language_semantics::declaration_selection::CollectionViewOperation;
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn helper_call_borrow_loan_place(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    call: &checked_trees::expression::TableCallExpression,
) -> Option<accesses::BorrowAccessPlace> {
    // A checked call is the same `TableCallExpression` the typed program
    // holds, and none of this function's callers carry `CheckFacts`, so the
    // retained `CollectionView` selection is out of reach here and the call
    // shape answers instead.
    //
    // `Bytes` stays excluded deliberately. `as_slice`/`as_mut_slice`/`as_view`
    // hand back a borrow of the receiver's own storage, which is exactly the
    // loan recorded below. `bytes` reads a view's byte content; whether that
    // result borrows the same storage is a question for the byte-carrier
    // semantics, and nothing here establishes it -- widening it would mint a
    // loan against a place this lane never proved is the source.
    if matches!(
        validation::collection_operations::collection_view_call(program, call),
        Some(
            CollectionViewOperation::SharedSlice
                | CollectionViewOperation::MutableSlice
                | CollectionViewOperation::TextView
        )
    ) {
        return borrow_access_place(
            program,
            state_symbol,
            statement_index,
            call.receiver,
            machine_symbol,
        );
    }

    // The borrow source (self, a named input, or none) is resolved by the same
    // logic the declaration check uses (`borrow::view_link`), so the loan we
    // track here always matches what the elision check accepted. Elision rules
    // 1/3 and stage-2 explicit lifetimes all flow through there; a generic
    // signature resolves the same relation under the call's closed bindings.
    // `&mut` structural-view results mint their loan at the callee's own
    // tail source — the structural custody model names the storage the
    // returned view borrows where signature elision can only name the
    // declared shape.
    if let Some(place) = super::mutable_view_results::mutable_view_call_loan_place(
        program,
        state_symbol,
        statement_index,
        machine_symbol,
        call,
    ) {
        return Some(place);
    }

    match call_view_return_source_at(program, state_symbol, call) {
        ViewReturnSource::NotApplicable
        | ViewReturnSource::Ambiguous(_)
        | ViewReturnSource::Fields { .. } => None,
        ViewReturnSource::Parameter { parameter_index } => {
            // The returned view borrows one named (or single) ref input; its
            // loan follows that argument's place. Arguments map 1:1 to parameters.
            let arguments = program.expression_table.expression_handles(call.arguments);
            let argument = arguments.get(parameter_index).copied()?;
            argument_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                argument,
            )
        }
    }
}

pub(super) fn call_view_return_source(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
) -> ViewReturnSource {
    let Some((parameters, return_type, _)) = call_view_signature(program, target_symbol) else {
        return ViewReturnSource::NotApplicable;
    };
    resolve_signature_view_return_source(program, parameters, return_type)
}

/// One call's declaration and applied telescope travel together. Returned
/// fields, direct-reference ancestry and nested transfers must consume this
/// same relation: looking up the raw signature again loses an evidence
/// binder's application and can silently erase its aggregate loans.
pub(in crate::borrow) struct CallViewReturnContract<'program> {
    parameters: &'program [StateParameter],
    return_type: TypeReferenceHandle,
    pub(super) substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    substitution_scope_starts: Vec<usize>,
}

impl CallViewReturnContract<'_> {
    pub(in crate::borrow) fn source(
        &self,
        program: &dyn ::validation::CheckingInput,
    ) -> ViewReturnSource {
        if self.substitutions.is_empty() {
            resolve_signature_view_return_source(program, self.parameters, self.return_type)
        } else {
            crate::borrow::view_link::resolve_substituted_view_return_source(
                program,
                self.parameters,
                self.return_type,
                &self.substitutions,
                &self.substitution_scope_starts,
            )
        }
    }

    pub(in crate::borrow) fn return_type_reference(
        &self,
        program: &dyn ::validation::CheckingInput,
    ) -> TypeReferenceHandle {
        self.carrier_reference(program, self.return_type)
    }

    pub(in crate::borrow) fn parameter_type_reference(
        &self,
        program: &dyn ::validation::CheckingInput,
        parameter_index: usize,
    ) -> TypeReferenceHandle {
        self.parameters
            .get(parameter_index)
            .map_or_else(TypeReferenceHandle::invalid, |parameter| {
                self.carrier_reference(program, parameter.type_reference)
            })
    }

    /// Only the borrow carrier is projected here; qualifications remain on
    /// the original declaration. Composite types stay borrowed nodes and the
    /// structural frontier walks them under the complete substitution roster.
    fn carrier_reference(
        &self,
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        let mut visited = Vec::new();
        loop {
            if visited.contains(&reference) {
                return TypeReferenceHandle::invalid();
            }
            visited.push(reference);
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Named { symbol, .. } => {
                    let Some((_, argument)) = self
                        .substitutions
                        .iter()
                        .rev()
                        .find(|(parameter, _)| parameter == symbol)
                    else {
                        return reference;
                    };
                    reference = *argument;
                }
                _ => return reference,
            }
        }
    }
}

pub(in crate::borrow) fn call_view_return_contract<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    call: &checked_trees::expression::TableCallExpression,
) -> Option<CallViewReturnContract<'program>> {
    let (caller, state) = validation::find_state_with_machine(program, state_symbol)?;
    let (parameters, return_type, substitutions, substitution_scope_starts) =
        if let Ok(Some(requirement)) =
            validation::named_conformance_target_requirement(program, caller, call.target_symbol)
        {
            (
                program.state_signature_parameters(requirement.signature),
                requirement.signature.return_type,
                requirement.application.substitutions,
                requirement.application.substitution_scope_starts,
            )
        } else {
            let (parameters, return_type, signature) =
                call_view_signature(program, call.target_symbol)?;
            let substitutions = signature
                .and_then(|signature| {
                    validation::closed_static_call_type_bindings(
                        program,
                        caller,
                        state,
                        signature,
                        &call.machine_arguments,
                        program.expression_table.expression_handles(call.arguments),
                    )
                })
                .unwrap_or_default();
            (parameters, return_type, substitutions, vec![0])
        };
    Some(CallViewReturnContract {
        parameters,
        return_type,
        substitutions,
        substitution_scope_starts,
    })
}

pub(in crate::borrow) fn call_view_return_source_at(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    call: &checked_trees::expression::TableCallExpression,
) -> ViewReturnSource {
    call_view_return_contract(program, state_symbol, call)
        .map_or(ViewReturnSource::NotApplicable, |contract| {
            contract.source(program)
        })
}

/// True when the call target's own declaration names one exact borrow source
/// for its returned view: a single direct reference parameter.
/// This is the promotion gate for call-result loan provenance --
/// the name-matched slice/view builtins, carrier-field sources, ambiguous
/// signatures, and view-free calls all stay deliberately unretained.
pub(crate) fn call_declares_direct_view_source(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
) -> bool {
    matches!(
        call_view_return_source(program, target_symbol),
        ViewReturnSource::Parameter { .. }
    )
}

fn call_view_signature(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
) -> Option<(
    &[typed_trees::signature::StateParameter],
    typed_trees::types::TypeReferenceHandle,
    Option<&typed_trees::signature::StateSignature>,
)> {
    if let Some(target_state) = program.state_by_symbol(target_symbol) {
        return Some((
            program.state_parameters(target_state),
            target_state.return_type,
            None,
        ));
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
        return Some((
            program.state_signature_parameters(signature),
            signature.return_type,
            Some(signature),
        ));
    }
    for trait_definition in program.traits() {
        if let Some(signature) =
            program.trait_machine_signature_by_symbol(trait_definition, target_symbol)
        {
            return Some((
                program.state_signature_parameters(signature),
                signature.return_type,
                Some(signature),
            ));
        }
    }
    None
}

/// The loan place for a call argument that the called machine's returned view
/// borrows (elision rule 1). Handles arguments that are themselves
/// view-producing calls (`bag.cells.as_mut_slice()`, or a nested one-ref-input
/// machine call) by recursing into the call's own loan place.
pub(crate) fn argument_borrow_loan_place(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    argument: ExpressionHandle,
) -> Option<accesses::BorrowAccessPlace> {
    match program.expression_table.expression(argument) {
        checked_trees::expression::ExpressionNode::Call(inner_call) => {
            helper_call_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                inner_call,
            )
        }
        checked_trees::expression::ExpressionNode::Borrow(inner)
            if matches!(
                program.expression_table.expression(inner.target),
                checked_trees::expression::ExpressionNode::Call(_)
            ) =>
        {
            argument_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                inner.target,
            )
        }
        _ => borrow_access_place(
            program,
            state_symbol,
            statement_index,
            argument,
            machine_symbol,
        ),
    }
}

pub(super) fn rebase_borrow_places_through_local_loans(
    program: &dyn ::validation::CheckingInput,
    place: accesses::BorrowAccessPlace,
    loan_trackers: &[StateLoanTracker],
) -> Vec<RebasedBorrowPlace> {
    let source_loans: Vec<&StateLoanTracker> = loan_trackers
        .iter()
        .rev()
        .filter(|loan| {
            loan.owner_symbol == place.root_symbol
                && owner_path_matches(program, &loan.owner_path, &place.segments)
        })
        .collect();
    if source_loans.is_empty() {
        return vec![RebasedBorrowPlace {
            place,
            source_owner_symbol: SymbolHandle::invalid(),
            parent_loan: Handle::invalid(),
            parent_lineage_is_retained: false,
        }];
    }

    source_loans
        .into_iter()
        .map(|source_loan| {
            let remainder = &place.segments[source_loan.owner_path.len()..];
            let mut rebased_segments = Vec::with_capacity(
                source_loan
                    .place
                    .segments
                    .len()
                    .saturating_add(remainder.len()),
            );
            rebased_segments.extend(source_loan.place.segments.iter().copied());
            rebased_segments.extend(remainder.iter().copied());

            RebasedBorrowPlace {
                place: accesses::BorrowAccessPlace {
                    incoming_reference: source_loan.place.incoming_reference,
                    root_symbol: source_loan.place.root_symbol,
                    segments: rebased_segments,
                },
                source_owner_symbol: source_loan.owner_symbol,
                parent_loan: source_loan.handle,
                // A call-result root is retained for receiver resolution but
                // carries no borrow ancestry: children formed through it stay
                // derived, so the reborrow resource model never sees more than
                // the suspension shapes it can represent.
                parent_lineage_is_retained: source_loan.lineage
                    != checked_trees::BorrowLoanLineage::UnretainedDerived
                    && !source_loan.call_result,
            }
        })
        .collect()
}

pub(super) fn retained_reference_lineage(
    program: &dyn ::validation::CheckingInput,
    formation: ExpressionHandle,
    source: &RebasedBorrowPlace,
    rebased: &[RebasedBorrowPlace],
) -> checked_trees::BorrowLoanLineage {
    // The formation operation decides lineage, not whether the descriptor is
    // stored in a reference local, an aggregate leaf or a replacement slot.
    let is_explicit_reborrow =
        super::reference_borrows::direct_reborrow_target(program, formation).is_some();
    let declared_source = match program.expression_table.expression(formation) {
        checked_trees::expression::ExpressionNode::Call(call) => {
            call_declares_direct_view_source(program, call.target_symbol)
        }
        _ => false,
    };
    let force_unretained = matches!(
        program.expression_table.expression(formation),
        checked_trees::expression::ExpressionNode::Cast(_)
            | checked_trees::expression::ExpressionNode::Call(_)
    ) && !declared_source
        && !is_explicit_reborrow;
    if force_unretained {
        return checked_trees::BorrowLoanLineage::UnretainedDerived;
    }
    if !source.source_owner_symbol.is_valid() {
        return checked_trees::BorrowLoanLineage::DirectRoot;
    }
    let parent_is_unique = rebased.len() == 1;
    if is_explicit_reborrow
        && source.parent_loan.is_valid()
        && source.parent_lineage_is_retained
        && parent_is_unique
    {
        checked_trees::BorrowLoanLineage::Reborrow {
            parent_loan: source.parent_loan,
        }
    } else {
        checked_trees::BorrowLoanLineage::UnretainedDerived
    }
}

/// The recast reference's `referee`/`access` and the machine `machine_symbol`
/// names — the shared prologue both indexed-recast borrow places peel before
/// validating their source.
pub(super) fn indexed_recast_loan_parts<'a>(
    program: &'a dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    local_data: &checked_trees::statement::TableLocalData,
) -> Option<(
    &'a typed_trees::machine::Machine,
    typed_trees::types::TypeReferenceHandle,
    language_semantics::ReferenceAccess,
)> {
    let typed_trees::types::TypeReferenceNode::Reference {
        referee, access, ..
    } = program
        .type_reference_table
        .type_reference(local_data.type_reference)
    else {
        return None;
    };
    let machine = program.machine(machine_symbol)?;
    Some((machine, *referee, *access))
}
