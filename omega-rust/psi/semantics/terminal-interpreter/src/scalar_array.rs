use super::*;

/// Exact row-major primitive payload; the type retains all array dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarArrayValue {
    pub structural_type: StructuralTypeId,
    pub elements: Vec<TerminalScalarValue>,
}

/// An owned unrestricted array returned without claims or qualifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarArrayResult {
    pub value: TerminalScalarArrayValue,
}

impl TerminalExecution {
    /// Copy a complete unrestricted actual into its exact callee parameter.
    /// Opaque host roots and projected/borrowed storage carry no array payload.
    pub(super) fn prepare_scalar_array_argument(
        &self,
        machine: &ExecutableMachine,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let value = self
            .scalar_array_values
            .get(&argument.place)
            .ok_or_else(invalid)?;
        let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
            self.structural_types.values(),
            parameter.structural_type,
        )
        .ok_or_else(invalid)?;
        if parameter.access != StructuralAccess::Owned
            || argument.access != StructuralAccess::Owned
            || !argument.path.is_empty()
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || value.structural_type != parameter.structural_type
            || u64::try_from(value.elements.len()).ok() != Some(count)
            || value
                .elements
                .iter()
                .any(|value| value.scalar_type() != scalar_type)
            || machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == parameter.place)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
            || self.structural_values.contains_key(&argument.place)
            || self.scalar_case_values.contains_key(&argument.place)
        {
            return Err(invalid());
        }
        Ok(value.clone())
    }

    /// Preflight the result and caller slot before paying the edge. None means
    /// the caller frame was restored; Some is exhaustion or entry completion.
    pub(super) fn return_scalar_array(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<Option<TerminalExecutionStatus>, TerminalInterpretError> {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            trivial_affine_discards,
            ..
        } = terminator
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let signature = self
            .machines
            .get(&self.current_machine)
            .and_then(|machine| machine.result.structural())
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let value = self
            .scalar_array_values
            .get(source)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let internal_result = match self.call_stack.last() {
            Some(SuspendedCall {
                structural_values,
                scalar_case_values,
                scalar_array_values,
                live_affine_frontier,
                result:
                    SuspendedCallResult::Structural {
                        result,
                        returned_claim_transfers,
                        ..
                    },
                ..
            }) if result.structural_type == signature.structural_type
                && result.multiplicity == StructuralMultiplicity::Unrestricted
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && returned_claim_transfers.is_empty()
                && !structural_values.contains_key(&result.place)
                && !scalar_case_values.contains_key(&result.place)
                && !scalar_array_values.contains_key(&result.place)
                && live_affine_frontier
                    .iter()
                    .all(|entry| entry.place != result.place) =>
            {
                Some(result.place)
            }
            Some(_) => {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            None => None,
        };
        if value.structural_type != signature.structural_type
            || signature.multiplicity != StructuralMultiplicity::Unrestricted
            || !signature.qualifications.is_empty()
            || !signature.projected_qualifications.is_empty()
            || !returned_claims.is_empty()
            || !self.live_claims.is_empty()
            || trivial_affine_discards.iter().any(|place| {
                *place == *source
                    || (!self.structural_values.contains_key(place)
                        && !self.scalar_case_values.contains_key(place))
            })
            || self.live_affine_frontier.iter().any(|entry| {
                !entry.path.is_empty() || !trivial_affine_discards.contains(&entry.place)
            })
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        // Exhaustion retains the callee payload and caller frame; custody
        // transfers only after the return edge has been paid.
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(Some);
        }
        let value = self
            .scalar_array_values
            .remove(source)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        for place in trivial_affine_discards {
            self.structural_values.remove(place);
            self.scalar_case_values.remove(place);
            remove_affine_root(&mut self.live_affine_frontier, *place);
        }
        if let Some(result_place) = internal_result {
            let caller = self
                .call_stack
                .pop()
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
            self.blocks = caller.blocks;
            self.values = caller.values;
            self.retire_plain_locals();
            self.structural_values = caller.structural_values;
            self.scalar_case_values = caller.scalar_case_values;
            // Caller arrays precede this call and survive alongside its result.
            self.scalar_array_values = caller.scalar_array_values;
            self.scalar_array_values.insert(result_place, value);
            self.byte_sequence_values = caller.byte_sequence_values;
            self.live_affine_frontier = caller.live_affine_frontier;
            self.live_claims = caller.live_claims;
            self.dynamic_parameters = caller.dynamic_parameters;
            self.current_machine = caller.current_machine;
            self.current = caller.current;
            self.next_operation = caller.next_operation;
            return Ok(None);
        }
        let result = TerminalExecutionResult::ScalarArray(TerminalScalarArrayResult { value });
        self.retire_plain_locals();
        self.result = Some(result.clone());
        Ok(Some(TerminalExecutionStatus::Complete(result)))
    }

    pub(super) fn execute_scalar_array_establishment(
        &mut self,
        operation: &terminal_psi::Operation,
        elements: &[ValueId],
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
            self.structural_types.values(),
            result.structural_type,
        )
        .ok_or_else(invalid)?;
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || u64::try_from(elements.len()).ok() != Some(count)
            || self.scalar_array_values.contains_key(&result.place)
            || self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
        {
            return Err(invalid());
        }
        let mut contents = Vec::with_capacity(elements.len());
        for element in elements {
            let value = self
                .values
                .get(element)
                .copied()
                .ok_or(TerminalInterpretError::VerifiedValueMissing(*element))?;
            if value.scalar_type() != scalar_type {
                return Err(invalid());
            }
            contents.push(value);
        }
        self.scalar_array_values.insert(
            result.place,
            TerminalScalarArrayValue {
                structural_type: result.structural_type,
                elements: contents,
            },
        );
        Ok(())
    }
}
