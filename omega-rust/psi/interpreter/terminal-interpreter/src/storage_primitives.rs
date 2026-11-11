/*
    Activation claim ledgers, primitive local storage, scalar backing cells,
    paid runtime row reservations, runtime indexes, storage addresses, and
    execution-wide structural identities.

    We manage live cell reservations (`RuntimeRows`), primitive local homes and
    encodings, activation-scoped claim ledgers, and fresh structural referent
    identities.
*/

pub(crate) mod activation_storage {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
    use crate::engine::execution::{OperationFlow, TerminalExecution};
    use crate::storage::byte_sequences::storage::ByteSequenceStorage;
    use crate::storage::byte_sequences::{
        ByteSequenceView,
        binding::{ByteOwnerHome, ByteSequenceBinding},
    };
    use crate::storage::runtime_rows::RuntimeRows;
    use evaluation_accounting::EvaluationInvocationAccount;
    use extents::{
        ActivationClaimBranch, ActivationClaimId, ActivationClaimRequest, ActivationClaimSiteId,
    };
    use semantic_vocabulary::{MachineId, OperationId};
    use terminal_psi::{Operation, OperationKind, StructuralPathSegment};

    pub(crate) mod ledger {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
        use extents::{ActivationClaimId, ActivationClaimLedger, ActivationClaimRequest};

        pub(crate) struct RuntimeActivationLedger {
            ledger: ActivationClaimLedger,
            lease: Option<EvaluationStorageLease>,
            account: Option<EvaluationInvocationAccount>,
        }

        impl RuntimeActivationLedger {
            pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
                Self {
                    ledger: ActivationClaimLedger::new(),
                    lease: None,
                    account: account.cloned(),
                }
            }

            fn reserve_claim(&mut self) -> Result<(), TerminalInterpretError> {
                if let Some(lease) = &mut self.lease {
                    lease.grow(1, 0).map_err(TerminalInterpretError::Storage)?;
                } else if let Some(account) = &self.account {
                    self.lease = Some(
                        account
                            .reserve_storage(1, 0)
                            .map_err(TerminalInterpretError::Storage)?,
                    );
                }
                Ok(())
            }

            fn retire_claims(&mut self) {
                if let Some(lease) = &mut self.lease {
                    lease.truncate_cells(
                        u64::try_from(self.ledger.live_claims().count()).unwrap_or(u64::MAX),
                    );
                }
            }

            pub(crate) fn is_empty(&self) -> bool {
                self.ledger.live_claims().next().is_none()
            }

            pub(crate) fn establish(
                &mut self,
                request: ActivationClaimRequest,
            ) -> Result<ActivationClaimId, TerminalInterpretError> {
                self.reserve_claim()?;
                let result = self.ledger.establish(request).map_err(|_| malformed());
                self.retire_claims();
                result
            }

            pub(crate) fn release(
                &mut self,
                identity: ActivationClaimId,
            ) -> Result<(), TerminalInterpretError> {
                self.ledger.release(identity).map_err(|_| malformed())?;
                self.retire_claims();
                Ok(())
            }

            pub(crate) fn split(
                &mut self,
                identity: ActivationClaimId,
                committed_at: u64,
                bound_at: u64,
            ) -> Result<Option<(ActivationClaimId, ActivationClaimId)>, TerminalInterpretError>
            {
                self.reserve_claim()?;
                let children = self
                    .ledger
                    .split_claim(identity, committed_at, bound_at)
                    .ok();
                self.retire_claims();
                Ok(children)
            }

