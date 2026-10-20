//! Claim authority is frame-local; descriptor storage identity is execution-wide.
//! A move changes the descriptor's home, never its private byte owner or ledger.

use crate::byte_sequences::{
    ByteSequenceView,
    binding::{ByteOwnerHome, ByteSequenceBinding},
};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, TerminalExecution};
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use extents::{
    ActivationClaimBranch, ActivationClaimId, ActivationClaimLedger, ActivationClaimRequest,
    ActivationClaimSiteId,
};
use semantic_vocabulary::{MachineId, OperationId};
use std::collections::BTreeMap;
use terminal_psi::{Operation, OperationKind, StructuralPathSegment};

type Frame = (usize, MachineId);
#[derive(Default)]
pub(crate) struct RuntimeActivationStorage {
    ledgers: BTreeMap<Frame, ActivationClaimLedger>,
    grants: BTreeMap<
        StructuralRuntimePlace,
        (
            Frame,
            ActivationClaimId,
            OperationId,
            semantic_vocabulary::StructuralTypeId,
        ),
    >,
    pub(crate) owners: BTreeMap<StructuralRuntimePlace, TerminalStructuralValue>,
    /// Owned claim descriptor cells, separate from borrowed byte-view holders.
    /// Moving a descriptor never moves its private activation backing.
    /// Metadata supplies no availability or ownership authority; consumers
    /// must also rejoin the ordinary live-claim and projection judgments.
    pub(crate) claim_descriptors: BTreeMap<StructuralRuntimePlace, ByteSequenceBinding>,
}

