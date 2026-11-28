//! Rebuild an activation outcome from the checked case and the actual values in custody.
use super::{ParkShape, ReplyClaims, TransferredReply};
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::{
    StructuralByteSequenceRuntimeField, StructuralFieldNamespace, StructuralRuntimePlace,
    StructuralScalarRuntimeField, TerminalStructuralValue,
};
use crate::engine::execution::{CompletedReturn, TerminalExecution};
use crate::storage::byte_sequences::binding::ByteSequenceBinding;
use crate::storage::element_views::ElementView;
use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
use crate::storage::record::storage::{ConstructorStorage, ConstructorValue};
use crate::storage::structural_storage::{HostStructuralPayload, RetainedStorage};
use crate::{TerminalExecutionResult, TerminalScalarValue};
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralOperationResult, StructuralPathQualification,
    StructuralPathSegment, StructuralTypeShape,
};
use terminal_semantics::calls::owned_activation::{PreparationShape, ResumeShape, StepShape};

type Result<T> = std::result::Result<T, TerminalInterpretError>;

pub(crate) enum RetainedDescriptor {
    Bytes(ByteSequenceBinding),
    Elements(ElementView),
}

/// The complete original loan, including the storage owner and slice bounds.
pub(crate) struct RetainedBacking {
    value: TerminalStructuralValue,
    descriptor: RetainedDescriptor,
    length: u64,
    claims: Vec<Vec<StructuralPathSegment>>,
}

impl RetainedBacking {
    pub(crate) fn capture(
        execution: &mut TerminalExecution<'_>,
        argument: &StructuralArgument,
    ) -> Result<Self> {
        let value =
            execution.resolve_live_projection(argument.place, &argument.path, argument.access)?;
        let (descriptor, length) = match &execution
            .structural_declaration(value.structural_type)?
            .shape
        {
            StructuralTypeShape::ElementView { .. } => {
                let view = execution.element_view_argument(argument, &value)?;
                if view.access() != StructuralAccess::MutableBorrow {
                    return Err(malformed());
                }
                (
                    RetainedDescriptor::Elements(view.clone()),
                    u64::try_from(view.len()).map_err(|_| malformed())?,
                )
            }
            StructuralTypeShape::ByteSequence(_) => {
                let binding = execution.byte_descriptor_binding(argument, &value)?;
                if binding.access() != StructuralAccess::MutableBorrow {
                    return Err(malformed());
                }
                let length = execution.byte_descriptor_length(&binding, &value)?;
                (RetainedDescriptor::Bytes(binding), length)
            }
            _ => return Err(malformed()),
        };
        Ok(Self {
            value,
            descriptor,
            length,
            claims: crate::engine::custody::projected_ownership_claims(
                &execution.live_claims,
                argument,
            )?
            .into_iter()
            .map(|(_, claim)| claim.path)
            .collect(),
        })
    }

    pub(crate) fn binding(&self) -> &RetainedDescriptor {
        &self.descriptor
    }
    pub(crate) fn len(&self) -> u64 {
        self.length
    }
    /// A byte slice alone guarantees byte alignment.
    pub(crate) fn alignment(&self) -> u64 {
        1
    }

    pub(crate) fn retain_storage(&self, retained: &mut RetainedStorage) {
        retained.value(&self.value);
        match &self.descriptor {
            RetainedDescriptor::Bytes(binding) => retained.byte_binding(binding),
            RetainedDescriptor::Elements(view) => view.retain_storage(retained),
        }
    }
}