            pub(crate) fn merge(
                &mut self,
                lower: ActivationClaimId,
                upper: ActivationClaimId,
            ) -> Option<ActivationClaimId> {
                let parent = self.ledger.merge_claims(lower, upper).ok();
                self.retire_claims();
                parent
            }
        }
    }
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
        pub(crate) claim_descriptors: RuntimeRows<StructuralRuntimePlace, ByteSequenceBinding>,
        split_children: RuntimeRows<StructuralRuntimePlace, (ActivationClaimId, ActivationClaimId)>,
    }

    impl RuntimeActivationStorage {
        pub(crate) fn measure_result_custody<'storage>(
            &'storage self,
            root: &StructuralRuntimePlace,
            custody: &mut crate::boundary::result_custody::ResultCustody<'storage>,
        ) -> Result<(), TerminalInterpretError> {
            custody.rows(&self.grants, root)?;
            custody.rows(&self.owners, root)?;
            custody.rows(&self.claim_descriptors, root)?;
            custody.rows(&self.split_children, root)?;
            for (_, binding) in
                crate::storage::structural_storage::cells(&self.claim_descriptors, root)
            {
                custody.binding(binding)?;
            }
            Ok(())
        }

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

    struct PreparedActivationDivision {
        cases: RuntimeRows<
            StructuralRuntimePlace,
            crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
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
                crate::boundary::values::TerminalScalarValue::Integer {
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
                crate::boundary::structural_inputs::case_membership::StructuralCaseContents {
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
                self.structural_byte_storage.insert(
                home.clone(),
                crate::boundary::structural_inputs::byte_arrays::storage::StructuralByteArrayStorage::new(
                    backing_type,
                    ByteSequenceView::new(bytes),
                    self.account.as_ref(),
                ),
            )?;
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
                let Some(StructuralPathSegment::Field(_)) = binding.path.last() else {
                    return Err(malformed());
                };
                let descriptor = TerminalStructuralValue {
                    opaque_identity: value.opaque_identity,
                    structural_type: shape.claim_type,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    path: binding.path.clone(),
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
                crate::engine::custody::bind_result_claim(
                    &mut self.live_claims,
                    binding,
                    result.place,
                    result.multiplicity,
                )?;
            }
            let mut frontier = self.live_affine_frontier.clone();
            self.stage_result_affine_frontier(result, &mut frontier, &|current| {
                (current.place == result.place && current.path.is_empty()).then_some(
                    if established {
                        established_case
                    } else {
                        rejected_case
                    },
                )
            })?;
            self.live_affine_frontier = frontier;
            self.structural_values.insert(result.place, value)?;
            Ok(OperationFlow::Advance)
        }

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
                && self.activation_storage.grants.get(home).is_some_and(
                    |(frame, _, _, claim_type)| {
                        *frame == (self.call_stack.len(), self.current_machine)
                            && *claim_type == selected.structural_type
                    },
                )
        }

        pub(crate) fn execute_release_activation_claim(
            &mut self,
            operation: &Operation,
        ) -> Result<OperationFlow, TerminalInterpretError> {
            let OperationKind::ReleaseActivationClaim { ref source, claim } = operation.kind else {
                return Err(malformed());
            };
            let selected = crate::engine::custody::resolve_single_structural_argument(
                &self.structural_types(),
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
            let (frame, identity, _, _) = *self.activation_storage.grants.require(home)?;
            if frame != (self.call_stack.len(), self.current_machine)
                || crate::engine::custody::has_live_loan(
                    self.reference_referents.as_map(),
                    &selected,
                )
            {
                return Err(malformed()
                    .in_argument_preparation("activation release frame or live reference loan"));
            }
            let claims =
                crate::engine::custody::projected_ownership_claims(&self.live_claims, source)?;
            if claims.len() != 1 || claims[0].0 != claim || !claims[0].1.path.is_empty() {
                return Err(malformed().in_argument_preparation("activation release exact claim"));
            }
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
                let containing_owner = self.live_structural_values().any(|owner| {
                    owner.opaque_identity == field.parent.opaque_identity
                        && field.parent.path.starts_with(&owner.path)
                });
                if !containing_owner {
                    continue;
                }
                let parent_type = self.actual_storage_type_from_owner(&field.parent)?;
                let case = match field.namespace {
                    crate::boundary::values::StructuralFieldNamespace::Member => None,
                    crate::boundary::values::StructuralFieldNamespace::CasePayload => {
                        Some(self.structural_cases.require(&field.parent)?.case)
                    }
                };
                let (fields, _) =
                    self.active_field_declarations(parent_type, &field.parent, case)?;
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
                crate::boundary::values::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    ..
                } => u64::try_from(value).map_err(|_| malformed())?,
                _ => return Err(malformed()),
            };
            let selected = crate::engine::custody::resolve_single_structural_argument(
                &self.structural_types(),
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
            let (grant_frame, identity, _, claim_type) =
                *self.activation_storage.grants.require(&home)?;
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
            let lower_selected = crate::engine::custody::resolve_single_structural_argument(
                &self.structural_types(),
                &self.structural_values,
                lower,
            )?;
            let upper_selected = crate::engine::custody::resolve_single_structural_argument(
                &self.structural_types(),
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
                crate::boundary::structural_inputs::case_membership::StructuralCaseContents {
                    structural_type: result.structural_type,
                    case: fallback_case,
                },
            )?;
            let mut descriptors = RuntimeRows::new(self.account.as_ref());
            for binding in &result.claims {
                let Some(StructuralPathSegment::Field(_)) = binding.path.last() else {
                    continue;
                };
                let descriptor = TerminalStructuralValue {
                    opaque_identity: value.opaque_identity,
                    structural_type: claim_type,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    path: binding.path.clone(),
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
                    .any(|binding| on_case(binding) && binding.path == descriptor.path)
            });
            for binding in &result.claims {
                if !matches!(binding.path.last(), Some(StructuralPathSegment::Field(_)))
                    || !on_case(binding)
                {
                    continue;
                }
                crate::engine::custody::bind_result_claim(
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
}

pub(crate) mod primitive_storage {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, MachineId, PlaceId, ScalarType,
        StructuralPlaceKind, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
        live_field_by_id,
    };

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralFieldNamespace, StructuralRuntimePlace, StructuralScalarRuntimeField,
    };
    use crate::boundary::values::{TerminalScalarValue, TerminalStructuralValue};
    use crate::engine::custody::resolve_structural_path_type;
    use crate::engine::execution::SuspendedCall;
    use crate::engine::execution::TerminalExecution;
    use crate::engine::execution::frame_field_mut;
    use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
    use terminal_psi::TerminalMachine;

    pub(crate) mod aggregate_projection {
        use super::*;
        use crate::storage::reference::backing_scalar_path;
        use terminal_semantics::reference_storage_byte_width;

        fn mask(bits: u8) -> Result<u128, TerminalInterpretError> {
            if bits == 0 || bits > 128 {
                return Err(malformed());
            }
            Ok(u128::MAX >> (128 - usize::from(bits)))
        }

        fn projection_bits(
            fragments: &[PrimitiveStorageFragment],
            mut read: impl FnMut(
                &PrimitiveStorageFragment,
            ) -> Result<TerminalScalarValue, TerminalInterpretError>,
        ) -> Result<u128, TerminalInterpretError> {
            let mut bits = 0_u128;
            for fragment in fragments {
                let source = encoding::encode(fragment.stored, read(fragment)?)?;
                let part = (source >> usize::from(fragment.source_bit_offset))
                    & mask(fragment.bit_length)?;
                bits |= part << usize::from(fragment.destination_bit_offset);
            }
            Ok(bits)
        }

        impl TerminalExecution<'_> {
            pub(crate) fn aggregate_projection_storage(
                &self,
                referent: &TerminalStructuralValue,
                observed: terminal_semantics::ReferenceStorageProjection,
                byte_length: u64,
                home: Option<&crate::storage::element_views::ElementViewHome>,
                writing: bool,
            ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
                let width =
                    reference_storage_byte_width(observed.stored_scalar).ok_or_else(malformed)?;
                if u64::from(width) != byte_length {
                    return Err(malformed());
                }
                let first = observed.byte_offset.checked_mul(8).ok_or_else(malformed)?;
                let end = observed
                    .byte_offset
                    .checked_add(byte_length)
                    .and_then(|end| end.checked_mul(8))
                    .ok_or_else(malformed)?;
                let types = self.structural_types();
                let geometry = crate::storage::reference::scalar_storage_fragments(
                    &types,
                    referent.structural_type,
                    observed.byte_offset,
                    byte_length,
                )
                .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                    "aggregate scalar window contains padding or noncontiguous storage",
                ))?;
                let mut fragments = Vec::with_capacity(geometry.len());
                let mut shared = false;
                for fragment in geometry {
                    let canonical =
                        backing_scalar_path(&types, referent.structural_type, &fragment.path)
                            .ok_or_else(malformed)?;
                    let (storage, actual, shared_view) = self.original_view_primitive_access(
                        referent,
                        home,
                        &canonical,
                        fragment.scalar.logical_scalar,
                        writing,
                    )?;
                    if actual != fragment.scalar.logical_scalar {
                        return Err(malformed());
                    }
                    shared |= shared_view;
                    let start = fragment.bit_offset.max(first);
                    let fragment_end = fragment
                        .bit_offset
                        .checked_add(fragment.bit_length)
                        .ok_or_else(malformed)?
                        .min(end);
                    let source_bit_offset = fragment
                        .source_bit_offset
                        .checked_add(start - fragment.bit_offset)
                        .ok_or_else(malformed)?;
                    let selected_length = fragment_end - start;
                    let stored_bits = u64::from(
                        reference_storage_byte_width(fragment.scalar.stored_scalar)
                            .ok_or_else(malformed)?,
                    )
                    .checked_mul(8)
                    .ok_or_else(malformed)?;
                    if selected_length == 0
                        || source_bit_offset
                            .checked_add(selected_length)
                            .is_none_or(|end| end > stored_bits)
                    {
                        return Err(malformed());
                    }
                    fragments.push(PrimitiveStorageFragment {
                        storage,
                        logical: fragment.scalar.logical_scalar,
                        stored: fragment.scalar.stored_scalar,
                        source_bit_offset: u8::try_from(source_bit_offset)
                            .map_err(|_| malformed())?,
                        destination_bit_offset: u8::try_from(start - first)
                            .map_err(|_| malformed())?,
                        bit_length: u8::try_from(selected_length).map_err(|_| malformed())?,
                        addressable_cell: fragment.addressable_cell,
                    });
                }
                Ok((
                    PrimitiveStorage::canonical(PrimitiveStorageEndpoint::Projection {
                        fragments,
                        logical: observed.logical_scalar,
                        stored: observed.stored_scalar,
                    }),
                    observed.logical_scalar,
                    shared,
                ))
            }

            pub(crate) fn read_storage_projection(
                &self,
                source: PlaceId,
                fragments: &[PrimitiveStorageFragment],
                stored: ScalarType,
                expected: ScalarType,
            ) -> Result<TerminalScalarValue, TerminalInterpretError> {
                let bits = projection_bits(fragments, |fragment| {
                    self.read_primitive_storage(
                        source,
                        fragment.storage.clone(),
                        fragment.logical,
                        fragment.logical,
                        false,
                    )
                })?;
                encoding::convert_scalar(encoding::decode(stored, bits)?, expected)
            }
        }

        impl PrimitiveStorageWrites {
            fn pending_storage_value(
                &self,
                execution: &TerminalExecution,
                destination: PlaceId,
                storage: &PrimitiveStorage,
                logical: ScalarType,
            ) -> Result<TerminalScalarValue, TerminalInterpretError> {
                if let PrimitiveStorageEndpoint::Projection {
                    fragments, stored, ..
                } = &storage.endpoint
                {
                    let bits = projection_bits(fragments, |fragment| {
                        self.pending_storage_value(
                            execution,
                            destination,
                            &fragment.storage,
                            fragment.logical,
                        )
                    })?;
                    return encoding::convert_scalar(encoding::decode(*stored, bits)?, logical);
                }
                if let Some(write) = self
                    .remaining_writes
                    .iter()
                    .rev()
                    .chain(self.first_write.iter())
                    .find(|write| write.endpoint == storage.endpoint)
                {
                    return if let Some(codec) = storage.encoding {
                        let bits = encoding::encode(codec.source, write.value)?;
                        encoding::convert_scalar(encoding::decode(codec.view, bits)?, logical)
                    } else {
                        encoding::convert_scalar(write.value, logical)
                    };
                }
                execution.read_primitive_storage(
                    destination,
                    storage.clone(),
                    logical,
                    logical,
                    false,
                )
            }

            pub(crate) fn stage_projection(
                &mut self,
                execution: &TerminalExecution,
                destination: PlaceId,
                fragments: &[PrimitiveStorageFragment],
                stored: ScalarType,
                value: TerminalScalarValue,
            ) -> Result<(), TerminalInterpretError> {
                let bits = encoding::encode(stored, value)?;
                for fragment in fragments {
                    let width =
                        reference_storage_byte_width(fragment.stored).ok_or_else(malformed)?;
                    let source_shift = usize::from(fragment.source_bit_offset);
                    let selected_mask = mask(fragment.bit_length)? << source_shift;
                    let old_bits = if fragment.source_bit_offset == 0
                        && u16::from(fragment.bit_length) == u16::from(width) * 8
                    {
                        0
                    } else {
                        encoding::encode(
                            fragment.stored,
                            self.pending_storage_value(
                                execution,
                                destination,
                                &fragment.storage,
                                fragment.logical,
                            )?,
                        )?
                    };
                    let replacement = (old_bits & !selected_mask)
                        | (((bits >> usize::from(fragment.destination_bit_offset))
                            & mask(fragment.bit_length)?)
                            << source_shift);
                    let replacement = encoding::convert_scalar(
                        encoding::decode(fragment.stored, replacement)?,
                        fragment.logical,
                    )?;
                    self.stage(
                        execution,
                        destination,
                        fragment.storage.clone(),
                        fragment.logical,
                        replacement,
                    )?;
                }
                Ok(())
            }
        }
    }
    pub(crate) mod encoding {
        use super::{
            PrimitiveLoan, PrimitiveStorage, PrimitiveStorageEncoding, PrimitiveStorageEndpoint,
        };
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::custody::resolve_structural_path_type;
        use crate::engine::execution::TerminalExecution;
        use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
        use semantic_vocabulary::{
            IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, PlaceId,
            ScalarType,
        };
        use terminal_semantics::{
            reference_storage_byte_width, reference_storage_view_is_compatible,
        };

        #[derive(PartialEq, Eq)]
        struct ReferenceByteRegionProjection {
            region: crate::storage::reference::ReferenceByteRegion,
            scalar: Option<(ScalarType, ScalarType)>,
        }

        impl ReferenceByteRegionProjection {
            fn scalar_storage(
                self,
            ) -> Result<(PrimitiveStorage, ScalarType), TerminalInterpretError> {
                let (logical_scalar, stored_scalar) =
                    self.scalar
                        .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                            "captured byte-region projection has no contiguous scalar geometry",
                        ))?;
                Ok((
                    PrimitiveStorage {
                        endpoint: self.region.endpoint,
                        encoding: Some(PrimitiveStorageEncoding {
                            source: stored_scalar,
                            view: stored_scalar,
                        }),
                    },
                    logical_scalar,
                ))
            }
        }

        pub(crate) fn scalar_fits_storage(stored: ScalarType, scalar: TerminalScalarValue) -> bool {
            reference_storage_view_is_compatible(stored, scalar.scalar_type())
                && encode(stored, scalar).is_ok()
        }

        impl TerminalExecution<'_> {
            pub(crate) fn reference_byte_region_access(
                &self,
                backing_view: &crate::boundary::values::TerminalStructuralValue,
                view: &crate::boundary::values::TerminalStructuralValue,
            ) -> Result<Option<(PrimitiveStorage, ScalarType)>, TerminalInterpretError>
            {
                if view.opaque_identity != backing_view.opaque_identity
                    || !view.path.starts_with(&backing_view.path)
                {
                    return Err(malformed());
                }
                let Some(projected) =
                    self.project_reference_byte_region(backing_view, &view.path)?
                else {
                    return Ok(None);
                };
                projected.scalar_storage().map(Some)
            }

            pub(crate) fn captured_byte_region_access(
                &self,
                anchor: &crate::boundary::values::TerminalStructuralValue,
                region: &crate::storage::reference::ReferenceByteRegion,
                path: &[terminal_psi::StructuralPathSegment],
            ) -> Result<(PrimitiveStorage, ScalarType), TerminalInterpretError> {
                let mut projected_path = anchor.path.clone();
                projected_path.extend_from_slice(path);
                self.project_byte_regions(
                    anchor,
                    &projected_path,
                    std::iter::once((anchor, region)),
                )?
                .ok_or_else(malformed)?
                .scalar_storage()
            }

            pub(crate) fn reference_byte_region(
                &self,
                view: &crate::boundary::values::TerminalStructuralValue,
            ) -> Result<
                Option<crate::storage::reference::ReferenceByteRegion>,
                TerminalInterpretError,
            > {
                Ok(self
                    .project_reference_byte_region(view, &view.path)?
                    .map(|projected| projected.region))
            }

            fn project_reference_byte_region(
                &self,
                view: &crate::boundary::values::TerminalStructuralValue,
                projected_path: &[terminal_psi::StructuralPathSegment],
            ) -> Result<Option<ReferenceByteRegionProjection>, TerminalInterpretError> {
                let references = self
                    .reference_referents
                    .values()
                    .filter_map(|captured| Some((&captured.view, captured.byte_region.as_ref()?)));
                let arguments = self
                    .aggregate_argument_backings
                    .values()
                    .filter_map(|backing| Some((&backing.value, backing.byte_region.as_ref()?)));
                self.project_byte_regions(view, projected_path, references.chain(arguments))
            }

            fn project_byte_regions<'captured>(
                &self,
                view: &crate::boundary::values::TerminalStructuralValue,
                projected_path: &[terminal_psi::StructuralPathSegment],
                captured: impl Iterator<
                    Item = (
                        &'captured crate::boundary::values::TerminalStructuralValue,
                        &'captured crate::storage::reference::ReferenceByteRegion,
                    ),
                >,
            ) -> Result<Option<ReferenceByteRegionProjection>, TerminalInterpretError> {
                let mut selected = None;
                for (anchor, region) in captured {
                    if view.opaque_identity != anchor.opaque_identity {
                        continue;
                    }
                    let Some(path) = view.path.strip_prefix(anchor.path.as_slice()) else {
                        continue;
                    };
                    if resolve_structural_path_type(
                        &self.structural_types(),
                        anchor.structural_type,
                        path,
                    )
                    .ok()
                        != Some(view.structural_type)
                    {
                        continue;
                    }
                    let path = projected_path
                        .strip_prefix(anchor.path.as_slice())
                        .ok_or_else(malformed)?;
                    let geometry = terminal_semantics::reference_storage_region(
                        self.structural_types().values(),
                        anchor.structural_type,
                        path,
                    )
                    .ok_or(
                        TerminalInterpretError::UnsupportedSemanticVariant(
                            "captured byte region has no contiguous geometry",
                        ),
                    )?;
                    let offset = usize::try_from(geometry.byte_offset).map_err(|_| malformed())?;
                    let length = usize::try_from(geometry.byte_length).map_err(|_| malformed())?;
                    if offset
                        .checked_add(length)
                        .is_none_or(|end| end > region.byte_length)
                    {
                        return Err(malformed());
                    }
                    let projected = ReferenceByteRegionProjection {
                        region: crate::storage::reference::ReferenceByteRegion {
                            endpoint: shifted_byte_endpoint(&region.endpoint, offset)?,
                            byte_length: length,
                        },
                        scalar: geometry
                            .scalar
                            .map(|scalar| (scalar.logical_scalar, scalar.stored_scalar)),
                    };
                    if selected
                        .as_ref()
                        .is_some_and(|previous| previous != &projected)
                    {
                        return Err(malformed());
                    }
                    selected = Some(projected);
                }
                Ok(selected)
            }

            pub(crate) fn apply_reference_storage_view(
                &self,
                loan: &mut PrimitiveLoan,
                byte_offset: usize,
                stored_scalar: ScalarType,
                logical_scalar: ScalarType,
            ) -> Result<(), TerminalInterpretError> {
                let width =
                    usize::from(reference_storage_byte_width(stored_scalar).ok_or_else(malformed)?);
                if !reference_storage_view_is_compatible(stored_scalar, logical_scalar) {
                    return Err(malformed());
                }
                if let Some(captured) = &mut loan.storage.encoding {
                    if byte_offset != 0
                        || reference_storage_byte_width(captured.source)
                            != reference_storage_byte_width(stored_scalar)
                    {
                        return Err(malformed());
                    }
                    captured.view = stored_scalar;
                } else {
                    let source_scalar = match loan.storage.endpoint {
                        PrimitiveStorageEndpoint::Scalar(_)
                        | PrimitiveStorageEndpoint::ScalarField(_) => loan.scalar_type,
                        _ => stored_scalar,
                    };
                    self.position_reference_storage(loan, byte_offset, width)?;
                    loan.storage.encoding = Some(PrimitiveStorageEncoding {
                        source: source_scalar,
                        view: stored_scalar,
                    });
                }
                loan.scalar_type = logical_scalar;
                Ok(())
            }

            pub(crate) fn position_reference_storage(
                &self,
                loan: &mut PrimitiveLoan,
                byte_offset: usize,
                width: usize,
            ) -> Result<(), TerminalInterpretError> {
                let endpoint = match &loan.storage.endpoint {
                    PrimitiveStorageEndpoint::Projection { .. } => "Projection",
                    PrimitiveStorageEndpoint::ArrayElement { .. } => "ArrayElement",
                    PrimitiveStorageEndpoint::OwnedArrayElement { .. } => "OwnedArrayElement",
                    PrimitiveStorageEndpoint::RepresentationByte(_) => "RepresentationByte",
                    PrimitiveStorageEndpoint::ViewByte { .. } => "ViewByte",
                    PrimitiveStorageEndpoint::Scalar(_) => "Scalar",
                    PrimitiveStorageEndpoint::ScalarField(_) => "ScalarField",
                };
                let scalar = loan.scalar_type;
                let codec = loan.storage.encoding;
                let referent = (
                    loan.referent.opaque_identity,
                    loan.referent.structural_type,
                    loan.referent.path.len(),
                );
                let mut phase = "endpoint already has a retained codec";
                let mut available = None;
                let mut span_start = None;
                let mut span_end = None;
                let mut requested_frame = None;
                let mut actual_machine = None;
                let mut actual_type = None;
                let mut fragment_index = None;
                let mut fragment_count = None;
                let mut projection_types = None;
                let mut first_fragment = None;
                let mut first_mismatched_scalar = None;
                let outcome = (|| {
                    if loan.storage.encoding.is_some() {
                        return Err(malformed());
                    }
                    match &mut loan.storage.endpoint {
                        PrimitiveStorageEndpoint::Projection {
                            fragments,
                            logical,
                            stored,
                        } => {
                            projection_types = Some((*logical, *stored));
                            first_fragment = fragments.first().map(|fragment| {
                                (
                                    fragment.logical,
                                    fragment.stored,
                                    fragment.source_bit_offset,
                                    fragment.destination_bit_offset,
                                    fragment.bit_length,
                                    fragment.addressable_cell,
                                    fragment.storage.encoding,
                                )
                            });
                            phase = "projection has no stored byte width";
                            let stored_width = usize::from(
                                reference_storage_byte_width(*stored).ok_or_else(malformed)?,
                            );
                            available = Some(stored_width);
                            fragment_count = Some(fragments.len());
                            phase = "projection end overflows";
                            let end = byte_offset.checked_add(width).ok_or_else(malformed)?;
                            span_end = Some(end);
                            phase = "region exceeds the stored projection";
                            if end > stored_width {
                                return Err(malformed());
                            }
                            let mut selected = Vec::new();
                            phase = "projection bit offset overflows";
                            let selected_start =
                                byte_offset.checked_mul(8).ok_or_else(malformed)?;
                            phase = "projection bit end overflows";
                            let selected_end = end.checked_mul(8).ok_or_else(malformed)?;
                            for (position, fragment) in fragments.iter().enumerate() {
                                fragment_index = Some(position);
                                let start = usize::from(fragment.destination_bit_offset);
                                let fragment_end = start + usize::from(fragment.bit_length);
                                let first = start.max(selected_start);
                                let last = fragment_end.min(selected_end);
                                if first >= last {
                                    continue;
                                }
                                let mut fragment = fragment.clone();
                                phase = "fragment source bit adjustment overflows";
                                fragment.source_bit_offset = fragment
                                    .source_bit_offset
                                    .checked_add(
                                        u8::try_from(first - start).map_err(|_| malformed())?,
                                    )
                                    .ok_or_else(malformed)?;
                                phase = "fragment destination bit offset is too large";
                                fragment.destination_bit_offset =
                                    u8::try_from(first - selected_start)
                                        .map_err(|_| malformed())?;
                                phase = "fragment bit length is too large";
                                fragment.bit_length =
                                    u8::try_from(last - first).map_err(|_| malformed())?;
                                selected.push(fragment);
                            }
                            *fragments = selected;
                            let _ = logical;
                        }
                        PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                            phase = "array endpoint is not a byte carrier";
                            if loan.scalar_type != byte_type()? {
                                return Err(malformed());
                            }
                            phase = "array start overflows";
                            let start = index.checked_add(byte_offset).ok_or_else(malformed)?;
                            span_start = Some(start);
                            phase = "array end overflows";
                            let end = start.checked_add(width).ok_or_else(malformed)?;
                            span_end = Some(end);
                            use crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorageHome;
                            requested_frame = match array.home {
                                ArrayStorageHome::Runtime => None,
                                ArrayStorageHome::Frame {
                                    depth,
                                    place,
                                    structural_type,
                                } => Some((depth, place, structural_type)),
                            };
                            phase = "array extent is unavailable";
                            let length = self.stored_byte_array_length(array)?;
                            available = Some(length);
                            phase = "region exceeds the captured array";
                            if end > length {
                                return Err(malformed());
                            }
                            *index = start;
                        }
                        PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                            phase = "owned array endpoint is not a byte carrier";
                            if loan.scalar_type != byte_type()? {
                                return Err(malformed());
                            }
                            requested_frame = Some((home.depth, home.place, home.structural_type));
                            phase = "owned array start overflows";
                            let start = index.checked_add(byte_offset).ok_or_else(malformed)?;
                            span_start = Some(start);
                            phase = "owned array end overflows";
                            let end = start.checked_add(width).ok_or_else(malformed)?;
                            span_end = Some(end);
                            phase = "owned array activation is unavailable";
                            let (machine, arrays) = if home.depth == self.call_stack.len() {
                                (self.current_machine, &self.scalar_array_values)
                            } else {
                                let frame =
                                    self.call_stack.get(home.depth).ok_or_else(malformed)?;
                                (frame.current_machine, &frame.scalar_array_values)
                            };
                            actual_machine = Some((home.machine, machine));
                            phase = "owned array backing is unavailable";
                            let array = arrays.backing(&home.place).ok_or_else(malformed)?;
                            actual_type = Some(array.structural_type);
                            available = Some(array.len());
                            if machine != home.machine
                                || array.structural_type != home.structural_type
                                || array.cells().get(start..end).is_none_or(|bytes| {
                                    bytes.iter().any(|byte| {
                                        let observed = byte.scalar_type();
                                        let mismatched = observed != loan.scalar_type;
                                        if mismatched {
                                            first_mismatched_scalar = Some(observed);
                                        }
                                        mismatched
                                    })
                                })
                            {
                                phase = if machine != home.machine {
                                    "owned array machine differs"
                                } else if array.structural_type != home.structural_type {
                                    "owned array type differs"
                                } else if first_mismatched_scalar.is_some() {
                                    "owned array cell carrier differs"
                                } else {
                                    "region exceeds the owned array"
                                };
                                return Err(malformed());
                            }
                            *index = start;
                        }
                        PrimitiveStorageEndpoint::RepresentationByte(byte) => {
                            phase = "representation endpoint is not a byte carrier";
                            if loan.scalar_type != byte_type()? {
                                return Err(malformed());
                            }
                            phase = "representation start overflows";
                            let start =
                                byte.index.checked_add(byte_offset).ok_or_else(malformed)?;
                            span_start = Some(start);
                            phase = "region exceeds the representation or its end overflows";
                            if start.checked_add(width).is_none_or(|end| {
                                span_end = Some(end);
                                let length = byte.view.len();
                                available = Some(length);
                                end > length
                            }) {
                                return Err(malformed());
                            }
                            byte.index = start;
                        }
                        PrimitiveStorageEndpoint::ViewByte { bytes, index } => {
                            phase = "view endpoint is not a byte carrier";
                            if loan.scalar_type != byte_type()? {
                                return Err(malformed());
                            }
                            phase = "view start overflows";
                            let start = index.checked_add(byte_offset).ok_or_else(malformed)?;
                            span_start = Some(start);
                            phase = "view end overflows";
                            let end = start.checked_add(width).ok_or_else(malformed)?;
                            span_end = Some(end);
                            let length = bytes.len();
                            available = Some(length);
                            phase = "region exceeds the captured view";
                            if end > length {
                                return Err(malformed());
                            }
                            *index = start;
                        }
                        PrimitiveStorageEndpoint::Scalar(_)
                        | PrimitiveStorageEndpoint::ScalarField(_) => {
                            phase = "scalar endpoint has no stored byte width";
                            let source_width = usize::from(
                                reference_storage_byte_width(loan.scalar_type)
                                    .ok_or_else(malformed)?,
                            );
                            available = Some(source_width);
                            phase = "region exceeds a single scalar cell";
                            if byte_offset != 0 || width > source_width {
                                return Err(malformed());
                            }
                        }
                    }
                    Ok(())
                })();
                outcome.map_err(|error| TerminalInterpretError::ReferenceFormation {
            stage: "position reference storage",
            detail: format!(
                "{phase}; endpoint={endpoint}, byte_offset={byte_offset}, width={width}, scalar={scalar:?}, codec={codec:?}, referent_identity_type_path_length={referent:?}, span_start={span_start:?}, span_end={span_end:?}, available={available:?}, requested_depth_place_type={requested_frame:?}, requested_actual_machine={actual_machine:?}, actual_type={actual_type:?}, current_depth={}, projection_logical_stored={projection_types:?}, fragment_count={fragment_count:?}, fragment_index={fragment_index:?}, first_fragment_logical_stored_offsets_length_addressable_codec={first_fragment:?}, first_mismatched_scalar={first_mismatched_scalar:?}: {error:?}",
                self.call_stack.len()
            ),
        })
            }

            pub(crate) fn read_encoded_primitive(
                &self,
                source: PlaceId,
                endpoint: &PrimitiveStorageEndpoint,
                stored_scalar: ScalarType,
                logical_scalar: ScalarType,
            ) -> Result<TerminalScalarValue, TerminalInterpretError> {
                let stored = match endpoint {
                    PrimitiveStorageEndpoint::Scalar(_)
                    | PrimitiveStorageEndpoint::ScalarField(_) => {
                        let scalar = self.endpoint_scalar(source, endpoint)?;
                        let bits = if scalar.scalar_type() == byte_type()? {
                            encode(byte_type()?, scalar)?
                        } else {
                            encode(stored_scalar, scalar)?
                        };
                        decode(stored_scalar, bits)?
                    }
                    _ => {
                        let width =
                            reference_storage_byte_width(stored_scalar).ok_or_else(malformed)?;
                        let mut bits = 0_u128;
                        for byte_index in 0..usize::from(width) {
                            let storage = shifted_byte_endpoint(endpoint, byte_index)?;
                            let byte = self.read_primitive_storage(
                                source,
                                PrimitiveStorage::canonical(storage),
                                byte_type()?,
                                byte_type()?,
                                false,
                            )?;
                            let TerminalScalarValue::Integer {
                                value: IntegerValue::Unsigned(byte),
                                ..
                            } = byte
                            else {
                                return Err(malformed());
                            };
                            bits |= byte << (byte_index * 8);
                        }
                        decode(stored_scalar, bits)?
                    }
                };
                if !reference_storage_view_is_compatible(stored_scalar, logical_scalar) {
                    return Err(malformed());
                }
                convert_scalar(stored, logical_scalar)
            }

            pub(crate) fn prepare_encoded_primitive(
                &self,
                writes: &mut super::PrimitiveStorageWrites,
                destination: PlaceId,
                endpoint: &PrimitiveStorageEndpoint,
                stored_scalar: ScalarType,
                scalar: TerminalScalarValue,
            ) -> Result<(), TerminalInterpretError> {
                if !reference_storage_view_is_compatible(stored_scalar, scalar.scalar_type()) {
                    return Err(malformed());
                }
                let bits = encode(stored_scalar, scalar)?;
                match endpoint {
                    PrimitiveStorageEndpoint::Scalar(_)
                    | PrimitiveStorageEndpoint::ScalarField(_) => {
                        let original = self.endpoint_scalar(destination, endpoint)?;
                        let replacement = if original.scalar_type() == byte_type()? {
                            decode(byte_type()?, bits)?
                        } else {
                            convert_scalar(decode(stored_scalar, bits)?, original.scalar_type())?
                        };
                        writes.stage(
                            self,
                            destination,
                            PrimitiveStorage::canonical(endpoint.clone()),
                            original.scalar_type(),
                            replacement,
                        )
                    }
                    _ => {
                        let width =
                            reference_storage_byte_width(stored_scalar).ok_or_else(malformed)?;
                        for byte_index in 0..usize::from(width) {
                            let byte = TerminalScalarValue::Integer {
                                scalar_type: byte_integer_type()?,
                                value: IntegerValue::Unsigned((bits >> (byte_index * 8)) & 255),
                            };
                            writes.stage(
                                self,
                                destination,
                                PrimitiveStorage::canonical(shifted_byte_endpoint(
                                    endpoint, byte_index,
                                )?),
                                byte_type()?,
                                byte,
                            )?;
                        }
                        Ok(())
                    }
                }
            }

            pub(crate) fn endpoint_scalar(
                &self,
                source: PlaceId,
                endpoint: &PrimitiveStorageEndpoint,
            ) -> Result<TerminalScalarValue, TerminalInterpretError> {
                let value = match endpoint {
            PrimitiveStorageEndpoint::Scalar(place) => self.structural_primitive_storage.get(place),
            PrimitiveStorageEndpoint::ScalarField(field) => {
                self.structural_scalar_fields.get(field)
            }
            _ => return Err(malformed()),
        }
        .ok_or(
            crate::boundary::errors::TerminalInterpretError::StructuralPrimitiveStorageMissing(
                source,
            ),
        )?
        .read()?;
                if !terminal_scalar_belongs_to_type(value) {
                    return Err(malformed());
                }
                Ok(value)
            }
        }

        pub(crate) fn shifted_byte_endpoint(
            endpoint: &PrimitiveStorageEndpoint,
            byte_offset: usize,
        ) -> Result<PrimitiveStorageEndpoint, TerminalInterpretError> {
            match endpoint {
                PrimitiveStorageEndpoint::Projection {
                    fragments, stored, ..
                } => {
                    let width =
                        usize::from(reference_storage_byte_width(*stored).ok_or_else(malformed)?);
                    if byte_offset >= width {
                        return Err(malformed());
                    }
                    let mut selected = Vec::new();
                    let byte_start = byte_offset.checked_mul(8).ok_or_else(malformed)?;
                    let byte_end = byte_start.checked_add(8).ok_or_else(malformed)?;
                    for fragment in fragments {
                        let first = usize::from(fragment.destination_bit_offset);
                        let last = first + usize::from(fragment.bit_length);
                        let start = first.max(byte_start);
                        let end = last.min(byte_end);
                        if start >= end {
                            continue;
                        }
                        let mut fragment = fragment.clone();
                        fragment.source_bit_offset = fragment
                            .source_bit_offset
                            .checked_add(u8::try_from(start - first).map_err(|_| malformed())?)
                            .ok_or_else(malformed)?;
                        fragment.destination_bit_offset =
                            u8::try_from(start - byte_start).map_err(|_| malformed())?;
                        fragment.bit_length = u8::try_from(end - start).map_err(|_| malformed())?;
                        selected.push(fragment);
                    }
                    let coverage = selected.iter().try_fold(0u16, |covered, fragment| {
                        (u16::from(fragment.destination_bit_offset) == covered)
                            .then(|| covered.checked_add(u16::from(fragment.bit_length)))
                            .flatten()
                    });
                    if coverage != Some(8) {
                        return Err(malformed());
                    }
                    Ok(PrimitiveStorageEndpoint::Projection {
                        fragments: selected,
                        logical: byte_type()?,
                        stored: byte_type()?,
                    })
                }
                PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                    Ok(PrimitiveStorageEndpoint::ArrayElement {
                        array: array.clone(),
                        index: index.checked_add(byte_offset).ok_or_else(malformed)?,
                    })
                }
                PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                    Ok(PrimitiveStorageEndpoint::OwnedArrayElement {
                        home: *home,
                        index: index.checked_add(byte_offset).ok_or_else(malformed)?,
                    })
                }
                PrimitiveStorageEndpoint::RepresentationByte(byte) => {
                    let index = byte.index.checked_add(byte_offset).ok_or_else(malformed)?;
                    if index >= byte.view.len() {
                        return Err(malformed());
                    }
                    Ok(PrimitiveStorageEndpoint::RepresentationByte(
                        super::RepresentationByte {
                            view: byte.view.clone(),
                            index,
                        },
                    ))
                }
                PrimitiveStorageEndpoint::ViewByte { bytes, index } => {
                    Ok(PrimitiveStorageEndpoint::ViewByte {
                        bytes: bytes.clone(),
                        index: index.checked_add(byte_offset).ok_or_else(malformed)?,
                    })
                }
                PrimitiveStorageEndpoint::Scalar(_) | PrimitiveStorageEndpoint::ScalarField(_)
                    if byte_offset == 0 =>
                {
                    Ok(endpoint.clone())
                }
                _ => Err(malformed()),
            }
        }

        fn byte_integer_type() -> Result<IntegerType, TerminalInterpretError> {
            IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())
        }
        fn byte_type() -> Result<ScalarType, TerminalInterpretError> {
            Ok(ScalarType::Integer(byte_integer_type()?))
        }

        pub(crate) fn convert_scalar(
            value: TerminalScalarValue,
            target: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            if !terminal_scalar_belongs_to_type(value) {
                return Err(malformed());
            }
            if terminal_semantics::scalar_reference_view_is_total(value.scalar_type(), target) {
                return super::scalar_reference_observation(value, target);
            }
            if let (TerminalScalarValue::Integer { scalar_type, value }, ScalarType::Boolean) =
                (value, target)
                && let Some(pair) = terminal_semantics::ScalarRepresentationViewPair::classify(
                    ScalarType::Integer(scalar_type),
                    target,
                )
                && let Some(terminal_semantics::ScalarViewValue::Boolean(value)) =
                    pair.view(terminal_semantics::ScalarViewValue::Integer(value))
            {
                return Ok(TerminalScalarValue::Boolean(value));
            }
            let (TerminalScalarValue::Integer { value, .. }, ScalarType::Integer(integer)) =
                (value, target)
            else {
                return Err(malformed());
            };
            if integer.is_address() {
                return Err(malformed());
            }
            let value = match (integer.sign(), value) {
                (IntegerSign::Signed, IntegerValue::Signed(value)) => IntegerValue::Signed(value),
                (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                    IntegerValue::Unsigned(value)
                }
                (IntegerSign::Signed, IntegerValue::Unsigned(value)) => {
                    IntegerValue::Signed(i128::try_from(value).map_err(|_| malformed())?)
                }
                (IntegerSign::Unsigned, IntegerValue::Signed(value)) => {
                    IntegerValue::Unsigned(u128::try_from(value).map_err(|_| malformed())?)
                }
            };
            if !integer.admits(value) {
                return Err(malformed());
            }
            Ok(TerminalScalarValue::Integer {
                scalar_type: integer,
                value,
            })
        }

        pub(crate) fn encode(
            stored: ScalarType,
            scalar: TerminalScalarValue,
        ) -> Result<u128, TerminalInterpretError> {
            let scalar = convert_scalar(scalar, stored)?;
            let width = reference_storage_byte_width(stored).ok_or_else(malformed)?;
            let bits = match scalar {
                TerminalScalarValue::Boolean(value) => u128::from(value),
                TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(value),
                    ..
                } => value,
                TerminalScalarValue::Integer {
                    value: IntegerValue::Signed(value),
                    ..
                } => value as u128,
                TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(bits)) => u128::from(bits),
                TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(bits)) => u128::from(bits),
            };
            let mask = if width == 16 {
                u128::MAX
            } else {
                (1_u128 << (usize::from(width) * 8)) - 1
            };
            Ok(bits & mask)
        }

        pub(crate) fn decode(
            stored: ScalarType,
            bits: u128,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            let width = reference_storage_byte_width(stored).ok_or_else(malformed)?;
            let result = match stored {
                ScalarType::Boolean => boolean_bits(bits)?,
                ScalarType::Integer(integer) => {
                    let value = match integer.sign() {
                        IntegerSign::Unsigned => IntegerValue::Unsigned(bits),
                        IntegerSign::Signed => {
                            let bit_count = usize::from(width) * 8;
                            let signed =
                                if bit_count == 128 || bits & (1_u128 << (bit_count - 1)) == 0 {
                                    bits as i128
                                } else {
                                    (bits | !((1_u128 << bit_count) - 1)) as i128
                                };
                            IntegerValue::Signed(signed)
                        }
                    };
                    TerminalScalarValue::Integer {
                        scalar_type: integer,
                        value,
                    }
                }
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => TerminalScalarValue::IeeeFloat(
                    IeeeFloatValue::Binary32(u32::try_from(bits).map_err(|_| malformed())?),
                ),
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => TerminalScalarValue::IeeeFloat(
                    IeeeFloatValue::Binary64(u64::try_from(bits).map_err(|_| malformed())?),
                ),
            };
            terminal_scalar_belongs_to_type(result)
                .then_some(result)
                .ok_or_else(malformed)
        }

        pub(crate) fn boolean_bits(
            bits: u128,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            Ok(match bits {
                0 => TerminalScalarValue::Boolean(false),
                1 => TerminalScalarValue::Boolean(true),
                _ => return Err(malformed()),
            })
        }

        #[cfg(test)]
        mod tests {
            use super::{convert_scalar, decode, encode, scalar_fits_storage};
            use crate::boundary::values::TerminalScalarValue;
            use semantic_vocabulary::{
                IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, ScalarType,
            };

            fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
                IntegerType::new(sign, bits).expect("test integer carrier")
            }

            #[test]
            fn signed_byte_codec_extends_and_rejects_nonfitting_writes() {
                let stored = ScalarType::Integer(integer(IntegerSign::Signed, 8));
                let logical = integer(IntegerSign::Signed, 64);
                let negative = TerminalScalarValue::Integer {
                    scalar_type: logical,
                    value: IntegerValue::Signed(-128),
                };
                assert_eq!(encode(stored, negative).expect("signed byte"), 128);
                assert_eq!(
                    convert_scalar(
                        decode(stored, 128).expect("decode byte"),
                        ScalarType::Integer(logical)
                    )
                    .expect("extend byte"),
                    negative
                );
                for value in [-129, 128] {
                    assert!(!scalar_fits_storage(
                        stored,
                        TerminalScalarValue::Integer {
                            scalar_type: logical,
                            value: IntegerValue::Signed(value)
                        }
                    ));
                }
                assert!(scalar_fits_storage(
                    stored,
                    TerminalScalarValue::Integer {
                        scalar_type: logical,
                        value: IntegerValue::Signed(127)
                    }
                ));
            }

            #[test]
            fn arbitrary_byte_aligned_integer_widths_keep_exact_sign() {
                let stored = ScalarType::Integer(integer(IntegerSign::Signed, 24));
                let decoded = decode(stored, 0x80_0000).expect("signed 24-bit decode");
                assert_eq!(
                    decoded,
                    TerminalScalarValue::Integer {
                        scalar_type: integer(IntegerSign::Signed, 24),
                        value: IntegerValue::Signed(-8_388_608)
                    }
                );
                let full = ScalarType::Integer(integer(IntegerSign::Signed, 128));
                assert_eq!(
                    encode(
                        full,
                        decode(full, 1_u128 << 127).expect("signed 128-bit decode")
                    )
                    .expect("signed 128-bit encode"),
                    1_u128 << 127
                );
            }

            #[test]
            fn boolean_and_ieee_codecs_keep_canonical_representation() {
                assert_eq!(
                    decode(ScalarType::Boolean, 1).expect("canonical boolean"),
                    TerminalScalarValue::Boolean(true)
                );
                assert!(decode(ScalarType::Boolean, 2).is_err());
                for byte in [0, 1] {
                    assert_eq!(
                        encode(
                            ScalarType::Boolean,
                            TerminalScalarValue::Integer {
                                scalar_type: integer(IntegerSign::Unsigned, 8),
                                value: IntegerValue::Unsigned(byte),
                            }
                        )
                        .expect("canonical Boolean byte"),
                        byte
                    );
                }
                assert!(
                    encode(
                        ScalarType::Boolean,
                        TerminalScalarValue::Integer {
                            scalar_type: integer(IntegerSign::Unsigned, 8),
                            value: IntegerValue::Unsigned(2),
                        }
                    )
                    .is_err()
                );
                let float =
                    TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(0xfff8_0000_0000_0123));
                let stored = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
                assert_eq!(
                    decode(stored, encode(stored, float).expect("IEEE encode"))
                        .expect("IEEE decode"),
                    float
                );
            }
        }
    }

    pub(crate) use encoding::{scalar_fits_storage, shifted_byte_endpoint};

    pub(crate) struct ObservedScalarField<'a> {
        pub(crate) carrier: TerminalStructuralValue,
        pub(crate) leaf: TerminalStructuralValue,
        pub(crate) declaration: &'a terminal_psi::StructuralFieldDeclaration,
        pub(crate) stored_type: ScalarType,
        pub(crate) cell: StructuralScalarRuntimeField,
    }

    pub(crate) fn scalar_field_projection(
        path: &[StructuralPathSegment],
    ) -> Option<(&[StructuralPathSegment], &[StructuralPathSegment])> {
        let (StructuralPathSegment::Field(_), carrier) = path.split_last()? else {
            return None;
        };
        let split = if matches!(carrier.last(), Some(StructuralPathSegment::Case(_))) {
            carrier.len() - 1
        } else {
            carrier.len()
        };
        Some(path.split_at(split))
    }

    #[derive(Clone, Copy)]
    enum ScalarStorageObservation {
        OrdinaryReference { shared_view: bool },
        ExclusiveSnapshot,
        OwnedSnapshot,
        AtomicEvent,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) struct PrimitiveStorageHome {
        depth: usize,
        machine: MachineId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    }

    impl PrimitiveStorageHome {
        pub(crate) fn captured_frame(
            depth: usize,
            machine: MachineId,
            place: PlaceId,
            structural_type: StructuralTypeId,
        ) -> Self {
            Self {
                depth,
                machine,
                place,
                structural_type,
            }
        }
        pub(crate) fn scalar_array_frame(self) -> (usize, PlaceId, StructuralTypeId) {
            (self.depth, self.place, self.structural_type)
        }
    }

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) enum PrimitiveStorageEndpoint {
        RepresentationByte(RepresentationByte),
        Projection {
            fragments: Vec<PrimitiveStorageFragment>,
            logical: ScalarType,
            stored: ScalarType,
        },
        Scalar(StructuralRuntimePlace),
        ScalarField(StructuralScalarRuntimeField),
        ArrayElement {
            array: crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorage,
            index: usize,
        },
        OwnedArrayElement {
            home: PrimitiveStorageHome,
            index: usize,
        },
        ViewByte {
            bytes: crate::storage::byte_sequences::ByteSequenceView,
            index: usize,
        },
    }

    #[derive(Clone)]
    pub(crate) struct RepresentationByte {
        pub(crate) view: std::sync::Arc<crate::storage::element_views::ElementView>,
        pub(crate) index: usize,
    }

    impl PartialEq for RepresentationByte {
        fn eq(&self, other: &Self) -> bool {
            std::sync::Arc::ptr_eq(&self.view, &other.view) && self.index == other.index
        }
    }
    impl Eq for RepresentationByte {}

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) struct PrimitiveStorageFragment {
        pub(crate) storage: PrimitiveStorage,
        logical: ScalarType,
        stored: ScalarType,
        pub(crate) source_bit_offset: u8,
        destination_bit_offset: u8,
        bit_length: u8,
        addressable_cell: bool,
    }

    impl PrimitiveStorageFragment {
        pub(crate) fn contiguous_address_endpoint(
            &self,
            observed: ScalarType,
        ) -> Option<PrimitiveStorageEndpoint> {
            let width = terminal_semantics::reference_storage_byte_width(observed)?;
            (self.addressable_cell
                && self.source_bit_offset == 0
                && self.destination_bit_offset == 0
                && u16::from(self.bit_length) == u16::from(width) * 8)
                .then(|| self.storage.endpoint.clone())
        }
    }

    pub(crate) enum ArrayCell {
        Frame {
            home: crate::storage::primitive_storage::PrimitiveStorageHome,
            index: usize,
        },
        Structural {
            array: StructuralRuntimePlace,
            index: usize,
        },
    }

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) struct PrimitiveStorage {
        pub(crate) endpoint: PrimitiveStorageEndpoint,
        pub(crate) encoding: Option<PrimitiveStorageEncoding>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct PrimitiveStorageEncoding {
        pub(crate) source: ScalarType,
        pub(crate) view: ScalarType,
    }

    impl PrimitiveStorageEncoding {
        pub(crate) fn source_value(
            self,
            value: TerminalScalarValue,
            source: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            let bits = encoding::encode(self.view, value)?;
            encoding::convert_scalar(encoding::decode(self.source, bits)?, source)
        }
    }

    impl PrimitiveStorage {
        fn canonical(endpoint: PrimitiveStorageEndpoint) -> Self {
            Self {
                endpoint,
                encoding: None,
            }
        }

        pub(crate) fn storage_scalar(&self) -> Option<ScalarType> {
            self.encoding
                .map(|encoding| encoding.view)
                .or(match &self.endpoint {
                    PrimitiveStorageEndpoint::Projection { stored, .. } => Some(*stored),
                    _ => None,
                })
        }
    }

    struct PreparedPrimitiveWrite {
        destination: PlaceId,
        endpoint: PrimitiveStorageEndpoint,
        value: TerminalScalarValue,
    }

    #[derive(Default)]
    pub(crate) struct PrimitiveStorageWrites {
        first_write: Option<PreparedPrimitiveWrite>,
        remaining_writes: Vec<PreparedPrimitiveWrite>,
    }

    impl PrimitiveStorageWrites {
        pub(crate) fn stage(
            &mut self,
            execution: &TerminalExecution,
            destination: PlaceId,
            storage: PrimitiveStorage,
            scalar_type: ScalarType,
            value: TerminalScalarValue,
        ) -> Result<(), TerminalInterpretError> {
            if value.scalar_type() != scalar_type || !terminal_scalar_belongs_to_type(value) {
                return Err(malformed());
            }
            match &storage.endpoint {
                PrimitiveStorageEndpoint::Scalar(home) => {
                    execution.require_guest_storage_mutation(home)?;
                }
                PrimitiveStorageEndpoint::ScalarField(field) => {
                    execution.require_guest_storage_mutation(&field.parent)?;
                }
                _ => {}
            }
            if let PrimitiveStorageEndpoint::Projection {
                fragments,
                logical,
                stored,
            } = &storage.endpoint
            {
                if *logical != scalar_type {
                    return Err(malformed());
                }
                return self.stage_projection(execution, destination, fragments, *stored, value);
            }
            if let Some(encoding) = storage.encoding {
                let value = if encoding.view != encoding.source {
                    encoding.source_value(value, encoding.source)?
                } else {
                    value
                };
                return execution.prepare_encoded_primitive(
                    self,
                    destination,
                    &storage.endpoint,
                    encoding.source,
                    value,
                );
            }
            match &storage.endpoint {
                PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                    execution.owned_array_home(home)?;
                    let arrays = if home.depth == execution.call_stack.len() {
                        &execution.scalar_array_values
                    } else {
                        &execution.frame_at_depth(home.depth)?.scalar_array_values
                    };
                    let stored = arrays
                        .backing(&home.place)
                        .filter(|array| array.structural_type == home.structural_type)
                        .and_then(|array| array.cell(*index))
                        .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                            destination,
                        ))?;
                    if stored.scalar_type() != scalar_type {
                        return Err(malformed());
                    }
                }
                PrimitiveStorageEndpoint::Scalar(home) => {
                    if execution
                        .structural_primitive_storage
                        .get(home)
                        .is_some_and(|stored| stored.scalar_type() != scalar_type)
                    {
                        return Err(malformed());
                    }
                }
                PrimitiveStorageEndpoint::ScalarField(field) => {
                    let stored = execution.structural_scalar_fields.get(field).ok_or(
                        TerminalInterpretError::StructuralScalarFieldMissing {
                            source: destination,
                            field: field.field,
                        },
                    )?;
                    if stored.scalar_type() != scalar_type {
                        return Err(malformed());
                    }
                }
                PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                    let TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(byte),
                        ..
                    } = value
                    else {
                        return Err(malformed());
                    };
                    u8::try_from(byte).map_err(|_| malformed())?;
                    if *index >= execution.stored_byte_array_length(array)? {
                        return Err(malformed());
                    }
                }
                PrimitiveStorageEndpoint::RepresentationByte(_)
                | PrimitiveStorageEndpoint::ViewByte { .. }
                | PrimitiveStorageEndpoint::Projection { .. } => return Err(malformed()),
            }
            let write = PreparedPrimitiveWrite {
                destination,
                endpoint: storage.endpoint,
                value,
            };
            if self.first_write.is_none() {
                self.first_write = Some(write);
            } else {
                self.remaining_writes.push(write);
            }
            Ok(())
        }

        pub(crate) fn commit(
            self,
            execution: &mut TerminalExecution,
        ) -> Result<(), TerminalInterpretError> {
            for write in self.first_write.into_iter().chain(self.remaining_writes) {
                execution.commit_primitive_store(write.destination, write.endpoint, write.value)?;
            }
            Ok(())
        }
    }

    #[derive(Clone)]
    pub(crate) struct PrimitiveLoan {
        pub(crate) storage: PrimitiveStorage,
        pub(crate) scalar_type: ScalarType,
        pub(crate) shared_view: bool,
        pub(crate) access: StructuralAccess,
        pub(crate) referent: TerminalStructuralValue,
    }

    pub(crate) struct ByteElementStorage {
        pub(crate) storage: PrimitiveStorage,
        pub(crate) referent: TerminalStructuralValue,
    }

    fn local_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
        let declaration = machine.structural_place(place)?;
        let (producer, structural_type) = declaration.kind.as_operation_result()?;
        let operation = machine.operation(producer)?;
        let result = operation.result.structural()?;
        (matches!(
            operation.kind,
            OperationKind::EstablishPrimitiveLocal { .. }
        ) && result.place == place
            && result.structural_type == structural_type
            && result.multiplicity == StructuralMultiplicity::Unrestricted
            && result.qualification_establishments.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty())
        .then_some(structural_type)
    }

    impl TerminalExecution<'_> {
        pub(crate) fn array_cell(
            &self,
            storage: &PrimitiveStorage,
        ) -> Result<Option<ArrayCell>, TerminalInterpretError> {
            use crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorageHome;
            let (home, index) = match &storage.endpoint {
                PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => (*home, *index),
                PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                    let (depth, place, structural_type) = match array.home {
                        ArrayStorageHome::Frame {
                            depth,
                            place,
                            structural_type,
                        } => (depth, place, structural_type),
                        ArrayStorageHome::Runtime => {
                            let payload = self.structural_byte_storage.require(&array.referent)?;
                            if *index >= payload.len() {
                                return Err(malformed());
                            }
                            return Ok(Some(ArrayCell::Structural {
                                array: array.referent.clone(),
                                index: *index,
                            }));
                        }
                    };
                    let (start, end) = self.frame_byte_array_window(array)?;
                    let index = start
                        .checked_add(*index)
                        .filter(|index| *index < end)
                        .ok_or_else(malformed)?;
                    let machine = if depth == self.call_stack.len() {
                        self.current_machine
                    } else {
                        self.call_stack
                            .get(depth)
                            .ok_or_else(malformed)?
                            .current_machine
                    };
                    (
                        crate::storage::primitive_storage::PrimitiveStorageHome::captured_frame(
                            depth,
                            machine,
                            place,
                            structural_type,
                        ),
                        index,
                    )
                }
                _ => return Ok(None),
            };
            self.owned_array_home(&home)?;
            let (depth, place, structural_type) = home.scalar_array_frame();
            let array = self.frame_array(depth, place).ok_or_else(malformed)?;
            if array.structural_type != structural_type || index >= array.len() {
                return Err(malformed());
            }
            Ok(Some(ArrayCell::Frame { home, index }))
        }

        pub(crate) fn record_original_scalar_write(
            &mut self,
            storage: &PrimitiveStorage,
        ) -> Result<(), TerminalInterpretError> {
            let changed = match &storage.endpoint {
                PrimitiveStorageEndpoint::Projection { fragments, .. } => {
                    for fragment in fragments {
                        self.record_original_scalar_write(&fragment.storage)?;
                    }
                    return Ok(());
                }
                PrimitiveStorageEndpoint::Scalar(home) => {
                    self.invalidate_stored_provenance(home);
                    return Ok(());
                }
                PrimitiveStorageEndpoint::ScalarField(field) => {
                    self.invalidate_stored_provenance(&field.parent);
                    return Ok(());
                }
                PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                    if *index >= self.stored_byte_array_length(array)? {
                        return Err(malformed());
                    }
                    let mut selected = array.referent.clone();
                    selected.path.push(StructuralPathSegment::FixedIndex(
                        u64::try_from(*index).map_err(|_| malformed())?,
                    ));
                    Some(selected)
                }
                PrimitiveStorageEndpoint::OwnedArrayElement { .. } => None,
                PrimitiveStorageEndpoint::RepresentationByte(_)
                | PrimitiveStorageEndpoint::ViewByte { .. } => return Err(malformed()),
            };
            let physical = match self.array_cell(storage)? {
                Some(ArrayCell::Frame { home, index }) => {
                    self.owned_array_home(&home)?;
                    let (depth, place, structural_type) = home.scalar_array_frame();
                    let array = self.frame_array(depth, place).ok_or_else(malformed)?;
                    if !matches!(
                        array.cell(index),
                        Some(crate::storage::scalar_storage::ScalarCell::Resident { .. })
                    ) {
                        return Err(malformed().in_argument_preparation(
                            "original frame write lost its resident cell",
                        ));
                    }
                    let path = terminal_semantics::scalar_array_leaf_path(
                        self.structural_types().values(),
                        structural_type,
                        u64::try_from(index).map_err(|_| malformed())?,
                    )
                    .ok_or_else(malformed)?;
                    Some(StructuralRuntimePlace {
                        opaque_identity: array.storage_identity.ok_or_else(malformed)?,
                        path,
                    })
                }
                Some(ArrayCell::Structural { array, index }) => {
                    if self
                        .structural_byte_storage
                        .require(&array)?
                        .capture_scalar(index)?
                        .1
                        .is_none()
                    {
                        return Err(malformed().in_argument_preparation(
                            "original structural write lost its resident cell",
                        ));
                    }
                    None
                }
                None => return Err(malformed()),
            };
            for owner in changed.into_iter().chain(physical) {
                self.invalidate_stored_provenance(&owner);
            }
            Ok(())
        }
        pub(crate) fn capture_primitive_loan(
            &self,
            parameter: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            identity_cursor: &mut Option<u64>,
        ) -> Result<PrimitiveLoan, TerminalInterpretError> {
            let terminal_psi::StructuralTypeShape::PrimitiveScalar(expected) = self
                .structural_declaration(parameter.structural_type)?
                .shape
            else {
                return Err(malformed());
            };
            if parameter.access != argument.access
                || !matches!(
                    parameter.access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            {
                return Err(malformed());
            }
            if argument.path.is_empty()
                && let Some(loan) = self.primitive_loan_values.get(&argument.place)
            {
                let value = self.structural_value(argument.place)?;
                if *value != loan.referent
                    || value.structural_type != parameter.structural_type
                    || (parameter.access != StructuralAccess::SharedBorrow
                        && loan.access == StructuralAccess::SharedBorrow)
                    || (parameter.access == StructuralAccess::SharedBorrow
                        && loan.access == StructuralAccess::WriteOnlyBorrow)
                    || (parameter.access == StructuralAccess::MutableBorrow
                        && loan.access == StructuralAccess::WriteOnlyBorrow)
                {
                    return Err(malformed());
                }
                let mut rebound = loan.clone();
                rebound.access = parameter.access;
                return Ok(rebound);
            }
            let retained_reference = if let Some(crossing) = argument
                .path
                .iter()
                .rposition(|segment| *segment == StructuralPathSegment::Referent)
                .filter(|crossing| *crossing + 1 == argument.path.len())
            {
                let carrier = self.resolve_live_projection(
                    argument.place,
                    &argument.path[..crossing],
                    argument.access,
                )?;
                self.reference_referents
                    .get(&StructuralRuntimePlace::from(&carrier))
                    .and_then(|referent| referent.primitive_loan.as_ref())
            } else {
                None
            };
            let (referent, storage, scalar_type, shared_view) = if let Some(loan) =
                retained_reference
            {
                let referent =
                    self.resolve_live_projection(argument.place, &argument.path, argument.access)?;
                if referent.structural_type != parameter.structural_type {
                    return Err(malformed());
                }
                (
                    referent,
                    loan.storage.clone(),
                    expected,
                    parameter.access == StructuralAccess::SharedBorrow,
                )
            } else if let Some(mut element) = self.byte_element_storage(argument)? {
                element.referent.structural_type = parameter.structural_type;
                (
                    element.referent,
                    element.storage,
                    ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
                    ),
                    parameter.access == StructuralAccess::SharedBorrow,
                )
            } else if self.scalar_array_values.contains_key(&argument.place) {
                let (storage, scalar_type, shared_view) = self.primitive_access(
                    argument.place,
                    parameter.access != StructuralAccess::SharedBorrow,
                    &argument.path,
                )?;
                let (opaque_identity, path) = match &storage.endpoint {
                    PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                        let mut path = array.referent.path.clone();
                        path.push(StructuralPathSegment::FixedIndex(
                            u64::try_from(*index).map_err(|_| malformed())?,
                        ));
                        (array.referent.opaque_identity, path)
                    }
                    PrimitiveStorageEndpoint::OwnedArrayElement { .. } => (
                        self.local_structural_identities
                            .allocate_staged(identity_cursor)?,
                        argument.path.clone(),
                    ),
                    _ => return Err(malformed()),
                };
                (
                    TerminalStructuralValue {
                        opaque_identity,
                        structural_type: parameter.structural_type,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        path,
                    },
                    storage,
                    scalar_type,
                    shared_view,
                )
            } else {
                let projected = self.resolve_live_primitive_projection(
                    argument.place,
                    &argument.path,
                    argument.access,
                    parameter.structural_type,
                )?;
                let value = projected.0;
                if value.structural_type != parameter.structural_type {
                    return Err(malformed());
                }
                let (storage, scalar_type, shared_view) = match projected.1 {
                    Some(storage) => (
                        storage,
                        expected,
                        parameter.access == StructuralAccess::SharedBorrow,
                    ),
                    None => {
                        let field = if let Some((prefix, field_path)) =
                            scalar_field_projection(&argument.path)
                        {
                            let (carrier, view) = self.resolve_live_view_projection(
                                argument.place,
                                prefix,
                                argument.access,
                            )?;
                            if view.is_none() && self.reference_byte_region(&carrier)?.is_none() {
                                // A field lent under another scalar carrier keeps its own
                                // stored carrier; the loan reads and writes through the
                                // admitted representation view of that storage.
                                let stored = terminal_semantics::runtime_structural_field(
                                    self.structural_types().values(),
                                    carrier.structural_type,
                                    field_path,
                                )
                                .and_then(|field| field.field_type.scalar_type())
                                .filter(|stored| {
                                    terminal_semantics::scalar_reference_view_admits(
                                        parameter.access,
                                        *stored,
                                        expected,
                                    )
                                })
                                .unwrap_or(expected);
                                self.scalar_field_at_carrier(carrier, view, field_path, stored)?
                                    .map(|observed| (observed.cell, stored))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        if let Some((field, stored)) = field {
                            (
                                PrimitiveStorage {
                                    endpoint: PrimitiveStorageEndpoint::ScalarField(field),
                                    encoding: (stored != expected).then_some(
                                        PrimitiveStorageEncoding {
                                            source: stored,
                                            view: expected,
                                        },
                                    ),
                                },
                                expected,
                                parameter.access == StructuralAccess::SharedBorrow,
                            )
                        } else {
                            self.primitive_view_access(
                                &value,
                                &[],
                                expected,
                                parameter.access == StructuralAccess::SharedBorrow,
                            )?
                        }
                    }
                };
                (value, storage, scalar_type, shared_view)
            };
            if scalar_type != expected {
                return Err(malformed());
            }
            Ok(PrimitiveLoan {
                storage,
                scalar_type,
                shared_view,
                access: parameter.access,
                referent,
            })
        }

        fn byte_element_storage(
            &self,
            argument: &StructuralArgument,
        ) -> Result<Option<ByteElementStorage>, TerminalInterpretError> {
            let Some((StructuralPathSegment::FixedIndex(index), prefix)) =
                argument.path.split_last()
            else {
                return Ok(None);
            };
            let types = self.structural_types();
            let root = self.live_structural_type(argument.place)?;
            if !terminal_semantics::runtime_structural_path_tip(types.values(), root, prefix)
                .is_some_and(|row| {
                    row.shape.is_borrowed_byte_carrier()
                        || row.shape.is_bounded_owned_byte_sequence()
                })
            {
                return Ok(None);
            }
            let source = StructuralArgument {
                place: argument.place,
                path: prefix.to_vec(),
                access: argument.access,
            };
            self.validate_descriptor_borrow(&source, argument.access)?;
            let descriptor = self.resolve_live_projection(source.place, prefix, source.access)?;
            let binding = self.byte_descriptor_binding(&source, &descriptor)?;
            self.bound_byte_element_storage(&descriptor, &binding, *index, argument.access)
                .map(Some)
        }

        pub(crate) fn bound_byte_element_storage(
            &self,
            descriptor: &TerminalStructuralValue,
            binding: &crate::storage::byte_sequences::binding::ByteSequenceBinding,
            index: u64,
            access: StructuralAccess,
        ) -> Result<ByteElementStorage, TerminalInterpretError> {
            use crate::storage::byte_sequences::binding::ByteSequenceBinding;
            if index >= self.byte_descriptor_length(binding, descriptor)? {
                return Err(malformed());
            }
            let mut referent = descriptor.clone();
            referent.qualifications.clear();
            referent.projected_qualifications.clear();
            referent.path.push(StructuralPathSegment::FixedIndex(index));
            let endpoint = match binding {
                ByteSequenceBinding::Representation(view)
                    if access == StructuralAccess::SharedBorrow =>
                {
                    PrimitiveStorageEndpoint::RepresentationByte(RepresentationByte {
                        view: std::sync::Arc::clone(view),
                        index: usize::try_from(index).map_err(|_| malformed())?,
                    })
                }
                ByteSequenceBinding::Immutable(bytes)
                    if access == StructuralAccess::SharedBorrow =>
                {
                    PrimitiveStorageEndpoint::ViewByte {
                        bytes: bytes.clone(),
                        index: usize::try_from(index).map_err(|_| malformed())?,
                    }
                }
                ByteSequenceBinding::MutableField { .. }
                | ByteSequenceBinding::MutableOwner { .. }
                    if access == StructuralAccess::SharedBorrow =>
                {
                    let (bytes, _) = self.exclusive_owner_bytes(binding)?;
                    PrimitiveStorageEndpoint::ViewByte {
                        bytes: bytes.clone(),
                        index: usize::try_from(index).map_err(|_| malformed())?,
                    }
                }
                ByteSequenceBinding::MutableArray {
                    array,
                    array_type,
                    offset,
                    ..
                } => {
                    let extent =
                        crate::boundary::structural_inputs::byte_arrays::byte_array_length(
                            &self.structural_types(),
                            *array_type,
                        )
                        .ok_or_else(malformed)?;
                    if self.stored_byte_array_length(array)? as u128 != u128::from(extent) {
                        return Err(malformed());
                    }
                    let index = offset.checked_add(index).ok_or_else(malformed)?;
                    referent.opaque_identity = array.referent.opaque_identity;
                    referent.path = array.referent.path.clone();
                    referent.path.push(StructuralPathSegment::FixedIndex(index));
                    PrimitiveStorageEndpoint::ArrayElement {
                        array: array.clone(),
                        index: usize::try_from(index).map_err(|_| malformed())?,
                    }
                }
                _ => return Err(malformed()),
            };
            Ok(ByteElementStorage {
                storage: PrimitiveStorage::canonical(endpoint),
                referent,
            })
        }

        pub(crate) fn live_structural_type(
            &self,
            place: PlaceId,
        ) -> Result<StructuralTypeId, TerminalInterpretError> {
            match (
                self.structural_values.get(&place),
                self.scalar_array_values.get(&place),
            ) {
                (Some(value), None) => Ok(value.structural_type),
                (None, Some(array)) => Ok(array.structural_type),
                (None, None) => Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    place,
                )),
                (Some(_), Some(_)) => Err(malformed()),
            }
        }

        pub(crate) fn primitive_access(
            &self,
            place: PlaceId,
            writing: bool,
            path: &[StructuralPathSegment],
        ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
            self.primitive_home_access(self.capture_primitive_home(place)?, writing, path)
        }

        fn reference_primitive_access(
            &self,
            owner: &TerminalStructuralValue,
            writing: bool,
            path: &[StructuralPathSegment],
        ) -> Result<Option<(PrimitiveStorage, ScalarType, bool)>, TerminalInterpretError> {
            if let Some(crossing) = path
                .iter()
                .rposition(|segment| *segment == StructuralPathSegment::Referent)
            {
                let access = if writing {
                    StructuralAccess::WriteOnlyBorrow
                } else {
                    StructuralAccess::SharedBorrow
                };
                let captured = self
                    .resolve_reference_storage_projection(owner, path, access)?
                    .ok_or_else(malformed)?;
                if let Some(storage) = captured.primitive_storage {
                    let projection_type = if writing {
                        terminal_semantics::primitive_projection_type
                    } else {
                        terminal_semantics::primitive_read_projection_type
                    };
                    let scalar_type = projection_type(
                        self.structural_types().values(),
                        owner.structural_type,
                        path,
                    )
                    .ok_or_else(malformed)?;
                    return Ok(Some((storage, scalar_type, !writing)));
                }
                let (referent, view) = self
                    .resolve_reference_view_path(owner, &path[..=crossing], access)?
                    .ok_or_else(malformed)?;
                if let Some(view) = view {
                    return self
                        .aggregate_view_primitive_access(
                            &referent,
                            view,
                            &path[crossing + 1..],
                            None,
                            writing,
                        )
                        .map(Some);
                }
                let suffix = &path[crossing + 1..];
                let mut projected = referent.clone();
                projected.path.extend_from_slice(suffix);
                if let Some((storage, scalar_type)) =
                    self.reference_byte_region_access(&referent, &projected)?
                {
                    return Ok(Some((storage, scalar_type, !writing)));
                }
                let projection_type = if writing {
                    terminal_semantics::primitive_projection_type
                } else {
                    terminal_semantics::primitive_read_projection_type
                };
                let scalar_type = projection_type(
                    self.structural_types().values(),
                    referent.structural_type,
                    suffix,
                )
                .ok_or_else(malformed)?;
                if suffix.is_empty() {
                    let (carrier, _) = self
                        .resolve_reference_view_path(owner, &path[..crossing], access)?
                        .ok_or_else(malformed)?;
                    if let Some(captured) = self
                        .reference_referents
                        .get(&StructuralRuntimePlace::from(&carrier))
                        && let Some(loan) = &captured.primitive_loan
                    {
                        if captured.view != loan.referent
                            || (writing
                                && !matches!(
                                    loan.access,
                                    terminal_psi::exclusive_borrow_pair_pattern!()
                                ))
                        {
                            return Err(malformed());
                        }
                        return Ok(Some((loan.storage.clone(), scalar_type, !writing)));
                    }
                }
                let canonical_path = if writing {
                    terminal_semantics::canonical_primitive_path
                } else {
                    terminal_semantics::canonical_primitive_read_path
                };
                let canonical = canonical_path(
                    self.structural_types().values(),
                    referent.structural_type,
                    suffix,
                )
                .ok_or_else(malformed)?;
                return self
                    .primitive_view_access(&referent, canonical.as_slice(), scalar_type, false)
                    .map(Some);
            }
            Ok(None)
        }

        fn byte_element_scalar(
            &self,
            structural_type: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Result<ScalarType, TerminalInterpretError> {
            if matches!(
                self.structural_declaration(structural_type)?.shape,
                terminal_psi::StructuralTypeShape::ByteSequence(_)
            ) {
                return Ok(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
                ));
            }
            terminal_semantics::primitive_projection_type(
                self.structural_types().values(),
                structural_type,
                path,
            )
            .ok_or_else(malformed)
        }

        pub(crate) fn aggregate_view_primitive_access(
            &self,
            referent: &TerminalStructuralValue,
            view: crate::storage::reference::AggregateViewTip,
            suffix: &[StructuralPathSegment],
            home: Option<&crate::storage::element_views::ElementViewHome>,
            writing: bool,
        ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
            let types = self.structural_types();
            let crate::storage::reference::AggregateViewTip::Aggregate { view, backing } = view
            else {
                return Err(malformed());
            };
            if backing != referent.structural_type {
                return Err(malformed());
            }
            let (suffix, tip) = crate::storage::reference::translate_aggregate_view_path(
                &types, view, backing, suffix,
            )
            .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                "aggregate re-view has no unique contiguous scalar endpoint",
            ))?;
            if let crate::storage::reference::AggregateViewTip::ScalarRegion {
                view: observed,
                byte_length,
            } = tip
            {
                return self.aggregate_projection_storage(
                    referent,
                    observed,
                    byte_length,
                    home,
                    writing,
                );
            }
            let crate::storage::reference::AggregateViewTip::Scalar {
                view: observed,
                backing: stored,
            } = tip
            else {
                return Err(malformed());
            };
            let canonical = crate::storage::reference::backing_scalar_path(
                &types, backing, &suffix,
            )
            .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                "aggregate re-view backing scalar has no retained cell coordinate",
            ))?;
            let (mut storage, _, shared_view) = self.original_view_primitive_access(
                referent,
                home,
                &canonical,
                stored.logical_scalar,
                writing,
            )?;
            if storage
                .encoding
                .is_some_and(|encoding| encoding.source != stored.stored_scalar)
                || !terminal_semantics::reference_storage_view_is_compatible(
                    observed.stored_scalar,
                    observed.logical_scalar,
                )
            {
                return Err(malformed());
            }
            if stored.stored_scalar != stored.logical_scalar
                || observed.logical_scalar != stored.logical_scalar
                || observed.stored_scalar != stored.stored_scalar
            {
                storage.encoding = Some(PrimitiveStorageEncoding {
                    source: stored.stored_scalar,
                    view: observed.stored_scalar,
                });
            }
            Ok((storage, observed.logical_scalar, shared_view))
        }

        fn original_view_primitive_access(
            &self,
            referent: &TerminalStructuralValue,
            home: Option<&crate::storage::element_views::ElementViewHome>,
            path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
            scalar_type: ScalarType,
            writing: bool,
        ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
            match home {
                Some(home) => {
                    let suffix = terminal_semantics::canonical_structural_storage_path(
                        self.structural_types().values(),
                        referent.structural_type,
                        path,
                    )
                    .ok_or_else(malformed)?;
                    let mut selected = home.path.clone();
                    selected.extend(suffix);
                    self.primitive_home_access(home.storage, writing, &selected)
                }
                None => self.primitive_view_access(referent, path, scalar_type, false),
            }
        }

        pub(crate) fn capture_primitive_home(
            &self,
            place: PlaceId,
        ) -> Result<PrimitiveStorageHome, TerminalInterpretError> {
            if self.scalar_array_values.retains_retired(&place) {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    place,
                ));
            }
            Ok(PrimitiveStorageHome {
                depth: self.call_stack.len(),
                machine: self.current_machine,
                place,
                structural_type: self.live_structural_type(place)?,
            })
        }

        fn primitive_home_contract(
            &self,
            home: PrimitiveStorageHome,
            writing: bool,
            path: &[StructuralPathSegment],
        ) -> Result<(StructuralTypeId, bool), TerminalInterpretError> {
            let machine_id = self.owned_array_home(&home)?;
            let machine = self.program.machine(machine_id)?;
            let preserves =
                |root,
                 whole: &[semantic_vocabulary::StructuralDomainId],
                 projected: &[terminal_psi::StructuralPathQualification],
                 additional_input: &[terminal_psi::StructuralPathQualification]| {
                    terminal_semantics::primitive_store_restriction(
                        self.structural_types().values(),
                        self.program.catalogs.structural_domains.iter(),
                        root,
                        whole,
                        projected,
                        additional_input,
                        path,
                    )
                    .is_some()
                };
            let place = home.place;
            let (structural_type, shared_view) = local_type(machine, place)
                .map(|structural_type| (structural_type, false))
                .or_else(|| {
                    if writing {
                        return None;
                    }
                    let semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type,
                        ..
                    } = machine.structural_place(place)?.kind
                    else {
                        return None;
                    };
                    Some((structural_type, true))
                })
                .or_else(|| {
                    machine
                        .structural_parameter(place)
                        .filter(|parameter| {
                            ((!path.is_empty() && parameter.access == StructuralAccess::Owned)
                                || if writing {
                                    matches!(
                                        parameter.access,
                                        terminal_psi::exclusive_borrow_pair_pattern!()
                                    )
                                } else {
                                    matches!(
                                        parameter.access,
                                        terminal_psi::reading_borrow_pair_pattern!()
                                    )
                                })
                                && (!writing
                                    || ((parameter.multiplicity
                                        == StructuralMultiplicity::Unrestricted
                                        || (!path.is_empty()
                                            && parameter.multiplicity
                                                == StructuralMultiplicity::Affine))
                                        && preserves(
                                            parameter.structural_type,
                                            &parameter.qualifications,
                                            &parameter.projected_qualifications,
                                            &parameter.additional_input_qualifications,
                                        )))
                        })
                        .map(|parameter| {
                            (
                                parameter.structural_type,
                                parameter.access == StructuralAccess::SharedBorrow,
                            )
                        })
                })
                .or_else(|| {
                    if writing && path.is_empty() {
                        return None;
                    }
                    machine
                        .operations()
                        .filter_map(|operation| operation.result.structural())
                        .find(|result| {
                            result.place == place
                                && (!writing
                                    || (result.claims.is_empty()
                                        && preserves(
                                            result.structural_type,
                                            &result.qualifications,
                                            &result.projected_qualifications,
                                            &result.additional_input_qualifications,
                                        )))
                        })
                        .map(|result| (result.structural_type, false))
                })
                .ok_or_else(malformed)?;
            if structural_type != home.structural_type {
                return Err(malformed());
            }
            Ok((structural_type, shared_view))
        }

        fn element_view_scalar_read(
            &self,
            home: PrimitiveStorageHome,
            path: &[StructuralPathSegment],
            expected: ScalarType,
        ) -> Result<Option<TerminalScalarValue>, TerminalInterpretError> {
            let types = self.structural_types();
            for (position, segment) in path.iter().enumerate() {
                let StructuralPathSegment::FixedIndex(index) = segment else {
                    continue;
                };
                let Some(descriptor) = terminal_semantics::runtime_structural_path_tip(
                    types.values(),
                    home.structural_type,
                    &path[..position],
                ) else {
                    continue;
                };
                if !matches!(
                    descriptor.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                ) {
                    continue;
                }
                self.primitive_home_contract(home, false, path)?;
                let homes = if home.depth == self.call_stack.len() {
                    &self.structural_values
                } else {
                    &self
                        .call_stack
                        .get(home.depth)
                        .ok_or_else(malformed)?
                        .structural_values
                };
                let owner = homes.get(&home.place).ok_or_else(malformed)?;
                if owner.structural_type != home.structural_type {
                    return Err(malformed());
                }
                let (selected, aggregate_view) = self
                    .resolve_reference_view_path(
                        owner,
                        &path[..position],
                        StructuralAccess::SharedBorrow,
                    )?
                    .ok_or_else(malformed)?;
                if selected.structural_type != descriptor.id || aggregate_view.is_some() {
                    return Err(malformed());
                }
                let view = self.element_view_at_resolved_home(&selected)?;
                let value = self.read_element_view_scalar(
                    view,
                    usize::try_from(*index).map_err(|_| malformed())?,
                    &path[position + 1..],
                    expected,
                )?;
                return Ok(Some(value));
            }
            Ok(None)
        }

        pub(crate) fn initialized_empty_array_in_frame(
            &self,
            home: PrimitiveStorageHome,
            path: &[StructuralPathSegment],
            expected: StructuralTypeId,
        ) -> Result<bool, TerminalInterpretError> {
            let (machine, values, arrays, views) = if home.depth == self.call_stack.len() {
                (
                    self.current_machine,
                    &self.structural_values,
                    &self.scalar_array_values,
                    &self.element_view_values,
                )
            } else {
                let frame = self.call_stack.get(home.depth).ok_or_else(malformed)?;
                (
                    frame.current_machine,
                    &frame.structural_values,
                    &frame.scalar_array_values,
                    &frame.element_view_values,
                )
            };
            if machine != home.machine {
                return Err(malformed());
            }
            if let Some(loan) = views.get(&home.place) {
                let value = values.get(&home.place).ok_or_else(malformed)?;
                if value.structural_type != home.structural_type
                    || !loan.matches_descriptor_home(value)
                {
                    return Err(malformed());
                }
                return loan.initialized_empty_scalar_array(self, path, expected);
            }
            let types = self.structural_types();
            if crate::engine::custody::resolve_structural_path_type(
                &types,
                home.structural_type,
                path,
            )? != expected
            {
                return Err(malformed());
            }
            if let Some(array) = arrays.backing(&home.place) {
                if array.structural_type != home.structural_type {
                    return Err(malformed());
                }
                return Ok(terminal_semantics::scalar_array_leaf_shape(
                    types.values(),
                    array.structural_type,
                )
                .is_some_and(|(_, count)| count == 0 && array.is_empty()));
            }
            let value = values.get(&home.place).ok_or_else(malformed)?;
            if value.structural_type != home.structural_type {
                return Err(malformed());
            }
            let selected = crate::engine::custody::project_structural_value(&types, value, path)?;
            self.initialized_empty_scalar_array(&StructuralRuntimePlace::from(&selected), expected)
        }

        pub(crate) fn primitive_home_access(
            &self,
            home: PrimitiveStorageHome,
            writing: bool,
            path: &[StructuralPathSegment],
        ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
            let place = home.place;
            let (
                machine_id,
                structural_values,
                scalar_array_values,
                byte_sequence_values,
                primitive_loan_values,
                element_view_values,
                aggregate_argument_backings,
            ) = if home.depth == self.call_stack.len() {
                (
                    self.current_machine,
                    &self.structural_values,
                    &self.scalar_array_values,
                    &self.byte_sequence_values,
                    &self.primitive_loan_values,
                    &self.element_view_values,
                    &self.aggregate_argument_backings,
                )
            } else {
                let frame = self.call_stack.get(home.depth).ok_or_else(malformed)?;
                (
                    frame.current_machine,
                    &frame.structural_values,
                    &frame.scalar_array_values,
                    &frame.byte_sequence_values,
                    &frame.primitive_loan_values,
                    &frame.element_view_values,
                    &frame.aggregate_argument_backings,
                )
            };
            if machine_id != home.machine {
                return Err(malformed());
            }
            if home.depth == self.call_stack.len()
                && let Some(element) = self.byte_element_storage(&StructuralArgument {
                    place,
                    path: path.to_vec(),
                    access: if writing {
                        StructuralAccess::MutableBorrow
                    } else {
                        StructuralAccess::SharedBorrow
                    },
                })?
            {
                let (_, shared_view) = self.primitive_home_contract(home, writing, path)?;
                return Ok((
                    element.storage,
                    ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
                    ),
                    shared_view,
                ));
            }
            if path.contains(&StructuralPathSegment::Referent) {
                self.primitive_home_contract(home, writing, path)?;
                let owner = structural_values.require(&place)?;
                if owner.structural_type != home.structural_type {
                    return Err(malformed());
                }
                if let Some(storage) = self.reference_primitive_access(owner, writing, path)? {
                    return Ok(storage);
                }
            }
            if let Some(loan) = element_view_values.get(&place) {
                self.primitive_home_contract(home, writing, path)?;
                let owner = structural_values.require(&place)?;
                if owner.structural_type != home.structural_type
                    || !loan.matches_descriptor_home(owner)
                {
                    return Err(malformed());
                }
                return loan.scalar_storage(self, path, writing);
            }
            let machine = self.program.machine(machine_id)?;
            if let Some(loan) = primitive_loan_values.get(&place) {
                let parameter = machine.structural_parameter(place).ok_or_else(malformed)?;
                if !path.is_empty()
                    || structural_values.get(&place) != Some(&loan.referent)
                    || loan.referent.structural_type != home.structural_type
                    || parameter.structural_type != home.structural_type
                    || parameter.access != loan.access
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || if writing {
                        !matches!(loan.access, terminal_psi::exclusive_borrow_pair_pattern!())
                    } else {
                        !matches!(loan.access, terminal_psi::reading_borrow_pair_pattern!())
                    }
                {
                    return Err(malformed());
                }
                return Ok((loan.storage.clone(), loan.scalar_type, loan.shared_view));
            }
            let (structural_type, shared_view) =
                self.primitive_home_contract(home, writing, path)?;
            if let Some(backing) = aggregate_argument_backings.get(&place) {
                if let Some(region) = backing.region {
                    let (storage, scalar_type, _) = self.aggregate_view_primitive_access(
                        &backing.value,
                        crate::storage::reference::AggregateViewTip::Aggregate {
                            view: region,
                            backing: backing.value.structural_type,
                        },
                        path,
                        backing.collection_home.as_ref(),
                        writing,
                    )?;
                    return Ok((storage, scalar_type, shared_view));
                }
                let region = backing.byte_region.as_ref().ok_or_else(malformed)?;
                let (storage, scalar_type) =
                    self.captured_byte_region_access(&backing.value, region, path)?;
                return Ok((storage, scalar_type, shared_view));
            }
            if let Some(binding) = byte_sequence_values.get(&place) {
                let descriptor = structural_values.require(&place)?;
                let [StructuralPathSegment::FixedIndex(index)] = path else {
                    return Err(malformed());
                };
                let scalar_type = self.byte_element_scalar(structural_type, path)?;
                let byte_type =
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
                if descriptor.structural_type != structural_type
                    || scalar_type != ScalarType::Integer(byte_type)
                {
                    return Err(malformed());
                }
                let element = self.bound_byte_element_storage(
                    descriptor,
                    binding,
                    *index,
                    if writing {
                        StructuralAccess::MutableBorrow
                    } else {
                        StructuralAccess::SharedBorrow
                    },
                )?;
                return Ok((element.storage, scalar_type, shared_view));
            }
            let projection_type = if writing {
                terminal_semantics::primitive_projection_type
            } else {
                terminal_semantics::primitive_read_projection_type
            };
            let scalar_type =
                projection_type(self.structural_types().values(), structural_type, path)
                    .ok_or_else(malformed)?;
            let canonical_path = if writing {
                terminal_semantics::canonical_primitive_path
            } else {
                terminal_semantics::canonical_primitive_read_path
            };
            let path = canonical_path(self.structural_types().values(), structural_type, path)
                .ok_or_else(malformed)?;
            let path = path.as_slice();
            if let Some(array) = scalar_array_values.backing(&place) {
                if array.structural_type != structural_type
                    || structural_values.contains_key(&place)
                {
                    return Err(malformed());
                }
                let mut carrier = structural_type;
                let mut leaf_offset = 0_u64;
                for segment in path {
                    let semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index) =
                        segment
                    else {
                        return Err(malformed());
                    };
                    let terminal_psi::StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } = self.structural_declaration(carrier)?.shape
                    else {
                        return Err(malformed());
                    };
                    if *index >= length {
                        return Err(malformed());
                    }
                    let leaf_count = match self.structural_declaration(element)?.shape {
                        terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => 1,
                        _ => {
                            terminal_semantics::scalar_array_leaf_shape(
                                self.structural_types().values(),
                                element,
                            )
                            .ok_or_else(malformed)?
                            .1
                        }
                    };
                    leaf_offset = leaf_offset
                        .checked_add(index.checked_mul(leaf_count).ok_or_else(malformed)?)
                        .ok_or_else(malformed)?;
                    carrier = element;
                }
                let index = usize::try_from(leaf_offset).map_err(|_| malformed())?;
                if array
                    .cell(index)
                    .is_none_or(|value| value.scalar_type() != scalar_type)
                {
                    return Err(malformed());
                }
                return Ok((
                    PrimitiveStorage::canonical(PrimitiveStorageEndpoint::OwnedArrayElement {
                        home,
                        index,
                    }),
                    scalar_type,
                    shared_view,
                ));
            }
            let view = crate::engine::custody::verified_structural_place(structural_values, place)?;
            if view.structural_type != structural_type {
                return Err(malformed());
            }
            self.primitive_view_access(view, path, scalar_type, shared_view)
        }

        pub(crate) fn primitive_view_access(
            &self,
            view: &TerminalStructuralValue,
            path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
            scalar_type: ScalarType,
            shared_view: bool,
        ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
            self.resolve_scalar_view_storage(
                view,
                path,
                scalar_type,
                ScalarStorageObservation::OrdinaryReference { shared_view },
            )
        }

        pub(crate) fn boundary_scalar_storage(
            &self,
            view: &TerminalStructuralValue,
            path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
            scalar_type: ScalarType,
            access: StructuralAccess,
        ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
            let observation = match access {
                StructuralAccess::Owned => ScalarStorageObservation::OwnedSnapshot,
                StructuralAccess::MutableBorrow => ScalarStorageObservation::ExclusiveSnapshot,
                StructuralAccess::SharedBorrow => {
                    ScalarStorageObservation::OrdinaryReference { shared_view: true }
                }
                _ => return Err(malformed()),
            };
            self.resolve_scalar_view_storage(view, path, scalar_type, observation)
        }

        fn resolve_scalar_view_storage(
            &self,
            view: &TerminalStructuralValue,
            path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
            scalar_type: ScalarType,
            observation: ScalarStorageObservation,
        ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
            if path.contains(&semantic_vocabulary::CanonicalStructuralPathSegment::Referent) {
                let physical = terminal_semantics::canonical_structural_storage_path(
                    self.structural_types().values(),
                    view.structural_type,
                    path,
                )
                .ok_or_else(malformed)?;
                let resolved = self
                    .reference_primitive_access(view, false, &physical)?
                    .ok_or_else(malformed)?;
                if resolved.1 != scalar_type {
                    return Err(malformed());
                }
                return Ok(resolved);
            }
            let shared_view = matches!(
                observation,
                ScalarStorageObservation::OrdinaryReference { shared_view: true }
            );
            let backing_view = view;
            let mut projected = view.clone();
            let mut carrier = view.structural_type;
            let mut selected_case = None;
            for (position, segment) in path.iter().enumerate() {
                let declaration = self.structural_declaration(carrier)?;
                match (segment, &declaration.shape) {
                    (
                        semantic_vocabulary::CanonicalStructuralPathSegment::Case(case),
                        terminal_psi::StructuralTypeShape::Sum { cases }
                        | terminal_psi::StructuralTypeShape::Mixed { cases, .. },
                    ) => {
                        if selected_case.is_some() {
                            return Err(malformed());
                        }
                        let actual = self
                            .structural_cases
                            .get(&StructuralRuntimePlace::from(&projected))
                            .ok_or_else(malformed)?;
                        if actual.structural_type != carrier || actual.case != *case {
                            return Err(malformed());
                        }
                        selected_case = Some(
                            cases
                                .iter()
                                .find(|row| row.id == *case)
                                .ok_or_else(malformed)?,
                        );
                    }
                    (semantic_vocabulary::CanonicalStructuralPathSegment::Field(identity), _) => {
                        let payload_case = selected_case.as_ref().map(|case| case.id);
                        let fields = if let Some(case) = selected_case.take() {
                            case.fields.as_slice()
                        } else {
                            declaration.shape.direct_fields().ok_or_else(malformed)?
                        };
                        let field = live_field_by_id(fields, *identity).ok_or_else(malformed)?;
                        if position + 1 == path.len()
                            && (!matches!(
                                field.field_type,
                                terminal_psi::StructuralFieldType::Atomic(_)
                                    | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
                            ) || matches!(
                                observation,
                                ScalarStorageObservation::ExclusiveSnapshot
                            ))
                            && field.field_type.scalar_type() == Some(scalar_type)
                        {
                            let mut leaf = projected.clone();
                            leaf.path.push(StructuralPathSegment::Field(field.identity));
                            if let Some((storage, logical_scalar)) =
                                self.reference_byte_region_access(backing_view, &leaf)?
                            {
                                if logical_scalar != scalar_type
                                    || matches!(
                                    field.field_type,
                                    terminal_psi::StructuralFieldType::Atomic(_)
                                        | terminal_psi::StructuralFieldType::AtomicBoundedInteger(
                                            _
                                        )
                                ) {
                                    return Err(malformed());
                                }
                                return Ok((storage, scalar_type, shared_view));
                            }
                            return Ok((
                                PrimitiveStorage::canonical(PrimitiveStorageEndpoint::ScalarField(
                                    if payload_case.is_some() {
                                        StructuralScalarRuntimeField::case_payload(
                                            StructuralRuntimePlace::from(&projected),
                                            field.id,
                                        )
                                    } else {
                                        StructuralScalarRuntimeField::member(
                                            StructuralRuntimePlace::from(&projected),
                                            field.id,
                                        )
                                    },
                                )),
                                scalar_type,
                                shared_view,
                            ));
                        }
                        let Some(child) = field.field_type.as_structural() else {
                            return Err(malformed());
                        };
                        if let Some(case) = payload_case {
                            projected.path.push(StructuralPathSegment::Case(case));
                        }
                        projected
                            .path
                            .push(StructuralPathSegment::Field(field.identity));
                        carrier = child;
                    }
                    (
                        semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index),
                        terminal_psi::StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        },
                    ) if index < length => {
                        projected
                            .path
                            .push(StructuralPathSegment::FixedIndex(*index));
                        carrier = *element;
                    }
                    _ => return Err(malformed()),
                }
            }
            projected.structural_type = carrier;
            let view = &projected;
            if let Some((storage, logical_scalar)) =
                self.reference_byte_region_access(backing_view, view)?
            {
                if logical_scalar != scalar_type {
                    return Err(malformed());
                }
                return Ok((storage, scalar_type, shared_view));
            }
            if let Some((StructuralPathSegment::FixedIndex(index), parent_path)) =
                view.path.split_last()
            {
                let array = StructuralRuntimePlace {
                    opaque_identity: view.opaque_identity,
                    path: parent_path.to_vec(),
                };
                if let Some(bytes) = self.structural_byte_storage.get(&array) {
                    let byte_type =
                        IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
                    let index = usize::try_from(*index).map_err(|_| malformed())?;
                    if scalar_type != ScalarType::Integer(byte_type)
                        || index >= bytes.len()
                        || self
                            .structural_primitive_storage
                            .contains_key(&StructuralRuntimePlace::from(view))
                    {
                        return Err(malformed());
                    }
                    return Ok((
                        PrimitiveStorage::canonical(PrimitiveStorageEndpoint::ArrayElement {
                            array: array.into(),
                            index,
                        }),
                        scalar_type,
                        shared_view,
                    ));
                }
            }
            if self.structural_byte_storage.keys().any(|array| {
                array.opaque_identity == view.opaque_identity && view.path.starts_with(&array.path)
            }) {
                return Err(malformed());
            }
            if let Some((StructuralPathSegment::Field(identity), parent_path)) =
                view.path.split_last()
            {
                let field = self.scalar_leaf_field(
                    view,
                    *identity,
                    parent_path,
                    scalar_type,
                    ScalarStorageObservation::OrdinaryReference { shared_view },
                )?;
                return Ok((
                    PrimitiveStorage::canonical(PrimitiveStorageEndpoint::ScalarField(field)),
                    scalar_type,
                    shared_view,
                ));
            }
            Ok((
                PrimitiveStorage::canonical(PrimitiveStorageEndpoint::Scalar(
                    StructuralRuntimePlace::from(view),
                )),
                scalar_type,
                shared_view,
            ))
        }

        pub(crate) fn atomic_cell_storage(
            &self,
            parent: &TerminalStructuralValue,
            view: Option<crate::storage::reference::AggregateViewTip>,
            path: &[StructuralPathSegment],
        ) -> Result<(PrimitiveStorage, ScalarType), TerminalInterpretError> {
            let declared = match view {
                None => parent.structural_type,
                Some(crate::storage::reference::AggregateViewTip::Aggregate { view, backing })
                    if backing == parent.structural_type =>
                {
                    view.structural_type
                }
                Some(_) => return Err(malformed()),
            };
            let types = self.structural_types();
            let cell = terminal_semantics::atomic_cell_type(types.values(), declared, path)
                .ok_or_else(malformed)?;
            let scalar = cell.scalar_type();
            let endpoint = if let Some((StructuralPathSegment::Field(identity), prefix)) =
                path.split_last()
            {
                let field =
                    terminal_semantics::runtime_structural_field(types.values(), declared, path)
                        .ok_or_else(malformed)?;
                if field.identity != *identity || field.field_type.atomic_cell_type() != Some(cell)
                {
                    return Err(malformed());
                }
                let (canonical, _) = terminal_semantics::static_field_projection(
                    types.values(),
                    declared,
                    prefix,
                    field.id,
                )
                .ok_or_else(malformed)?;
                PrimitiveStorageEndpoint::ScalarField(
                    self.observed_field_cell_in(parent, view, &canonical, field.id)?,
                )
            } else {
                if !path.is_empty()
                    || self.structural_declaration(declared)?.shape
                        != terminal_psi::StructuralTypeShape::AtomicCell(cell)
                {
                    return Err(malformed());
                }
                let mut leaf = parent.clone();
                leaf.structural_type = declared;
                match leaf.path.split_last() {
                    Some((StructuralPathSegment::Field(identity), prefix)) => {
                        PrimitiveStorageEndpoint::ScalarField(self.scalar_leaf_field(
                            &leaf,
                            *identity,
                            prefix,
                            scalar,
                            ScalarStorageObservation::AtomicEvent,
                        )?)
                    }
                    _ if view.is_none() => {
                        PrimitiveStorageEndpoint::Scalar(StructuralRuntimePlace::from(&leaf))
                    }
                    _ => return Err(malformed()),
                }
            };
            Ok((PrimitiveStorage::canonical(endpoint), scalar))
        }

        fn scalar_leaf_field(
            &self,
            view: &TerminalStructuralValue,
            identity: terminal_psi::TerminalIdentityIndex,
            parent_path: &[StructuralPathSegment],
            scalar_type: ScalarType,
            observation: ScalarStorageObservation,
        ) -> Result<StructuralScalarRuntimeField, TerminalInterpretError> {
            let shared_view = matches!(
                observation,
                ScalarStorageObservation::OrdinaryReference { shared_view: true }
            );
            let root = self
                .live_structural_values()
                .filter(|value| value.path.is_empty())
                .find_map(|value| {
                    (value.opaque_identity == view.opaque_identity).then_some(value.structural_type)
                })
                .ok_or_else(malformed)?;
            let parent = resolve_structural_path_type(&self.structural_types(), root, parent_path)?;
            let declaration = self.structural_declaration(parent)?;
            let parent_place = StructuralRuntimePlace {
                opaque_identity: view.opaque_identity,
                path: parent_path.to_vec(),
            };
            let member = match &declaration.shape {
                terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                    terminal_psi::live_field_by_identity(fields, identity)
                }
                _ => None,
            };
            let payload = match &declaration.shape {
                terminal_psi::StructuralTypeShape::Sum { cases }
                | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => {
                    let stored = self.structural_cases.require(&parent_place)?;
                    if stored.structural_type != parent {
                        return Err(malformed());
                    }
                    let selected = terminal_psi::find_structural_case(cases, stored.case)
                        .ok_or_else(malformed)?;
                    terminal_psi::live_field_by_identity(&selected.fields, identity)
                }
                _ => None,
            };
            let (field, namespace) = match (member, payload) {
                (Some(field), None) => (field, StructuralFieldNamespace::Member),
                (None, Some(field)) => (field, StructuralFieldNamespace::CasePayload),
                _ => return Err(malformed()),
            };
            if matches!(observation, ScalarStorageObservation::AtomicEvent) {
                let actual = field.field_type.atomic_cell_type().ok_or_else(malformed)?;
                if actual.scalar_type() != scalar_type
                    || self.structural_declaration(view.structural_type)?.shape
                        != terminal_psi::StructuralTypeShape::AtomicCell(actual)
                {
                    return Err(malformed());
                }
                return Ok(StructuralScalarRuntimeField {
                    parent: parent_place,
                    namespace,
                    field: field.id,
                });
            }
            let access = if shared_view {
                StructuralAccess::SharedBorrow
            } else {
                let types = self.structural_types();
                let expected = types.declaration(&view.structural_type)?;
                let selected = terminal_semantics::borrowed_field_presentation(
                    &field.field_type,
                    StructuralAccess::MutableBorrow,
                    Some(expected),
                )
                .and_then(|leaf| leaf.leaf_declaration(types.values()))
                .ok_or_else(malformed)?;
                if selected.id != view.structural_type {
                    return Err(malformed());
                }
                StructuralAccess::MutableBorrow
            };
            let Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(stored_type)) =
                field.field_type.shared_observation_shape()
            else {
                return Err(malformed());
            };
            if !terminal_semantics::scalar_reference_view_admits(access, stored_type, scalar_type) {
                return Err(malformed());
            }
            let cell = StructuralScalarRuntimeField {
                parent: parent_place,
                namespace,
                field: field.id,
            };
            Ok(cell)
        }

        pub(crate) fn execute_primitive_establishment(
            &mut self,
            operation: &Operation,
            value: ValueId,
        ) -> Result<(), TerminalInterpretError> {
            let result = operation.result.structural().ok_or_else(malformed)?;
            let machine = self.program.machine(self.current_machine)?;
            if local_type(machine, result.place) != Some(result.structural_type)
            || !machine.structural_places.iter().any(|place| {
                place.id == result.place && matches!(place.kind,
                    StructuralPlaceKind::OperationResult { producer, .. } if producer == operation.id)
            })
            || self.structural_values.contains_key(&result.place)
        {
            return Err(malformed());
        }
            let Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(expected)) = self
                .structural_types()
                .get(&result.structural_type)
                .map(|entry| &entry.shape)
            else {
                return Err(malformed());
            };
            let scalar = self.scalar_value(value)?;
            let restriction = self
                .structural_types()
                .get(&result.structural_type)
                .and_then(|declaration| declaration.scalar_restriction.as_ref());
            if scalar.scalar_type() != *expected
                || !terminal_scalar_belongs_to_type(scalar)
                || restriction.is_some_and(|restriction| !scalar.satisfies_field(restriction))
            {
                return Err(malformed());
            }
            let identity = self.local_structural_identities.allocate()?;
            let view = TerminalStructuralValue::minted(
                identity,
                result.structural_type,
                result.qualifications.clone(),
            );
            self.structural_values.insert(result.place, view.clone())?;
            self.structural_primitive_storage
                .insert(StructuralRuntimePlace::from(&view), scalar)?;
            Ok(())
        }

        pub(crate) fn scalar_field_at_carrier(
            &self,
            mut carrier: TerminalStructuralValue,
            view: Option<crate::storage::reference::AggregateViewTip>,
            field_path: &[StructuralPathSegment],
            scalar_type: ScalarType,
        ) -> Result<Option<ObservedScalarField<'_>>, TerminalInterpretError> {
            let translated;
            let (field_path, stored_type) = match view {
                None => (field_path, scalar_type),
                Some(crate::storage::reference::AggregateViewTip::Aggregate { view, backing })
                    if backing == carrier.structural_type =>
                {
                    let (path, tip) = crate::storage::reference::translate_aggregate_view_path(
                        &self.structural_types(),
                        view,
                        backing,
                        field_path,
                    )
                    .ok_or_else(malformed)?;
                    let crate::storage::reference::AggregateViewTip::Scalar {
                        view: observed,
                        backing: stored,
                    } = tip
                    else {
                        return Ok(None);
                    };
                    if observed.logical_scalar != scalar_type {
                        return Err(malformed());
                    }
                    let Some((prefix, namespace)) = scalar_field_projection(&path) else {
                        return Ok(None);
                    };
                    carrier = crate::engine::custody::project_shared_structural_observation(
                        &self.structural_types(),
                        &carrier,
                        prefix,
                    )?;
                    translated = namespace.to_vec();
                    (translated.as_slice(), stored.logical_scalar)
                }
                Some(crate::storage::reference::AggregateViewTip::ScalarRegion { .. }) => {
                    return Ok(None);
                }
                Some(_) => return Err(malformed()),
            };
            let Some(field) = terminal_semantics::runtime_structural_field(
                self.structural_types().values(),
                carrier.structural_type,
                field_path,
            ) else {
                return Ok(None);
            };
            if matches!(
                field.field_type,
                terminal_psi::StructuralFieldType::Atomic(_)
                    | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
            ) {
                return Err(malformed());
            }
            let Some(declared) = field.field_type.scalar_type() else {
                return Ok(None);
            };
            if declared != stored_type {
                return Err(malformed());
            }
            let (_, namespace) = field_path.split_last().ok_or_else(malformed)?;
            let namespace = match namespace {
                [] => Vec::new(),
                [StructuralPathSegment::Case(case)] => {
                    vec![semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                        *case,
                    )]
                }
                _ => return Err(malformed()),
            };
            let cell = self.observed_field_cell_in(&carrier, None, &namespace, field.id)?;
            let mut leaf = carrier.clone();
            leaf.path.extend_from_slice(field_path);
            Ok(Some(ObservedScalarField {
                carrier,
                leaf,
                declaration: field,
                stored_type,
                cell,
            }))
        }

        fn scalar_field_read(
            &self,
            source: PlaceId,
            path: &[StructuralPathSegment],
            scalar_type: ScalarType,
        ) -> Result<Option<TerminalScalarValue>, TerminalInterpretError> {
            let Some((prefix, field_path)) = scalar_field_projection(path) else {
                return Ok(None);
            };
            let projection = self.resolve_live_storage_projection(
                source,
                prefix,
                StructuralAccess::SharedBorrow,
            )?;
            self.scalar_field_projection_read(
                source,
                projection,
                field_path,
                scalar_type,
                self.aggregate_argument_backings
                    .get(&source)
                    .and_then(|backing| backing.collection_home.as_ref()),
            )
        }

        fn scalar_field_projection_read(
            &self,
            source: PlaceId,
            projection: crate::storage::reference::ResolvedStructuralProjection,
            field_path: &[StructuralPathSegment],
            scalar_type: ScalarType,
            fallback_collection_home: Option<&crate::storage::element_views::ElementViewHome>,
        ) -> Result<Option<TerminalScalarValue>, TerminalInterpretError> {
            let carrier = projection.value;
            let view = projection.view_tip;
            if let Some(view) = view {
                let (storage, logical_scalar, shared_view) = self.aggregate_view_primitive_access(
                    &carrier,
                    view,
                    field_path,
                    projection
                        .collection_home
                        .as_ref()
                        .or(fallback_collection_home),
                    false,
                )?;
                return self
                    .read_primitive_storage(
                        source,
                        storage,
                        logical_scalar,
                        scalar_type,
                        shared_view,
                    )
                    .map(Some);
            }
            let Some(observed) =
                self.scalar_field_at_carrier(carrier, view, field_path, scalar_type)?
            else {
                return Ok(None);
            };
            if let Some((storage, logical_scalar)) =
                self.reference_byte_region_access(&observed.carrier, &observed.leaf)?
            {
                return self
                    .read_primitive_storage(source, storage, logical_scalar, scalar_type, false)
                    .map(Some);
            }
            let mut value = self
                .structural_scalar_fields
                .get(&observed.cell)
                .map(crate::storage::scalar_storage::ScalarCell::read)
                .transpose()?
                .ok_or(TerminalInterpretError::StructuralScalarFieldMissing {
                    source,
                    field: observed.declaration.id,
                })?;
            if value.scalar_type() != observed.stored_type {
                return Err(malformed());
            }
            if observed.stored_type != scalar_type {
                value = scalar_reference_observation(value, scalar_type)?;
            }
            Ok(Some(value))
        }

        pub(crate) fn read_retained_reference_scalar(
            &self,
            source: PlaceId,
            owner: &TerminalStructuralValue,
            path: &[StructuralPathSegment],
            expected: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            if let Some((prefix, field_path)) = scalar_field_projection(path) {
                let projection = self
                    .resolve_reference_storage_projection(
                        owner,
                        prefix,
                        StructuralAccess::SharedBorrow,
                    )?
                    .ok_or_else(malformed)?;
                if let Some(value) = self
                    .scalar_field_projection_read(source, projection, field_path, expected, None)?
                {
                    return if terminal_scalar_belongs_to_type(value) {
                        Ok(value)
                    } else {
                        Err(malformed())
                    };
                }
            }
            let (storage, scalar, shared) = self
                .reference_primitive_access(owner, false, path)?
                .ok_or_else(malformed)?;
            self.read_primitive_storage(source, storage, scalar, expected, shared)
        }

        pub(crate) fn execute_primitive_read(
            &mut self,
            operation: &Operation,
            source: PlaceId,
            path: &[StructuralPathSegment],
        ) -> Result<(), TerminalInterpretError> {
            let result = operation.result.scalar().ok_or(malformed())?;
            let scalar = self.read_primitive_place(source, path, result.scalar_type)?;
            self.values.insert(result.id, scalar)?;
            Ok(())
        }

        pub(crate) fn read_primitive_place(
            &self,
            source: PlaceId,
            path: &[StructuralPathSegment],
            expected: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            if let Some(value) =
                self.element_view_scalar_read(self.capture_primitive_home(source)?, path, expected)?
            {
                return Ok(value);
            }
            if let Some(value) = self.scalar_field_read(source, path, expected)? {
                if !terminal_scalar_belongs_to_type(value) {
                    return Err(malformed());
                }
                return Ok(value);
            }
            let (storage, scalar_type, shared_view) = self.primitive_access(source, false, path)?;
            self.read_primitive_storage(source, storage, scalar_type, expected, shared_view)
        }

        pub(crate) fn read_primitive_argument(
            &self,
            argument: &StructuralArgument,
            completed: &TerminalStructuralValue,
            relative_path: &[StructuralPathSegment],
            expected: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            if !matches!(
                argument.access,
                StructuralAccess::Owned
                    | StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
            ) {
                return Err(malformed());
            }
            let mut path = argument.path.clone();
            path.extend_from_slice(relative_path);
            if path.is_empty() {
                let machine = self.program.machine(self.current_machine)?;
                if let Some(parameter) = machine
                    .structural_parameter(argument.place)
                    .filter(|parameter| parameter.access == StructuralAccess::Owned)
                {
                    let current = self.structural_value(argument.place)?;
                    if current != completed
                        || current.structural_type != parameter.structural_type
                        || !matches!(self.structural_declaration(parameter.structural_type)?.shape,
                        terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) if scalar == expected)
                    {
                        return Err(malformed());
                    }
                    let (storage, actual, shared) =
                        self.primitive_view_access(completed, &[], expected, false)?;
                    return self.read_primitive_storage(
                        argument.place,
                        storage,
                        actual,
                        expected,
                        shared,
                    );
                }
            }
            self.read_primitive_place(argument.place, &path, expected)
        }

        pub(crate) fn read_primitive_home(
            &self,
            home: PrimitiveStorageHome,
            path: &[StructuralPathSegment],
            expected: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            if let Some(value) = self.element_view_scalar_read(home, path, expected)? {
                return Ok(value);
            }
            let (storage, scalar_type, shared_view) =
                self.primitive_home_access(home, false, path)?;
            self.read_primitive_storage(home.place, storage, scalar_type, expected, shared_view)
        }

        pub(crate) fn read_primitive_storage(
            &self,
            source: PlaceId,
            storage: PrimitiveStorage,
            scalar_type: ScalarType,
            expected: ScalarType,
            shared_view: bool,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            if expected != scalar_type {
                return Err(malformed());
            }
            if let PrimitiveStorageEndpoint::Projection {
                fragments,
                logical,
                stored,
            } = &storage.endpoint
            {
                if *logical != scalar_type {
                    return Err(malformed());
                }
                return self.read_storage_projection(source, fragments, *stored, expected);
            }
            if let Some(encoding) = storage.encoding {
                if encoding.view != encoding.source {
                    let stored = self.read_encoded_primitive(
                        source,
                        &storage.endpoint,
                        encoding.source,
                        encoding.source,
                    )?;
                    let bits = encoding::encode(encoding.source, stored)?;
                    let viewed = encoding::decode(encoding.view, bits)?;
                    return encoding::convert_scalar(viewed, expected);
                }
                return self.read_encoded_primitive(
                    source,
                    &storage.endpoint,
                    encoding.source,
                    expected,
                );
            }
            let scalar = match storage.endpoint {
                PrimitiveStorageEndpoint::RepresentationByte(byte) => {
                    use crate::storage::byte_sequences::read::RepresentationByteReader;
                    let value = self.read_representation_byte(&byte.view, byte.index)?;
                    Ok(TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            8,
                        )
                        .map_err(|_| malformed())?,
                        value: IntegerValue::Unsigned(u128::from(value)),
                    })
                }
                PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                    self.owned_array_home(&home)?;
                    let arrays = if home.depth == self.call_stack.len() {
                        &self.scalar_array_values
                    } else {
                        &self.frame_at_depth(home.depth)?.scalar_array_values
                    };
                    arrays
                        .backing(&home.place)
                        .filter(|array| array.structural_type == home.structural_type)
                        .ok_or_else(malformed)?
                        .element(index)?
                        .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                            source,
                        ))
                }
                PrimitiveStorageEndpoint::Scalar(storage) => self
                    .structural_primitive_storage
                    .get(&storage)
                    .map(crate::storage::scalar_storage::ScalarCell::read)
                    .transpose()?
                    .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                        source,
                    )),
                PrimitiveStorageEndpoint::ScalarField(field) => self
                    .structural_scalar_fields
                    .get(&field)
                    .map(crate::storage::scalar_storage::ScalarCell::read)
                    .transpose()?
                    .ok_or(TerminalInterpretError::StructuralScalarFieldMissing {
                        source,
                        field: field.field,
                    }),
                PrimitiveStorageEndpoint::Projection { .. } => return Err(malformed()),
                PrimitiveStorageEndpoint::ViewByte { bytes, index } => {
                    let Some(integer) = scalar_type.as_integer() else {
                        return Err(malformed());
                    };
                    bytes
                        .get(index)
                        .map(|byte| TerminalScalarValue::Integer {
                            scalar_type: integer,
                            value: IntegerValue::Unsigned(u128::from(*byte)),
                        })
                        .ok_or_else(malformed)
                }
                PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                    let Some(integer) = scalar_type.as_integer() else {
                        return Err(malformed());
                    };
                    self.stored_byte_array_read(&array, index).map(|byte| {
                        TerminalScalarValue::Integer {
                            scalar_type: integer,
                            value: IntegerValue::Unsigned(u128::from(byte)),
                        }
                    })
                }
            }?;
            if expected != scalar_type || !terminal_scalar_belongs_to_type(scalar) {
                return Err(malformed());
            }
            if scalar.scalar_type() == scalar_type {
                return Ok(scalar);
            }
            if !shared_view {
                return Err(malformed());
            }
            scalar_reference_observation(scalar, scalar_type)
        }

        pub(crate) fn execute_primitive_store(
            &mut self,
            operation: &Operation,
            destination: PlaceId,
            value: ValueId,
            path: &[StructuralPathSegment],
        ) -> Result<(), TerminalInterpretError> {
            if operation.result != OperationResult::Unit {
                return Err(malformed());
            }
            let scalar = self.scalar_value(value)?;
            let machine = self.program.machine(self.current_machine)?;
            let (root, whole, projected, additional_input) = machine
                .structural_parameter(destination)
                .map(|parameter| {
                    (
                        parameter.structural_type,
                        parameter.qualifications.as_slice(),
                        parameter.projected_qualifications.as_slice(),
                        parameter.additional_input_qualifications.as_slice(),
                    )
                })
                .or_else(|| {
                    machine
                        .operations()
                        .filter_map(|operation| operation.result.structural())
                        .find(|result| result.place == destination)
                        .map(|result| {
                            (
                                result.structural_type,
                                result.qualifications.as_slice(),
                                result.projected_qualifications.as_slice(),
                                result.additional_input_qualifications.as_slice(),
                            )
                        })
                })
                .ok_or_else(malformed)?;
            let restriction = terminal_semantics::primitive_store_restriction(
                self.structural_types().values(),
                self.program.catalogs.structural_domains.iter(),
                root,
                whole,
                projected,
                additional_input,
                path,
            )
            .ok_or_else(malformed)?;
            let predicates = terminal_semantics::primitive_store_byte_predicates(
                self.structural_types().values(),
                self.program.catalogs.structural_domains.iter(),
                root,
                whole,
                projected,
                additional_input,
                path,
            )
            .ok_or_else(malformed)?;
            if predicates
                .predicates()
                .any(|predicate| predicate == terminal_psi::ByteSequencePredicate::ValidUtf8)
                && !predicates
                    .predicates()
                    .any(|predicate| predicate == terminal_psi::ByteSequencePredicate::AsciiOnly)
            {
                self.validate_ascii_backing_for_primitive_write(
                    destination,
                    root,
                    &path[..path.len() - 1],
                )?;
            }
            if let Some(restriction) = restriction {
                let literal = match scalar {
                    TerminalScalarValue::Integer { scalar_type, value } => {
                        semantic_vocabulary::ScalarTerm::integer(scalar_type, value)
                            .map_err(|_| malformed())?
                    }
                    TerminalScalarValue::Boolean(value) => {
                        semantic_vocabulary::ScalarTerm::Boolean(value)
                    }
                    TerminalScalarValue::IeeeFloat(value) => {
                        semantic_vocabulary::ScalarTerm::IeeeFloatLiteral(value)
                    }
                };
                if !terminal_semantics::scalar_field_accepts_literal(&restriction, &literal) {
                    return Err(malformed());
                }
            }
            self.store_primitive_place(destination, path, scalar)
        }

        fn validate_ascii_backing_for_primitive_write(
            &self,
            destination: PlaceId,
            root: StructuralTypeId,
            array_path: &[StructuralPathSegment],
        ) -> Result<(), TerminalInterpretError> {
            let declaration = terminal_semantics::runtime_structural_path_tip(
                self.structural_types().values(),
                root,
                array_path,
            )
            .ok_or_else(malformed)?;
            let terminal_psi::StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(length),
                ..
            } = declaration.shape
            else {
                return Err(malformed());
            };
            let byte_type = semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                8,
            )
            .map_err(|_| malformed())?;
            let mut selected = array_path.to_vec();
            for index in 0..length {
                selected.push(StructuralPathSegment::FixedIndex(index));
                let (storage, stored_type, shared_view) =
                    self.primitive_access(destination, true, &selected)?;
                let byte = self.read_primitive_storage(
                    destination,
                    storage,
                    stored_type,
                    semantic_vocabulary::ScalarType::Integer(byte_type),
                    shared_view,
                )?;
                selected.pop();
                if !matches!(byte, TerminalScalarValue::Integer { value: IntegerValue::Unsigned(value), .. } if value <= 127)
                {
                    return Err(malformed());
                }
            }
            Ok(())
        }

        pub(crate) fn store_primitive_place(
            &mut self,
            destination: PlaceId,
            path: &[StructuralPathSegment],
            scalar: TerminalScalarValue,
        ) -> Result<(), TerminalInterpretError> {
            let (storage, scalar_type, _) = self.primitive_access(destination, true, path)?;
            self.store_primitive_storage(destination, storage, scalar_type, scalar)
        }

        pub(crate) fn store_primitive_home(
            &mut self,
            home: PrimitiveStorageHome,
            path: &[StructuralPathSegment],
            scalar: TerminalScalarValue,
        ) -> Result<(), TerminalInterpretError> {
            let (storage, scalar_type, _) = self.primitive_home_access(home, true, path)?;
            self.store_primitive_storage(home.place, storage, scalar_type, scalar)
        }

        pub(crate) fn store_primitive_storage(
            &mut self,
            destination: PlaceId,
            storage: PrimitiveStorage,
            scalar_type: ScalarType,
            scalar: TerminalScalarValue,
        ) -> Result<(), TerminalInterpretError> {
            let mut writes = PrimitiveStorageWrites::default();
            writes.stage(self, destination, storage, scalar_type, scalar)?;
            writes.commit(self)
        }

        fn commit_primitive_store(
            &mut self,
            destination: PlaceId,
            endpoint: PrimitiveStorageEndpoint,
            scalar: TerminalScalarValue,
        ) -> Result<(), TerminalInterpretError> {
            match endpoint {
                PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                    let arrays = frame_field_mut!(self, home.depth, scalar_array_values);
                    let array = arrays.backing_mut(&home.place).ok_or(
                        TerminalInterpretError::StructuralPrimitiveStorageMissing(destination),
                    )?;
                    array.set_element(index, scalar)?;
                }
                PrimitiveStorageEndpoint::Scalar(storage) => {
                    self.structural_primitive_storage
                        .insert(storage.clone(), scalar)?;
                    self.invalidate_stored_provenance(&storage);
                }
                PrimitiveStorageEndpoint::ScalarField(field) => {
                    self.invalidate_stored_provenance(&field.parent);
                    let stored = self.structural_scalar_fields.get_mut(&field).ok_or(
                        TerminalInterpretError::StructuralScalarFieldMissing {
                            source: destination,
                            field: field.field,
                        },
                    )?;
                    stored.write(scalar)?;
                }
                PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                    let TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(value),
                        ..
                    } = scalar
                    else {
                        return Err(malformed());
                    };
                    let byte = u8::try_from(value).map_err(|_| malformed())?;
                    self.stored_byte_array_write(&array, index, byte)?;
                }
                PrimitiveStorageEndpoint::RepresentationByte(_)
                | PrimitiveStorageEndpoint::ViewByte { .. }
                | PrimitiveStorageEndpoint::Projection { .. } => return Err(malformed()),
            }
            Ok(())
        }

        pub(crate) fn retire_plain_locals(&mut self) {
            self.retire_unrestricted_views();
            let Some(machine) = self.program.machines().get(&self.current_machine) else {
                return;
            };
            for place in &machine.structural_places {
                if local_type(machine, place.id).is_some()
                    && let Some(view) = self.structural_values.remove(&place.id)
                {
                    self.structural_primitive_storage
                        .remove(&StructuralRuntimePlace::from(&view));
                    let origin = self
                        .reference_referents
                        .get(&StructuralRuntimePlace::from(&view))
                        .cloned();
                    self.retired_structural_homes.push((place.id, view, origin));
                }
            }
        }

        pub(crate) fn retire_reentered_results(
            &mut self,
            target: semantic_vocabulary::BlockId,
        ) -> Result<(), TerminalInterpretError> {
            let machine = self.program.verified_machine(self.current_machine)?;
            let block = machine
                .block(&target)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
            let mut retired = Vec::new();
            for place in block
                .operations
                .iter()
                .filter_map(|operation| operation.result.structural())
                .map(|result| result.place)
            {
                if self.live_affine_discard_at(place).is_some()
                    || crate::engine::custody::place_is_claimed(&self.live_claims, place)
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                if let Some(previous) = self.structural_values.remove(&place) {
                    let root = StructuralRuntimePlace::from(&previous);
                    if local_type(machine, place).is_some() {
                        self.structural_primitive_storage.remove(&root);
                    }
                    retired.push(root);
                }
                if let Some(home) = self.linear_homes.remove(&place) {
                    retired.push(StructuralRuntimePlace::from(&home));
                }
                crate::storage::scalar_array::PackedHomeRetirement::retire_packed_home(
                    &mut self.scalar_array_values,
                    &place,
                );
                self.byte_sequence_values.remove(&place);
                self.primitive_loan_values.remove(&place);
                self.element_view_values.remove(&place);
            }
            for root in retired {
                self.queue_retired_descriptor_home(root);
            }
            Ok(())
        }

        pub(crate) fn owned_array_home(
            &self,
            home: &PrimitiveStorageHome,
        ) -> Result<MachineId, TerminalInterpretError> {
            let owner_machine = if home.depth == self.call_stack.len() {
                self.current_machine
            } else {
                self.call_stack
                    .get(home.depth)
                    .ok_or_else(malformed)?
                    .current_machine
            };
            if owner_machine != home.machine {
                return Err(malformed());
            }
            Ok(owner_machine)
        }

        fn frame_at_depth(&self, depth: usize) -> Result<&SuspendedCall, TerminalInterpretError> {
            self.call_stack.get(depth).ok_or_else(malformed)
        }
    }

    pub(crate) fn scalar_reference_observation(
        stored: TerminalScalarValue,
        target: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        use terminal_semantics::ScalarViewValue;
        let input = crate::boundary::values::scalar_view_value(&stored);
        let observed =
            terminal_semantics::scalar_reference_view_value(stored.scalar_type(), target, input)
                .ok_or(malformed())?;
        match (observed, target) {
            (ScalarViewValue::Boolean(value), ScalarType::Boolean) => {
                Ok(TerminalScalarValue::Boolean(value))
            }
            (ScalarViewValue::Integer(value), ScalarType::Integer(scalar_type)) => {
                Ok(TerminalScalarValue::Integer { scalar_type, value })
            }
            (ScalarViewValue::IeeeFloat(value), ScalarType::IeeeFloat(_)) => {
                Ok(TerminalScalarValue::IeeeFloat(value))
            }
            _ => Err(malformed()),
        }
    }
}

