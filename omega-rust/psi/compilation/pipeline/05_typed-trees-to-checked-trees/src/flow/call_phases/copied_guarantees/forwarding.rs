//! Checking a later write can depend on the helper that returned its saved
//! reference. Keep that obligation with the captured supplier before selecting
//! storage coordinates. A lifetime relation does not prove pointer forwarding.

use super::{ArgumentSources, CapturedSource};
use arena::Handle;
use checked_trees::{
    CheckedUnitStructuralPathSegment, FlowControlFacts, FlowOperandRetention,
    FlowReferenceForwarding, FlowReferenceForwardingSource, ViewReturnInputSource,
};
use facts::PlaceSegment;

impl ArgumentSources<'_, '_> {
    pub(super) fn reference_forwarding_dependencies(
        &self,
        control: &FlowControlFacts,
        capture: &CapturedSource,
        path: &[PlaceSegment],
    ) -> Result<Vec<FlowReferenceForwarding>, &'static str> {
        let profiles = validation::completed_storage::captured_retention_profiles_at_path(
            self.program,
            control,
            &capture.operand,
            path,
        )
        .ok_or("the captured reference has no complete retained supplier profile")?;
        let mut requested = Vec::new();
        let mut active = Vec::new();
        for profile in profiles {
            self.profile_forwarding(control, profile, &mut active, &mut requested)?;
        }
        Ok(requested)
    }

    fn profile_forwarding(
        &self,
        control: &FlowControlFacts,
        handle: Handle<FlowOperandRetention>,
        active: &mut Vec<Handle<FlowOperandRetention>>,
        requested: &mut Vec<FlowReferenceForwarding>,
    ) -> Result<(), &'static str> {
        if !control.operand_retentions.is_valid(handle) || active.contains(&handle) {
            return Err("the reference supplier profile is invalid or cyclic");
        }
        active.push(handle);
        let profile = control.operand_retentions.get(handle);
        for alternative in profile.alternatives.handles() {
            self.profile_forwarding(control, alternative, active, requested)?;
        }
        for source in control
            .operand_reborrow_sources
            .span(profile.reborrow_sources)
            .ok_or("the reference reborrow supplier roster is unavailable")?
        {
            if !control.operands.is_valid(source.operand) {
                return Err("the reference reborrow supplier has no recorded capture");
            }
            let path = control
                .operand_retained_paths
                .span(source.source_path)
                .ok_or("the reference reborrow supplier path is unavailable")?;
            let profiles = validation::completed_storage::captured_retention_profiles_at_path(
                self.program,
                control,
                control.operands.get(source.operand),
                path,
            )
            .ok_or("the upstream reborrow reference profile is unavailable")?;
            for profile in profiles {
                self.profile_forwarding(control, profile, active, requested)?;
            }
        }
        let has_descriptor = profile.descriptor.is_valid();
        if has_descriptor {
            if !control.operands.is_valid(profile.descriptor) {
                return Err("the body-proven descriptor capture is unavailable");
            }
            let profiles = validation::completed_storage::captured_retention_profiles_at_path(
                self.program,
                control,
                control.operands.get(profile.descriptor),
                &[],
            )
            .ok_or("the body-proven descriptor profile is unavailable")?;
            for profile in profiles {
                self.profile_forwarding(control, profile, active, requested)?;
            }
        }
        // A body-proven descriptor relation already establishes this edge.
        // Its input values can themselves depend on an earlier opaque output.
        let suppliers = if !profile.descriptor_sources.is_empty() {
            Some(profile.descriptor_sources)
        } else if has_descriptor {
            None
        } else if profile.output_lifetime_sources.is_valid() {
            Some(self.completed_output_suppliers(control, profile.output_lifetime_sources)?)
        } else {
            // A checked helper's parameter output keeps its completed input
            // captures as permission suppliers. Follow those captures for
            // upstream obligations; this roster does not prove the helper's
            // pointer geometry or request a new forwarding promise from it.
            profile
                .lifetime_sources
                .or_else(|| (!profile.suppliers.is_empty()).then_some(profile.suppliers))
        };
        if let Some(suppliers) = suppliers {
            for supplier in control
                .operand_retained_suppliers
                .span(suppliers)
                .ok_or("the reference supplier roster is unavailable")?
            {
                if !control.operands.is_valid(supplier.operand) {
                    return Err("the reference supplier has no recorded operand");
                }
                if profile.descriptor_sources.is_empty()
                    && let Some(contract) = supplier.contract
                {
                    crate::borrow::lifetime_contract::source_type(
                        self.program,
                        self.borrow,
                        control,
                        Some(self.call_scope),
                        supplier,
                    )?;
                    self.request_forwarding(control, &contract, requested)?;
                    if contract.source == ViewReturnInputSource::Stored {
                        let stored = control
                            .operand_stored_sources
                            .span(contract.stored_sources)
                            .ok_or(
                                "the forwarding supplier has no invocation-cut stored frontier",
                            )?;
                        if stored.is_empty() {
                            return Err("the forwarding supplier stored frontier is empty");
                        }
                        for source in stored {
                            let checked_trees::FlowOperandStoredSource::Value {
                                operand,
                                projection,
                                ..
                            } = source
                            else {
                                return Err(
                                    "the forwarding supplier has an unresolved incoming stored value",
                                );
                            };
                            if !control.operands.is_valid(*operand) {
                                return Err(
                                    "the invocation-cut stored supplier capture is unavailable",
                                );
                            }
                            let path = control.operand_retained_paths.span(*projection).ok_or(
                                "the invocation-cut stored supplier projection is unavailable",
                            )?;
                            let profiles =
                                validation::completed_storage::captured_retention_profiles_at_path(
                                    self.program,
                                    control,
                                    control.operands.get(*operand),
                                    path,
                                )
                                .ok_or(
                                    "the invocation-cut stored supplier profile is unavailable",
                                )?;
                            for profile in profiles {
                                self.profile_forwarding(control, profile, active, requested)?;
                            }
                        }
                        continue;
                    }
                }
                let path = control
                    .operand_retained_paths
                    .span(supplier.projection)
                    .ok_or("the reference supplier projection is unavailable")?;
                let profiles = validation::completed_storage::captured_retention_profiles_at_path(
                    self.program,
                    control,
                    control.operands.get(supplier.operand),
                    path,
                )
                .ok_or("an upstream reference supplier profile is unavailable")?;
                for profile in profiles {
                    self.profile_forwarding(control, profile, active, requested)?;
                }
            }
        }
        active.pop();
        Ok(())
    }

    /// The output's captured inputs identify the obligation to request. They
    /// prove no pointer equality; the selected provider still owes that proof.
    fn completed_output_suppliers(
        &self,
        control: &FlowControlFacts,
        handle: Handle<checked_trees::FlowOutputLifetimeSources>,
    ) -> Result<arena::HandleSpan<checked_trees::FlowOperandRetainedSupplier>, &'static str> {
        if !control.output_lifetime_sources.is_valid(handle) {
            return Err("the reference output has no recorded input receipt");
        }
        let receipt = control.output_lifetime_sources.get(handle);
        if !receipt.output.is_valid() {
            return Err("the reference output receipt has no completed occurrence");
        }
        let machine = self
            .program
            .machine_holding_state(receipt.output.state_symbol)
            .ok_or("the reference output receipt has no machine owner")?;
        let call = validation::find_call_site(
            self.program,
            machine.symbol,
            receipt.output.state_symbol,
            receipt.output.statement_index,
            receipt.output.call_ordinal,
        )
        .ok_or("the reference output receipt has no source call")?;
        let parameters = call
            .parameters(self.program, machine.symbol)
            .ok_or("the reference output receipt has no applied formal roster")?;
        let path = control
            .operand_retained_paths
            .span(receipt.owner_path)
            .ok_or("the reference output receipt has no complete owner path")?;
        let suppliers = receipt
            .sources
            .ok_or("the reference output has no complete captured inputs")?;
        let rows = control
            .operand_retained_suppliers
            .span(suppliers)
            .filter(|rows| !rows.is_empty())
            .ok_or("the reference output has no captured input alternatives")?;
        for supplier in rows {
            let contract = supplier
                .contract
                .ok_or("the reference output input has no applied contract")?;
            if contract.state_symbol != receipt.output.state_symbol
                || contract.statement_index != receipt.output.statement_index
                || contract.call_ordinal != receipt.output.call_ordinal
                || contract
                    .output_parameter
                    .and_then(|position| parameters.get(position))
                    .is_none_or(|parameter| parameter.symbol != receipt.output.parameter_symbol)
                || control.operand_retained_paths.span(contract.owner_path) != Some(path)
            {
                return Err("the reference output input names another output occurrence");
            }
        }
        Ok(suppliers)
    }

    fn request_forwarding(
        &self,
        control: &FlowControlFacts,
        contract: &checked_trees::FlowOperandLifetimeContract,
        requested: &mut Vec<FlowReferenceForwarding>,
    ) -> Result<(), &'static str> {
        let output = contract
            .output_parameter
            .ok_or("a returned reference needs its own public forwarding output")?;
        // The current state's FlowStateFact is published only after all of
        // its calls. Rejoin the producer's recorded occurrence through the
        // immutable call owner, rather than requiring that later state row.
        let machine = self
            .program
            .machine_holding_state(contract.state_symbol)
            .ok_or("the reference producer has no declared machine owner")?;
        let call = validation::find_call_site(
            self.program,
            machine.symbol,
            contract.state_symbol,
            contract.statement_index,
            contract.call_ordinal,
        )
        .ok_or("the reference producer has no recorded source occurrence")?;
        let same_invocation = match call {
            validation::CallSite::Expression { expression, .. } => {
                expression == contract.invocation
            }
            validation::CallSite::Statement(_) | validation::CallSite::TransitionNamed { .. } => {
                !contract.invocation.is_valid()
            }
        };
        if !same_invocation {
            return Err("the reference supplier names another source invocation");
        }
        let parameters = call
            .parameters(self.program, machine.symbol)
            .ok_or("the reference producer has no applied formal roster")?;
        let output_type = parameters
            .get(output)
            .ok_or("the reference producer output formal is absent")?
            .type_reference;
        let output_carrier =
            validation::reference_result_custody::input_carrier_type(self.program, output_type)
                .ok_or("the reference output has no stored carrier")?;
        let owner_path = control
            .operand_retained_paths
            .span(contract.owner_path)
            .ok_or("the reference output path is unavailable")?;
        let output_path = stored_path(self.program, output_carrier, owner_path)?;
        let position = u32::try_from(contract.parameter_index)
            .map_err(|_| "the reference supplier position exceeds u32")?;
        let source_path = control
            .operand_retained_paths
            .span(contract.source_path)
            .ok_or("the reference supplier path is unavailable")?;
        let source = match contract.source {
            ViewReturnInputSource::Referent => {
                if !source_path.is_empty() {
                    return Err("a projected borrow is not an unchanged outer reference value");
                }
                FlowReferenceForwardingSource::Reference {
                    argument_position: position,
                }
            }
            ViewReturnInputSource::Stored => {
                let carrier = validation::reference_result_custody::input_carrier_type(
                    self.program,
                    contract.source_type,
                )
                .ok_or("the stored reference supplier has no readable carrier")?;
                FlowReferenceForwardingSource::Stored {
                    argument_position: position,
                    path: stored_path(self.program, carrier, source_path)?,
                }
            }
        };
        let output_parameter =
            u32::try_from(output).map_err(|_| "the reference output position exceeds u32")?;
        if let Some(row) = requested.iter_mut().find(|row| {
            row.state_symbol == contract.state_symbol
                && row.statement_index == contract.statement_index
                && row.invocation == contract.invocation
                && row.call_ordinal == contract.call_ordinal
                && row.output_parameter == output_parameter
                && row.output_path == output_path
        }) {
            if !row.sources.contains(&source) {
                row.sources.push(source);
            }
        } else {
            requested.push(FlowReferenceForwarding {
                state_symbol: contract.state_symbol,
                statement_index: contract.statement_index,
                invocation: contract.invocation,
                call_ordinal: contract.call_ordinal,
                output_parameter,
                output_path,
                sources: vec![source],
            });
        }
        Ok(())
    }
}