impl TerminalExecution<'_> {
    pub(crate) fn activation_token(&mut self, input: &StructuralArgument) -> Result<u64> {
        self.require_owned_value_claims(input)?;
        let value = self.resolve_live_projection(input.place, &input.path, input.access)?;
        match self
            .structural_host_atoms
            .get(&StructuralRuntimePlace::from(&value))
        {
            Some(HostStructuralPayload::Activation(token))
                if input.access == StructuralAccess::Owned =>
            {
                Ok(*token)
            }
            _ => Err(malformed()),
        }
    }

    /// Close precisely this operand's custody. Other operation arguments can have
    /// moved into a worker frame, or still belong to the caller after a mismatch.
    pub(crate) fn consume_activation_input(
        &mut self,
        op: &Operation,
        input: &StructuralArgument,
    ) -> Result<()> {
        self.prepare_activation_consumption(op, std::slice::from_ref(input))?
            .commit(self)
    }

    fn prepare_activation_consumption(
        &self,
        op: &Operation,
        inputs: &[StructuralArgument],
    ) -> Result<crate::storage::record::operands::PreparedConstructorOperands> {
        let OperationKind::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = &op.kind
        else {
            return Err(malformed());
        };
        let mut receipts = Vec::new();
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        for (index, input) in inputs.iter().enumerate() {
            let position = structural_arguments
                .iter()
                .position(|arg| arg == input)
                .ok_or(malformed())?;
            for receipt in completion_receipts
                .iter()
                .filter(|r| r.argument_index as usize == position)
            {
                let mut local = receipt.clone();
                local.argument_index = u32::try_from(index).map_err(|_| malformed())?;
                receipts.push(local);
            }
            if input.access == StructuralAccess::Owned {
                self.stage_constructor_operand(input, &mut frontier, &mut consumed)?;
            }
        }
        let claims = crate::engine::custody::complete_claims(&self.live_claims, inputs, &receipts)?;
        self.prepare_constructor_operands(frontier, &consumed, claims)
    }
}

struct ActivationCase {
    frontier: std::collections::BTreeSet<terminal_psi::StructuralAffineDiscard>,
    consumed: Vec<StructuralArgument>,
    value: TerminalStructuralValue,
    storage: ConstructorStorage,
    case: StructuralCaseId,
    claim_paths: Vec<Vec<StructuralPathSegment>>,
}

impl ActivationCase {
    fn new(
        exec: &mut TerminalExecution<'_>,
        result: &StructuralOperationResult,
        case: StructuralCaseId,
    ) -> Result<Self> {
        if exec.structural_values.contains_key(&result.place)
            || exec.scalar_array_values.contains_key(&result.place)
        {
            return Err(malformed());
        }
        let StructuralTypeShape::Sum { cases } =
            &exec.structural_declaration(result.structural_type)?.shape
        else {
            return Err(malformed());
        };
        terminal_psi::find_structural_case(cases, case).ok_or(malformed())?;
        let value = TerminalStructuralValue::minted(
            exec.local_structural_identities.allocate()?,
            result.structural_type,
            result.qualifications.clone(),
        );
        let mut storage = ConstructorStorage::new(exec.account.as_ref());
        storage.stage_case(
            StructuralRuntimePlace::from(&value),
            result.structural_type,
            case,
        )?;
        Ok(Self {
            value,
            storage,
            case,
            frontier: exec.live_affine_frontier.clone(),
            consumed: Vec::new(),
            claim_paths: Vec::new(),
        })
    }

    fn field(
        &self,
        exec: &TerminalExecution<'_>,
        id: StructuralFieldId,
    ) -> Result<StructuralFieldDeclaration> {
        let StructuralTypeShape::Sum { cases } = &exec
            .structural_declaration(self.value.structural_type)?
            .shape
        else {
            return Err(malformed());
        };
        terminal_psi::find_structural_case(cases, self.case)
            .and_then(|case| case.fields.iter().find(|f| f.id == id))
            .cloned()
            .ok_or(malformed())
    }

    fn member(&self, field: &StructuralFieldDeclaration) -> StructuralRuntimePlace {
        StructuralRuntimePlace {
            opaque_identity: self.value.opaque_identity,
            path: vec![
                StructuralPathSegment::Case(self.case),
                StructuralPathSegment::Field(field.identity),
            ],
        }
    }