pub(crate) mod runtime_indexes {
    use terminal_psi::{
        Operation, OperationKind, StructuralAccess, StructuralArgument, StructuralPathSegment,
        StructuralTypeShape,
    };

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::TerminalScalarValue;
    use crate::engine::custody::resolve_structural_path_type;
    use crate::engine::execution::TerminalExecution;

    impl TerminalExecution<'_> {
        pub(crate) fn select_runtime_elements(
            &self,
            operation: &Operation,
        ) -> Result<Operation, TerminalInterpretError> {
            let mut selected = operation.clone();
            let projections = operation.kind.structural_projections();
            let paths = selected.kind.structural_projection_paths_mut();
            if paths.len() != projections.len() {
                return Err(malformed());
            }
            for (projection, path) in projections.into_iter().zip(paths) {
                if !path.iter().any(|segment| segment.runtime_index().is_some()) {
                    continue;
                }
                let root = projection.root;
                let root_type = self.live_structural_type(root)?;
                let root_type = if matches!(operation.kind, OperationKind::ElementViewRead { .. }) {
                    match self.structural_declaration(root_type)?.shape {
                        StructuralTypeShape::ElementView { element, .. } => element,
                        _ => return Err(malformed()),
                    }
                } else {
                    root_type
                };
                self.select_runtime_projection(root, root_type, projection.path, path)?;
            }
            Ok(selected)
        }