fn stored_path(
    program: &dyn validation::CheckingInput,
    mut carrier: typed_trees::types::TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Result<Vec<CheckedUnitStructuralPathSegment>, &'static str> {
    let mut path = Vec::new();
    let mut consumed = 0;
    while consumed < segments.len() {
        while let Some(shell) = validation::reference_shell(program, carrier) {
            path.push(CheckedUnitStructuralPathSegment::Referent);
            carrier = shell.referee;
        }
        let normalized =
            validation::corresponding_place_projection(program, carrier, &segments[consumed..])
                .ok_or("the stored forwarding path has no applied member correspondence")?;
        let length = (1..=normalized.len())
            .find(|length| {
                validation::project_type_reference_from_segments(
                    program,
                    carrier,
                    &normalized[..*length],
                )
                .is_some_and(|selected| validation::reference_shell(program, selected).is_some())
            })
            .unwrap_or(normalized.len());
        if length == 0 {
            return Err("the stored forwarding path made no progress");
        }
        let (selected_path, selected) =
            validation::structural_storage_path(program, carrier, &normalized[..length])?;
        path.extend(selected_path);
        carrier = selected;
        consumed += length;
    }
    if validation::reference_shell(program, carrier).is_none() {
        return Err("the forwarding choice is not a stored reference value");
    }
    Ok(path)
}
