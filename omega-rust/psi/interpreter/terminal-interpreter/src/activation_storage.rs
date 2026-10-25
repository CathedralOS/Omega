//! Claim authority is frame-local; descriptor storage identity is execution-wide.
//! A move changes the descriptor's home, never its private byte owner or ledger.

use crate::byte_sequences::storage::ByteSequenceStorage;
use crate::byte_sequences::{
    ByteSequenceView,
    binding::{ByteOwnerHome, ByteSequenceBinding},
};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, TerminalExecution};
use crate::runtime_rows::RuntimeRows;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use evaluation_accounting::EvaluationInvocationAccount;
use extents::{
    ActivationClaimBranch, ActivationClaimId, ActivationClaimRequest, ActivationClaimSiteId,
};
use semantic_vocabulary::{MachineId, OperationId};
use terminal_psi::{Operation, OperationKind, StructuralPathSegment};

mod ledger;
use ledger::RuntimeActivationLedger;

type Frame = (usize, MachineId);
pub(crate) struct RuntimeActivationStorage {
    ledgers: RuntimeRows<Frame, RuntimeActivationLedger>,
    grants: RuntimeRows<
        StructuralRuntimePlace,
        (
            Frame,
            ActivationClaimId,
            OperationId,
            semantic_vocabulary::StructuralTypeId,
        ),
    >,
    pub(crate) owners: RuntimeRows<StructuralRuntimePlace, TerminalStructuralValue>,
    /// Owned claim descriptor cells, separate from borrowed byte-view holders.
    /// Moving a descriptor never moves its private activation backing.
    /// Metadata supplies no availability or ownership authority; consumers
    /// must also rejoin the ordinary live-claim and projection judgments.
    pub(crate) claim_descriptors: RuntimeRows<StructuralRuntimePlace, ByteSequenceBinding>,
    /// Ledger ids of the two children that currently partition one backing home.
    split_children: RuntimeRows<StructuralRuntimePlace, (ActivationClaimId, ActivationClaimId)>,
}

impl RuntimeActivationStorage {
    pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
        Self {
            ledgers: RuntimeRows::new(account),
            grants: RuntimeRows::new(account),
            owners: RuntimeRows::new(account),
            claim_descriptors: RuntimeRows::new(account),
            split_children: RuntimeRows::new(account),
        }
    }
}