        pub(crate) fn select_runtime_projection(
            &self,
            root: semantic_vocabulary::PlaceId,
            root_type: semantic_vocabulary::StructuralTypeId,
            declared_path: &[StructuralPathSegment],
            path: &mut [StructuralPathSegment],
        ) -> Result<(), TerminalInterpretError> {
            for position in 0..path.len() {
                let Some((index, _)) = path[position].runtime_index() else {
                    continue;
                };
                let TerminalScalarValue::Integer { value, .. } = self.scalar_value(index)? else {
                    return Err(malformed());
                };
                let array = resolve_structural_path_type(
                    &self.structural_types(),
                    root_type,
                    &declared_path[..position],
                )?;
                let length = match self.structural_declaration(array)?.shape {
                    StructuralTypeShape::FixedArray {
                        length: terminal_psi::ArrayExtent::Literal(length),
                        ..
                    } => length,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. },
                    ) => capacity,
                    StructuralTypeShape::ElementView { .. } => {
                        let source = StructuralArgument {
                            place: root,
                            path: path[..position].to_vec(),
                            access: StructuralAccess::SharedBorrow,
                        };
                        let selected =
                            self.resolve_live_projection(root, &source.path, source.access)?;
                        u64::try_from(self.element_view_argument(&source, &selected)?.len())
                            .map_err(|_| malformed())?
                    }
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ) => {
                        let source = StructuralArgument {
                            place: root,
                            path: path[..position].to_vec(),
                            access: StructuralAccess::SharedBorrow,
                        };
                        let selected =
                            self.resolve_live_projection(root, &source.path, source.access)?;
                        let binding = self.byte_descriptor_binding(&source, &selected)?;
                        self.byte_descriptor_length(&binding, &selected)?
                    }
                    _ => return Err(malformed()),
                };
                let element = terminal_semantics::runtime_index_selects(value, length)
                    .ok_or_else(malformed)?;
                path[position] = StructuralPathSegment::FixedIndex(element);
            }
            Ok(())
        }
    }
}

