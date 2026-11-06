//! A completed call supplies permission to use each returned reference.
//! Its input lifetimes are retained separately; these rows do not claim that
//! the pointer selects an input's storage. Checked bodies still owe return
//! checking, and opaque providers still owe admission of their safe contract.
//! Bare references and reference fields share the same invocation formation.

use super::{PendingIncomingReference, StatementBorrowLoan};
use checked_trees::expression::ExpressionHandle;
use checked_trees::statement::TableLocalData;
use checked_trees::{
    BorrowLoanLineage, CapturedPlace, CheckedUnitStructuralArgumentSourcePlan,
    IncomingReferenceIdentity, IncomingReferenceOrigin,
};
use facts::PlaceRoot;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;
use validation::reference_result_custody::{
    loan_owner_path, loan_place_segments, record_carrier_sources,
};

pub(super) fn form(
    program: &dyn validation::CheckingInput,
    state_symbol: SymbolHandle,
    machine_symbol: SymbolHandle,
    local: &TableLocalData,
    expression: ExpressionHandle,
    target: SymbolHandle,
    result_type: TypeReferenceHandle,
) -> Option<Vec<StatementBorrowLoan>> {
    record_carrier_sources(
        program,
        result_type,
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: local.symbol,
        },
    )?
    .into_iter()
    .map(|leaf| {
        let owner_path = loan_owner_path(program, result_type, &leaf.path)?;
        let segments = loan_place_segments(program, result_type, &leaf.path)?;
        let carrier_type =
            validation::project_type_reference_from_segments(program, result_type, &segments)?;
        Some(StatementBorrowLoan {
            formation_owner_path: Some(owner_path.clone()),
            incoming_origin: Some(PendingIncomingReference {
                identity: IncomingReferenceIdentity {
                    machine_symbol,
                    state_symbol,
                    origin: IncomingReferenceOrigin::Invocation { expression, target },
                    carrier_type,
                    owner_path: Default::default(),
                },
                // Different descriptors of the same invocation retain distinct
                // supplied identities even when their carrier types coincide.
                owner_path: owner_path.clone(),
            }),
            formation_expression: expression,
            representation_address: None,
            owner_symbol: local.symbol,
            storage_owner: PlaceRoot::Symbol(local.symbol),
            owner_name: local.name.clone(),
            owner_path,
            place: CapturedPlace {
                incoming_reference: Default::default(),
                incoming_owner_path: Default::default(),
                root: PlaceRoot::Expression(expression),
                segments: Vec::new(),
            },
            retained_operand: None,
            source_owner_symbol: SymbolHandle::invalid(),
            supplying_loans: Vec::new(),
            lineage: BorrowLoanLineage::DirectRoot,
            kind: leaf.source.access.borrow_access_kind()?,
            formation: super::BorrowFormation::ReturnedDescriptor,
        })
    })
    .collect()
}

/// Publish one opaque call's returned value at its actual normal completion.
/// Binding and direct forwarding then retain the same descriptor cohort.
#[allow(clippy::too_many_arguments)]
pub(in crate::borrow) fn complete(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    statement_entry: &[super::super::tracker::StateLoanTracker],
    completed_values: &[super::super::tracker::StateLoanTracker],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Result<Vec<StatementBorrowLoan>, &'static str> {
    let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(expression)
    else {
        return Ok(Vec::new());
    };
    if validation::call_target_has_checked_body_contract(
        program,
        validation::call_target_symbol(program, call),
    ) {
        return Ok(Vec::new());
    }
    let contract = super::call_view_return_contract(program, state.symbol, call)
        .ok_or("completed call result has no applied contract")?;
    let result = contract.return_type_reference(program);
    if !super::types::is_reference_type(program, result)
        && !crate::borrow::view_link::is_borrow_carrying_data(program, result)
    {
        return Ok(Vec::new());
    }
    let value = checked_trees::statement::TableLocalData {
        role: language_core::LocalBindingRole::Value,
        symbol: state.symbol,
        name: state.name.clone(),
        type_reference: result,
        initial_value: expression,
        is_mutable: false,
        type_is_inferred: false,
        is_operand_capture: false,
        relevance: language_core::BindingRelevance::Relevant,
        reflected_query_origin: None,
    };
    // Replay mutable slot effects from statement entry exactly once. Result
    // homes are immutable evaluated values, so earlier completions can join
    // this roster without retargeting an earlier argument's saved pointer.
    let mut incoming = statement_entry
        .iter()
        .filter(|source| !matches!(source.storage_owner, PlaceRoot::Expression(_)))
        .cloned()
        .collect::<Vec<_>>();
    incoming.extend(
        completed_values
            .iter()
            .filter(|source| matches!(source.storage_owner, PlaceRoot::Expression(_)))
            .cloned(),
    );
    let sources = super::BorrowFormationSources {
        statement_entry: &incoming,
        call_frames: frames,
    };
    let mut loans = super::returned_carriers::relative_result_loans(
        program,
        state.symbol,
        statement_index,
        machine.symbol,
        &value,
        expression,
        &incoming,
        &sources,
    )?
    .ok_or("completed call result has no complete reference cohort")?;
    for loan in &mut loans {
        loan.storage_owner = PlaceRoot::Expression(expression);
        loan.formation_expression = expression;
    }
    Ok(loans)
}