impl TerminalExecution<'_> {
    pub(crate) fn execute_establish_activation_claim(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let module = self.program.catalogs.as_ref().as_ref();
        let shape =
            terminal_semantics::activation_storage::activation_claim_shape(module, operation)
                .map_err(|_| malformed())?;
        let OperationKind::EstablishActivationClaim {
            bound,
            committed,
            backing_type,
            established_case,
            rejected_case,
        } = operation.kind
        else {
            return Err(malformed());
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        let committed = match self.scalar_value(committed)? {
            crate::values::TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            } => u64::try_from(value).map_err(|_| malformed())?,
            _ => return Err(malformed()),
        };
        let frame = (self.call_stack.len(), self.current_machine);
        if self
            .activation_storage
            .grants
            .values()
            .any(|(owner, _, site, _)| *owner == frame && *site == operation.id)
        {
            return Err(malformed());
        }
        let established = committed <= bound;
        let value = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        let root = StructuralRuntimePlace::from(&value);
        self.structural_cases.insert(
            root.clone(),
            crate::structural_inputs::case_membership::StructuralCaseContents {
                structural_type: result.structural_type,
                case: if established {
                    established_case
                } else {
                    rejected_case
                },
            },
        );
        if established {
            let site = ActivationClaimSiteId::from_normalized_identity(operation.id.get())
                .map_err(|_| malformed())?;
            let identity = self
                .activation_storage
                .ledgers
                .entry(frame)
                .or_default()
                .establish(ActivationClaimRequest {
                    site,
                    branch: ActivationClaimBranch::trunk(),
                    bound,
                    committed,
                    alignment: 1,
                })
                .map_err(|_| malformed())?;
            let owner = TerminalStructuralValue::minted(
                self.local_structural_identities.allocate()?,
                backing_type,
                Vec::new(),
            );
            let home = StructuralRuntimePlace::from(&owner);
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(usize::try_from(bound).map_err(|_| malformed())?)
                .map_err(|_| malformed())?;
            bytes.resize(usize::try_from(committed).map_err(|_| malformed())?, 0);
            self.structural_byte_storage
                .insert(home.clone(), ByteSequenceView::new(bytes));
            self.activation_storage
                .owners
                .insert(home.clone(), owner.clone());
            self.activation_storage.grants.insert(
                home.clone(),
                (frame, identity, operation.id, shape.claim_type),
            );
            let [binding] = result.claims.as_slice() else {
                return Err(malformed());
            };
            let Some(StructuralPathSegment::Field(field)) = binding.path.last() else {
                return Err(malformed());
            };
            let descriptor = TerminalStructuralValue {
                opaque_identity: value.opaque_identity,
                structural_type: shape.claim_type,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                path: vec![StructuralPathSegment::Field(*field)],
            };
            self.activation_storage.claim_descriptors.insert(
                StructuralRuntimePlace::from(&descriptor),
                ByteSequenceBinding::MutableOwner {
                    home: ByteOwnerHome::Storage(home),
                    capacity: bound,
                    referent: owner,
                    descriptor_home: Some(descriptor),
                },
            );
            crate::custody::bind_result_claim(
                &mut self.live_claims,
                binding,
                result.place,
                result.multiplicity,
            )?;
        }
        let mut frontier = self.live_affine_frontier.clone();
        self.stage_result_affine_frontier(result, &mut frontier, &|current| {
            (current.place == result.place && current.path.is_empty()).then_some(if established {
                established_case
            } else {
                rejected_case
            })
        })?;
        self.live_affine_frontier = frontier;
        self.structural_values.insert(result.place, value);
        Ok(OperationFlow::Advance)
    }

    /// Authenticate an owned descriptor against the live grant minted by this
    /// activation, not against its byte-view geometry or diagnostic identity.
    pub(crate) fn is_live_activation_descriptor(
        &self,
        binding: &ByteSequenceBinding,
        selected: &TerminalStructuralValue,
    ) -> bool {
        let ByteSequenceBinding::MutableOwner {
            home: ByteOwnerHome::Storage(home),
            descriptor_home: Some(descriptor),
            ..
        } = binding
        else {
            return false;
        };
        descriptor == selected
            && self
                .activation_storage
                .grants
                .get(home)
                .is_some_and(|(frame, _, _, claim_type)| {
                    *frame == (self.call_stack.len(), self.current_machine)
                        && *claim_type == selected.structural_type
                })
    }

    pub(crate) fn execute_release_activation_claim(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ReleaseActivationClaim { ref source, claim } = operation.kind else {
            return Err(malformed());
        };
        let selected = crate::custody::resolve_single_structural_argument(
            &self.program.structural_types(),
            &self.structural_values,
            source,
        )?;
        let binding = self
            .byte_descriptor_binding(source, &selected)
            .map_err(|error| error.in_argument_preparation("activation release descriptor"))?;
        let ByteSequenceBinding::MutableOwner {
            home: ByteOwnerHome::Storage(ref home),
            ..
        } = binding
        else {
            return Err(malformed());
        };
        let (frame, identity, _, _) = *self
            .activation_storage
            .grants
            .get(home)
            .ok_or_else(malformed)?;
        if frame != (self.call_stack.len(), self.current_machine)
            || crate::custody::has_live_loan(&self.reference_referents, &selected)
        {
            return Err(malformed()
                .in_argument_preparation("activation release frame or live reference loan"));
        }
        let claims = crate::custody::projected_ownership_claims(&self.live_claims, source)?;
        if claims.len() != 1 || claims[0].0 != claim || !claims[0].1.path.is_empty() {
            return Err(malformed().in_argument_preparation("activation release exact claim"));
        }
        // Binding maps retain backing metadata after a holder retires. Join
        // each place to the current value in its own frame before treating it
        // as a live loan; a mismatched current descriptor fails closed.
        let targets_backing = |binding: &ByteSequenceBinding| {
            matches!(binding,
            ByteSequenceBinding::MutableOwner { home: ByteOwnerHome::Storage(candidate), .. }
                if candidate == home)
        };
        let frame_has_loan =
            |bindings: &BTreeMap<semantic_vocabulary::PlaceId, ByteSequenceBinding>,
             values: &BTreeMap<semantic_vocabulary::PlaceId, TerminalStructuralValue>|
             -> Result<bool, TerminalInterpretError> {
                for (place, binding) in bindings {
                    if !targets_backing(binding) {
                        continue;
                    }
                    let Some(current) = values.get(place) else {
                        continue;
                    };
                    if binding.descriptor_home() != Some(current) {
                        return Err(malformed().in_argument_preparation(
                            "activation release current descriptor mismatch",
                        ));
                    }
                    if current != &selected {
                        return Ok(true);
                    }
                }
                Ok(false)
            };
        let mut retained_loan =
            frame_has_loan(&self.byte_sequence_values, &self.structural_values)?;
        for frame in &self.call_stack {
            retained_loan |= frame_has_loan(&frame.byte_sequence_values, &frame.structural_values)?;
        }
        for (field, binding) in &self.structural_byte_sequence_fields {
            if !targets_backing(binding) {
                continue;
            }
            // Storage association alone is not availability. A missing
            // containing owner proves this cached field holder retired. With
            // any current owner, malformed projection/namespace evidence must
            // refuse, never masquerade as an absent loan.
            let containing_owner = self
                .structural_values
                .values()
                .chain(
                    self.call_stack
                        .iter()
                        .flat_map(|frame| frame.structural_values.values()),
                )
                .any(|owner| {
                    owner.opaque_identity == field.parent.opaque_identity
                        && field.parent.path.starts_with(&owner.path)
                });
            if !containing_owner {
                continue;
            }
            let parent_type = self.actual_storage_type_from_owner(&field.parent)?;
            let case = match field.namespace {
                crate::values::StructuralFieldNamespace::Member => None,
                crate::values::StructuralFieldNamespace::CasePayload => Some(
                    self.structural_cases
                        .get(&field.parent)
                        .ok_or_else(malformed)?
                        .case,
                ),
            };
            let (fields, _) = self.active_field_declarations(parent_type, &field.parent, case)?;
            if !fields.iter().any(|row| row.id == field.field) {
                return Err(malformed()
                    .in_argument_preparation("activation release byte field declaration"));
            }
            retained_loan = true;
        }
        if retained_loan {
            return Err(
                malformed().in_argument_preparation("activation release retained byte loan")
            );
        }
        let mut frontier = self.live_affine_frontier.clone();
        self.consume_affine_call_argument(&mut frontier, source)?;
        let mut claims = self.live_claims.clone();
        claims.remove(&claim);
        self.activation_storage
            .ledgers
            .get_mut(&frame)
            .ok_or_else(malformed)?
            .release(identity)
            .map_err(|_| malformed())?;
        self.commit_constructor_operands(frontier, std::slice::from_ref(source), claims)?;
        self.activation_storage
            .claim_descriptors
            .retain(|_, binding| {
                !matches!(binding,
            ByteSequenceBinding::MutableOwner { home: ByteOwnerHome::Storage(candidate), .. }
                if candidate == home)
            });
        self.activation_storage.grants.remove(home);
        self.activation_storage.owners.remove(home);
        self.structural_byte_storage.remove(home);
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn complete_activation_storage(&mut self) -> Result<(), TerminalInterpretError> {
        let frame = (self.call_stack.len(), self.current_machine);
        if let Some(ledger) = self.activation_storage.ledgers.get(&frame)
            && ledger.live_claims().next().is_some()
        {
            return Err(malformed());
        }
        self.activation_storage.ledgers.remove(&frame);
        Ok(())
    }
}