pub(crate) mod runtime_rows {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use evaluation_accounting::{
        EvaluationAccountingError, EvaluationInvocationAccount, EvaluationStorageLease,
    };
    use std::collections::{BTreeMap, btree_map};

    fn reserve_row(
        lease: &mut Option<EvaluationStorageLease>,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<(), TerminalInterpretError> {
        if let Some(lease) = lease {
            lease.grow(1, 0).map_err(TerminalInterpretError::Storage)?;
        } else if let Some(account) = account {
            *lease = Some(
                account
                    .reserve_storage(1, 0)
                    .map_err(TerminalInterpretError::Storage)?,
            );
        }
        Ok(())
    }

    #[derive(Debug)]
    pub(crate) struct RuntimeRows<Key, Value> {
        rows: BTreeMap<Key, Value>,
        lease: Option<EvaluationStorageLease>,
        account: Option<EvaluationInvocationAccount>,
    }

    impl<Key: Ord, Value> RuntimeRows<Key, Value> {
        pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
            Self {
                rows: BTreeMap::new(),
                lease: None,
                account: account.cloned(),
            }
        }

        pub(crate) fn from_rows(
            account: Option<&EvaluationInvocationAccount>,
            rows: impl IntoIterator<Item = (Key, Value)>,
        ) -> Result<Self, TerminalInterpretError> {
            let mut owned = Self::new(account);
            for (key, value) in rows {
                owned.insert(key, value)?;
            }
            Ok(owned)
        }

        pub(crate) fn insert(
            &mut self,
            key: Key,
            value: Value,
        ) -> Result<Option<Value>, TerminalInterpretError> {
            match self.rows.entry(key) {
                btree_map::Entry::Occupied(mut entry) => Ok(Some(entry.insert(value))),
                btree_map::Entry::Vacant(entry) => {
                    reserve_row(&mut self.lease, self.account.as_ref())?;
                    entry.insert(value);
                    Ok(None)
                }
            }
        }

        pub(crate) fn insert_vacant(
            &mut self,
            key: Key,
            value: Value,
        ) -> Result<&mut Value, TerminalInterpretError> {
            let btree_map::Entry::Vacant(entry) = self.rows.entry(key) else {
                return Err(malformed());
            };
            reserve_row(&mut self.lease, self.account.as_ref())?;
            Ok(entry.insert(value))
        }

        fn retire_removed_rows(&mut self) {
            if let Some(lease) = &mut self.lease {
                lease.truncate_cells(u64::try_from(self.rows.len()).unwrap_or(u64::MAX));
            }
        }

        pub(crate) fn remove(&mut self, key: &Key) -> Option<Value> {
            let removed = self.rows.remove(key);
            self.retire_removed_rows();
            removed
        }

        pub(crate) fn clear(&mut self) {
            self.rows.clear();
            self.lease = None;
        }

        pub(crate) fn retain(&mut self, keep: impl FnMut(&Key, &mut Value) -> bool) {
            self.rows.retain(keep);
            self.retire_removed_rows();
        }

        pub(crate) fn take(&mut self) -> Self {
            let empty = Self::new(self.account.as_ref());
            std::mem::replace(self, empty)
        }

        pub(crate) fn validate_append(&self, other: &Self) -> Result<(), TerminalInterpretError> {
            if other.rows.is_empty() {
                return Ok(());
            }
            let compatible = match (&self.account, &other.account) {
                (Some(left), Some(right)) => left.same_invocation(right),
                (None, None) => true,
                _ => false,
            };
            if !compatible {
                return Err(TerminalInterpretError::Storage(
                    EvaluationAccountingError::LeaseAccountMismatch,
                ));
            }
            Ok(())
        }

        pub(crate) fn append(&mut self, other: &mut Self) -> Result<(), TerminalInterpretError> {
            self.validate_append(other)?;
            if other.rows.is_empty() {
                return Ok(());
            }
            match (&mut self.lease, &mut other.lease) {
                (Some(destination), Some(source)) => destination
                    .absorb(source)
                    .map_err(TerminalInterpretError::Storage)?,
                (None, _) => self.lease = other.lease.take(),
                _ => {}
            }
            self.rows.append(&mut other.rows);
            self.retire_removed_rows();
            Ok(())
        }

        pub(crate) fn try_clone(&self) -> Result<Self, TerminalInterpretError>
        where
            Key: Clone,
            Value: Clone,
        {
            let cells = u64::try_from(self.rows.len()).map_err(|_| malformed())?;
            let lease = self
                .account
                .as_ref()
                .map(|account| account.reserve_storage(cells, 0))
                .transpose()
                .map_err(TerminalInterpretError::Storage)?;
            Ok(Self {
                rows: self.rows.clone(),
                lease,
                account: self.account.clone(),
            })
        }

        pub(crate) fn get(&self, key: &Key) -> Option<&Value> {
            self.rows.get(key)
        }

        pub(crate) fn require(&self, key: &Key) -> Result<&Value, TerminalInterpretError> {
            self.get(key).ok_or_else(malformed)
        }
        pub(crate) fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
            self.rows.get_mut(key)
        }
        pub(crate) fn contains_key(&self, key: &Key) -> bool {
            self.rows.contains_key(key)
        }
        pub(crate) fn len(&self) -> usize {
            self.rows.len()
        }
        pub(crate) fn is_empty(&self) -> bool {
            self.rows.is_empty()
        }
        pub(crate) fn keys(&self) -> btree_map::Keys<'_, Key, Value> {
            self.rows.keys()
        }
        pub(crate) fn values(&self) -> btree_map::Values<'_, Key, Value> {
            self.rows.values()
        }
        pub(crate) fn iter(&self) -> btree_map::Iter<'_, Key, Value> {
            self.rows.iter()
        }
        pub(crate) fn iter_mut(&mut self) -> btree_map::IterMut<'_, Key, Value> {
            self.rows.iter_mut()
        }

        pub(crate) fn as_map(&self) -> &BTreeMap<Key, Value> {
            &self.rows
        }
    }

    impl<'rows, Key, Value> IntoIterator for &'rows RuntimeRows<Key, Value> {
        type Item = (&'rows Key, &'rows Value);
        type IntoIter = btree_map::Iter<'rows, Key, Value>;
        fn into_iter(self) -> Self::IntoIter {
            self.rows.iter()
        }
    }

    impl<Key, Value> std::borrow::Borrow<BTreeMap<Key, Value>> for RuntimeRows<Key, Value> {
        fn borrow(&self) -> &BTreeMap<Key, Value> {
            &self.rows
        }
    }

    #[cfg(test)]
    mod tests {
        use super::RuntimeRows;
        use evaluation_accounting::{
            BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
            TerminalStorageScheduleIdentity,
        };

        fn sponsor(cells: u64) -> BuildEvaluationSponsor {
            BuildEvaluationSponsor::new_with_measurement(
                BuildEvaluationSponsorLimits::new(100, 8, 2, 2, cells, 100, 100, 100).unwrap(),
                EvaluationMeasurementIdentity::TerminalPsi {
                    work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
                    storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
                },
            )
            .unwrap()
        }

        #[test]
        fn replacement_keeps_its_cell_and_refused_growth_keeps_the_table() {
            let sponsor = sponsor(1);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let mut rows = RuntimeRows::new(Some(&account));
            assert_eq!(rows.insert(1u32, 7u32).unwrap(), None);
            assert_eq!(rows.insert(1, 8).unwrap(), Some(7));
            assert!(rows.insert(2, 9).is_err());
            assert_eq!(
                rows.iter()
                    .map(|(key, value)| (*key, *value))
                    .collect::<Vec<_>>(),
                [(1, 8)]
            );
            assert_eq!(sponsor.live_storage_cells(), 1);
            assert_eq!(rows.remove(&2), None);
            assert_eq!(sponsor.live_storage_cells(), 1);
            assert_eq!(rows.remove(&1), Some(8));
            assert_eq!(sponsor.live_storage_cells(), 0);
            rows.insert(3, 10).unwrap();
            drop(rows);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }

        #[test]
        fn frame_transfer_keeps_both_the_reservation_and_the_next_frames_account() {
            let sponsor = sponsor(2);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let mut active =
                RuntimeRows::from_rows(Some(&account), [(1u32, 7u32), (2, 8)]).unwrap();
            let mut saved = active.take();
            assert_eq!(sponsor.live_storage_cells(), 2);
            assert_eq!(active.len(), 0);
            assert!(active.insert(3, 9).is_err());
            saved.clear();
            active.insert(3, 9).unwrap();
            assert_eq!(sponsor.live_storage_cells(), 1);
            drop(active);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }

        #[test]
        fn state_join_transfers_cells_and_retires_replaced_destinations_without_recharging() {
            let sponsor = sponsor(4);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let mut active =
                RuntimeRows::from_rows(Some(&account), [(1u32, 7u32), (2, 8)]).unwrap();
            let mut staged = RuntimeRows::from_rows(Some(&account), [(2, 20), (3, 30)]).unwrap();
            assert_eq!(sponsor.live_storage_cells(), 4);
            active.append(&mut staged).unwrap();
            assert_eq!(active.get(&2), Some(&20));
            assert_eq!(active.len(), 3);
            assert_eq!(staged.len(), 0);
            assert_eq!(sponsor.live_storage_cells(), 3);
            staged.insert(4, 40).unwrap();
            assert_eq!(sponsor.live_storage_cells(), 4);
            drop(active);
            assert_eq!(sponsor.live_storage_cells(), 1);
            drop(staged);
            assert_eq!(sponsor.live_storage_cells(), 0);
            assert_eq!(
                account.snapshot().unwrap().record().peak_live_storage_cells,
                4
            );
        }

        #[test]
        fn copies_reserve_their_overlap_and_partial_construction_retires_rows() {
            for limit in [3, 4] {
                let sponsor = sponsor(limit);
                let account = sponsor
                    .begin_invocation(sponsor.measurement(), 100)
                    .unwrap();
                let rows = RuntimeRows::from_rows(Some(&account), [(1u32, 7u32), (2, 8)]).unwrap();
                let copy = rows.try_clone();
                if limit == 3 {
                    assert!(copy.is_err());
                    assert_eq!(sponsor.live_storage_cells(), 2);
                    assert!(RuntimeRows::from_rows(Some(&account), [(3, 9), (4, 10)]).is_err());
                    assert_eq!(sponsor.live_storage_cells(), 2);
                } else {
                    let mut copy = copy.unwrap();
                    assert_eq!(sponsor.live_storage_cells(), 4);
                    copy.insert(1, 99).unwrap();
                    assert_eq!(rows.get(&1), Some(&7));
                    assert_eq!(copy.get(&1), Some(&99));
                    drop(copy);
                    assert_eq!(sponsor.live_storage_cells(), 2);
                }
                drop(rows);
                assert_eq!(sponsor.live_storage_cells(), 0);
            }
        }

        #[test]
        fn sibling_invocations_share_limits_but_cannot_transfer_account_identity() {
            let sponsor = sponsor(2);
            let first = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let second = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let mut left = RuntimeRows::from_rows(Some(&first), [(1u32, 7u32)]).unwrap();
            let mut right = RuntimeRows::from_rows(Some(&second), [(2, 8)]).unwrap();
            assert!(left.insert(3, 9).is_err());
            assert!(left.append(&mut right).is_err());
            assert_eq!(left.get(&1), Some(&7));
            assert_eq!(right.get(&2), Some(&8));
            assert_eq!(sponsor.live_storage_cells(), 2);
            drop(left);
            assert_eq!(sponsor.live_storage_cells(), 1);
            right.insert(3, 9).unwrap();
            assert_eq!(sponsor.live_storage_cells(), 2);
            drop(right);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }

        #[test]
        fn vacant_receiving_row_is_funded_before_mutation_and_never_replaces() {
            let sponsor = sponsor(1);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let mut rows = RuntimeRows::new(Some(&account));
            let receiving = rows.insert_vacant(1u32, 7u32).unwrap();
            assert_eq!(sponsor.live_storage_cells(), 1);
            *receiving = 8;
            assert!(rows.insert_vacant(1, 99).is_err());
            assert!(rows.insert_vacant(2, 99).is_err());
            assert_eq!(rows.get(&1), Some(&8));
            assert_eq!(rows.get(&2), None);
            drop(rows);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }
    }
}