/// A call-result home retains the performed invocation that owns it. Stored
/// output slots use their separate replacement relation.
pub(crate) fn is_completed_value(
    borrow: &checked_trees::BorrowFacts,
    loan: &checked_trees::BorrowLoanFact,
) -> bool {
    matches!(loan.storage_owner, PlaceRoot::Expression(expression)
        if expression == loan.formation_expression
            && borrow.calls.is_valid(loan.established_by_call)
            && borrow.calls.get(loan.established_by_call).authored_expression == expression)
}

/// Saving an evaluated carrier transports every descriptor and input debt.
/// It adds no new permission, and lifetime debts keep their original access.
pub(super) fn retain(
    program: &dyn validation::CheckingInput,
    local: &TableLocalData,
    current: &[super::super::tracker::StateLoanTracker],
) -> Result<Option<Vec<StatementBorrowLoan>>, &'static str> {
    if !super::types::is_reference_type(program, local.type_reference)
        && !crate::borrow::view_link::is_borrow_carrying_data(program, local.type_reference)
    {
        return Ok(None);
    }
    let cohort = current
        .iter()
        .filter(|source| source.storage_owner == PlaceRoot::Expression(local.initial_value))
        .collect::<Vec<_>>();
    if !cohort
        .iter()
        .any(|source| source.lineage != BorrowLoanLineage::LifetimeDependency)
    {
        return Ok(None);
    }
    cohort
        .into_iter()
        .map(|source| {
            if !source.handle.is_valid() {
                return Err("saved descriptor has no established permission occurrence");
            }
            // A directly produced leaf uses its formation coordinate. Once
            // wrapped in another carrier, its current owner path includes the
            // wrapper and is the receiving value's coordinate instead.
            let owner_path = if source.formation_expression == local.initial_value {
                source
                    .formation_owner_path
                    .as_ref()
                    .unwrap_or(&source.owner_path)
            } else {
                &source.owner_path
            };
            let kind = if source.lineage == BorrowLoanLineage::LifetimeDependency {
                source.kind.clone()
            } else {
                let path = super::owner_place_segments(owner_path);
                let reference = validation::project_type_reference_from_segments(
                    program,
                    local.type_reference,
                    &path,
                )
                .ok_or("saved descriptor has no declared receiving leaf")?;
                let kind = super::types::reference_borrow_access_kind(program, reference)
                    .ok_or("saved descriptor's receiving leaf is not a reference")?;
                source
                    .kind
                    .direct_reborrow_effect(&kind)
                    .ok_or("saved descriptor exceeds its completed source access")?;
                kind
            };
            Ok(StatementBorrowLoan {
                incoming_origin: None,
                formation_expression: local.initial_value,
                formation_owner_path: Some(owner_path.clone()),
                owner_symbol: local.symbol,
                storage_owner: PlaceRoot::Symbol(local.symbol),
                owner_name: local.name.clone(),
                owner_path: owner_path.clone(),
                place: source.place.clone(),
                representation_address: source.representation_address.clone(),
                retained_operand: source.retained_operand,
                source_owner_symbol: source.owner_symbol,
                supplying_loans: vec![source.handle],
                lineage: if source.lineage == BorrowLoanLineage::LifetimeDependency {
                    BorrowLoanLineage::LifetimeDependency
                } else {
                    BorrowLoanLineage::UnretainedDerived
                },
                kind,
                formation: super::BorrowFormation::Ordinary,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}