    fn scalar(
        &mut self,
        exec: &TerminalExecution<'_>,
        id: StructuralFieldId,
        scalar: TerminalScalarValue,
    ) -> Result<()> {
        let field = self.field(exec, id)?;
        if field.field_type.scalar_type() != Some(scalar.scalar_type())
            || !scalar.satisfies_field(&field.field_type)
        {
            return Err(malformed());
        }
        self.storage.stage_scalar_endpoint(
            &PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField {
                parent: StructuralRuntimePlace::from(&self.value),
                namespace: StructuralFieldNamespace::CasePayload,
                field: id,
            }),
            scalar,
        )
    }

    fn source(
        &mut self,
        exec: &TerminalExecution<'_>,
        id: StructuralFieldId,
        arg: &StructuralArgument,
    ) -> Result<()> {
        let field = self.field(exec, id)?;
        let member = self.member(&field);
        let child = ConstructorValue::resolve(exec, arg)?;
        let declared = exec.structural_declaration(child.structural_type())?;
        if field.field_type.as_structural() != Some(child.structural_type())
            && field.field_type.canonical_leaf_shape().as_ref() != Some(&declared.shape)
        {
            return Err(malformed());
        }
        child.append_memberships(&member.path, &mut self.value.projected_qualifications);
        for (_, claim) in
            crate::engine::custody::projected_ownership_claims(&exec.live_claims, arg)?
        {
            let mut path = member.path.clone();
            path.extend(claim.path);
            self.claim_paths.push(path);
        }
        if let Some(bytes) = exec.prepare_constructor_byte_field(
            &field,
            arg,
            &child,
            &mut self.frontier,
            &mut self.consumed,
        )? {
            return self.storage.stage_byte_field(
                StructuralByteSequenceRuntimeField {
                    parent: StructuralRuntimePlace::from(&self.value),
                    namespace: StructuralFieldNamespace::CasePayload,
                    field: id,
                },
                bytes,
            );
        }
        let moving = exec.stage_constructor_operand(arg, &mut self.frontier, &mut self.consumed)?;
        if let Some(scalar) = field.field_type.scalar_type() {
            let ConstructorValue::Access(ref value) = child else {
                return Err(malformed());
            };
            return self.scalar(
                exec,
                id,
                exec.read_primitive_argument(arg, value, &[], scalar)?,
            );
        }
        self.storage.stage(exec, arg, &child, &member, moving)
    }

    fn backing(
        &mut self,
        exec: &TerminalExecution<'_>,
        id: StructuralFieldId,
        backing: &RetainedBacking,
    ) -> Result<()> {
        let field = self.field(exec, id)?;
        let home = self.member(&field);
        let matches_type = field.field_type.as_structural() == Some(backing.value.structural_type)
            || field.field_type.canonical_leaf_shape().as_ref()
                == Some(
                    &exec
                        .structural_declaration(backing.value.structural_type)?
                        .shape,
                );
        if !matches_type {
            return Err(malformed());
        }
        for suffix in &backing.claims {
            let mut path = home.path.clone();
            path.extend_from_slice(suffix);
            self.claim_paths.push(path);
        }
        let mut value = backing.value.clone();
        value.opaque_identity = home.opaque_identity;
        value.path.clone_from(&home.path);
        StructuralPathQualification::append_under(
            &value.qualifications,
            &value.projected_qualifications,
            &home.path,
            &mut self.value.projected_qualifications,
        );
        match &backing.descriptor {
            RetainedDescriptor::Bytes(binding)
                if field.field_type.byte_sequence_carrier().is_some() =>
            {
                self.storage.stage_byte_field(
                    StructuralByteSequenceRuntimeField {
                        parent: StructuralRuntimePlace::from(&self.value),
                        namespace: StructuralFieldNamespace::CasePayload,
                        field: id,
                    },
                    binding.clone().with_descriptor_home(value),
                )
            }
            RetainedDescriptor::Bytes(binding) => self
                .storage
                .stage_byte_descriptor(home, binding.clone().with_descriptor_home(value)),
            RetainedDescriptor::Elements(view) => self
                .storage
                .stage_element_view(home, view.clone().with_descriptor_home(value)),
        }
    }

    fn publish(
        mut self,
        exec: &mut TerminalExecution<'_>,
        op: &Operation,
        remaining_inputs: &[&StructuralArgument],
    ) -> Result<()> {
        let result = op.result.as_structural().ok_or(malformed())?;
        self.value.projected_qualifications.sort();
        self.value.projected_qualifications.dedup();
        self.storage.validate_projected_memberships(
            exec,
            &self.value,
            &result.projected_qualifications,
        )?;
        let membership = |region: &terminal_psi::StructuralAffineDiscard| {
            self.storage
                .case_at_projection(exec, &self.value, &region.path)
        };
        let mut active = Vec::new();
        for binding in &result.claims {
            if exec.constructor_claim_is_active(result, &binding.path, &membership)? {
                active.push(binding);
            }
        }
        let mut required: Vec<_> = active.iter().map(|binding| binding.path.clone()).collect();
        required.sort();
        self.claim_paths.sort();
        if required != self.claim_paths {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        for &argument in remaining_inputs {
            if argument.access == StructuralAccess::Owned && !self.consumed.contains(argument) {
                exec.stage_constructor_operand(argument, &mut self.frontier, &mut self.consumed)?;
            }
        }
        exec.stage_result_affine_frontier(result, &mut self.frontier, &membership)?;
        let mut claims = if remaining_inputs.is_empty() {
            exec.live_claims.clone()
        } else {
            let OperationKind::BoundaryCall {
                structural_arguments,
                completion_receipts,
                ..
            } = &op.kind
            else {
                return Err(malformed());
            };
            if !structural_arguments
                .iter()
                .eq(remaining_inputs.iter().copied())
            {
                return Err(malformed());
            }
            crate::engine::custody::complete_claims(
                &exec.live_claims,
                structural_arguments,
                completion_receipts,
            )?
        };
        for binding in active {
            crate::engine::custody::bind_result_claim(&mut claims, binding, result)?;
        }
        let retirement =
            exec.prepare_constructor_operands(self.frontier, &self.consumed, claims)?;
        let loans = exec.loan_completions_for_operation(op.id)?;
        self.storage.publish(exec)?;
        exec.structural_values.insert(result.place, self.value)?;
        retirement.commit(exec)?;
        exec.complete_loan_custody(&loans)
    }
}