/// Fund both possible division outcomes before consuming a source claim.
/// Selection drops unused descriptors and replaces the staged case in place;
/// publication only transfers paid rows.
struct PreparedActivationDivision {
    cases: RuntimeRows<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    values: RuntimeRows<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
    descriptors: RuntimeRows<StructuralRuntimePlace, ByteSequenceBinding>,
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
        )?;
        if established {
            let site = ActivationClaimSiteId::from_normalized_identity(operation.id.get())
                .map_err(|_| malformed())?;
            if !self.activation_storage.ledgers.contains_key(&frame) {
                self.activation_storage
                    .ledgers
                    .insert(frame, RuntimeActivationLedger::new(self.account.as_ref()))?;
            }
            let identity = self
                .activation_storage
                .ledgers
                .get_mut(&frame)
                .ok_or_else(malformed)?
                .establish(ActivationClaimRequest {
                    site,
                    branch: ActivationClaimBranch::trunk(),
                    bound,
                    committed,
                    alignment: 1,
                })?;
            let owner = TerminalStructuralValue::minted(
                self.local_structural_identities.allocate()?,
                backing_type,
                Vec::new(),
            );
            let home = StructuralRuntimePlace::from(&owner);
            let bytes = ByteSequenceStorage::zeroed(
                usize::try_from(committed).map_err(|_| malformed())?,
                self.account.as_ref(),
            )?;
            self.structural_byte_storage
                .insert(home.clone(), ByteSequenceView::new(bytes))?;
            self.activation_storage
                .owners
                .insert(home.clone(), owner.clone())?;
            self.activation_storage.grants.insert(
                home.clone(),
                (frame, identity, operation.id, shape.claim_type),
            )?;
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
            )?;
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
        self.structural_values.insert(result.place, value)?;
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
            || crate::custody::has_live_loan(self.reference_referents.as_map(), &selected)
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
        let owned_places = self
            .live_affine_frontier
            .iter()
            .map(|entry| entry.place)
            .chain(self.live_claims.values().filter_map(|claim| claim.place))
            .collect::<std::collections::BTreeSet<_>>();
        let frame_has_loan =
            |bindings: &RuntimeRows<semantic_vocabulary::PlaceId, ByteSequenceBinding>,
             values: &RuntimeRows<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
             honor_retired: bool|
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
                    // A consumed claim leaves its descriptor in the map after
                    // the frontier and live-claim rows move to the children.
                    // That metadata is not a loan of the backing. Suspended
                    // frames keep their own ownership and are not filtered.
                    if honor_retired && !owned_places.contains(place) {
                        continue;
                    }
                    if current != &selected {
                        return Ok(true);
                    }
                }
                Ok(false)
            };
        let mut retained_loan =
            frame_has_loan(&self.byte_sequence_values, &self.structural_values, true)?;
        for frame in &self.call_stack {
            retained_loan |=
                frame_has_loan(&frame.byte_sequence_values, &frame.structural_values, false)?;
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
            .release(identity)?;
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
            && !ledger.is_empty()
        {
            return Err(malformed());
        }
        self.activation_storage.ledgers.remove(&frame);
        Ok(())
    }

    pub(crate) fn execute_split_activation_claim(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::SplitActivationClaim {
            ref source,
            bound_at,
            committed_at,
            covered_case,
            rejected_case,
            ..
        } = operation.kind
        else {
            return Err(malformed());
        };
        let committed_at = match self.scalar_value(committed_at)? {
            crate::values::TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            } => u64::try_from(value).map_err(|_| malformed())?,
            _ => return Err(malformed()),
        };
        let selected = crate::custody::resolve_single_structural_argument(
            &self.program.structural_types(),
            &self.structural_values,
            source,
        )?;
        let binding = self
            .byte_descriptor_binding(source, &selected)
            .map_err(|error| error.in_argument_preparation("activation split descriptor"))?;
        let ByteSequenceBinding::MutableOwner {
            home: ByteOwnerHome::Storage(ref home),
            capacity,
            ref referent,
            ..
        } = binding
        else {
            return Err(malformed());
        };
        let home = home.clone();
        let referent = referent.clone();
        let frame = (self.call_stack.len(), self.current_machine);
        let (grant_frame, identity, _, claim_type) = *self
            .activation_storage
            .grants
            .get(&home)
            .ok_or_else(malformed)?;
        if grant_frame != frame {
            return Err(malformed());
        }
        if !self.structural_byte_storage.contains_key(&home) {
            return Err(malformed());
        }
        let publication = self.prepare_activation_division(
            operation,
            rejected_case,
            &home,
            capacity,
            &referent,
            claim_type,
        )?;
        // Reserve the split association before the ledger retires its parent.
        // Provisional ids never enter execution storage; success replaces them.
        let mut children = RuntimeRows::new(self.account.as_ref());
        children.insert(home.clone(), (identity, identity))?;
        self.activation_storage
            .split_children
            .validate_append(&children)?;
        let covered = self
            .activation_storage
            .ledgers
            .get_mut(&frame)
            .ok_or_else(malformed)?
            .split(identity, committed_at, bound_at)?;
        if let Some((lower_id, upper_id)) = covered {
            *children.get_mut(&home).ok_or_else(malformed)? = (lower_id, upper_id);
            self.activation_storage
                .split_children
                .append(&mut children)?;
            let mut frontier = self.live_affine_frontier.clone();
            self.consume_affine_call_argument(&mut frontier, source)?;
            self.live_affine_frontier = frontier;
            self.live_claims
                .retain(|_, claim| claim.place != Some(source.place));
        }
        self.publish_activation_division(
            operation,
            if covered.is_some() {
                covered_case
            } else {
                rejected_case
            },
            publication,
        )
    }

    pub(crate) fn execute_merge_activation_claim(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::MergeActivationClaim {
            ref lower,
            ref upper,
            covered_case,
            rejected_case,
            ..
        } = operation.kind
        else {
            return Err(malformed());
        };
        let lower_selected = crate::custody::resolve_single_structural_argument(
            &self.program.structural_types(),
            &self.structural_values,
            lower,
        )?;
        let upper_selected = crate::custody::resolve_single_structural_argument(
            &self.program.structural_types(),
            &self.structural_values,
            upper,
        )?;
        let lower_binding = self
            .byte_descriptor_binding(lower, &lower_selected)
            .map_err(|error| error.in_argument_preparation("activation merge descriptor"))?;
        let upper_binding = self
            .byte_descriptor_binding(upper, &upper_selected)
            .map_err(|error| error.in_argument_preparation("activation merge descriptor"))?;
        let ByteSequenceBinding::MutableOwner {
            home: ByteOwnerHome::Storage(ref lower_home),
            capacity,
            ref referent,
            ..
        } = lower_binding
        else {
            return Err(malformed());
        };
        let ByteSequenceBinding::MutableOwner {
            home: ByteOwnerHome::Storage(ref upper_home),
            ..
        } = upper_binding
        else {
            return Err(malformed());
        };
        let claim_type = self
            .activation_storage
            .grants
            .get(lower_home)
            .map(|grant| grant.3)
            .ok_or_else(malformed)?;
        let publication = self.prepare_activation_division(
            operation,
            rejected_case,
            lower_home,
            capacity,
            referent,
            claim_type,
        )?;
        if lower_home != upper_home {
            return self.publish_activation_division(operation, rejected_case, publication);
        }
        let home = lower_home.clone();
        let frame = (self.call_stack.len(), self.current_machine);
        let merged = self
            .activation_storage
            .split_children
            .get(&home)
            .copied()
            .and_then(|(lower_id, upper_id)| {
                self.activation_storage
                    .ledgers
                    .get_mut(&frame)
                    .and_then(|ledger| ledger.merge(lower_id, upper_id))
            });
        if let Some(merged) = merged {
            self.activation_storage.split_children.remove(&home);
            if let Some(grant) = self.activation_storage.grants.get_mut(&home) {
                grant.1 = merged;
            }
            let mut frontier = self.live_affine_frontier.clone();
            self.consume_affine_call_argument(&mut frontier, lower)?;
            self.consume_affine_call_argument(&mut frontier, upper)?;
            self.live_affine_frontier = frontier;
            self.live_claims.retain(|_, claim| {
                claim.place != Some(lower.place) && claim.place != Some(upper.place)
            });
            self.publish_activation_division(operation, covered_case, publication)
        } else {
            self.publish_activation_division(operation, rejected_case, publication)
        }
    }

    fn prepare_activation_division(
        &mut self,
        operation: &Operation,
        fallback_case: semantic_vocabulary::StructuralCaseId,
        home: &StructuralRuntimePlace,
        capacity: u64,
        referent: &TerminalStructuralValue,
        claim_type: semantic_vocabulary::StructuralTypeId,
    ) -> Result<PreparedActivationDivision, TerminalInterpretError> {
        let result = operation.result.structural().ok_or_else(malformed)?;
        let value = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        let mut cases = RuntimeRows::new(self.account.as_ref());
        cases.insert(
            StructuralRuntimePlace::from(&value),
            crate::structural_inputs::case_membership::StructuralCaseContents {
                structural_type: result.structural_type,
                case: fallback_case,
            },
        )?;
        let mut descriptors = RuntimeRows::new(self.account.as_ref());
        for binding in &result.claims {
            let Some(StructuralPathSegment::Field(field)) = binding.path.last() else {
                continue;
            };
            // Case selects the payload namespace and is not part of the
            // physical descriptor path the arrival projection compares.
            let descriptor = TerminalStructuralValue {
                opaque_identity: value.opaque_identity,
                structural_type: claim_type,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                path: vec![StructuralPathSegment::Field(*field)],
            };
            descriptors.insert(
                StructuralRuntimePlace::from(&descriptor),
                ByteSequenceBinding::MutableOwner {
                    home: ByteOwnerHome::Storage(home.clone()),
                    capacity,
                    referent: referent.clone(),
                    descriptor_home: Some(descriptor),
                },
            )?;
        }
        let mut values = RuntimeRows::new(self.account.as_ref());
        values.insert(result.place, value)?;
        self.structural_cases.validate_append(&cases)?;
        self.structural_values.validate_append(&values)?;
        self.activation_storage
            .claim_descriptors
            .validate_append(&descriptors)?;
        Ok(PreparedActivationDivision {
            cases,
            values,
            descriptors,
        })
    }

    fn publish_activation_division(
        &mut self,
        operation: &Operation,
        case: semantic_vocabulary::StructuralCaseId,
        mut publication: PreparedActivationDivision,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let result = operation.result.structural().ok_or_else(malformed)?;
        let value = publication
            .values
            .get(&result.place)
            .ok_or_else(malformed)?;
        let root = StructuralRuntimePlace::from(value);
        publication.cases.get_mut(&root).ok_or_else(malformed)?.case = case;
        let on_case = |binding: &terminal_psi::StructuralResultClaimBinding| {
            binding.path.iter().any(|segment| {
                matches!(segment, StructuralPathSegment::Case(selected) if *selected == case)
            })
        };
        publication.descriptors.retain(|descriptor, _| {
            result
                .claims
                .iter()
                .any(|binding| on_case(binding) && binding.path.last() == descriptor.path.last())
        });
        for binding in &result.claims {
            if !matches!(binding.path.last(), Some(StructuralPathSegment::Field(_)))
                || !on_case(binding)
            {
                continue;
            }
            crate::custody::bind_result_claim(
                &mut self.live_claims,
                binding,
                result.place,
                result.multiplicity,
            )?;
        }
        let mut frontier = self.live_affine_frontier.clone();
        self.stage_result_affine_frontier(result, &mut frontier, &|current| {
            (current.place == result.place && current.path.is_empty()).then_some(case)
        })?;
        self.live_affine_frontier = frontier;
        self.activation_storage
            .claim_descriptors
            .append(&mut publication.descriptors)?;
        self.structural_cases.append(&mut publication.cases)?;
        self.structural_values.append(&mut publication.values)?;
        Ok(OperationFlow::Advance)
    }
}
