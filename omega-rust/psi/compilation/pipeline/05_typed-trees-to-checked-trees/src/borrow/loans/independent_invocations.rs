//! Establish the descriptor permissions supplied by one independent invocation.
//! A complete public return relation with no input lifetime source permits this
//! invocation namespace; it does not lend caller storage or identify image
//! backing. Bodies still owe return-escape checking and opaque providers still
//! owe admission. Bare references, aggregate fields and projected call results
//! use this same formation; owner paths retain conditional case slots.

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
            call_result: true,
        })
    })
    .collect()
}