impl TerminalExecution<'_> {
    pub(crate) fn issue_activation_owner(
        &mut self,
        op: &Operation,
        case: StructuralCaseId,
        field: StructuralFieldId,
        token: u64,
    ) -> Result<()> {
        let result = op.result.as_structural().ok_or(malformed())?;
        let mut output = ActivationCase::new(self, result, case)?;
        let declaration = output.field(self, field)?;
        let owner = declaration.field_type.as_structural().ok_or(malformed())?;
        if !self.structural_declaration(owner)?.owns_linear_claim {
            return Err(malformed());
        }
        let member = output.member(&declaration);
        output.claim_paths.push(member.path.clone());
        output.storage.stage_activation_owner(member, token)?;
        output.publish(self, op, &[])
    }

    pub(crate) fn return_preparation_inputs(
        &mut self,
        op: &Operation,
        shape: &PreparationShape,
    ) -> Result<()> {
        let OperationKind::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = &op.kind
        else {
            return Err(malformed());
        };
        let (retained, structures) = structural_arguments.split_last().ok_or(malformed())?;
        let backing = RetainedBacking::capture(self, retained)?;
        let mut output = ActivationCase::new(
            self,
            op.result.as_structural().ok_or(malformed())?,
            shape.rejected,
        )?;
        match (arguments.as_slice(), structures) {
            ([arg], []) => output.scalar(self, shape.arguments, self.scalar_value(*arg)?)?,
            ([], [arg]) => output.source(self, shape.arguments, arg)?,
            _ => return Err(malformed()),
        }
        output.backing(self, shape.retained, &backing)?;
        output.publish(self, op, &structural_arguments.iter().collect::<Vec<_>>())
    }
}