pub(crate) mod scalar_storage {
    use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
    use crate::{
        boundary::effects::TerminalEffectRejection,
        boundary::errors::{TerminalInterpretError, malformed},
        boundary::values::TerminalScalarValue,
        storage::runtime_rows::RuntimeRows,
    };
    use evaluation_accounting::EvaluationInvocationAccount;
    use semantic_vocabulary::ScalarType;
    use std::{collections::BTreeMap, rc::Rc};

    pub(crate) mod component_loans {
        use super::{ScalarCell, TerminalScalarBacking};
        use crate::boundary::effects::TerminalEffectRejection;
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
        use semantic_vocabulary::ScalarType;
        use std::cell::Cell;
        use std::rc::{Rc, Weak};
        use terminal_psi::StructuralAccess;

        pub(crate) struct ScalarLoanScope(Rc<()>);

        impl ScalarLoanScope {
            pub(crate) fn new() -> Self {
                Self(Rc::new(()))
            }

            pub(crate) fn lend(
                &self,
                original: &mut ScalarCell,
                access: StructuralAccess,
            ) -> Result<ScalarCell, TerminalInterpretError> {
                if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
                    return Err(malformed());
                }
                if let ScalarCell::Local(value) = original {
                    let scalar_type = value.scalar_type();
                    let backing = Rc::new(GuestScalarHome {
                        value: Cell::new(*value),
                        scalar_type,
                    });
                    *original = ScalarCell::Resident {
                        scalar_type,
                        backing,
                    };
                }
                let ScalarCell::Resident {
                    scalar_type,
                    backing,
                } = original
                else {
                    return Err(malformed());
                };
                Ok(ScalarCell::Resident {
                    scalar_type: *scalar_type,
                    backing: Rc::new(ComponentScalarView {
                        original: Rc::clone(backing),
                        lifetime: Rc::downgrade(&self.0),
                        access,
                        scalar_type: *scalar_type,
                    }),
                })
            }
        }

