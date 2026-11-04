//! A returned saved pointer still comes from its earlier read, even when the
//! input field has since changed. Record that original source for the result.
//! The lifetime contract limits where the pointer can be returned; it does not
//! choose the pointer's address.

use super::{Captures, is_mutable_structural_parameter, reference_access_can_supply};
use checked_trees::{
    BorrowReturnedLoanFact, CheckedReferenceOriginProjection, CheckedReferenceOriginRoot,
    CheckedReferenceOriginSourcePlan, CheckedReferenceResultSourcePlan,
    CheckedUnitStructuralPathSegment,
};
use facts::PlaceSegment;

impl Captures<'_> {
    pub(super) fn loaded_descriptor_origins(
        &self,
        state: &typed_trees::state::State,
        loan: &BorrowReturnedLoanFact,
        leaf: &CheckedReferenceResultSourcePlan,
        segments: &[PlaceSegment],
    ) -> Option<Vec<CheckedReferenceOriginSourcePlan>> {
        let result = self.loaded_descriptor_origins_inner(state, loan, leaf, segments);
        if let Err(reason) = &result
            && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
        {
            eprintln!(
                "CHECKSITE returned-descriptor-load state={:?} identity={:?} formation={:?} result_path={:?} refusal={reason}",
                state.symbol, loan.incoming_reference, loan.formation_expression, leaf.path,
            );
        }
        result.ok()
    }

    fn loaded_descriptor_origins_inner(
        &self,
        state: &typed_trees::state::State,
        loan: &BorrowReturnedLoanFact,
        leaf: &CheckedReferenceResultSourcePlan,
        segments: &[PlaceSegment],
    ) -> Result<Vec<CheckedReferenceOriginSourcePlan>, &'static str> {
        let flow = self
            .flow(state.symbol)
            .ok_or("the returned descriptor has no exact state flow")?;
        let loaded = super::super::completed_descriptor_load(
            self.program,
            self.facts,
            flow,
            loan.incoming_reference,
        )?;
        let incoming = self
            .facts
            .borrow
            .incoming_references
            .get(loan.incoming_reference);
        if loan.root != incoming.place_root() {
            return Err("the returned pointer differs from its completed load namespace");
        }
        let capture = self.facts.flow.control.operands.get(loaded.capture);
        let result_path = super::loan_place_segments(self.program, state.return_type, &leaf.path)
            .ok_or("the returned descriptor has no exact result leaf")?;
        let mut sources = vec![loaded.sources];
        let mut output = Vec::new();
        while let Some(source) = sources.pop() {
            let slot = match source {
                crate::completed_storage::StoredOperand::Incoming(slot) => slot,
                crate::completed_storage::StoredOperand::Alternatives(alternatives)
                    if !alternatives.is_empty() =>
                {
                    sources.extend(alternatives);
                    continue;
                }
                _ => {
                    return Err(
                        "the saved pointer has an unavailable or non-entry source alternative",
                    );
                }
            };
            let (parameter_index, parameter) = self
                .program
                .state_parameters(state)
                .iter()
                .filter(|parameter| is_mutable_structural_parameter(self.program, parameter))
                .enumerate()
                .find(|(_, parameter)| slot.root == facts::PlaceRoot::Symbol(parameter.symbol))
                .ok_or("the saved pointer's source is not an exact structural input")?;
            let carrier = super::super::input_carrier_type(self.program, parameter.type_reference)
                .ok_or("the saved pointer's input has no readable carrier")?;
            let (mut path, descriptor_type, slot_extent) =
                super::super::reference_origin_path(self.program, carrier, &slot.segments)
                    .ok_or("the saved pointer's source slot has no exact typed path")?;
            if slot_extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                || !crate::storage_origin_types_match(
                    self.program,
                    descriptor_type,
                    capture.type_reference,
                )
            {
                return Err("the saved pointer's source slot differs from its loaded descriptor");
            }
            let descriptors =
                super::super::declared_carrier_references(self.program, descriptor_type)
                    .ok_or("the saved pointer's source slot has no complete descriptor shape")?;
            let [descriptor] = descriptors.as_slice() else {
                return Err("the saved pointer's source slot does not select one descriptor");
            };
            if !descriptor.path.is_empty()
                || !reference_access_can_supply(descriptor.access, leaf.source.access)
            {
                return Err("the saved pointer's descriptor changes its source path or access");
            }
            let selected = if descriptor.crosses_reference {
                path.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
                super::stored_loan_parts(self.program, descriptor_type)
                    .ok_or("the saved pointer has no declared referent")?
                    .0
            } else {
                descriptor_type
            };
            let (suffix, endpoint, extent) =
                super::super::reference_origin_path(self.program, selected, segments)
                    .ok_or("the saved pointer cannot project its captured referent")?;
            let extent = self
                .result_origin_extent(
                    endpoint,
                    state.return_type,
                    &result_path,
                    leaf.source.type_identity,
                    extent,
                )
                .ok_or("the saved pointer's captured referent does not supply the result")?;
            path.extend(suffix);
            let source = CheckedReferenceOriginSourcePlan {
                root: CheckedReferenceOriginRoot::Parameter {
                    index: u32::try_from(parameter_index)
                        .map_err(|_| "the saved pointer's input ordinal exceeds u32")?,
                },
                entry_reference_hops: 0,
                path,
                extent,
                type_identity: self.program.normalized_type_identity(endpoint),
                access: leaf.source.access,
            };
            if !output.contains(&source) {
                output.push(source);
            }
            if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                return Err("the saved pointer exceeds the returned-origin roster limit");
            }
        }
        if output.is_empty() {
            return Err("the saved pointer has no complete input origin alternatives");
        }
        Ok(output)
    }
}