impl TerminalExecution<'_> {
    pub(crate) fn complete_activation_output(
        &mut self,
        op: &Operation,
        shape: &StepShape,
        backing: RetainedBacking,
        completed: CompletedReturn,
    ) -> Result<()> {
        let result = op.result.as_structural().ok_or(malformed())?;
        let mut output = ActivationCase::new(self, result, shape.returned)?;
        output.backing(self, shape.retained, &backing)?;
        let field = output.field(self, shape.value)?;
        let member = output.member(&field);
        if completed.destination.is_some() || completed.rebound_claims.is_some() {
            return Err(malformed());
        }
        let mut cursor = self.local_structural_identities.cursor();
        if !matches!(&completed.value, TerminalExecutionResult::Structural(_))
            && (!completed.returned_claims.is_empty()
                || completed.byte_sequence.is_some()
                || completed.element_view.is_some())
        {
            return Err(malformed());
        }
        match completed.value {
            TerminalExecutionResult::Unit => {
                let valid = matches!(field.field_type, StructuralFieldType::Erased { scalar:None,.. })
                    || field.field_type.as_structural().is_some_and(|ty| self.structural_declaration(ty).is_ok_and(|d| matches!(&d.shape,StructuralTypeShape::Record { fields,.. } if fields.is_empty())));
                if !valid {
                    return Err(malformed());
                }
            }
            TerminalExecutionResult::Scalar(scalar) => output.scalar(self, shape.value, scalar)?,
            TerminalExecutionResult::ScalarArray(returned) => {
                if field.field_type.as_structural() != Some(returned.value.structural_type) {
                    return Err(malformed());
                }
                output.storage.stage_array(
                    self,
                    returned.value.structural_type,
                    returned.value.elements(),
                    &member,
                )?;
                StructuralPathQualification::append_under(
                    &returned.value.qualifications,
                    &[],
                    &member.path,
                    &mut output.value.projected_qualifications,
                );
            }
            TerminalExecutionResult::ScalarCase(returned) => {
                let returned = returned.value();
                if field.field_type.as_structural() != Some(returned.structural_type) {
                    return Err(malformed());
                }
                let mut child = TerminalStructuralValue::minted(
                    member.opaque_identity,
                    returned.structural_type,
                    vec![],
                );
                child.path.clone_from(&member.path);
                if let Some(case) = returned.result_case {
                    output
                        .storage
                        .stage_case(member.clone(), returned.structural_type, case)?;
                }
                self.stage_structural_result_contents(
                    result.place,
                    &child,
                    &returned.canonical_contents(),
                    &mut output.storage,
                    &mut cursor,
                )?;
            }
            TerminalExecutionResult::Structural(returned) => {
                if !returned.claims.iter().copied().eq(completed
                    .returned_claims
                    .iter()
                    .map(|binding| binding.claim))
                {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                let child = returned.observation_value();
                let declared = self.structural_declaration(child.structural_type)?;
                if field.field_type.as_structural() != Some(child.structural_type)
                    && field.field_type.canonical_leaf_shape().as_ref() != Some(&declared.shape)
                {
                    return Err(malformed());
                }
                let mut relocated = child.as_ref().clone();
                relocated.opaque_identity = member.opaque_identity;
                relocated.path.clone_from(&member.path);
                StructuralPathQualification::append_under(
                    &child.qualifications,
                    &child.projected_qualifications,
                    &member.path,
                    &mut output.value.projected_qualifications,
                );
                for binding in &completed.returned_claims {
                    let mut path = member.path.clone();
                    path.extend_from_slice(&binding.path);
                    output.claim_paths.push(path);
                }
                match (completed.byte_sequence, completed.element_view) {
                    (Some(binding), None) if field.field_type.byte_sequence_carrier().is_some() => {
                        output.storage.stage_byte_field(
                            StructuralByteSequenceRuntimeField {
                                parent: StructuralRuntimePlace::from(&output.value),
                                namespace: StructuralFieldNamespace::CasePayload,
                                field: shape.value,
                            },
                            binding.with_descriptor_home(relocated),
                        )?
                    }
                    (Some(binding), None) => output
                        .storage
                        .stage_byte_descriptor(member, binding.with_descriptor_home(relocated))?,
                    (None, Some(view)) => output
                        .storage
                        .stage_element_view(member, view.with_descriptor_home(relocated))?,
                    (None, None) => output.storage.stage_subtree(
                        self,
                        &StructuralRuntimePlace::from(child.as_ref()),
                        &member,
                        true,
                    )?,
                    (Some(_), Some(_)) => return Err(malformed()),
                }
            }
        }
        output.publish(self, op, &[])?;
        self.local_structural_identities.commit_cursor(cursor);
        Ok(())
    }

    pub(crate) fn return_mismatched_reply(
        &mut self,
        op: &Operation,
        shape: &ResumeShape,
    ) -> Result<()> {
        let OperationKind::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = &op.kind
        else {
            return Err(malformed());
        };
        let [owner, permit, rest @ ..] = structural_arguments.as_slice() else {
            return Err(malformed());
        };
        let mut output = ActivationCase::new(
            self,
            op.result.as_structural().ok_or(malformed())?,
            shape.mismatch,
        )?;
        output.source(self, shape.mismatch_owner, owner)?;
        output.source(self, shape.mismatch_permit, permit)?;
        match (arguments.as_slice(), rest) {
            ([value], []) => {
                output.scalar(self, shape.mismatch_value, self.scalar_value(*value)?)?
            }
            ([], [value]) => output.source(self, shape.mismatch_value, value)?,
            _ => return Err(malformed()),
        }
        output.publish(self, op, &structural_arguments.iter().collect::<Vec<_>>())
    }

    pub(crate) fn take_activation_answer(
        &mut self,
        op: &Operation,
        permit: &StructuralArgument,
        reply: &ParkShape,
    ) -> Result<TransferredReply> {
        let OperationKind::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = &op.kind
        else {
            return Err(malformed());
        };
        let [_, actual, rest @ ..] = structural_arguments.as_slice() else {
            return Err(malformed());
        };
        if actual != permit {
            return Err(malformed());
        }
        // Match the authenticated permit field before transferring any custody.
        super::publication_slot(self, permit, reply.anchor_identity)?;
        let consumed = self.prepare_activation_consumption(op, &structural_arguments[1..])?;
        let mut source_claims = None;
        let completed = match (arguments.as_slice(), rest) {
            ([arg], []) => {
                CompletedReturn::plain(TerminalExecutionResult::Scalar(self.scalar_value(*arg)?))
            }
            ([], [arg]) => {
                let child = ConstructorValue::resolve(self, arg)?;
                let ty = child.structural_type();
                let multiplicity = self.structural_declaration(ty)?.multiplicity;
                let mut cursor = self.local_structural_identities.cursor();
                let retained = self.stage_owned_value(arg, ty, multiplicity, &mut cursor)?;
                let claims =
                    crate::engine::custody::projected_ownership_claims(&self.live_claims, arg)?;
                let bindings = claims
                    .iter()
                    .map(|(claim, live)| terminal_psi::StructuralResultClaimBinding {
                        claim: *claim,
                        path: live.path.clone(),
                    })
                    .collect();
                // A descriptor returned as the whole reply also needs its normal
                // local binding when the stopped park installs its result.
                let (bytes, view) = match (&self.structural_declaration(ty)?.shape, &child) {
                    (StructuralTypeShape::ByteSequence(_), ConstructorValue::Access(value)) => (
                        Some(
                            self.byte_descriptor_binding(arg, value)?
                                .with_descriptor_home(retained.value.clone()),
                        ),
                        None,
                    ),
                    (StructuralTypeShape::ElementView { .. }, ConstructorValue::Access(value)) => (
                        None,
                        Some(
                            self.element_view_argument(arg, value)?
                                .clone()
                                .with_descriptor_home(retained.value.clone()),
                        ),
                    ),
                    _ => (None, None),
                };
                let mut completed = CompletedReturn::plain(TerminalExecutionResult::Structural(
                    crate::TerminalStructuralResult::new(
                        retained.value,
                        claims.iter().map(|(id, _)| *id).collect(),
                    ),
                ));
                completed.returned_claims = bindings;
                completed.byte_sequence = bytes;
                completed.element_view = view;
                if !claims.is_empty() {
                    source_claims = Some(ReplyClaims {
                        place: arg.place,
                        held: claims.into_iter().collect(),
                    });
                }
                retained.storage.publish(self)?;
                self.local_structural_identities.commit_cursor(cursor);
                completed
            }
            _ => return Err(malformed()),
        };
        consumed.commit(self)?;
        Ok(TransferredReply {
            completed,
            source_claims,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::calls::frames::tests::empty_program;
    use terminal_psi::TerminalIdentityIndex;

    #[test]
    fn relocating_a_nested_execution_owner_requires_custody_and_preserves_its_issuer() {
        let mut execution = TerminalExecution::start_with_program(
            empty_program(),
            &[],
            crate::TerminalStructuralInputs::default(),
            None,
            None,
        )
        .unwrap();
        let root = StructuralRuntimePlace {
            opaque_identity: execution.local_structural_identities.allocate().unwrap(),
            path: vec![],
        };
        let mut member = root.clone();
        member
            .path
            .push(StructuralPathSegment::Field(TerminalIdentityIndex(2)));
        let destination = StructuralRuntimePlace {
            opaque_identity: execution.local_structural_identities.allocate().unwrap(),
            path: vec![],
        };
        let mut payload = ConstructorStorage::new(None);
        payload.stage_activation_owner(member.clone(), 71).unwrap();
        payload.publish(&mut execution).unwrap();
        let mut copied = ConstructorStorage::new(None);
        assert!(matches!(
            copied.stage_subtree(&execution, &root, &destination, false),
            Err(TerminalInterpretError::ClaimTransferMismatch)
        ));
        let mut moved = ConstructorStorage::new(None);
        moved
            .stage_subtree(&execution, &root, &destination, true)
            .unwrap();
        moved.publish(&mut execution).unwrap();
        let relocated = StructuralRuntimePlace {
            opaque_identity: destination.opaque_identity,
            path: member.path.clone(),
        };
        assert!(matches!(
            execution.structural_host_atoms.get(&relocated),
            Some(HostStructuralPayload::Activation(71))
        ));
        let imported = StructuralRuntimePlace {
            opaque_identity: execution.local_structural_identities.allocate().unwrap(),
            path: vec![],
        };
        execution
            .structural_host_atoms
            .insert(imported.clone(), HostStructuralPayload::Opaque(71))
            .unwrap();
        let next = StructuralRuntimePlace {
            opaque_identity: execution.local_structural_identities.allocate().unwrap(),
            path: vec![],
        };
        let mut moved = ConstructorStorage::new(None);
        moved
            .stage_subtree(&execution, &imported, &next, true)
            .unwrap();
        moved.publish(&mut execution).unwrap();
        assert!(matches!(
            execution.structural_host_atoms.get(&next),
            Some(HostStructuralPayload::Opaque(71))
        ));
    }
}