        #[derive(Debug)]
        struct GuestScalarHome {
            value: Cell<TerminalScalarValue>,
            scalar_type: ScalarType,
        }

        impl TerminalScalarBacking for GuestScalarHome {
            fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
                Ok(self.value.get())
            }

            fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
                validate_scalar(self.scalar_type, value)?;
                self.value.set(value);
                Ok(())
            }
        }

        #[derive(Debug)]
        struct ComponentScalarView {
            original: Rc<dyn TerminalScalarBacking>,
            lifetime: Weak<()>,
            access: StructuralAccess,
            scalar_type: ScalarType,
        }

        impl ComponentScalarView {
            fn live(&self) -> Result<Rc<()>, TerminalEffectRejection> {
                self.lifetime
                    .upgrade()
                    .ok_or_else(|| TerminalEffectRejection::new("component scalar loan has ended"))
            }
        }

        impl TerminalScalarBacking for ComponentScalarView {
            fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
                let _lifetime = self.live()?;
                let value = self.original.read()?;
                validate_scalar(self.scalar_type, value)?;
                Ok(value)
            }

            fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
                let _lifetime = self.live()?;
                if self.access != StructuralAccess::MutableBorrow {
                    return Err(TerminalEffectRejection::new(
                        "shared component scalar loan cannot mutate its original home",
                    ));
                }
                validate_scalar(self.scalar_type, value)?;
                self.original.write(value)
            }
        }

        fn validate_scalar(
            expected: ScalarType,
            value: TerminalScalarValue,
        ) -> Result<(), TerminalEffectRejection> {
            if value.scalar_type() != expected || !terminal_scalar_belongs_to_type(value) {
                return Err(TerminalEffectRejection::new(
                    "component scalar loan changed its stored carrier",
                ));
            }
            Ok(())
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

            fn word(value: u128) -> TerminalScalarValue {
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    value: IntegerValue::Unsigned(value),
                }
            }

            #[test]
            fn component_views_write_original_cells_and_cannot_outlive_their_owner() {
                let mut original = ScalarCell::Local(word(0));
                let owner = ScalarLoanScope::new();
                let mut view = owner
                    .lend(&mut original, StructuralAccess::MutableBorrow)
                    .unwrap();
                view.write(word(73)).unwrap();
                assert_eq!(original.read().unwrap(), word(73));
                let mut retained = view.clone();
                drop(owner);
                assert!(retained.write(word(91)).is_err());
                assert!(retained.read().is_err());
                assert_eq!(original.read().unwrap(), word(73));
            }

            #[test]
            fn nested_views_preserve_the_immediate_parents_access_and_lifetime() {
                let mut original = ScalarCell::Local(word(37));
                let parent = ScalarLoanScope::new();
                let mut shared = parent
                    .lend(&mut original, StructuralAccess::SharedBorrow)
                    .unwrap();
                let child = ScalarLoanScope::new();
                let mut nested = child
                    .lend(&mut shared, StructuralAccess::MutableBorrow)
                    .unwrap();
                assert_eq!(nested.read().unwrap(), word(37));
                assert!(nested.write(word(73)).is_err());
                assert_eq!(original.read().unwrap(), word(37));
                drop(parent);
                assert!(nested.read().is_err());
                assert_eq!(original.read().unwrap(), word(37));
            }
        }
    }
    pub(crate) use component_loans::ScalarLoanScope;

    pub trait TerminalScalarBacking: std::fmt::Debug {
        fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection>;
        fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection>;
    }

    #[derive(Debug, Clone)]
    pub(crate) enum ScalarCell {
        Local(TerminalScalarValue),
        Resident {
            scalar_type: ScalarType,
            backing: Rc<dyn TerminalScalarBacking>,
        },
    }

    impl PartialEq for ScalarCell {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::Local(left), Self::Local(right)) => left == right,
                (
                    Self::Resident {
                        scalar_type: left_type,
                        backing: left,
                    },
                    Self::Resident {
                        scalar_type: right_type,
                        backing: right,
                    },
                ) => left_type == right_type && Rc::ptr_eq(left, right),
                _ => false,
            }
        }
    }

    impl Eq for ScalarCell {}

    impl ScalarCell {
        pub(crate) fn resident(
            original: TerminalScalarValue,
            backing: Rc<dyn TerminalScalarBacking>,
        ) -> Result<Self, TerminalInterpretError> {
            let cell = Self::Resident {
                scalar_type: original.scalar_type(),
                backing,
            };
            if cell.read()? != original {
                return Err(TerminalInterpretError::ResidentStorage(
                    TerminalEffectRejection::new(
                        "resident initialization differs from original scalar contents",
                    ),
                ));
            }
            Ok(cell)
        }
        pub(crate) fn scalar_type(&self) -> ScalarType {
            match self {
                Self::Local(value) => value.scalar_type(),
                Self::Resident { scalar_type, .. } => *scalar_type,
            }
        }
        pub(crate) fn read(&self) -> Result<TerminalScalarValue, TerminalInterpretError> {
            match self {
                Self::Local(value) => Ok(*value),
                Self::Resident {
                    scalar_type,
                    backing,
                } => {
                    let value = backing
                        .read()
                        .map_err(TerminalInterpretError::ResidentStorage)?;
                    if value.scalar_type() != *scalar_type
                        || !terminal_scalar_belongs_to_type(value)
                    {
                        return Err(malformed());
                    }
                    Ok(value)
                }
            }
        }

        pub(crate) fn write(
            &mut self,
            value: TerminalScalarValue,
        ) -> Result<(), TerminalInterpretError> {
            match self {
                Self::Local(previous) => *previous = value,
                Self::Resident {
                    scalar_type,
                    backing,
                } => {
                    if value.scalar_type() != *scalar_type
                        || !terminal_scalar_belongs_to_type(value)
                    {
                        return Err(malformed());
                    }
                    backing
                        .write(value)
                        .map_err(TerminalInterpretError::ResidentStorage)?;
                }
            }
            Ok(())
        }

        pub(crate) fn relocated(&self, transfer: bool) -> Result<Self, TerminalInterpretError> {
            if transfer {
                Ok(self.clone())
            } else {
                self.read().map(Self::Local)
            }
        }
    }

    #[derive(Debug)]
    pub(crate) struct ScalarStorageRows<Key>(RuntimeRows<Key, ScalarCell>);

    impl<Key: Ord> ScalarStorageRows<Key> {
        pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
            Self(RuntimeRows::new(account))
        }
        pub(crate) fn insert(
            &mut self,
            key: Key,
            value: TerminalScalarValue,
        ) -> Result<Option<ScalarCell>, TerminalInterpretError> {
            if let Some(cell) = self.0.get_mut(&key) {
                let previous = cell.clone();
                cell.write(value)?;
                Ok(Some(previous))
            } else {
                self.0.insert(key, ScalarCell::Local(value))
            }
        }
        pub(crate) fn insert_cell(
            &mut self,
            key: Key,
            cell: ScalarCell,
        ) -> Result<Option<ScalarCell>, TerminalInterpretError> {
            if self
                .0
                .get(&key)
                .is_some_and(|cell| matches!(cell, ScalarCell::Resident { .. }))
            {
                return self.insert(key, cell.read()?);
            }
            self.0.insert(key, cell)
        }
        pub(crate) fn prune(&mut self, root: &crate::boundary::values::StructuralRuntimePlace)
        where
            Key: crate::storage::structural_storage::StorageKey,
        {
            crate::storage::structural_storage::prune_rows(&mut self.0, root);
        }
        pub(crate) fn get(&self, key: &Key) -> Option<&ScalarCell> {
            self.0.get(key)
        }
        pub(crate) fn get_mut(&mut self, key: &Key) -> Option<&mut ScalarCell> {
            self.0.get_mut(key)
        }
        pub(crate) fn remove(&mut self, key: &Key) -> Option<ScalarCell> {
            self.0.remove(key)
        }
        pub(crate) fn contains_key(&self, key: &Key) -> bool {
            self.0.contains_key(key)
        }
        pub(crate) fn len(&self) -> usize {
            self.0.len()
        }
        pub(crate) fn keys(&self) -> impl Iterator<Item = &Key> {
            self.0.keys()
        }
        pub(crate) fn retain(&mut self, keep: impl FnMut(&Key, &mut ScalarCell) -> bool) {
            self.0.retain(keep);
        }
        pub(crate) fn take(&mut self) -> Self {
            Self(self.0.take())
        }
        pub(crate) fn validate_append(&self, other: &Self) -> Result<(), TerminalInterpretError> {
            self.0.validate_append(&other.0)
        }
        pub(crate) fn append(&mut self, other: &mut Self) -> Result<(), TerminalInterpretError>
        where
            Key: crate::storage::structural_storage::StorageKey,
        {
            self.0.validate_append(&other.0)?;
            other
                .prepare_resident_replacement(self, None)?
                .commit(other)?;
            self.0.append(&mut other.0)
        }

        pub(crate) fn prepare_resident_replacement(
            &self,
            original: &Self,
            root: Option<&crate::boundary::values::StructuralRuntimePlace>,
        ) -> Result<ResidentReplacementWrites<Key>, TerminalInterpretError>
        where
            Key: crate::storage::structural_storage::StorageKey,
        {
            if let Some(root) = root {
                for (key, cell) in crate::storage::structural_storage::cells(original, root) {
                    if matches!(cell, ScalarCell::Resident { .. }) && !self.0.contains_key(key) {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "resident replacement needs a codec for its changed active layout",
                        ));
                    }
                }
            }
            let mut writes = Vec::new();
            for (key, incoming) in self.0.iter() {
                if let Some(
                    resident @ ScalarCell::Resident {
                        scalar_type,
                        backing,
                    },
                ) = original.0.get(key)
                {
                    if matches!(incoming, ScalarCell::Resident { backing: other, .. } if Rc::ptr_eq(backing, other))
                    {
                        continue;
                    }
                    let value = incoming.read()?;
                    if value.scalar_type() != *scalar_type {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "resident replacement needs a codec for its changed scalar declaration",
                        ));
                    }
                    writes.push((key.clone(), resident.clone(), value));
                }
            }
            Ok(ResidentReplacementWrites(writes))
        }
    }

    impl<Key> std::borrow::Borrow<BTreeMap<Key, ScalarCell>> for ScalarStorageRows<Key> {
        fn borrow(&self) -> &BTreeMap<Key, ScalarCell> {
            self.0.borrow()
        }
    }

    pub(crate) struct ResidentReplacementWrites<Key>(Vec<(Key, ScalarCell, TerminalScalarValue)>);

    impl<Key: Ord> ResidentReplacementWrites<Key> {
        pub(crate) fn commit(
            self,
            staged: &mut ScalarStorageRows<Key>,
        ) -> Result<(), TerminalInterpretError> {
            if self.0.iter().any(|(key, _, _)| !staged.0.contains_key(key)) {
                return Err(malformed());
            }
            for (key, mut resident, value) in self.0 {
                resident.write(value)?;
                staged.0.insert(key, resident)?;
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::IntegerType;
        use std::cell::{Cell, RefCell};

        #[derive(Debug)]
        struct Backing {
            value: RefCell<TerminalScalarValue>,
            faulted: Cell<bool>,
        }
        impl TerminalScalarBacking for Backing {
            fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
                if self.faulted.get() {
                    return Err(TerminalEffectRejection::new("original home is faulted"));
                }
                Ok(*self.value.borrow())
            }
            fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
                if self.faulted.get() {
                    return Err(TerminalEffectRejection::new("original home is faulted"));
                }
                *self.value.borrow_mut() = value;
                Ok(())
            }
        }
        fn word(value: u128) -> TerminalScalarValue {
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                    .unwrap(),
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
            }
        }
        fn backing() -> Rc<Backing> {
            Rc::new(Backing {
                value: RefCell::new(word(37)),
                faulted: Cell::new(false),
            })
        }

        #[test]
        fn resident_observations_and_moves_share_one_original_but_copies_capture_values() {
            let home = backing();
            let resident = ScalarCell::resident(word(37), home.clone()).unwrap();
            *home.value.borrow_mut() = word(91);
            assert_eq!(resident.read().unwrap(), word(91));
            let mut moved = resident.relocated(true).unwrap();
            let copied = resident.relocated(false).unwrap();
            moved.write(word(99)).unwrap();
            assert_eq!(home.read().unwrap(), word(99));
            assert_eq!(resident.read().unwrap(), word(99));
            assert_eq!(copied.read().unwrap(), word(91));
            let mut rows = ScalarStorageRows::new(None);
            rows.insert_cell(1, moved).unwrap();
            rows.insert(1, word(101)).unwrap();
            assert_eq!(home.read().unwrap(), word(101));
            assert_eq!(rows.get(&1).unwrap().read().unwrap(), word(101));
        }

        #[test]
        fn mismatched_or_faulted_original_storage_cannot_fall_back_to_local_contents() {
            let home = backing();
            assert!(ScalarCell::resident(word(91), home.clone()).is_err());
            let mut resident = ScalarCell::resident(word(37), home.clone()).unwrap();
            assert!(resident.write(TerminalScalarValue::Boolean(true)).is_err());
            assert_eq!(home.read().unwrap(), word(37));
            home.faulted.set(true);
            assert!(matches!(
                resident.read(),
                Err(TerminalInterpretError::ResidentStorage(_))
            ));
            assert!(resident.relocated(false).is_err());
            assert!(resident.write(word(99)).is_err());
            assert_eq!(*home.value.borrow(), word(37));
        }

        #[test]
        fn aggregate_replacement_keeps_the_original_home_after_pruning_old_rows() {
            let home = backing();
            let root = crate::boundary::values::StructuralRuntimePlace {
                opaque_identity: 11,
                path: Vec::new(),
            };
            let mut original = ScalarStorageRows::new(None);
            original
                .insert_cell(
                    root.clone(),
                    ScalarCell::resident(word(37), home.clone()).unwrap(),
                )
                .unwrap();
            let mut replacement = ScalarStorageRows::new(None);
            replacement.insert(root.clone(), word(91)).unwrap();
            let writes = replacement
                .prepare_resident_replacement(&original, Some(&root))
                .unwrap();
            assert_eq!(
                home.read().unwrap(),
                word(37),
                "preflight performs no writes"
            );
            writes.commit(&mut replacement).unwrap();
            original.prune(&root);
            original.append(&mut replacement).unwrap();
            assert_eq!(home.read().unwrap(), word(91));
            *home.value.borrow_mut() = word(99);
            assert_eq!(original.get(&root).unwrap().read().unwrap(), word(99));
            let mut next = ScalarStorageRows::new(None);
            next.insert(root.clone(), word(101)).unwrap();
            original.append(&mut next).unwrap();
            assert_eq!(
                home.read().unwrap(),
                word(101),
                "overlapping append retains the resident"
            );
        }

        #[test]
        fn incomplete_or_retyped_resident_replacement_refuses_before_writing() {
            let home = backing();
            let root = crate::boundary::values::StructuralRuntimePlace {
                opaque_identity: 11,
                path: Vec::new(),
            };
            let mut original = ScalarStorageRows::new(None);
            original
                .insert_cell(
                    root.clone(),
                    ScalarCell::resident(word(37), home.clone()).unwrap(),
                )
                .unwrap();
            let mut replacement = ScalarStorageRows::new(None);
            assert!(
                replacement
                    .prepare_resident_replacement(&original, Some(&root))
                    .is_err()
            );
            replacement
                .insert(root.clone(), TerminalScalarValue::Boolean(true))
                .unwrap();
            assert!(
                replacement
                    .prepare_resident_replacement(&original, Some(&root))
                    .is_err()
            );
            assert_eq!(home.read().unwrap(), word(37));
        }
    }
}

pub(crate) mod storage_addresses {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorageHome;
    use crate::boundary::values::{
        StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
    };
    use crate::engine::execution::TerminalExecution;
    use crate::engine::execution::frame_field_mut;
    use crate::storage::primitive_storage::{PrimitiveStorageEndpoint, scalar_field_projection};
    use semantic_vocabulary::{IntegerValue, ScalarType};
    use terminal_psi::{Operation, StructuralArgument, StructuralPathSegment};
    use terminal_semantics::reference_storage_size;

    fn exhausted() -> TerminalInterpretError {
        TerminalInterpretError::StructuralIdentityExhausted
    }

    type Coordinate = (u64, u64);

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum ScalarStorageAddressKey {
        Scalar(StructuralRuntimePlace),
        Field(StructuralScalarRuntimeField),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ScalarStorageAddress {
        namespace: u64,
        root: u64,
        path: Vec<Coordinate>,
    }

    fn encode(
        namespace: u64,
        root: u64,
        path: &[Coordinate],
    ) -> Result<u64, TerminalInterpretError> {
        fn append(bits: &mut u64, value: u64) -> Result<(), TerminalInterpretError> {
            let value = value.checked_add(1).ok_or_else(exhausted)?;
            let width = u64::BITS - value.leading_zeros();
            let shift = width
                .checked_mul(2)
                .and_then(|width| width.checked_sub(1))
                .ok_or_else(exhausted)?;
            let scale = 1_u64.checked_shl(shift).ok_or_else(exhausted)?;
            *bits = bits
                .checked_mul(scale)
                .and_then(|bits| bits.checked_add(value))
                .ok_or_else(exhausted)?;
            Ok(())
        }
        let mut bits = 1;
        append(&mut bits, namespace)?;
        append(&mut bits, root)?;
        append(
            &mut bits,
            u64::try_from(path.len()).map_err(|_| exhausted())?,
        )?;
        for (kind, coordinate) in path {
            append(&mut bits, *kind)?;
            append(&mut bits, *coordinate)?;
        }
        bits.checked_mul(2)
            .and_then(|bits| bits.checked_add(1))
            .ok_or_else(exhausted)
    }

    fn path_coordinates(
        path: &[StructuralPathSegment],
    ) -> Result<Vec<Coordinate>, TerminalInterpretError> {
        path.iter()
            .map(|segment| match segment {
                StructuralPathSegment::FixedIndex(index) => Ok((0, *index)),
                StructuralPathSegment::Field(field) => Ok((1, u64::from(field.0))),
                StructuralPathSegment::Case(case) => Ok((2, case.get())),
                _ => Err(malformed()),
            })
            .collect()
    }

    impl TerminalExecution<'_> {
        pub(crate) fn scalar_storage_address(
            &self,
            key: &ScalarStorageAddressKey,
        ) -> Result<ScalarStorageAddress, TerminalInterpretError> {
            if let Some(original) = self.component_borrowed_addresses.get(key) {
                return Ok(original.clone());
            }
            let (home, final_field) = match key {
                ScalarStorageAddressKey::Scalar(home) => (home, None),
                ScalarStorageAddressKey::Field(field) => {
                    let kind = match field.namespace {
                        crate::boundary::values::StructuralFieldNamespace::Member => 3,
                        crate::boundary::values::StructuralFieldNamespace::CasePayload => 4,
                    };
                    (&field.parent, Some((kind, field.field.get())))
                }
            };
            let mut path = path_coordinates(&home.path)?;
            path.extend(final_field);
            Ok(ScalarStorageAddress {
                namespace: self
                    .local_structural_identities
                    .exposure_namespace
                    .ok_or_else(exhausted)?,
                root: home.opaque_identity,
                path,
            })
        }

        fn storage_address_endpoint(
            &mut self,
            source: &StructuralArgument,
            scalar_type: Option<ScalarType>,
        ) -> Result<PrimitiveStorageEndpoint, TerminalInterpretError> {
            let Some(scalar_type) = scalar_type else {
                let selected = self.element_collection_storage(source)?;
                if let Some(crate::storage::reference::AggregateViewTip::Aggregate {
                    view,
                    backing,
                }) = selected.view
                    && (view.byte_offset != 0
                        || reference_storage_size(self.structural_types().values(), backing)
                            != Some(view.byte_length))
                {
                    return Err(TerminalInterpretError::StructuralArgumentPreparation {
                        stage: "storage address",
                        detail:
                            "an aggregate re-view window has no canonical storage address endpoint"
                                .into(),
                    });
                }
                let place = match selected.home {
                    Some(home) => {
                        self.owned_array_home(&home.storage)?;
                        let (depth, place, _) = home.storage.scalar_array_frame();
                        StructuralRuntimePlace {
                            opaque_identity: self.array_storage_identity(depth, place)?,
                            path: home.path,
                        }
                    }
                    None => StructuralRuntimePlace::from(
                        selected.collection.as_ref().ok_or_else(malformed)?,
                    ),
                };
                return Ok(PrimitiveStorageEndpoint::Scalar(place));
            };
            if let Some((prefix, field_path)) = scalar_field_projection(&source.path) {
                let (carrier, view) = self.resolve_live_view_projection(
                    source.place,
                    prefix,
                    terminal_psi::StructuralAccess::SharedBorrow,
                )?;
                if let Some(view) = view {
                    return self
                        .aggregate_view_primitive_access(
                            &carrier,
                            view,
                            field_path,
                            self.aggregate_argument_backings
                                .get(&source.place)
                                .and_then(|backing| backing.collection_home.as_ref()),
                            false,
                        )
                        .map(|(storage, _, _)| storage.endpoint);
                }
                if let Some(observed) =
                    self.scalar_field_at_carrier(carrier, view, field_path, scalar_type)?
                {
                    return Ok(self
                        .reference_byte_region_access(&observed.carrier, &observed.leaf)?
                        .map(|(storage, _)| storage.endpoint)
                        .unwrap_or(PrimitiveStorageEndpoint::ScalarField(observed.cell)));
                }
            }
            self.primitive_access(source.place, false, &source.path)
                .map(|(storage, _, _)| storage.endpoint)
        }

        fn array_storage_identity(
            &mut self,
            depth: usize,
            place: semantic_vocabulary::PlaceId,
        ) -> Result<u64, TerminalInterpretError> {
            let mut cursor = self.local_structural_identities.cursor();
            let identity = self.array_storage_identity_staged(depth, place, &mut cursor)?;
            self.local_structural_identities.commit_cursor(cursor);
            Ok(identity)
        }

        pub(crate) fn array_storage_identity_staged(
            &mut self,
            depth: usize,
            place: semantic_vocabulary::PlaceId,
            cursor: &mut Option<u64>,
        ) -> Result<u64, TerminalInterpretError> {
            if let Some(identity) = self
                .frame_array(depth, place)
                .ok_or_else(malformed)?
                .storage_identity
            {
                return Ok(identity);
            }
            let identity = self.local_structural_identities.allocate_staged(cursor)?;
            let arrays = frame_field_mut!(self, depth, scalar_array_values);
            arrays
                .backing_mut(&place)
                .ok_or_else(malformed)?
                .storage_identity = Some(identity);
            Ok(identity)
        }

        fn endpoint_identity(
            &mut self,
            endpoint: PrimitiveStorageEndpoint,
        ) -> Result<(Option<u64>, u64, Vec<Coordinate>), TerminalInterpretError> {
            let selected = match endpoint {
                PrimitiveStorageEndpoint::RepresentationByte(_) => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "representation byte address requires its original scalar byte placement",
                    ));
                }
                PrimitiveStorageEndpoint::Projection {
                    fragments, stored, ..
                } => {
                    let [first] = fragments.as_slice() else {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "composed scalar observations have no single storage-address endpoint",
                        ));
                    };
                    let endpoint = first.contiguous_address_endpoint(stored).ok_or(
                    TerminalInterpretError::UnsupportedSemanticVariant(
                        "storage-address exposure requires a contiguous original scalar endpoint",
                    ),
                )?;
                    return self.endpoint_identity(endpoint);
                }
                PrimitiveStorageEndpoint::Scalar(place) => {
                    let address =
                        self.scalar_storage_address(&ScalarStorageAddressKey::Scalar(place))?;
                    return Ok((Some(address.namespace), address.root, address.path));
                }
                PrimitiveStorageEndpoint::ScalarField(field) => {
                    let address =
                        self.scalar_storage_address(&ScalarStorageAddressKey::Field(field))?;
                    return Ok((Some(address.namespace), address.root, address.path));
                }
                PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                    let (depth, place, _) = home.scalar_array_frame();
                    StructuralRuntimePlace {
                        opaque_identity: self.array_storage_identity(depth, place)?,
                        path: vec![StructuralPathSegment::FixedIndex(
                            u64::try_from(index).map_err(|_| exhausted())?,
                        )],
                    }
                }
                PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                    let mut place = match array.home {
                        ArrayStorageHome::Runtime => array.referent,
                        ArrayStorageHome::Frame { depth, place, .. } => StructuralRuntimePlace {
                            opaque_identity: self.array_storage_identity(depth, place)?,
                            path: Vec::new(),
                        },
                    };
                    place.path.push(StructuralPathSegment::FixedIndex(
                        u64::try_from(index).map_err(|_| exhausted())?,
                    ));
                    place
                }
                PrimitiveStorageEndpoint::ViewByte { bytes, index } => {
                    let (namespace, index) = bytes
                        .storage_address_coordinate(index)
                        .ok_or_else(exhausted)?;
                    return Ok((
                        Some(namespace),
                        0,
                        path_coordinates(&[StructuralPathSegment::FixedIndex(index)])?,
                    ));
                }
            };
            Ok((
                None,
                selected.opaque_identity,
                path_coordinates(&selected.path)?,
            ))
        }

        pub(crate) fn execute_storage_address(
            &mut self,
            operation: &Operation,
            source: &StructuralArgument,
        ) -> Result<(), TerminalInterpretError> {
            let result = operation.result.scalar().ok_or_else(malformed)?;
            let ScalarType::Integer(integer) = result.scalar_type else {
                return Err(malformed());
            };
            if !integer.is_address()
                || source.access != terminal_psi::StructuralAccess::SharedBorrow
            {
                return Err(malformed());
            }
            let scalar_type = terminal_semantics::primitive_read_projection_type(
                self.structural_types().values(),
                self.live_structural_type(source.place)?,
                &source.path,
            );
            let endpoint = self.storage_address_endpoint(source, scalar_type)?;
            let (namespace, root, path) = self.endpoint_identity(endpoint)?;
            let namespace = namespace
                .or(self.local_structural_identities.exposure_namespace)
                .ok_or_else(exhausted)?;
            let encoded = encode(namespace, root, &path)?;
            let value = IntegerValue::Unsigned(u128::from(encoded));
            if !integer.admits(value) {
                return Err(exhausted());
            }
            self.values.insert(
                result.id,
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value,
                },
            )?;
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn field_and_array_coordinates_do_not_collide() {
            let field = encode(0, 7, &[(3, 1)]).unwrap();
            let nested = encode(0, 7, &[(0, 0), (1, 1)]).unwrap();
            assert_ne!(field, nested);
            assert_ne!(field, encode(1, 7, &[(3, 1)]).unwrap());
            assert_eq!(field, encode(0, 7, &[(3, 1)]).unwrap());
        }

        #[test]
        fn complete_identity_overflow_rejects() {
            assert!(encode(u64::MAX, 0, &[]).is_err());
            assert!(encode(0, u64::MAX, &[]).is_err());
            assert_ne!(encode(0, 0, &[]).unwrap(), 0);
            assert_eq!(encode(0, 0, &[]).unwrap() & 1, 1);
        }
    }
}

pub(crate) mod structural_identities {
    use std::collections::BTreeSet;

    use std::sync::atomic::Ordering;
    use terminal_psi::StructuralPlaceDeclaration;

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::TerminalStructuralValue;

    pub(crate) struct LocalStructuralIdentities {
        reserved: BTreeSet<u64>,
        next: Option<u64>,
        pub(crate) exposure_namespace: Option<u64>,
    }

    impl LocalStructuralIdentities {
        pub(crate) fn new<'a>(
            structural_places: impl IntoIterator<Item = &'a StructuralPlaceDeclaration>,
            arguments: &[TerminalStructuralValue],
        ) -> Self {
            Self {
                reserved: structural_places
                    .into_iter()
                    .map(|place| place.id.get())
                    .chain(arguments.iter().map(|value| value.opaque_identity))
                    .collect(),
                next: Some(0),
                exposure_namespace: new_exposure_namespace(),
            }
        }

        pub(crate) fn allocate(&mut self) -> Result<u64, TerminalInterpretError> {
            let mut cursor = self.next;
            let result = self.allocate_staged(&mut cursor);
            self.next = cursor;
            result
        }

        pub(crate) fn owns_allocated(&self, identity: u64) -> bool {
            self.next.is_none_or(|next| identity < next) && !self.reserved.contains(&identity)
        }

        pub(crate) fn cursor(&self) -> Option<u64> {
            self.next
        }

        pub(crate) fn allocate_staged(
            &self,
            cursor: &mut Option<u64>,
        ) -> Result<u64, TerminalInterpretError> {
            loop {
                let identity = cursor.ok_or(TerminalInterpretError::StructuralIdentityExhausted)?;
                *cursor = identity.checked_add(1);
                if !self.reserved.contains(&identity) {
                    return Ok(identity);
                }
            }
        }

        pub(crate) fn commit_cursor(&mut self, cursor: Option<u64>) {
            self.next = cursor;
        }

        pub(crate) fn reserve_host_at_cursor(
            &mut self,
            value: &TerminalStructuralValue,
            cursor: Option<u64>,
        ) -> Result<(), TerminalInterpretError> {
            if match (self.next, cursor) {
                (Some(current), Some(staged)) => staged < current,
                (None, Some(_)) => true,
                _ => false,
            } {
                return Err(malformed());
            }
            let identity = value.opaque_identity;
            if cursor.is_none_or(|next| identity < next) && !self.reserved.contains(&identity) {
                return Err(TerminalInterpretError::StructuralArgumentAliasing(identity));
            }
            self.reserved.insert(identity);
            Ok(())
        }
    }

    pub(crate) fn new_exposure_namespace() -> Option<u64> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        NEXT.try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(1)
        })
        .ok()
    }
}
