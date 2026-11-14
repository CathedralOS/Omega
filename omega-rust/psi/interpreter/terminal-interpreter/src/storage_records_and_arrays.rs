/*
    Record construction staging, packed scalar arrays, and identity-bounded
    structural storage tables in the Terminal Psi interpreter.

    We stage record and array constructor payloads under paid row reservations,
    preserve independent common and case-payload field namespaces, and retire
    replaced or disposed subtrees cleanly.
*/

pub(crate) mod record {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::boundary::values::StructuralScalarRuntimeField;
    use crate::boundary::values::TerminalStructuralValue;
    use crate::engine::execution::TerminalExecution;
    use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
    use crate::storage::runtime_rows::RuntimeRows;
    use terminal_psi::StructuralAccess;
    use terminal_psi::StructuralFieldType;
    use terminal_psi::StructuralMultiplicity;
    use terminal_psi::StructuralParameterDeclaration;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::StructuralTypeShape;

    pub(crate) mod byte_fields {
        use super::storage::ConstructorValue;
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::engine::execution::TerminalExecution;
        use crate::storage::byte_sequences::binding::ByteSequenceBinding;
        use std::collections::BTreeSet;
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
            StructuralFieldDeclaration, StructuralTypeShape,
        };
        use terminal_semantics::inline_carrier_declaration;

        impl TerminalExecution<'_> {
            pub(crate) fn prepare_constructor_byte_field(
                &self,
                declaration: &StructuralFieldDeclaration,
                argument: &StructuralArgument,
                child: &ConstructorValue<'_>,
                frontier: &mut BTreeSet<StructuralAffineDiscard>,
                consumed: &mut Vec<StructuralArgument>,
            ) -> Result<Option<ByteSequenceBinding>, TerminalInterpretError> {
                let Some(carrier) = declaration.field_type.byte_sequence_carrier() else {
                    return Ok(None);
                };
                let ConstructorValue::Access(child) = child else {
                    return Err(malformed());
                };
                let binding = self.byte_sequence_values.require(&argument.place)?;
                if argument.access != StructuralAccess::Owned || !argument.path.is_empty() {
                    return Err(malformed());
                }
                if carrier.is_shared_readable_view() {
                    let bytes = binding.snapshot(self)?;
                    if !self
                        .structural_types()
                        .get(&child.structural_type)
                        .is_some_and(|row| row.shape.is_shared_readable_byte_view())
                        || !terminal_semantics::byte_literal_qualifications_hold_in(
                            self.program.catalogs.structural_domains.iter(),
                            child.structural_type,
                            &child.qualifications,
                            bytes.bytes(),
                        )
                        || !declaration
                            .field_type
                            .byte_predicates()
                            .holds_for(bytes.bytes())
                    {
                        return Err(malformed());
                    }
                    self.stage_constructor_operand(argument, frontier, consumed)?;
                } else if let ByteSequenceCarrier::BoundedOwned { capacity } = carrier {
                    let bytes = binding.immutable()?;
                    let predicates = terminal_semantics::byte_qualification_predicates_in(
                        self.program.catalogs.structural_domains.iter(),
                        child.structural_type,
                        &child.qualifications,
                    )
                    .ok_or_else(malformed)?;
                    if !child.path.is_empty()
                        || !self
                            .structural_types()
                            .get(&child.structural_type)
                            .is_some_and(|row| {
                                row.shape == StructuralTypeShape::ByteSequence(carrier)
                            })
                        || !predicates.implies(declaration.field_type.byte_predicates())
                        || u64::try_from(bytes.len()).map_or(true, |length| length > capacity)
                        || self.live_affine_discard_at(argument.place).is_some()
                    {
                        return Err(malformed());
                    }
                } else if carrier.is_borrowed() {
                    let declaration_type = self.structural_types();
                    let actual = declaration_type
                        .get(&child.structural_type)
                        .ok_or_else(malformed)?;
                    let expected =
                        inline_carrier_declaration(declaration_type.values(), declaration)
                            .ok_or_else(|| {
                                malformed().in_argument_preparation(
                                    "byte field has no stored descriptor type",
                                )
                            })?;
                    let machine = self.program.machine(self.current_machine)?;
                    let (declared_type, multiplicity) = machine
                        .structural_parameter(argument.place)
                        .map(|parameter| (parameter.structural_type, parameter.multiplicity))
                        .or_else(|| {
                            let (operation, structural_type) =
                                machine.producing_operation(argument.place)?;
                            let result = operation.result.structural()?;
                            (result.place == argument.place
                                && result.structural_type == structural_type)
                                .then_some((structural_type, result.multiplicity))
                        })
                        .ok_or_else(|| {
                            malformed()
                                .in_argument_preparation("byte operand has no declared custody")
                        })?;
                    if declared_type != actual.id
                        || !terminal_semantics::descriptor_value_transfer_matches(
                            declaration_type.values(),
                            actual.id,
                            expected.id,
                            multiplicity,
                        )
                    {
                        return Err(malformed().in_argument_preparation(
                            "byte field operand changes its descriptor or access",
                        ));
                    }
                    if matches!(
                        binding,
                        ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::Representation(_)
                    ) || !child.projected_qualifications.is_empty()
                        || !terminal_semantics::byte_qualification_predicates_in(
                            self.program.catalogs.structural_domains.iter(),
                            child.structural_type,
                            &child.qualifications,
                        )
                        .ok_or_else(malformed)?
                        .implies(declaration.field_type.byte_predicates())
                    {
                        return Err(malformed());
                    }
                    binding.validate_mutable_referent(self, child)?;
                    let length = self.byte_descriptor_length(binding, child)?;
                    if matches!(carrier, ByteSequenceCarrier::BoundedView { capacity, .. } if length != capacity)
                        || !self.stage_constructor_operand(argument, frontier, consumed)?
                    {
                        return Err(malformed());
                    }
                } else {
                    return Err(malformed());
                }
                Ok(Some(binding.clone()))
            }
        }
    }
    pub(crate) mod operands {
        use crate::boundary::errors::TerminalInterpretError;
        use crate::boundary::structural_inputs::case_membership::case_projection_with_membership_is_active;
        use crate::boundary::values::StructuralRuntimePlace;
        use crate::engine::custody::{
            resolve_single_structural_argument, resolve_structural_path_type, whole_path_discard,
        };
        use crate::engine::execution::{LiveClaim, TerminalExecution};
        use crate::storage::scalar_array::empty_storage::EmptyArrayRetirement;
        use semantic_vocabulary::StructuralCaseId;
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{
            StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
            StructuralOperationResult, StructuralPathSegment,
        };

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ResultClaimOrigin {
            Construction,
            Transfer,
        }

        pub(crate) struct PreparedConstructorOperands {
            homes: Vec<(StructuralArgument, EmptyArrayRetirement)>,
            frontier: BTreeSet<StructuralAffineDiscard>,
            claims: BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>,
        }

        impl PreparedConstructorOperands {
            pub(crate) fn commit(
                self,
                execution: &mut TerminalExecution<'_>,
            ) -> Result<(), TerminalInterpretError> {
                for (argument, retirement) in self.homes {
                    execution.prune_subtree(retirement)?;
                    if argument.path.is_empty() {
                        execution.structural_values.remove(&argument.place);
                    }
                }
                execution.live_affine_frontier = self.frontier;
                execution.live_claims = self.claims;
                Ok(())
            }
        }

        impl TerminalExecution<'_> {
            pub(crate) fn validate_transferred_affine_complement(
                &self,
                source: &StructuralArgument,
                result: &StructuralOperationResult,
                result_frontier: &BTreeSet<StructuralAffineDiscard>,
                result_case: &impl Fn(&StructuralAffineDiscard) -> Option<StructuralCaseId>,
            ) -> Result<(), TerminalInterpretError> {
                let types = self.structural_types();
                for (_, claim) in
                    crate::engine::custody::projected_ownership_claims(&self.live_claims, source)?
                {
                    if claim.path.is_empty() && claim.multiplicity != Some(result.multiplicity) {
                        return Err(TerminalInterpretError::ClaimTransferMismatch);
                    }
                }
                let mut actual = self.live_affine_frontier.clone();
                if let Some(containing) = actual
                    .iter()
                    .find(|entry| {
                        entry.place == source.place
                            && source.path.starts_with(&entry.path)
                            && entry.path != source.path
                    })
                    .cloned()
                {
                    actual.remove(&containing);
                    crate::engine::custody::split_affine_frontier_at_projection(
                        &types,
                        &mut actual,
                        containing,
                        &source.path,
                        &|current| {
                            crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                        &types,
                        self.structural_values.get(&current.place)?,
                        self.structural_cases.as_map(),
                        &current.path,
                    )
                        },
                    )?;
                    actual.insert(StructuralAffineDiscard {
                        place: source.place,
                        path: source.path.clone(),
                        structural_type: result.structural_type,
                    });
                }
                let mut remaining = result_frontier
                    .iter()
                    .filter(|entry| entry.place == result.place)
                    .cloned()
                    .collect::<BTreeSet<_>>();
                for entry in actual.iter().filter(|entry| {
                    entry.place == source.place && entry.path.starts_with(&source.path)
                }) {
                    let path = &entry.path[source.path.len()..];
                    let containing = remaining
                        .iter()
                        .find(|expected| path.starts_with(&expected.path))
                        .cloned()
                        .ok_or(TerminalInterpretError::AffineFrontierMismatch)?;
                    let selected =
                        resolve_structural_path_type(&types, result.structural_type, path)?;
                    if selected != entry.structural_type {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    remaining.remove(&containing);
                    crate::engine::custody::split_affine_frontier_at_projection(
                        &types,
                        &mut remaining,
                        containing,
                        path,
                        result_case,
                    )?;
                }
                if !remaining.is_empty() {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                Ok(())
            }

            pub(crate) fn stage_constructor_operand(
                &self,
                argument: &StructuralArgument,
                frontier: &mut BTreeSet<StructuralAffineDiscard>,
                consumed: &mut Vec<StructuralArgument>,
            ) -> Result<bool, TerminalInterpretError> {
                self.require_owned_value_claims(argument)?;
                let affine = self.live_affine_frontier.iter().any(|entry| {
                    entry.place == argument.place
                        && terminal_semantics::structural_paths_may_overlap(
                            &entry.path,
                            &argument.path,
                        )
                });
                if affine {
                    if argument.path.is_empty() {
                        let before = frontier.len();
                        frontier.retain(|entry| entry.place != argument.place);
                        if frontier.len() == before {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                    } else if !crate::engine::custody::consume_projected_affine_complement(
                        &self.structural_types(),
                        &self.structural_values,
                        self.structural_cases.as_map(),
                        frontier,
                        argument,
                    )? {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    consumed.push(argument.clone());
                }
                let claimed = self.live_claims.values().any(|claim| {
                    claim.place == Some(argument.place) && claim.path.starts_with(&argument.path)
                });
                if claimed {
                    if consumed.iter().any(|previous| {
                        previous.place == argument.place
                            && (previous.path.starts_with(&argument.path)
                                || argument.path.starts_with(&previous.path))
                    }) && !affine
                    {
                        return Err(TerminalInterpretError::ClaimTransferMismatch);
                    }
                    if !affine {
                        consumed.push(argument.clone());
                    }
                } else if !affine && !argument.path.is_empty() {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                Ok(affine || claimed)
            }

            pub(crate) fn stage_constructor_claims(
                &self,
                origin: ResultClaimOrigin,
                result: &StructuralOperationResult,
                operands: &[(&StructuralArgument, Vec<StructuralPathSegment>)],
                frontier: &mut BTreeSet<StructuralAffineDiscard>,
                case_membership: &impl Fn(&StructuralAffineDiscard) -> Option<StructuralCaseId>,
            ) -> Result<BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>, TerminalInterpretError>
            {
                let invalid = || TerminalInterpretError::ClaimTransferMismatch;
                let nominal_root = match result.claims.as_slice() {
                    [binding]
                        if origin == ResultClaimOrigin::Construction
                            && result.multiplicity == StructuralMultiplicity::Linear
                            && binding.path.is_empty()
                            && !self.live_claims.contains_key(&binding.claim)
                            && self
                                .structural_types()
                                .get(&result.structural_type)
                                .is_some_and(|declaration| {
                                    matches!(
                                        declaration.shape,
                                        terminal_psi::StructuralTypeShape::Record { .. }
                                            | terminal_psi::StructuralTypeShape::Sum { .. }
                                            | terminal_psi::StructuralTypeShape::Mixed { .. }
                                    )
                                }) =>
                    {
                        Some(binding.claim)
                    }
                    _ => None,
                };
                let mut remaining = self.live_claims.clone();
                let mut transported = BTreeMap::new();
                for (argument, destination) in operands {
                    for (identity, claim) in &self.live_claims {
                        if claim.place != Some(argument.place) {
                            continue;
                        }
                        let Some(suffix) = claim.path.strip_prefix(argument.path.as_slice()) else {
                            continue;
                        };
                        let mut path = destination.clone();
                        path.extend_from_slice(suffix);
                        if transported.insert(*identity, path).is_some()
                            || remaining.remove(identity).is_none()
                        {
                            return Err(invalid());
                        }
                    }
                }
                for binding in &result.claims {
                    if !self.constructor_claim_is_active(result, &binding.path, case_membership)? {
                        if transported.contains_key(&binding.claim)
                            || self.live_claims.contains_key(&binding.claim)
                        {
                            return Err(invalid());
                        }
                        continue;
                    }
                    match transported.remove(&binding.claim) {
                        Some(path) if path == binding.path => {}
                        None if nominal_root == Some(binding.claim) => {}
                        _ => return Err(invalid()),
                    }
                    if remaining
                        .insert(
                            binding.claim,
                            LiveClaim::at(result.place, binding.path.clone(), result.multiplicity),
                        )
                        .is_some()
                    {
                        return Err(invalid());
                    }
                }
                for path in transported.values() {
                    if nominal_root.is_none()
                        || resolve_structural_path_type(
                            &self.structural_types(),
                            result.structural_type,
                            path,
                        )
                        .is_err()
                        || !self.constructor_claim_is_active(result, path, case_membership)?
                    {
                        return Err(invalid());
                    }
                }
                self.stage_result_affine_frontier(result, frontier, case_membership)?;
                Ok(remaining)
            }

            pub(crate) fn constructor_claim_is_active(
                &self,
                result: &StructuralOperationResult,
                path: &[StructuralPathSegment],
                membership: &impl Fn(&StructuralAffineDiscard) -> Option<StructuralCaseId>,
            ) -> Result<bool, TerminalInterpretError> {
                let types = self.structural_types();
                case_projection_with_membership_is_active(
                    &types,
                    result.structural_type,
                    path,
                    &|prefix| {
                        let structural_type =
                            resolve_structural_path_type(&types, result.structural_type, prefix)
                                .ok()?;
                        membership(&StructuralAffineDiscard {
                            place: result.place,
                            path: prefix.to_vec(),
                            structural_type,
                        })
                    },
                )
            }

            pub(crate) fn stage_result_affine_frontier(
                &self,
                result: &StructuralOperationResult,
                frontier: &mut BTreeSet<StructuralAffineDiscard>,
                case_membership: &impl Fn(&StructuralAffineDiscard) -> Option<StructuralCaseId>,
            ) -> Result<(), TerminalInterpretError> {
                if result.multiplicity == StructuralMultiplicity::Affine {
                    let mut active_claims = Vec::new();
                    for claim in &result.claims {
                        if self.constructor_claim_is_active(result, &claim.path, case_membership)? {
                            active_claims.push((result.place, claim.path.as_slice()));
                        }
                    }
                    let root = whole_path_discard(result.place, result.structural_type);
                    if !frontier.insert(root) {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    crate::engine::custody::exclude_linear_regions(
                        &self.structural_types(),
                        frontier,
                        active_claims,
                        case_membership,
                    )?;
                }
                Ok(())
            }

            pub(crate) fn commit_constructor_operands(
                &mut self,
                frontier: BTreeSet<StructuralAffineDiscard>,
                consumed: &[StructuralArgument],
                claims: BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>,
            ) -> Result<(), TerminalInterpretError> {
                self.prepare_constructor_operands(frontier, consumed, claims)?
                    .commit(self)?;
                Ok(())
            }

            pub(crate) fn prepare_constructor_operands(
                &self,
                frontier: BTreeSet<StructuralAffineDiscard>,
                consumed: &[StructuralArgument],
                claims: BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>,
            ) -> Result<PreparedConstructorOperands, TerminalInterpretError> {
                let homes = consumed
                    .iter()
                    .map(|argument| {
                        let child = resolve_single_structural_argument(
                            &self.structural_types(),
                            &self.structural_values,
                            argument,
                        )?;
                        Ok((
                            argument.clone(),
                            self.prepare_subtree_retirement(&StructuralRuntimePlace::from(&child))?,
                        ))
                    })
                    .collect::<Result<Vec<_>, TerminalInterpretError>>()?;
                Ok(PreparedConstructorOperands {
                    homes,
                    frontier,
                    claims,
                })
            }
        }
    }
    pub(crate) mod owned_value {
        use super::storage::{ConstructorStorage, ConstructorValue};
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
        use crate::engine::execution::TerminalExecution;
        use semantic_vocabulary::StructuralTypeId;
        use std::collections::BTreeSet;
        use terminal_psi::{
            StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
            StructuralOperationResult, StructuralPathSegment, StructuralResultClaimBinding,
        };

        pub(crate) struct StagedOwnedValue {
            pub(crate) value: TerminalStructuralValue,
            pub(crate) storage: ConstructorStorage,
            pub(crate) claim_paths: Vec<Vec<StructuralPathSegment>>,
            pub(crate) affine_regions: Vec<OwnedAffineRegion>,
        }

        pub(crate) struct OwnedAffineRegion {
            pub(crate) path: Vec<StructuralPathSegment>,
            pub(crate) structural_type: StructuralTypeId,
        }

        impl TerminalExecution<'_> {
            pub(crate) fn stage_owned_value(
                &self,
                source: &StructuralArgument,
                structural_type: StructuralTypeId,
                multiplicity: StructuralMultiplicity,
                identity_cursor: &mut Option<u64>,
            ) -> Result<StagedOwnedValue, TerminalInterpretError> {
                if source.access != StructuralAccess::Owned {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                let child = ConstructorValue::resolve(self, source)?;
                if child.structural_type() != structural_type {
                    return Err(malformed());
                }
                if let ConstructorValue::Access(value) = &child
                    && crate::engine::custody::has_live_loan(
                        self.reference_referents.as_map(),
                        value,
                    )
                {
                    return Err(TerminalInterpretError::ContentTransformationMismatch(
                        "owned value cannot transfer while its storage is borrowed",
                    ));
                }
                let claims =
                    crate::engine::custody::projected_ownership_claims(&self.live_claims, source)?;
                let claim_paths = claims
                    .iter()
                    .map(|(_, claim)| claim.path.clone())
                    .collect::<Vec<_>>();
                let (qualifications, projected_qualifications) = child.memberships();
                let retained = TerminalStructuralValue {
                    opaque_identity: self
                        .local_structural_identities
                        .allocate_staged(identity_cursor)?,
                    structural_type,
                    qualifications: qualifications.to_vec(),
                    projected_qualifications: projected_qualifications.to_vec(),
                    path: Vec::new(),
                };
                let transfers_custody = !claims.is_empty()
                    || self.live_affine_frontier.iter().any(|region| {
                        region.place == source.place
                            && terminal_semantics::structural_paths_may_overlap(
                                &region.path,
                                &source.path,
                            )
                    });
                if multiplicity != StructuralMultiplicity::Unrestricted && !transfers_custody {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                let mut storage = ConstructorStorage::new(self.account.as_ref());
                storage.stage(
                    self,
                    source,
                    &child,
                    &StructuralRuntimePlace::from(&retained),
                    transfers_custody,
                )?;

                let expected = StructuralOperationResult {
                    place: source.place,
                    structural_type,
                    multiplicity,
                    qualifications: retained.qualifications.clone(),
                    projected_qualifications: retained.projected_qualifications.clone(),
                    additional_input_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: claims
                        .iter()
                        .map(|(identity, claim)| StructuralResultClaimBinding {
                            claim: *identity,
                            path: claim.path.clone(),
                        })
                        .collect(),
                };
                let membership = |region: &StructuralAffineDiscard| {
                    storage.case_at_projection(self, &retained, &region.path)
                };
                let mut frontier = BTreeSet::new();
                self.stage_result_affine_frontier(&expected, &mut frontier, &membership)?;
                self.validate_transferred_affine_complement(
                    source,
                    &expected,
                    &frontier,
                    &membership,
                )?;
                let affine_regions = frontier
                    .into_iter()
                    .map(|region| OwnedAffineRegion {
                        path: region.path,
                        structural_type: region.structural_type,
                    })
                    .collect();

                Ok(StagedOwnedValue {
                    value: retained,
                    storage,
                    claim_paths,
                    affine_regions,
                })
            }
        }
    }
    pub(crate) use operands::ResultClaimOrigin;
    pub(crate) mod storage {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::{
            StructuralByteSequenceRuntimeField, StructuralRuntimePlace,
            StructuralScalarRuntimeField,
        };
        use crate::boundary::values::{TerminalScalarValue, TerminalStructuralValue};
        use crate::engine::custody::{fixed_index_step, resolve_single_structural_argument};
        use crate::engine::execution::TerminalExecution;
        use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
        use crate::storage::byte_sequences::ByteSequenceView;
        use crate::storage::byte_sequences::storage::ByteSequenceStorage;
        use crate::storage::runtime_rows::RuntimeRows;
        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

        pub(crate) mod aggregate {
            use super::ConstructorStorage;
            use crate::boundary::errors::{TerminalInterpretError, malformed};
            use crate::boundary::values::{
                ScalarAggregatePart, ScalarAggregatePlan, StructuralRuntimePlace,
                StructuralScalarRuntimeField, TerminalScalarAggregateValue,
            };
            use crate::engine::execution::TerminalExecution;
            use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
            use semantic_vocabulary::CanonicalStructuralPathSegment as Path;

            impl ConstructorStorage {
                pub(crate) fn stage_scalar_aggregate(
                    &mut self,
                    execution: &TerminalExecution,
                    aggregate: &TerminalScalarAggregateValue,
                    root: &StructuralRuntimePlace,
                ) -> Result<(), TerminalInterpretError> {
                    let destination = |path: &[Path]| {
                        let mut destination = root.clone();
                        destination.path.extend(
                            terminal_semantics::canonical_structural_storage_path(
                                execution.structural_types().values(),
                                aggregate.structural_type,
                                path,
                            )
                            .ok_or_else(malformed)?,
                        );
                        Ok::<_, TerminalInterpretError>(destination)
                    };
                    if let Some(selected) = aggregate.result_case {
                        self.stage_case(root.clone(), aggregate.structural_type, selected)?;
                        for (path, scalar) in &aggregate.fields {
                            let [Path::Field(field)] = path.as_slice() else {
                                return Err(malformed());
                            };
                            self.stage_scalar_endpoint(
                                &PrimitiveStorageEndpoint::ScalarField(
                                    StructuralScalarRuntimeField::case_payload(
                                        root.clone(),
                                        *field,
                                    ),
                                ),
                                *scalar,
                            )?;
                        }
                        return Ok(());
                    }
                    let plan = ScalarAggregatePlan::new(
                        execution.structural_declaration(aggregate.structural_type)?,
                        |identity| execution.structural_types().get(&identity),
                    )?;
                    let mut fields = aggregate.fields.iter();
                    plan.storage_parts(|path, part| {
                        let endpoint = match part {
                            ScalarAggregatePart::Array {
                                structural_type,
                                count,
                            } => {
                                let elements = fields
                                    .by_ref()
                                    .take(count)
                                    .map(|(_, value)| *value)
                                    .collect::<Vec<_>>();
                                if elements.len() != count {
                                    return Err(malformed());
                                }
                                self.stage_array(
                                    execution,
                                    structural_type,
                                    &elements,
                                    &destination(path)?,
                                )?;
                                return Ok(());
                            }
                            ScalarAggregatePart::Primitive(_) => {
                                PrimitiveStorageEndpoint::Scalar(destination(path)?)
                            }
                            ScalarAggregatePart::Field(_) => {
                                let Some((Path::Field(field), parents)) = path.split_last() else {
                                    return Err(malformed());
                                };
                                PrimitiveStorageEndpoint::ScalarField(
                                    StructuralScalarRuntimeField::member(
                                        destination(parents)?,
                                        *field,
                                    ),
                                )
                            }
                        };
                        let (_, scalar) = fields.next().ok_or_else(malformed)?;
                        self.stage_scalar_endpoint(&endpoint, *scalar)
                    })?;
                    if fields.next().is_some() {
                        return Err(malformed());
                    }
                    Ok(())
                }
            }
        }

        pub(crate) enum ConstructorValue<'execution> {
            Access(TerminalStructuralValue),
            PackedScalars {
                structural_type: StructuralTypeId,
                elements: &'execution [crate::storage::scalar_storage::ScalarCell],
                qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
                projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
            },
        }

        impl<'execution> ConstructorValue<'execution> {
            pub(crate) fn resolve(
                execution: &'execution TerminalExecution,
                argument: &terminal_psi::StructuralArgument,
            ) -> Result<Self, TerminalInterpretError> {
                if let Some(array) = execution.scalar_array_values.get(&argument.place) {
                    if execution.structural_values.contains_key(&argument.place) {
                        return Err(malformed());
                    }
                    let types = execution.structural_types();
                    let mut structural_type = array.structural_type;
                    let mut elements = array.cells();
                    for segment in &argument.path {
                        let (element, index, length) =
                            fixed_index_step(&types, structural_type, segment)?;
                        let length = usize::try_from(length).map_err(|_| malformed())?;
                        if elements.len() % length != 0 {
                            return Err(malformed());
                        }
                        let count = elements.len() / length;
                        let start = usize::try_from(index)
                            .map_err(|_| malformed())?
                            .checked_mul(count)
                            .ok_or_else(malformed)?;
                        elements = elements
                            .get(start..start.checked_add(count).ok_or_else(malformed)?)
                            .ok_or_else(malformed)?;
                        structural_type = element;
                    }
                    let (qualifications, projected_qualifications) =
                        terminal_psi::StructuralPathQualification::select(
                            &array.qualifications,
                            &[],
                            &argument.path,
                        );
                    return Ok(Self::PackedScalars {
                        structural_type,
                        elements,
                        qualifications,
                        projected_qualifications,
                    });
                }
                resolve_single_structural_argument(
                    &execution.structural_types(),
                    &execution.structural_values,
                    argument,
                )
                .map(Self::Access)
            }

            pub(crate) fn structural_type(&self) -> StructuralTypeId {
                match self {
                    Self::Access(value) => value.structural_type,
                    Self::PackedScalars {
                        structural_type, ..
                    } => *structural_type,
                }
            }

            pub(crate) fn replacement_writes(
                &self,
                execution: &TerminalExecution,
                source: &terminal_psi::StructuralArgument,
                destination: semantic_vocabulary::PlaceId,
                destination_path: &[StructuralPathSegment],
            ) -> Result<
                Option<crate::storage::primitive_storage::PrimitiveStorageWrites>,
                TerminalInterpretError,
            > {
                if terminal_semantics::scalar_array_leaf_shape(
                    execution.structural_types().values(),
                    self.structural_type(),
                )
                .is_some_and(|(_, count)| count == 0)
                {
                    return Ok(None);
                }
                let mut writes =
                    crate::storage::primitive_storage::PrimitiveStorageWrites::default();
                let supported = self.visit_scalar_leaves(
                    execution,
                    source,
                    |selectors, scalar_type, value| {
                        let mut path = destination_path.to_vec();
                        path.extend_from_slice(selectors);
                        let (storage, actual, _) =
                            execution.primitive_access(destination, true, &path)?;
                        if actual != scalar_type {
                            return Err(malformed());
                        }
                        writes.stage(execution, destination, storage, scalar_type, value)
                    },
                )?;
                Ok(supported.map(|_| writes))
            }

            pub(crate) fn visit_scalar_leaves(
                &self,
                execution: &TerminalExecution,
                source: &terminal_psi::StructuralArgument,
                mut visit: impl FnMut(
                    &[StructuralPathSegment],
                    semantic_vocabulary::ScalarType,
                    TerminalScalarValue,
                ) -> Result<(), TerminalInterpretError>,
            ) -> Result<Option<semantic_vocabulary::ScalarType>, TerminalInterpretError>
            {
                let types = execution.structural_types();
                let root = self.structural_type();
                let (scalar_type, count) = match types.declaration(&root)?.shape {
                    StructuralTypeShape::PrimitiveScalar(scalar_type) => (scalar_type, 1),
                    StructuralTypeShape::FixedArray { .. } => {
                        let Some(shape) =
                            terminal_semantics::scalar_array_leaf_shape(types.values(), root)
                        else {
                            return Ok(None);
                        };
                        shape
                    }
                    _ => return Ok(None),
                };
                if let Self::PackedScalars { elements, .. } = self
                    && u64::try_from(elements.len()).ok() != Some(count)
                {
                    return Err(malformed());
                }
                let mut dimensions = Vec::new();
                let mut current = root;
                while let StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } = types.declaration(&current)?.shape
                {
                    dimensions.push(length);
                    current = element;
                }
                let mut selectors = vec![0; dimensions.len()];
                for leaf_ordinal in 0..count {
                    let mut remaining = leaf_ordinal;
                    for (selector, length) in selectors.iter_mut().zip(&dimensions).rev() {
                        *selector = remaining.checked_rem(*length).ok_or_else(malformed)?;
                        remaining = remaining.checked_div(*length).ok_or_else(malformed)?;
                    }
                    let path = selectors
                        .iter()
                        .copied()
                        .map(StructuralPathSegment::FixedIndex)
                        .collect::<Vec<_>>();
                    let value = match self {
                        Self::PackedScalars { elements, .. } => elements
                            .get(usize::try_from(leaf_ordinal).map_err(|_| malformed())?)
                            .ok_or_else(malformed)?
                            .read()?,
                        Self::Access(value) => {
                            execution.read_primitive_argument(source, value, &path, scalar_type)?
                        }
                    };
                    if value.scalar_type() != scalar_type || !terminal_scalar_belongs_to_type(value)
                    {
                        return Err(malformed());
                    }
                    visit(&path, scalar_type, value)?;
                }
                Ok(Some(scalar_type))
            }

            pub(crate) fn memberships(
                &self,
            ) -> (
                &[semantic_vocabulary::StructuralDomainId],
                &[terminal_psi::StructuralPathQualification],
            ) {
                match self {
                    Self::Access(value) => (&value.qualifications, &value.projected_qualifications),
                    Self::PackedScalars {
                        qualifications,
                        projected_qualifications,
                        ..
                    } => (qualifications, projected_qualifications),
                }
            }

            pub(crate) fn append_memberships(
                &self,
                prefix: &[StructuralPathSegment],
                destination: &mut Vec<terminal_psi::StructuralPathQualification>,
            ) {
                let (whole, projected) = self.memberships();
                terminal_psi::StructuralPathQualification::append_under(
                    whole,
                    projected,
                    prefix,
                    destination,
                );
            }
        }

        pub(crate) struct ConstructorStorage {
    original_roots: RuntimeRows<
        StructuralRuntimePlace,
        crate::boundary::effects::original_storage::OriginalStorageAllocation,
    >,
    completed_profiles: RuntimeRows<
        StructuralRuntimePlace,
        crate::storage::provider_storage::profiles::CompletedResourceProfile,
    >,
    provider_descriptors: RuntimeRows<
        StructuralRuntimePlace,
        crate::storage::provider_storage::ProviderStorageBinding,
    >,
    activation_descriptors: RuntimeRows<
        StructuralRuntimePlace,
        crate::storage::byte_sequences::binding::ByteSequenceBinding,
    >,
    host_atoms: RuntimeRows<
        StructuralRuntimePlace,
        crate::storage::structural_storage::HostStructuralPayload,
    >,
    scalars: crate::storage::scalar_storage::ScalarStorageRows<StructuralScalarRuntimeField>,
    bytes: RuntimeRows<
        StructuralByteSequenceRuntimeField,
        crate::storage::byte_sequences::binding::ByteSequenceBinding,
    >,
    views: RuntimeRows<StructuralRuntimePlace, crate::storage::element_views::ElementView>,
    byte_descriptors: RuntimeRows<
        StructuralRuntimePlace,
        crate::storage::byte_sequences::binding::ByteSequenceBinding,
    >,
    references: RuntimeRows<StructuralRuntimePlace, crate::storage::reference::ReferenceReferent>,
    primitives: crate::storage::scalar_storage::ScalarStorageRows<StructuralRuntimePlace>,
    byte_arrays: RuntimeRows<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::byte_arrays::storage::StructuralByteArrayStorage,
    >,
    empty_arrays: crate::storage::scalar_array::empty_storage::EmptyArrayStorage,
    owner_writes: Vec<(
        crate::storage::byte_sequences::binding::ByteOwnerHome,
        ByteSequenceView,
    )>,
    cases: RuntimeRows<
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
    >,
}

        pub(crate) struct DetachedConstructorStorage(ConstructorStorage);

        struct ConstructorResidentWrites {
    scalars: crate::storage::scalar_storage::ResidentReplacementWrites<StructuralScalarRuntimeField>,
    primitives: crate::storage::scalar_storage::ResidentReplacementWrites<StructuralRuntimePlace>,
    byte_arrays: Vec<(
        StructuralRuntimePlace,
        crate::boundary::structural_inputs::byte_arrays::storage::PreparedStructuralByteArrayReplacement,
    )>,
}

        impl ConstructorResidentWrites {
            fn commit(
                self,
                storage: &mut ConstructorStorage,
            ) -> Result<(), TerminalInterpretError> {
                self.scalars.commit(&mut storage.scalars)?;
                self.primitives.commit(&mut storage.primitives)?;
                for (home, replacement) in self.byte_arrays {
                    *storage.byte_arrays.get_mut(&home).ok_or_else(malformed)? =
                        replacement.commit()?;
                }
                Ok(())
            }
        }

        pub(crate) struct PreparedConstructorReplacement {
            retirement: crate::storage::scalar_array::empty_storage::EmptyArrayRetirement,
            storage: ConstructorStorage,
            residents: ConstructorResidentWrites,
        }

        impl PreparedConstructorReplacement {
            pub(crate) fn publish(
                mut self,
                execution: &mut TerminalExecution<'_>,
            ) -> Result<(), TerminalInterpretError> {
                self.residents.commit(&mut self.storage)?;
                execution.prune_subtree(self.retirement)?;
                self.storage.publish(execution)
            }
        }

        impl DetachedConstructorStorage {
            pub(crate) fn install(
                self,
                execution: &mut TerminalExecution<'_>,
            ) -> Result<(), TerminalInterpretError> {
                self.0.publish_cells(execution)
            }
        }

        impl ConstructorStorage {
            pub(crate) fn retain_storage(
                &self,
                retained: &mut crate::storage::structural_storage::RetainedStorage,
            ) {
                for reference in self.references.values() {
                    retained.reference(reference);
                }
                for view in self.views.values() {
                    view.retain_storage(retained);
                }
                for binding in self
                    .bytes
                    .values()
                    .chain(self.byte_descriptors.values())
                    .chain(self.activation_descriptors.values())
                {
                    retained.byte_binding(binding);
                }
            }

            pub(crate) fn measure_result_custody<'storage>(
                &'storage self,
                execution: &'storage TerminalExecution<'_>,
                custody: &mut crate::boundary::result_custody::ResultCustody<'storage>,
            ) -> Result<(), TerminalInterpretError> {
                for count in [
                    self.original_roots.len(),
                    self.completed_profiles.len(),
                    self.provider_descriptors.len(),
                    self.activation_descriptors.len(),
                    self.host_atoms.len(),
                    self.scalars.len(),
                    self.bytes.len(),
                    self.views.len(),
                    self.byte_descriptors.len(),
                    self.references.len(),
                    self.primitives.len(),
                    self.byte_arrays.len(),
                    self.empty_arrays.rows.len(),
                    self.cases.len(),
                ] {
                    custody.add_cells(count)?;
                }
                for binding in self.provider_descriptors.values() {
                    custody.provider(execution, *binding)?;
                }
                for binding in self.activation_descriptors.values() {
                    custody.binding(binding)?;
                }
                for binding in self.bytes.values() {
                    custody.binding(binding)?;
                }
                for binding in self.byte_descriptors.values() {
                    custody.binding(binding)?;
                }
                for bytes in self.byte_arrays.values() {
                    bytes.measure_result_custody(custody)?;
                }
                for (_, bytes) in &self.owner_writes {
                    custody.bytes(bytes)?;
                }
                for view in self.views.values() {
                    custody.element_view(view)?;
                }
                for reference in self.references.values() {
                    custody.reference(reference)?;
                }
                Ok(())
            }

            pub(crate) fn new(
                account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
            ) -> Self {
                Self {
                    original_roots: RuntimeRows::new(account),
                    completed_profiles: RuntimeRows::new(account),
                    provider_descriptors: RuntimeRows::new(account),
                    activation_descriptors: RuntimeRows::new(account),
                    host_atoms: RuntimeRows::new(account),
                    scalars: crate::storage::scalar_storage::ScalarStorageRows::new(account),
                    bytes: RuntimeRows::new(account),
                    views: RuntimeRows::new(account),
                    byte_descriptors: RuntimeRows::new(account),
                    references: RuntimeRows::new(account),
                    primitives: crate::storage::scalar_storage::ScalarStorageRows::new(account),
                    byte_arrays: RuntimeRows::new(account),
                    empty_arrays:
                        crate::storage::scalar_array::empty_storage::EmptyArrayStorage::new(account),
                    owner_writes: Vec::new(),
                    cases: RuntimeRows::new(account),
                }
            }

            pub(crate) fn stage_resident_image(
                &mut self,
                execution: &TerminalExecution<'_>,
                retained: &super::owned_value::StagedOwnedValue,
                destination: &StructuralRuntimePlace,
                mut decoded: ConstructorStorage,
            ) -> Result<(), TerminalInterpretError> {
                let original = &retained.storage;
                if !original.references.is_empty() || !original.owner_writes.is_empty() {
                    return Err(
                        crate::boundary::errors::TerminalInterpretError::UnsupportedSemanticVariant(
                            "resident extraction requires live forwarding of carried loans",
                        ),
                    );
                }
                let source = StructuralRuntimePlace::from(&retained.value);
                let relocate = |home: &StructuralRuntimePlace| {
                    if home.opaque_identity != source.opaque_identity
                        || !home.path.starts_with(&source.path)
                    {
                        return Err(malformed());
                    }
                    let mut path = destination.path.clone();
                    path.extend_from_slice(&home.path[source.path.len()..]);
                    Ok(StructuralRuntimePlace {
                        opaque_identity: destination.opaque_identity,
                        path,
                    })
                };
                for (home, root) in &original.original_roots {
                    self.original_roots.insert(relocate(home)?, *root)?;
                }
                for (home, binding) in &original.provider_descriptors {
                    if !execution.provider_storage.binding_is_live(*binding) {
                        return Err(
                    crate::boundary::errors::TerminalInterpretError::ContentTransformationMismatch(
                        "retained value lost its provider allocation",
                    ),
                );
                    }
                    self.provider_descriptors
                        .insert(relocate(home)?, *binding)?;
                }
                for (home, binding) in &original.activation_descriptors {
                    let target = relocate(home)?;
                    let mut descriptor = binding.descriptor_home().ok_or_else(malformed)?.clone();
                    descriptor.opaque_identity = target.opaque_identity;
                    descriptor.path = target.path.clone();
                    self.activation_descriptors
                        .insert(target, binding.clone().with_descriptor_home(descriptor))?;
                }
                for (home, profile) in &original.completed_profiles {
                    self.completed_profiles
                        .insert(relocate(home)?, profile.clone())?;
                }
                for (home, payload) in &original.host_atoms {
                    self.host_atoms.insert(relocate(home)?, *payload)?;
                }
                for (home, view) in &original.views {
                    let target = relocate(home)?;
                    self.views
                        .insert(target.clone(), view.relocated_descriptor(home, &target)?)?;
                }
                for (home, case) in &original.cases {
                    let destination = relocate(home)?;
                    if decoded.cases.get(&destination).is_some_and(|decoded| {
                        decoded.structural_type != case.structural_type || decoded.case != case.case
                    }) {
                        return Err(TerminalInterpretError::ContentTransformationMismatch(
                            "resident case bytes disagree with the retained payload custody",
                        ));
                    }
                    self.cases.insert(destination, *case)?;
                }
                self.scalars.append(&mut decoded.scalars)?;
                self.primitives.append(&mut decoded.primitives)?;
                self.bytes.append(&mut decoded.bytes)?;
                self.byte_arrays.append(&mut decoded.byte_arrays)?;
                self.empty_arrays
                    .rows
                    .append(&mut decoded.empty_arrays.rows)?;
                self.cases.append(&mut decoded.cases)?;
                Ok(())
            }

            pub(crate) fn retains_references(&self) -> bool {
                !self.references.is_empty()
            }

            pub(crate) fn resident_field_has_bindings(
                &self,
                root: &StructuralRuntimePlace,
                field: terminal_psi::TerminalIdentityIndex,
            ) -> bool {
                let mut path = root.path.clone();
                path.push(terminal_psi::StructuralPathSegment::Field(field));
                let overlaps = |home: &StructuralRuntimePlace| {
                    home.opaque_identity == root.opaque_identity
                        && terminal_semantics::structural_paths_may_overlap(&home.path, &path)
                };
                self.provider_descriptors.keys().any(overlaps)
                    || self.activation_descriptors.keys().any(overlaps)
                    || self.completed_profiles.keys().any(overlaps)
                    || self.host_atoms.keys().any(overlaps)
                    || self.views.keys().any(overlaps)
                    || self.references.keys().any(overlaps)
                    || !self.owner_writes.is_empty()
            }

            pub(crate) fn into_detached(
                self,
            ) -> Result<DetachedConstructorStorage, TerminalInterpretError> {
                if !self.owner_writes.is_empty() {
                    return Err(
                crate::boundary::errors::TerminalInterpretError::ContentTransformationMismatch(
                    "detached value unexpectedly writes through an existing owner",
                ),
            );
                }
                Ok(DetachedConstructorStorage(self))
            }

            pub(crate) fn take_provider_descriptor(
                &mut self,
                home: &StructuralRuntimePlace,
                expected: crate::storage::provider_storage::ProviderStorageBinding,
            ) -> Result<(), TerminalInterpretError> {
                if self.provider_descriptors.get(home).copied() != Some(expected) {
                    return Err(malformed());
                }
                self.provider_descriptors.remove(home);
                Ok(())
            }
            pub(crate) fn validate_projected_memberships(
                &self,
                execution: &TerminalExecution,
                root: &TerminalStructuralValue,
                required: &[terminal_psi::StructuralPathQualification],
            ) -> Result<(), TerminalInterpretError> {
                for row in required {
                    let active = crate::boundary::structural_inputs::case_membership::case_projection_with_membership_is_active(
                &execution.structural_types(), root.structural_type, &row.path,
                &|prefix| self.case_at_projection(execution, root, prefix),
            )?;
                    if active && !root.projected_qualifications.contains(row) {
                        return Err(malformed());
                    }
                }
                Ok(())
            }

            pub(crate) fn case_at_projection(
                &self,
                execution: &TerminalExecution,
                root: &TerminalStructuralValue,
                path: &[StructuralPathSegment],
            ) -> Option<semantic_vocabulary::StructuralCaseId> {
                crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                    &execution.structural_types(),
                    root,
                    self.cases.as_map(),
                    path,
                )
            }

            pub(crate) fn stage_byte_field(
                &mut self,
                destination: StructuralByteSequenceRuntimeField,
                bytes: crate::storage::byte_sequences::binding::ByteSequenceBinding,
            ) -> Result<(), TerminalInterpretError> {
                self.bytes.insert(destination, bytes)?;
                Ok(())
            }

            pub(crate) fn stage_byte_descriptor(
                &mut self,
                destination: StructuralRuntimePlace,
                bytes: crate::storage::byte_sequences::binding::ByteSequenceBinding,
            ) -> Result<(), TerminalInterpretError> {
                if self.byte_descriptors.insert(destination, bytes)?.is_some() {
                    return Err(malformed());
                }
                Ok(())
            }

            pub(crate) fn stage_reference(
                &mut self,
                destination: StructuralRuntimePlace,
                referent: crate::storage::reference::ReferenceReferent,
            ) -> Result<(), TerminalInterpretError> {
                if self.references.insert(destination, referent)?.is_some() {
                    return Err(malformed());
                }
                Ok(())
            }

            pub(crate) fn stage_byte_array(
                &mut self,
                destination: StructuralRuntimePlace,
                structural_type: StructuralTypeId,
                bytes: crate::storage::byte_sequences::storage::ByteSequenceStorage,
            ) -> Result<(), TerminalInterpretError> {
                self.primitives.retain(|home, _| {
                    home.opaque_identity != destination.opaque_identity
                        || !home.path.starts_with(&destination.path)
                });
                let account = bytes.account().cloned();
                self.byte_arrays.insert(
            destination,
            crate::boundary::structural_inputs::byte_arrays::storage::StructuralByteArrayStorage::new(
                structural_type,
                ByteSequenceView::new(bytes),
                account.as_ref(),
            ),
        )?;
                Ok(())
            }
            pub(crate) fn retain_replacement_subtree(
                &mut self,
                execution: &TerminalExecution,
                root: &StructuralRuntimePlace,
            ) -> Result<(), TerminalInterpretError> {
                self.stage_subtree(execution, root, root, true)
            }

            pub(crate) fn retain_byte_field(
                &mut self,
                execution: &TerminalExecution,
                parent: &StructuralRuntimePlace,
                field: semantic_vocabulary::StructuralFieldId,
                namespace: crate::boundary::values::StructuralFieldNamespace,
            ) -> Result<(), TerminalInterpretError> {
                let cell = StructuralByteSequenceRuntimeField {
                    parent: parent.clone(),
                    namespace,
                    field,
                };
                let bytes = execution.structural_byte_sequence_fields.require(&cell)?;
                self.bytes.insert(cell, bytes.clone())?;
                Ok(())
            }

            pub(crate) fn stage_case(
                &mut self,
                destination: StructuralRuntimePlace,
                structural_type: StructuralTypeId,
                case: semantic_vocabulary::StructuralCaseId,
            ) -> Result<(), TerminalInterpretError> {
                self.cases.insert(
                    destination,
                    crate::boundary::structural_inputs::case_membership::StructuralCaseContents {
                        structural_type,
                        case,
                    },
                )?;
                Ok(())
            }

            pub(crate) fn stage_scalar_endpoint(
                &mut self,
                destination: &crate::storage::primitive_storage::PrimitiveStorageEndpoint,
                value: TerminalScalarValue,
            ) -> Result<(), TerminalInterpretError> {
                match destination {
                    crate::storage::primitive_storage::PrimitiveStorageEndpoint::Scalar(home) => {
                        self.primitives.insert(home.clone(), value)?;
                    }
                    crate::storage::primitive_storage::PrimitiveStorageEndpoint::ScalarField(
                        field,
                    ) => {
                        self.scalars.insert(field.clone(), value)?;
                    }
                    _ => return Err(malformed()),
                }
                Ok(())
            }
            pub(crate) fn stage(
                &mut self,
                execution: &TerminalExecution,
                argument: &terminal_psi::StructuralArgument,
                child: &ConstructorValue<'_>,
                destination: &StructuralRuntimePlace,
                transfers_custody: bool,
            ) -> Result<(), TerminalInterpretError> {
                match child {
                    ConstructorValue::Access(value) => self.stage_access(
                        execution,
                        argument,
                        value,
                        destination,
                        transfers_custody,
                    ),
                    ConstructorValue::PackedScalars {
                        structural_type,
                        elements,
                        ..
                    } => {
                        if transfers_custody {
                            return Err(malformed());
                        }
                        let values = elements
                            .iter()
                            .map(crate::storage::scalar_storage::ScalarCell::read)
                            .collect::<Result<Vec<_>, _>>()?;
                        self.stage_array(execution, *structural_type, &values, destination)
                    }
                }
            }

            pub(crate) fn stage_array(
                &mut self,
                execution: &TerminalExecution,
                structural_type: StructuralTypeId,
                elements: &[TerminalScalarValue],
                destination: &StructuralRuntimePlace,
            ) -> Result<(), TerminalInterpretError> {
                let declaration = execution.structural_declaration(structural_type)?;
                match declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                        let [value] = elements else {
                            return Err(malformed());
                        };
                        if value.scalar_type() != scalar_type
                            || !terminal_scalar_belongs_to_type(*value)
                        {
                            return Err(malformed());
                        }
                        self.primitives.insert(destination.clone(), *value)?;
                    }
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } => {
                        let (scalar_type, leaf_count) =
                            terminal_semantics::scalar_array_leaf_shape(
                                execution.structural_types().values(),
                                structural_type,
                            )
                            .ok_or_else(malformed)?;
                        if u64::try_from(elements.len()).ok() != Some(leaf_count) {
                            return Err(malformed());
                        }
                        if leaf_count == 0 {
                            self.empty_arrays.establish(
                                &execution.structural_types(),
                                destination.clone(),
                                structural_type,
                            )?;
                            return Ok(());
                        }
                        if matches!(
                            execution
                                .structural_types()
                                .get(&element)
                                .map(|row| &row.shape),
                            Some(StructuralTypeShape::PrimitiveScalar(_))
                        ) && matches!(scalar_type, semantic_vocabulary::ScalarType::Integer(carrier)
                        if carrier.bits() == 8 && !carrier.is_address()
                            && carrier.sign() == semantic_vocabulary::IntegerSign::Unsigned)
                        {
                            let values = elements.iter().map(|value| {
                                if value.scalar_type() != scalar_type {
                                    return Err(malformed());
                                }
                                let TerminalScalarValue::Integer {
                                    value: semantic_vocabulary::IntegerValue::Unsigned(byte),
                                    ..
                                } = value
                                else {
                                    return Err(malformed());
                                };
                                u8::try_from(*byte).map_err(|_| malformed())
                            });
                            let bytes = ByteSequenceStorage::from_values(
                                elements.len(),
                                values,
                                execution.account.as_ref(),
                            )?;
                            self.byte_arrays
                        .insert(destination.clone(), crate::boundary::structural_inputs::byte_arrays::storage::StructuralByteArrayStorage::new(structural_type, ByteSequenceView::new(bytes), execution.account.as_ref()))?;
                        } else {
                            let element_count = match execution
                                .structural_types()
                                .get(&element)
                                .map(|row| &row.shape)
                            {
                                Some(StructuralTypeShape::PrimitiveScalar(_)) => 1,
                                _ => {
                                    terminal_semantics::scalar_array_leaf_shape(
                                        execution.structural_types().values(),
                                        element,
                                    )
                                    .ok_or_else(malformed)?
                                    .1
                                }
                            };
                            for element_index in 0..length {
                                let start = usize::try_from(
                                    element_index
                                        .checked_mul(element_count)
                                        .ok_or_else(malformed)?,
                                )
                                .map_err(|_| malformed())?;
                                let end = start
                                    .checked_add(
                                        usize::try_from(element_count).map_err(|_| malformed())?,
                                    )
                                    .ok_or_else(malformed)?;
                                let mut home = destination.clone();
                                home.path
                                    .push(StructuralPathSegment::FixedIndex(element_index));
                                self.stage_array(
                                    execution,
                                    element,
                                    elements.get(start..end).ok_or_else(malformed)?,
                                    &home,
                                )?;
                            }
                        }
                    }
                    _ => return Err(malformed()),
                }
                Ok(())
            }

            fn stage_access(
                &mut self,
                execution: &TerminalExecution,
                argument: &terminal_psi::StructuralArgument,
                child: &TerminalStructuralValue,
                destination: &StructuralRuntimePlace,
                transfers_custody: bool,
            ) -> Result<(), TerminalInterpretError> {
                if execution
                    .structural_types()
                    .get(&child.structural_type)
                    .is_some_and(|row| row.owns_linear_claim && row.shape.is_borrowed_byte_view())
                {
                    let binding = execution.byte_descriptor_binding(argument, child)?;
                    if !transfers_custody
                        || !execution.is_live_activation_descriptor(&binding, child)
                    {
                        return Err(malformed());
                    }
                    let mut descriptor = child.clone();
                    descriptor.opaque_identity = destination.opaque_identity;
                    descriptor.path = destination.path.clone();
                    self.activation_descriptors.insert(
                        destination.clone(),
                        binding.with_descriptor_home(descriptor),
                    )?;
                    return Ok(());
                }

                if let Some(declaration) = execution.structural_types().get(&child.structural_type)
                    && declaration.shape.is_borrowed_byte_carrier()
                {
                    let shared = declaration.shape.is_shared_readable_byte_view();
                    if !transfers_custody && !shared {
                        return Err(malformed());
                    }
                    let binding = if shared {
                        execution.shared_byte_descriptor_binding(argument, child)?
                    } else {
                        execution.byte_descriptor_binding(argument, child)?
                    };
                    let mut descriptor = child.clone();
                    descriptor.opaque_identity = destination.opaque_identity;
                    descriptor.path = destination.path.clone();
                    self.byte_descriptors.insert(
                        destination.clone(),
                        binding.with_descriptor_home(descriptor),
                    )?;
                    return Ok(());
                }

                if execution
                    .structural_types()
                    .get(&child.structural_type)
                    .is_some_and(|row| matches!(row.shape, StructuralTypeShape::ElementView { .. }))
                {
                    let view = execution.element_view_argument(argument, child)?;
                    // Indexed descriptor storage is relocated once by the
                    // shared subtree walk below. A root/frame binding lives in
                    // another table, so capture that binding here instead.
                    if !execution
                        .structural_element_view_fields
                        .contains_key(&StructuralRuntimePlace::from(child))
                    {
                        // The root table already authenticated this exact
                        // descriptor. Retain that home before the shared move,
                        // including legacy root bindings with no inline home.
                        let view = view.clone().with_descriptor_home(child.clone());
                        self.views.insert(
                            destination.clone(),
                            view.relocated_descriptor(
                                &StructuralRuntimePlace::from(child),
                                destination,
                            )?,
                        )?;
                    }
                }
                if execution
                    .structural_types()
                    .get(&child.structural_type)
                    .is_some_and(|row| row.shape.is_bounded_owned_byte_sequence())
                {
                    let source = terminal_psi::StructuralArgument {
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                        ..argument.clone()
                    };
                    let (_, bytes) = execution.bounded_owner_snapshot(&source)?;
                    match execution.captured_byte_owner_binding_at(destination, child.structural_type) {
                Some(
                    crate::storage::byte_sequences::binding::ByteSequenceBinding::MutableField {
                        field,
                        ..
                    },
                ) => {
                    self.bytes.insert(
                        field.clone(),
                        crate::storage::byte_sequences::binding::ByteSequenceBinding::Immutable(
                            bytes.owned_snapshot(),
                        ),
                    )?;
                }
                Some(
                    crate::storage::byte_sequences::binding::ByteSequenceBinding::MutableOwner {
                        home,
                        ..
                    },
                ) => {
                    execution.byte_owner_bytes(home)?;
                    self.owner_writes
                        .push((home.clone(), bytes.owned_snapshot()));
                }
                Some(_) => return Err(malformed()),
                None => {
                    self.byte_arrays
                        .insert(destination.clone(), crate::boundary::structural_inputs::byte_arrays::storage::StructuralByteArrayStorage::new(child.structural_type, bytes.owned_snapshot(), execution.account.as_ref()))?;
                }
            }
                }
                self.stage_subtree(
                    execution,
                    &StructuralRuntimePlace::from(child),
                    destination,
                    transfers_custody,
                )
            }

            pub(crate) fn stage_subtree(
                &mut self,
                execution: &TerminalExecution,
                source: &StructuralRuntimePlace,
                destination: &StructuralRuntimePlace,
                transfers_custody: bool,
            ) -> Result<(), TerminalInterpretError> {
                let relocate = |home: &StructuralRuntimePlace| {
                    if home.opaque_identity != source.opaque_identity
                        || !home.path.starts_with(&source.path)
                    {
                        return None;
                    }
                    let mut path = destination.path.clone();
                    path.extend_from_slice(&home.path[source.path.len()..]);
                    Some(StructuralRuntimePlace {
                        opaque_identity: destination.opaque_identity,
                        path,
                    })
                };
                if transfers_custody {
                    for (home, original) in crate::storage::structural_storage::cells(
                        &execution.original_storage_roots,
                        source,
                    ) {
                        if let Some(target) = relocate(home) {
                            self.original_roots.insert(target, *original)?;
                        }
                    }
                    for (home, profile) in crate::storage::structural_storage::cells(
                        &execution.provider_storage.completed_profiles,
                        source,
                    ) {
                        if let Some(target) = relocate(home) {
                            self.completed_profiles.insert(target, profile.clone())?;
                        }
                    }
                }
                for (home, binding) in crate::storage::structural_storage::cells(
                    &execution.provider_storage.descriptors,
                    source,
                ) {
                    if let Some(target) = relocate(home) {
                        if !transfers_custody
                            || !execution.is_live_provider_descriptor(home, binding)
                        {
                            return Err(
                        crate::boundary::errors::TerminalInterpretError::ContentTransformationMismatch(
                            "provider storage transport requires owned custody and live backing",
                        ),
                    );
                        }
                        self.provider_descriptors.insert(target, *binding)?;
                    }
                }
                for (home, binding) in crate::storage::structural_storage::cells(
                    &execution.activation_storage.claim_descriptors,
                    source,
                ) {
                    if let Some(target) = relocate(home) {
                        let old = binding.descriptor_home().ok_or_else(malformed)?;
                        if !transfers_custody
                            || !execution.is_live_activation_descriptor(binding, old)
                        {
                            return Err(malformed());
                        }
                        let mut descriptor = old.clone();
                        descriptor.opaque_identity = target.opaque_identity;
                        descriptor.path = target.path.clone();
                        self.activation_descriptors
                            .insert(target, binding.clone().with_descriptor_home(descriptor))?;
                    }
                }
                for (home, binding) in crate::storage::structural_storage::cells(
                    &execution.structural_byte_descriptors,
                    source,
                ) {
                    if let Some(target) = relocate(home) {
                        if !transfers_custody
                    && !matches!(
                        binding,
                        crate::storage::byte_sequences::binding::ByteSequenceBinding::Immutable(_)
                            | crate::storage::byte_sequences::binding::ByteSequenceBinding::Representation(
                                _
                            )
                    )
                {
                    return Err(malformed());
                }
                        if !transfers_custody && target.path.is_empty() {
                            continue;
                        }
                        self.byte_descriptors
                            .insert(target.clone(), binding.relocated_descriptor(home, &target)?)?;
                    }
                }
                for (home, descriptor) in crate::storage::structural_storage::cells(
                    &execution.structural_element_view_fields,
                    source,
                ) {
                    if let Some(target) = relocate(home) {
                        self.views.insert(
                            target.clone(),
                            descriptor.relocated_descriptor(home, &target)?,
                        )?;
                    }
                }
                for (home, referent) in crate::storage::structural_storage::cells(
                    &execution.reference_referents,
                    source,
                ) {
                    if let Some(target) = relocate(home) {
                        if !transfers_custody {
                            let carrier_type = execution.actual_storage_type_from_owner(home)?;
                            if !execution.structural_types().get(&carrier_type).is_some_and(
                                |declaration| {
                                    matches!(
                                        declaration.shape,
                                        StructuralTypeShape::Reference {
                                            access: terminal_psi::StructuralAccess::SharedBorrow,
                                            ..
                                        }
                                    )
                                },
                            ) {
                                return Err(malformed());
                            }
                        }
                        self.references.insert(target, referent.clone())?;
                    }
                }
                for (field, scalar) in crate::storage::structural_storage::cells(
                    &execution.structural_scalar_fields,
                    source,
                ) {
                    if let Some(parent) = relocate(&field.parent) {
                        self.scalars.insert_cell(
                            field.relocated(parent),
                            scalar.relocated(transfers_custody)?,
                        )?;
                    }
                }
                for (field, bytes) in crate::storage::structural_storage::cells(
                    &execution.structural_byte_sequence_fields,
                    source,
                ) {
                    if let Some(parent) = relocate(&field.parent) {
                        if !transfers_custody
                    && !matches!(
                        bytes,
                        crate::storage::byte_sequences::binding::ByteSequenceBinding::Immutable(_)
                            | crate::storage::byte_sequences::binding::ByteSequenceBinding::Representation(
                                _
                            )
                    )
                {
                    return Err(malformed());
                }
                        self.bytes.insert(field.relocated(parent), bytes.clone())?;
                    }
                }
                for (home, value) in crate::storage::structural_storage::cells(
                    &execution.structural_primitive_storage,
                    source,
                ) {
                    if let Some(target) = relocate(home) {
                        self.primitives
                            .insert_cell(target, value.relocated(transfers_custody)?)?;
                    }
                }
                for (home, bytes) in crate::storage::structural_storage::cells(
                    &execution.structural_byte_storage,
                    source,
                ) {
                    if let Some(target) = relocate(home) {
                        self.byte_arrays
                            .insert(target, bytes.relocated(transfers_custody)?)?;
                    }
                }
                self.empty_arrays.stage_subtree(
                    &execution.structural_types(),
                    &execution.empty_array_storage,
                    &execution.structural_byte_storage,
                    source,
                    destination,
                )?;
                for (home, atom) in crate::storage::structural_storage::cells(
                    &execution.structural_host_atoms,
                    source,
                ) {
                    if let Some(target) = relocate(home) {
                        self.host_atoms.insert(target, *atom)?;
                    }
                }
                for (home, case) in
                    crate::storage::structural_storage::cells(&execution.structural_cases, source)
                {
                    if let Some(target) = relocate(home) {
                        self.cases.insert(target, *case)?;
                    }
                }
                Ok(())
            }

            pub(crate) fn preserve_replaced_residents(
                &mut self,
                execution: &TerminalExecution<'_>,
                root: &StructuralRuntimePlace,
            ) -> Result<(), TerminalInterpretError> {
                self.preserve_original_roots(execution, root)?;
                self.prepare_resident_writes(execution, root)?.commit(self)
            }

            pub(crate) fn prepare_replacement(
                mut self,
                execution: &TerminalExecution<'_>,
                root: StructuralRuntimePlace,
            ) -> Result<PreparedConstructorReplacement, TerminalInterpretError> {
                self.preserve_original_roots(execution, &root)?;
                let residents = self.prepare_resident_writes(execution, &root)?;
                Ok(PreparedConstructorReplacement {
                    retirement: execution.prepare_subtree_retirement(&root)?,
                    storage: self,
                    residents,
                })
            }

            fn preserve_original_roots(
                &mut self,
                execution: &TerminalExecution<'_>,
                root: &StructuralRuntimePlace,
            ) -> Result<(), TerminalInterpretError> {
                for (home, original) in crate::storage::structural_storage::cells(
                    &execution.original_storage_roots,
                    root,
                ) {
                    if self
                        .original_roots
                        .get(home)
                        .is_some_and(|incoming| incoming != original)
                    {
                        return Err(TerminalInterpretError::ResidentStorage(
                            crate::TerminalEffectRejection::new(
                                "replacement cannot substitute another original allocation",
                            ),
                        ));
                    }
                    self.original_roots.insert(home.clone(), *original)?;
                }
                Ok(())
            }

            fn prepare_resident_writes(
                &self,
                execution: &TerminalExecution<'_>,
                root: &StructuralRuntimePlace,
            ) -> Result<ConstructorResidentWrites, TerminalInterpretError> {
                self.validate_publication(execution)?;
                let scalars = self.scalars.prepare_resident_replacement(
                    &execution.structural_scalar_fields,
                    Some(root),
                )?;
                let primitives = self.primitives.prepare_resident_replacement(
                    &execution.structural_primitive_storage,
                    Some(root),
                )?;
                let mut byte_arrays = Vec::new();
                for (home, original) in crate::storage::structural_storage::cells(
                    &execution.structural_byte_storage,
                    root,
                ) {
                    if original.has_residents() {
                        let incoming = self.byte_arrays.get(home).ok_or_else(malformed)?;
                        if let Some(replacement) = original.prepare_replacement(incoming)? {
                            byte_arrays.push((home.clone(), replacement));
                        }
                    }
                }
                for (home, _) in &self.owner_writes {
                    execution.byte_owner_bytes(home)?;
                }
                Ok(ConstructorResidentWrites {
                    scalars,
                    primitives,
                    byte_arrays,
                })
            }

            pub(crate) fn publish(
                mut self,
                execution: &mut TerminalExecution,
            ) -> Result<(), TerminalInterpretError> {
                self.validate_publication(execution)?;
                for (home, _) in &self.owner_writes {
                    execution.byte_owner_bytes(home)?;
                }
                for (home, bytes) in std::mem::take(&mut self.owner_writes) {
                    *execution.byte_owner_bytes_mut(&home)? = bytes;
                }
                self.publish_cells(execution)
            }

            fn validate_publication(
                &self,
                execution: &TerminalExecution<'_>,
            ) -> Result<(), TerminalInterpretError> {
                execution
                    .original_storage_roots
                    .validate_append(&self.original_roots)?;
                execution
                    .provider_storage
                    .descriptors
                    .validate_append(&self.provider_descriptors)?;
                execution
                    .provider_storage
                    .completed_profiles
                    .validate_append(&self.completed_profiles)?;
                execution
                    .activation_storage
                    .claim_descriptors
                    .validate_append(&self.activation_descriptors)?;
                execution
                    .structural_scalar_fields
                    .validate_append(&self.scalars)?;
                execution
                    .structural_primitive_storage
                    .validate_append(&self.primitives)?;
                execution
                    .structural_byte_sequence_fields
                    .validate_append(&self.bytes)?;
                execution
                    .structural_element_view_fields
                    .validate_append(&self.views)?;
                execution
                    .structural_byte_descriptors
                    .validate_append(&self.byte_descriptors)?;
                execution
                    .reference_referents
                    .validate_append(&self.references)?;
                execution
                    .structural_byte_storage
                    .validate_append(&self.byte_arrays)?;
                execution
                    .empty_array_storage
                    .rows
                    .validate_append(&self.empty_arrays.rows)?;
                execution.structural_cases.validate_append(&self.cases)?;
                execution
                    .structural_host_atoms
                    .validate_append(&self.host_atoms)?;
                Ok(())
            }

            fn publish_cells(
                mut self,
                execution: &mut TerminalExecution<'_>,
            ) -> Result<(), TerminalInterpretError> {
                self.validate_publication(execution)?;
                execution
                    .original_storage_roots
                    .append(&mut self.original_roots)?;
                execution
                    .structural_scalar_fields
                    .append(&mut self.scalars)?;
                execution
                    .structural_primitive_storage
                    .append(&mut self.primitives)?;
                execution
                    .activation_storage
                    .claim_descriptors
                    .append(&mut self.activation_descriptors)?;
                execution
                    .provider_storage
                    .descriptors
                    .append(&mut self.provider_descriptors)?;
                execution
                    .provider_storage
                    .completed_profiles
                    .append(&mut self.completed_profiles)?;
                execution
                    .structural_byte_sequence_fields
                    .append(&mut self.bytes)?;
                execution
                    .structural_element_view_fields
                    .append(&mut self.views)?;
                execution
                    .structural_byte_descriptors
                    .append(&mut self.byte_descriptors)?;
                execution.reference_referents.append(&mut self.references)?;
                execution
                    .structural_byte_storage
                    .append(&mut self.byte_arrays)?;
                execution
                    .empty_array_storage
                    .rows
                    .append(&mut self.empty_arrays.rows)?;
                execution.structural_cases.append(&mut self.cases)?;
                execution
                    .structural_host_atoms
                    .append(&mut self.host_atoms)?;
                Ok(())
            }
        }
    }
    pub(crate) mod structural_array {
        use super::storage::{ConstructorStorage, ConstructorValue};
        use super::*;
        use crate::engine::execution::operation_kind;
        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::OperationKind;

        impl TerminalExecution<'_> {
            // The same array operation owns the supplied element roster. A
            // stored-length result retains those values in Terminal's existing
            // immutable sequence backing rather than relabeling inline storage.
            fn establish_immutable_elements(
                &mut self,
                result: &terminal_psi::StructuralOperationResult,
                element: StructuralTypeId,
                elements: &[terminal_psi::StructuralArgument],
            ) -> Result<crate::engine::execution::OperationFlow, TerminalInterpretError>
            {
                use crate::storage::element_views::{
                    ElementView, immutable_sequence::ImmutableStructuralSequence,
                };
                if !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || self.result_place_in_use(result.place)
                    || self.structural_declaration(element)?.owns_linear_claim
                    || !self.structural_declaration(element)?.no_code_affine_cleanup
                    || !self.structural_declaration(element)?.descriptor_cleanup
                {
                    return Err(malformed());
                }
                let mut cursor = self.local_structural_identities.cursor();
                let descriptor = TerminalStructuralValue::minted(
                    self.local_structural_identities
                        .allocate_staged(&mut cursor)?,
                    result.structural_type,
                    Vec::new(),
                );
                let mut staged = ConstructorStorage::new(self.account.as_ref());
                let mut frontier = self.live_affine_frontier.clone();
                let mut consumed = Vec::new();
                let mut claim_operands = Vec::new();
                let account = self.account.clone();
                // from_values reserves and meters the complete retained roster
                // before asking this iterator to construct any element.
                let values = ImmutableStructuralSequence::from_values(
                    elements.len(),
                    elements.iter().enumerate().map(|(index, argument)| {
                        let child = ConstructorValue::resolve(self, argument)?;
                        if argument.access != StructuralAccess::Owned
                            || argument.place == result.place
                            || child.structural_type() != element
                        {
                            return Err(malformed());
                        }
                        let transfers =
                            self.stage_constructor_operand(argument, &mut frontier, &mut consumed)?;
                        let mut value = TerminalStructuralValue::minted(
                            self.local_structural_identities
                                .allocate_staged(&mut cursor)?,
                            element,
                            Vec::new(),
                        );
                        child.append_memberships(&[], &mut value.projected_qualifications);
                        let destination = StructuralRuntimePlace::from(&value);
                        staged.stage(self, argument, &child, &destination, transfers)?;
                        claim_operands.push((
                            argument,
                            vec![StructuralPathSegment::FixedIndex(
                                u64::try_from(index).map_err(|_| malformed())?,
                            )],
                        ));
                        Ok(value)
                    }),
                    account.as_ref(),
                )?;
                staged.validate_projected_memberships(
                    self,
                    &descriptor,
                    &result.projected_qualifications,
                )?;
                let claims = self.stage_constructor_claims(
                    super::ResultClaimOrigin::Construction,
                    result,
                    &claim_operands,
                    &mut frontier,
                    &|current| staged.case_at_projection(self, &descriptor, &current.path),
                )?;
                let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
                // Reserve both published homes before any staged storage is
                // made visible. Append transfers those already funded rows.
                let mut homes = RuntimeRows::from_rows(
                    self.account.as_ref(),
                    [(result.place, descriptor.clone())],
                )?;
                let mut views = RuntimeRows::from_rows(
                    self.account.as_ref(),
                    [(
                        result.place,
                        ElementView::immutable_sequence(values, descriptor, result.place),
                    )],
                )?;
                self.structural_values.validate_append(&homes)?;
                self.element_view_values.validate_append(&views)?;
                staged.publish(self)?;
                self.element_view_values.append(&mut views)?;
                self.structural_values.append(&mut homes)?;
                retirement.commit(self)?;
                self.local_structural_identities.commit_cursor(cursor);
                self.track_affine_result(result);
                Ok(crate::engine::execution::OperationFlow::Advance)
            }

            pub(crate) fn execute_establish_structural_array(
                &mut self,
                operation: &terminal_psi::Operation,
            ) -> Result<crate::engine::execution::OperationFlow, TerminalInterpretError>
            {
                operation_kind!(operation => EstablishStructuralArray { elements });
                let result = operation.result.structural().ok_or_else(malformed)?;
                if let StructuralTypeShape::ElementView {
                    element,
                    access: None | Some(StructuralAccess::SharedBorrow),
                } = self.structural_declaration(result.structural_type)?.shape
                {
                    return self.establish_immutable_elements(result, element, elements);
                }
                let Some(StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                }) = self
                    .structural_types()
                    .get(&result.structural_type)
                    .map(|row| &row.shape)
                else {
                    return Err(malformed());
                };
                let element = *element;
                if u64::try_from(elements.len()).ok() != Some(*length)
                    || !result.qualifications.is_empty()
                    || self.live_affine_discard_at(result.place).is_some()
                    || crate::engine::custody::place_is_claimed(&self.live_claims, result.place)
                {
                    return Err(malformed());
                }
                let mut cursor = self.local_structural_identities.cursor();
                let mut value = TerminalStructuralValue::minted(
                    self.local_structural_identities
                        .allocate_staged(&mut cursor)?,
                    result.structural_type,
                    Vec::new(),
                );
                let mut staged = ConstructorStorage::new(self.account.as_ref());
                let mut frontier = self.live_affine_frontier.clone();
                let mut consumed = Vec::new();
                let mut claim_operands = Vec::new();
                for (index, argument) in elements.iter().enumerate() {
                    let child = ConstructorValue::resolve(self, argument)?;
                    if argument.access != StructuralAccess::Owned
                        || argument.place == result.place
                        || child.structural_type() != element
                    {
                        return Err(malformed());
                    }
                    let transfers_custody =
                        self.stage_constructor_operand(argument, &mut frontier, &mut consumed)?;
                    let destination = StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: vec![StructuralPathSegment::FixedIndex(
                            u64::try_from(index).map_err(|_| malformed())?,
                        )],
                    };
                    child
                        .append_memberships(&destination.path, &mut value.projected_qualifications);
                    claim_operands.push((argument, destination.path.clone()));
                    staged.stage(self, argument, &child, &destination, transfers_custody)?;
                }
                value.projected_qualifications.sort();
                value.projected_qualifications.dedup();
                staged.validate_projected_memberships(
                    self,
                    &value,
                    &result.projected_qualifications,
                )?;
                let claims = self.stage_constructor_claims(
                    super::ResultClaimOrigin::Construction,
                    result,
                    &claim_operands,
                    &mut frontier,
                    &|current| staged.case_at_projection(self, &value, &current.path),
                )?;
                let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
                staged.publish(self)?;
                self.structural_values.insert(result.place, value)?;
                retirement.commit(self)?;
                self.local_structural_identities.commit_cursor(cursor);
                Ok(crate::engine::execution::OperationFlow::Advance)
            }
        }
    }
    use storage::{ConstructorStorage, ConstructorValue};

    impl TerminalExecution<'_> {
        pub(crate) fn establish_record(
            &mut self,
            result: &terminal_psi::StructuralOperationResult,
            fields: &[terminal_psi::RecordFieldInitializer],
        ) -> Result<(), TerminalInterpretError> {
            if self.structural_values.contains_key(&result.place) {
                return Err(malformed());
            }
            let declaration = self
                .program
                .structural_declaration(result.structural_type)?;
            let StructuralTypeShape::Record {
                fields: declarations,
                ..
            } = &declaration.shape
            else {
                return Err(malformed());
            };
            let declarations = declarations
                .iter()
                .filter(|field| !field.relevance.is_erased());
            if fields.len() != declarations.clone().count() {
                return Err(malformed());
            }
            let mut value = TerminalStructuralValue::minted(
                self.local_structural_identities.allocate()?,
                result.structural_type,
                result.qualifications.clone(),
            );
            let mut storage = ConstructorStorage::new(self.account.as_ref());

            let mut consumed = Vec::new();
            let mut claim_operands = Vec::new();
            let mut frontier = self.live_affine_frontier.clone();
            for (binding, declaration) in fields.iter().zip(declarations) {
                if binding.field != declaration.id || declaration.relevance.is_erased() {
                    return Err(malformed());
                }
                match &binding.value {
                    terminal_psi::RecordFieldValue::Scalar { value: operand, .. } => {
                        let scalar = self.values.get(operand).copied().ok_or_else(malformed)?;
                        if declaration.field_type.scalar_type() != Some(scalar.scalar_type())
                            || !terminal_scalar_belongs_to_type(scalar)
                        {
                            return Err(malformed());
                        }
                        storage.stage_scalar_endpoint(
                        &crate::storage::primitive_storage::PrimitiveStorageEndpoint::ScalarField(
                            StructuralScalarRuntimeField::member(
                                StructuralRuntimePlace::from(&value),
                                binding.field,
                            ),
                        ),
                        scalar,
                    )?;
                    }
                    terminal_psi::RecordFieldValue::Structural(argument) => {
                        claim_operands.push((
                            argument,
                            vec![StructuralPathSegment::Field(declaration.identity)],
                        ));
                        let child = ConstructorValue::resolve(self, argument)?;
                        if matches!(declaration.field_type, StructuralFieldType::Structural(_)) {
                            child.append_memberships(
                                &[StructuralPathSegment::Field(declaration.identity)],
                                &mut value.projected_qualifications,
                            );
                        }
                        if let Some(bytes) = self.prepare_constructor_byte_field(
                            declaration,
                            argument,
                            &child,
                            &mut frontier,
                            &mut consumed,
                        )? {
                            storage.stage_byte_field(
                                crate::boundary::values::StructuralByteSequenceRuntimeField {
                                    parent: StructuralRuntimePlace::from(&value),
                                    namespace:
                                        crate::boundary::values::StructuralFieldNamespace::Member,
                                    field: binding.field,
                                },
                                bytes,
                            )?;
                            continue;
                        }
                        if argument.access != StructuralAccess::Owned
                            || declaration.field_type
                                != StructuralFieldType::Structural(child.structural_type())
                        {
                            return Err(malformed());
                        }
                        let transfers_custody =
                            self.stage_constructor_operand(argument, &mut frontier, &mut consumed)?;
                        let mut member_path = value.path.clone();
                        member_path.push(StructuralPathSegment::Field(declaration.identity));
                        storage.stage(
                            self,
                            argument,
                            &child,
                            &StructuralRuntimePlace {
                                opaque_identity: value.opaque_identity,
                                path: member_path,
                            },
                            transfers_custody,
                        )?;
                    }
                }
            }
            value.projected_qualifications.sort();
            value.projected_qualifications.dedup();
            storage.validate_projected_memberships(
                self,
                &value,
                &result.projected_qualifications,
            )?;
            let claims = self.stage_constructor_claims(
                ResultClaimOrigin::Construction,
                result,
                &claim_operands,
                &mut frontier,
                &|current| storage.case_at_projection(self, &value, &current.path),
            )?;
            let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
            storage.publish(self)?;
            self.structural_values.insert(result.place, value)?;
            retirement.commit(self)?;
            Ok(())
        }
        pub(crate) fn stage_owned_value_arguments(
            &self,
            parameters: &[StructuralParameterDeclaration],
            values: &mut RuntimeRows<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
            cursor: &mut Option<u64>,
        ) -> Result<ConstructorStorage, TerminalInterpretError> {
            let mut payload = ConstructorStorage::new(self.account.as_ref());
            for parameter in parameters {
                if parameter.access != StructuralAccess::Owned
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                {
                    continue;
                }
                let types = self.structural_types();
                let declaration = types.declaration(&parameter.structural_type)?;
                let composite = declaration.shape.is_composite_shape();
                let primitive = match declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(scalar_type) => Some(scalar_type),
                    _ => None,
                };
                if !composite
                    && primitive.is_none()
                    && !declaration.is_bounded_owned_byte_sequence()
                {
                    continue;
                }
                let value = values.get_mut(&parameter.place).ok_or(
                    TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
                )?;
                let source = StructuralRuntimePlace::from(&*value);
                let identity = self.local_structural_identities.allocate_staged(cursor)?;
                if let Some(scalar_type) = primitive {
                    let (storage, actual, shared_view) =
                        self.primitive_view_access(value, &[], scalar_type, false)?;
                    let scalar = self.read_primitive_storage(
                        parameter.place,
                        storage,
                        actual,
                        scalar_type,
                        shared_view,
                    )?;
                    payload.stage_scalar_endpoint(
                        &crate::storage::primitive_storage::PrimitiveStorageEndpoint::Scalar(
                            StructuralRuntimePlace {
                                opaque_identity: identity,
                                path: Vec::new(),
                            },
                        ),
                        scalar,
                    )?;
                } else if composite {
                    payload.stage_subtree(
                        self,
                        &source,
                        &StructuralRuntimePlace {
                            opaque_identity: identity,
                            path: Vec::new(),
                        },
                        false,
                    )?;
                }
                value.opaque_identity = identity;
                value.path.clear();
            }
            Ok(payload)
        }
    }
}

pub(crate) mod scalar_array {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::TerminalScalarValue;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::scalar_storage::ScalarCell;
    use semantic_vocabulary::ScalarType;
    use semantic_vocabulary::StructuralTypeId;
    use semantic_vocabulary::ValueId;
    use terminal_psi::StructuralAccess;
    use terminal_psi::StructuralArgument;
    use terminal_psi::StructuralMultiplicity;
    use terminal_psi::StructuralParameterDeclaration;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::StructuralTypeShape;
    use terminal_psi::TerminalMachine;

    pub(crate) mod bindings {
        use super::{PackedScalarArrayValue, TerminalScalarArrayValue};
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::storage::runtime_rows::RuntimeRows;
        use evaluation_accounting::EvaluationInvocationAccount;
        use semantic_vocabulary::PlaceId;

        struct PackedBinding {
            live: bool,
            value: PackedScalarArrayValue,
        }

        pub(crate) struct ScalarArrayBindings {
            rows: RuntimeRows<PlaceId, PackedBinding>,
            live_count: usize,
        }

        impl ScalarArrayBindings {
            pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
                Self {
                    rows: RuntimeRows::new(account),
                    live_count: 0,
                }
            }

            pub(crate) fn take(&mut self) -> Self {
                Self {
                    rows: self.rows.take(),
                    live_count: std::mem::replace(&mut self.live_count, 0),
                }
            }

            pub(crate) fn len(&self) -> usize {
                self.live_count
            }

            pub(crate) fn get(&self, place: &PlaceId) -> Option<&PackedScalarArrayValue> {
                self.rows
                    .get(place)
                    .filter(|binding| binding.live)
                    .map(|binding| &binding.value)
            }

            pub(crate) fn contains_key(&self, place: &PlaceId) -> bool {
                self.get(place).is_some()
            }

            pub(crate) fn keys(&self) -> impl Iterator<Item = &PlaceId> {
                self.rows
                    .iter()
                    .filter(|(_, binding)| binding.live)
                    .map(|(place, _)| place)
            }

            pub(crate) fn remove(&mut self, place: &PlaceId) {
                self.retire_packed_home(place);
            }

            pub(crate) fn publish(
                &mut self,
                place: PlaceId,
                value: TerminalScalarArrayValue,
            ) -> Result<(), TerminalInterpretError> {
                let live_count = self.live_count.checked_add(1).ok_or_else(malformed)?;
                self.rows.insert_vacant(
                    place,
                    PackedBinding {
                        live: true,
                        value: value.into_runtime()?,
                    },
                )?;
                self.live_count = live_count;
                Ok(())
            }

            pub(crate) fn backing(&self, place: &PlaceId) -> Option<&PackedScalarArrayValue> {
                self.rows.get(place).map(|binding| &binding.value)
            }

            pub(crate) fn backing_mut(
                &mut self,
                place: &PlaceId,
            ) -> Option<&mut PackedScalarArrayValue> {
                self.rows.get_mut(place).map(|binding| &mut binding.value)
            }

            pub(crate) fn has_retired(&self) -> bool {
                self.rows.len() != self.live_count
            }

            pub(crate) fn retains_retired(&self, place: &PlaceId) -> bool {
                self.rows.get(place).is_some_and(|binding| !binding.live)
            }

            pub(crate) fn collect_retired(&mut self, mut retained: impl FnMut(PlaceId) -> bool) {
                self.rows
                    .retain(|place, binding| binding.live || retained(*place));
            }
        }

        pub(crate) trait PackedHomeRetirement {
            fn contains_packed_home(&self, place: &PlaceId) -> bool;
            fn retire_packed_home(&mut self, place: &PlaceId);
        }

        impl PackedHomeRetirement for ScalarArrayBindings {
            fn contains_packed_home(&self, place: &PlaceId) -> bool {
                self.contains_key(place)
            }

            fn retire_packed_home(&mut self, place: &PlaceId) {
                if let Some(binding) = self.rows.get_mut(place)
                    && binding.live
                {
                    binding.live = false;
                    self.live_count -= 1;
                }
            }
        }

        impl PackedHomeRetirement for std::collections::BTreeSet<PlaceId> {
            fn contains_packed_home(&self, place: &PlaceId) -> bool {
                self.contains(place)
            }

            fn retire_packed_home(&mut self, place: &PlaceId) {
                self.remove(place);
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::boundary::values::TerminalScalarValue;
            use semantic_vocabulary::StructuralTypeId;

            fn payload() -> TerminalScalarArrayValue {
                TerminalScalarArrayValue::collect(
                    StructuralTypeId::new(1).unwrap(),
                    Vec::new(),
                    1,
                    [Ok(TerminalScalarValue::Boolean(true))],
                    None,
                )
                .unwrap()
            }

            #[test]
            fn binding_retirement_preserves_captured_payload_without_copying_or_rebinding() {
                let place = PlaceId::new(1).unwrap();
                let mut arrays = ScalarArrayBindings::new(None);
                arrays.publish(place, payload()).unwrap();
                let original = arrays.get(&place).unwrap().cells().as_ptr();
                arrays.remove(&place);
                assert!(arrays.get(&place).is_none());
                assert_eq!(arrays.backing(&place).unwrap().cells().as_ptr(), original);
                assert!(arrays.publish(place, payload()).is_err());
                arrays.collect_retired(|retained| retained == place);
                assert!(arrays.backing(&place).is_some());
                arrays.collect_retired(|_| false);
                assert!(arrays.backing(&place).is_none());
                arrays.publish(place, payload()).unwrap();
            }

            #[test]
            fn exact_capacity_retirement_keeps_funding_until_the_last_home_is_released() {
                use evaluation_accounting::{
                    BuildEvaluationSponsor, BuildEvaluationSponsorLimits,
                    EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
                };
                let sponsor = BuildEvaluationSponsor::new_with_measurement(
                    BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 2, 100, 100, 100).unwrap(),
                    EvaluationMeasurementIdentity::TerminalPsi {
                        work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
                        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
                    },
                )
                .unwrap();
                let account = sponsor
                    .begin_invocation(sponsor.measurement(), 100)
                    .unwrap();
                let place = PlaceId::new(1).unwrap();
                let other = PlaceId::new(2).unwrap();
                let value = TerminalScalarArrayValue::collect(
                    StructuralTypeId::new(1).unwrap(),
                    Vec::new(),
                    1,
                    [Ok(TerminalScalarValue::Boolean(true))],
                    Some(&account),
                )
                .unwrap();
                let mut arrays = ScalarArrayBindings::new(Some(&account));
                arrays.publish(place, value).unwrap();
                let original = arrays.get(&place).unwrap().cells().as_ptr();
                assert_eq!(
                    sponsor.live_storage_cells(),
                    2,
                    "one payload cell and one home row"
                );
                arrays.retire_packed_home(&place);
                assert_eq!(
                    sponsor.live_storage_cells(),
                    2,
                    "retirement must not reserve another row"
                );
                assert!(arrays.get(&place).is_none());
                assert_eq!(arrays.backing(&place).unwrap().cells().as_ptr(), original);
                let alias = payload();
                assert!(
                    arrays.publish(place, alias).is_err(),
                    "a captured home cannot be rebound"
                );
                arrays.collect_retired(|retained| retained == place);
                assert_eq!(
                    sponsor.live_storage_cells(),
                    2,
                    "the live loan retains its funded payload"
                );
                let mut saved = arrays.take();
                let alias = payload();
                assert!(
                    arrays.publish(other, alias).is_err(),
                    "the empty next frame keeps its account"
                );
                assert_eq!(sponsor.live_storage_cells(), 2);
                saved.collect_retired(|_| false);
                assert!(saved.backing(&place).is_none());
                assert_eq!(
                    sponsor.live_storage_cells(),
                    0,
                    "GC retires both row and payload leases"
                );
                drop(saved);
                drop(arrays);
                assert_eq!(sponsor.peak_live_cells(), 2);
            }
        }
    }
    pub(crate) mod empty_storage {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::structural_inputs::byte_arrays::byte_array_length;
        use crate::boundary::structural_inputs::byte_arrays::storage::StructuralByteArrayStorage;
        use crate::boundary::values::StructuralRuntimePlace;
        use crate::embedding::program::ProgramDeclarations;
        use crate::engine::custody::fixed_index_step;
        use crate::storage::runtime_rows::RuntimeRows;
        use evaluation_accounting::EvaluationInvocationAccount;
        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

        #[derive(Clone, Copy)]
        pub(crate) struct EmptyArrayState {
            structural_type: StructuralTypeId,
            initialized: bool,
        }

        pub(crate) struct EmptyArrayRetirement {
            root: StructuralRuntimePlace,
            vacancy: RuntimeRows<StructuralRuntimePlace, EmptyArrayState>,
        }

        impl EmptyArrayRetirement {
            pub(crate) fn root(&self) -> &StructuralRuntimePlace {
                &self.root
            }
        }

        pub(crate) struct EmptyArrayStorage {
            pub(crate) rows: RuntimeRows<StructuralRuntimePlace, EmptyArrayState>,
        }

        impl EmptyArrayStorage {
            pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
                Self {
                    rows: RuntimeRows::new(account),
                }
            }

            pub(crate) fn establish(
                &mut self,
                types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
                home: StructuralRuntimePlace,
                structural_type: StructuralTypeId,
            ) -> Result<(), TerminalInterpretError> {
                if terminal_semantics::scalar_array_leaf_shape(types.values(), structural_type)
                    .is_none_or(|(_, count)| count != 0)
                {
                    return Err(malformed());
                }
                crate::storage::structural_storage::prune_rows(&mut self.rows, &home);
                self.rows.insert(
                    home,
                    EmptyArrayState {
                        structural_type,
                        initialized: true,
                    },
                )?;
                Ok(())
            }

            fn containing(
                &self,
                home: &StructuralRuntimePlace,
                strict: bool,
            ) -> Option<(&StructuralRuntimePlace, &EmptyArrayState)> {
                let identity = StructuralRuntimePlace {
                    opaque_identity: home.opaque_identity,
                    path: Vec::new(),
                };
                crate::storage::structural_storage::cells(&self.rows, &identity)
                    .filter(|(parent, _)| {
                        parent.opaque_identity == home.opaque_identity
                            && home.path.starts_with(&parent.path)
                            && (!strict || parent.path.len() < home.path.len())
                    })
                    .max_by_key(|(parent, _)| parent.path.len())
            }

            pub(crate) fn initialized(
                &self,
                types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
                bytes: &RuntimeRows<StructuralRuntimePlace, StructuralByteArrayStorage>,
                home: &StructuralRuntimePlace,
                structural_type: StructuralTypeId,
            ) -> Result<bool, TerminalInterpretError> {
                if terminal_semantics::scalar_array_leaf_shape(types.values(), structural_type)
                    .is_none_or(|(_, count)| count != 0)
                {
                    return Err(malformed());
                }
                let mut pending = vec![(home.clone(), structural_type)];
                while let Some((home, structural_type)) = pending.pop() {
                    if let Some((parent, state)) = self.containing(&home, false) {
                        if select(
                            types,
                            state.structural_type,
                            &home.path[parent.path.len()..],
                        )? != structural_type
                        {
                            return Err(malformed());
                        }
                        if state.initialized {
                            let mut previous: Option<&StructuralRuntimePlace> = None;
                            for (child, state) in
                                crate::storage::structural_storage::cells(&self.rows, &home)
                            {
                                if state.initialized
                                    || previous
                                        .is_some_and(|parent| child.path.starts_with(&parent.path))
                                {
                                    continue;
                                }
                                pending.push((child.clone(), state.structural_type));
                                previous = Some(child);
                            }
                            continue;
                        }
                    }
                    if byte_array_length(types, structural_type) == Some(0)
                        && bytes.get(&home).is_some_and(|view| view.len() == 0)
                    {
                        continue;
                    }
                    let StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } = types.declaration(&structural_type)?.shape
                    else {
                        return Err(malformed());
                    };
                    if length == 0 {
                        return Ok(false);
                    }
                    let mut indices = Vec::new();
                    for child in crate::storage::structural_storage::cells(&self.rows, &home)
                        .map(|(key, _)| key)
                        .chain(
                            crate::storage::structural_storage::cells(bytes, &home)
                                .map(|(key, _)| key),
                        )
                    {
                        if let Some(StructuralPathSegment::FixedIndex(index)) =
                            child.path.get(home.path.len())
                        {
                            if *index >= length {
                                return Err(malformed());
                            }
                            indices.push(*index);
                        }
                    }
                    indices.sort_unstable();
                    indices.dedup();
                    if u64::try_from(indices.len()).ok() != Some(length) {
                        return Ok(false);
                    }
                    for index in indices {
                        let mut child = home.clone();
                        child.path.push(StructuralPathSegment::FixedIndex(index));
                        pending.push((child, element));
                    }
                }
                Ok(true)
            }

            pub(crate) fn initialized_type(
                &self,
                types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
                bytes: &RuntimeRows<StructuralRuntimePlace, StructuralByteArrayStorage>,
                home: &StructuralRuntimePlace,
            ) -> Result<Option<StructuralTypeId>, TerminalInterpretError> {
                let Some((parent, state)) = self.containing(home, false) else {
                    return Ok(None);
                };
                let selected = select(
                    types,
                    state.structural_type,
                    &home.path[parent.path.len()..],
                )?;
                Ok(self
                    .initialized(types, bytes, home, selected)?
                    .then_some(selected))
            }

            pub(crate) fn initialized_byte_array(
                &self,
                types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
                bytes: &RuntimeRows<StructuralRuntimePlace, StructuralByteArrayStorage>,
                home: &StructuralRuntimePlace,
            ) -> Result<bool, TerminalInterpretError> {
                Ok(self
                    .initialized_type(types, bytes, home)?
                    .is_some_and(|selected| byte_array_length(types, selected) == Some(0)))
            }

            pub(crate) fn stage_subtree(
                &mut self,
                types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
                original: &Self,
                bytes: &RuntimeRows<StructuralRuntimePlace, StructuralByteArrayStorage>,
                source: &StructuralRuntimePlace,
                destination: &StructuralRuntimePlace,
            ) -> Result<(), TerminalInterpretError> {
                if let Some(structural_type) = original.initialized_type(types, bytes, source)? {
                    self.establish(types, destination.clone(), structural_type)?;
                }
                for (home, state) in
                    crate::storage::structural_storage::cells(&original.rows, source)
                {
                    let mut target = destination.clone();
                    target
                        .path
                        .extend_from_slice(&home.path[source.path.len()..]);
                    self.rows.insert(target, *state)?;
                }
                Ok(())
            }

            pub(crate) fn prepare_retirement(
                &self,
                types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
                account: Option<&EvaluationInvocationAccount>,
                root: &StructuralRuntimePlace,
            ) -> Result<EmptyArrayRetirement, TerminalInterpretError> {
                let mut vacancy = RuntimeRows::new(account);
                if let Some((parent, state)) = self.containing(root, true)
                    && state.initialized
                {
                    let structural_type = select(
                        types,
                        state.structural_type,
                        &root.path[parent.path.len()..],
                    )?;
                    vacancy.insert(
                        root.clone(),
                        EmptyArrayState {
                            structural_type,
                            initialized: false,
                        },
                    )?;
                }
                self.rows.validate_append(&vacancy)?;
                Ok(EmptyArrayRetirement {
                    root: root.clone(),
                    vacancy,
                })
            }

            pub(crate) fn retire(
                &mut self,
                mut retirement: EmptyArrayRetirement,
            ) -> Result<(), TerminalInterpretError> {
                crate::storage::structural_storage::prune_rows(&mut self.rows, &retirement.root);
                self.rows.append(&mut retirement.vacancy)
            }
        }

        fn select(
            types: &ProgramDeclarations<'_, StructuralTypeDeclaration>,
            mut structural_type: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Result<StructuralTypeId, TerminalInterpretError> {
            for segment in path {
                let (element, _, _) = fixed_index_step(types, structural_type, segment)?;
                structural_type = element;
            }
            if terminal_semantics::scalar_array_leaf_shape(types.values(), structural_type)
                .is_none_or(|(_, count)| count != 0)
            {
                return Err(malformed());
            }
            Ok(structural_type)
        }
    }
    pub(crate) mod loans {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalStructuralValue;
        use crate::engine::custody::{project_structural_value, resolve_structural_path_type};
        use crate::engine::execution::TerminalExecution;
        use crate::storage::element_views::{ElementView, ElementViewHome};
        use crate::storage::runtime_rows::RuntimeRows;
        use semantic_vocabulary::{PlaceId, StructuralTypeId};
        use terminal_psi::{
            StructuralAccess, StructuralArgument, StructuralMultiplicity,
            StructuralParameterDeclaration, StructuralTypeShape,
        };

        struct CapturedArrayLoan {
            value: TerminalStructuralValue,
            companion: ElementView,
        }

        impl CapturedArrayLoan {
            fn into_companion(self) -> (TerminalStructuralValue, ElementView) {
                (self.value, self.companion)
            }
        }

        impl TerminalExecution<'_> {
            pub(crate) fn prepare_scalar_array_reference(
                &self,
                referent_type: StructuralTypeId,
                argument: &StructuralArgument,
                identity_cursor: &mut Option<u64>,
                local_roots: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
            ) -> Result<Option<(TerminalStructuralValue, ElementView)>, TerminalInterpretError>
            {
                if self.lent_collection_type(referent_type, argument).is_none() {
                    return Ok(None);
                }
                Ok(self
                    .capture_scalar_array_loan(
                        referent_type,
                        argument,
                        identity_cursor,
                        local_roots,
                    )?
                    .map(CapturedArrayLoan::into_companion))
            }

            pub(crate) fn prepare_scalar_array_loan(
                &self,
                parameter: &StructuralParameterDeclaration,
                argument: &StructuralArgument,
                identity_cursor: &mut Option<u64>,
                local_roots: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
            ) -> Result<Option<(TerminalStructuralValue, ElementView)>, TerminalInterpretError>
            {
                if parameter.access == StructuralAccess::Owned {
                    return Ok(None);
                }
                if self
                    .lent_collection_type(parameter.structural_type, argument)
                    .is_none()
                {
                    return Ok(None);
                }
                let types = self.structural_types();
                let fixed_array_formal = matches!(
                    types.get(&parameter.structural_type).map(|row| &row.shape),
                    Some(StructuralTypeShape::FixedArray { .. })
                );
                let supported_memberships = fixed_array_formal
                    || parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty();
                if argument.access != parameter.access
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !supported_memberships
                {
                    return Err(malformed());
                }
                let Some(captured) = self.capture_scalar_array_loan(
                    parameter.structural_type,
                    argument,
                    identity_cursor,
                    local_roots,
                )?
                else {
                    return Ok(None);
                };
                if fixed_array_formal
                    && !borrowed_array_memberships_match(&captured.value, parameter)
                {
                    return Err(malformed());
                }
                Ok(Some(captured.into_companion()))
            }

            fn capture_scalar_array_loan(
                &self,
                structural_type: StructuralTypeId,
                argument: &StructuralArgument,
                identity_cursor: &mut Option<u64>,
                local_roots: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
            ) -> Result<Option<CapturedArrayLoan>, TerminalInterpretError> {
                if argument.access == StructuralAccess::Owned {
                    return Ok(None);
                }
                let Some(collection_type) = self.lent_collection_type(structural_type, argument)
                else {
                    return Ok(None);
                };
                let types = self.structural_types();
                let fixed_array_formal = matches!(
                    types.get(&structural_type).map(|row| &row.shape),
                    Some(StructuralTypeShape::FixedArray { .. })
                );
                let preserves_membership_carrier = |whole: &[_], projected: &[_]| {
                    fixed_array_formal || whole.is_empty() && projected.is_empty()
                };
                let StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(length),
                    ..
                } = self.structural_declaration(collection_type)?.shape
                else {
                    return Err(malformed());
                };
                let length = usize::try_from(length).map_err(|_| malformed())?;
                let root_type = self.live_structural_type(argument.place)?;
                let source_access =
                    self.storage_carrier_access(argument.place, root_type, &argument.path)?;
                if !argument.access.admits(source_access) {
                    return Err(malformed());
                }
                if argument.path.is_empty()
                    && let Some(loan) = self.element_view_values.get(&argument.place)
                {
                    let referent = self.structural_value(argument.place)?;
                    if referent.structural_type != collection_type
                        || !preserves_membership_carrier(
                            &referent.qualifications,
                            &referent.projected_qualifications,
                        )
                        || !loan.matches_descriptor_home(referent)
                        || loan.len() != length
                    {
                        return Err(malformed());
                    }
                    let mut presented = referent.clone();
                    presented.structural_type = structural_type;
                    let loan = loan
                        .subslice(0, length, argument.access)
                        .ok_or_else(malformed)?
                        .with_descriptor_home(presented.clone());
                    return Ok(Some(CapturedArrayLoan {
                        value: presented,
                        companion: loan,
                    }));
                }
                let (root, _) =
                    self.array_storage_root_staged(argument.place, identity_cursor, local_roots)?;
                let (referent, captured_home) = if self
                    .scalar_array_values
                    .contains_key(&argument.place)
                {
                    (
                        project_structural_value(&self.structural_types(), &root, &argument.path)?,
                        None,
                    )
                } else {
                    let projected = self.resolve_live_storage_projection(
                        argument.place,
                        &argument.path,
                        argument.access,
                    )?;
                    if projected.view_tip.is_some() {
                        return Ok(None);
                    }
                    (projected.value, projected.collection_home)
                };
                if referent.structural_type != collection_type
                    || !preserves_membership_carrier(
                        &referent.qualifications,
                        &referent.projected_qualifications,
                    )
                {
                    return Err(malformed());
                }
                let mut presented = referent.clone();
                presented.structural_type = structural_type;
                let home = match captured_home {
                    Some(home) => Some(home),
                    None if let Some(loan) = self.element_view_values.get(&argument.place) => {
                        if !loan.matches_descriptor_home(&root)
                            || !argument.access.admits_referent(loan.access())
                        {
                            return Err(malformed());
                        }
                        loan.captured_storage_home(&self.structural_types(), &argument.path)?
                    }
                    None if self.scalar_array_values.contains_key(&argument.place) => {
                        Some(ElementViewHome {
                            storage: self.capture_primitive_home(argument.place)?,
                            path: argument.path.clone(),
                        })
                    }
                    None => None,
                };
                let loan = ElementView::structural(
                    referent,
                    length,
                    argument.access,
                    home,
                    argument.place,
                )
                .with_descriptor_home(presented.clone());
                Ok(Some(CapturedArrayLoan {
                    value: presented,
                    companion: loan,
                }))
            }

            fn lent_collection_type(
                &self,
                structural_type: StructuralTypeId,
                argument: &StructuralArgument,
            ) -> Option<StructuralTypeId> {
                let types = self.structural_types();
                match types.get(&structural_type)?.shape {
                    StructuralTypeShape::FixedArray { .. } => {
                        terminal_semantics::scalar_array_leaf_shape(
                            types.values(),
                            structural_type,
                        )?;
                        Some(structural_type)
                    }
                    StructuralTypeShape::ElementView { element, .. } => {
                        let root_type = self.live_structural_type(argument.place).ok()?;
                        let collection_type =
                            resolve_structural_path_type(&types, root_type, &argument.path).ok()?;
                        matches!(
                    types.get(&collection_type)?.shape,
                    StructuralTypeShape::FixedArray { element: source, .. } if source == element
                )
                .then_some(collection_type)
                    }
                    _ => None,
                }
            }
        }

        pub(crate) fn borrowed_array_memberships_match(
            value: &TerminalStructuralValue,
            parameter: &StructuralParameterDeclaration,
        ) -> bool {
            parameter
                .qualifications
                .iter()
                .all(|domain| value.qualifications.contains(domain))
                && parameter
                    .projected_qualifications
                    .iter()
                    .all(|row| value.projected_qualifications.contains(row))
                && (parameter.access == StructuralAccess::SharedBorrow
                    || value
                        .qualifications
                        .iter()
                        .all(|domain| parameter.qualifications.contains(domain))
                        && value
                            .projected_qualifications
                            .iter()
                            .all(|row| parameter.projected_qualifications.contains(row)))
        }
    }
    pub(crate) mod storage {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalScalarValue;
        use crate::storage::scalar_storage::ScalarCell;
        use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

        #[derive(compiler_storage::Record)]
        pub(crate) struct ScalarArrayStorage<Element> {
            elements: Vec<Element>,
            #[record(skip)]
            _lease: Option<EvaluationStorageLease>,
            #[record(skip)]
            account: Option<EvaluationInvocationAccount>,
        }

        impl<Element> ScalarArrayStorage<Element> {
            pub(crate) fn collect(
                count: u64,
                values: impl IntoIterator<Item = Result<Element, TerminalInterpretError>>,
                account: Option<&EvaluationInvocationAccount>,
            ) -> Result<Self, TerminalInterpretError> {
                let lease = account
                    .map(|account| account.reserve_storage(count, 0))
                    .transpose()
                    .map_err(TerminalInterpretError::Storage)?;
                let length = usize::try_from(count).map_err(|_| malformed())?;
                let requested_bytes = length
                    .checked_mul(std::mem::size_of::<Element>())
                    .ok_or_else(malformed)?;
                let mut elements = Vec::new();
                elements.try_reserve_exact(length).map_err(|reason| {
                    TerminalInterpretError::StorageAllocationFailed {
                        requested_bytes,
                        reason: reason.to_string(),
                    }
                })?;
                let mut values = values.into_iter();
                for _ in 0..length {
                    elements.push(values.next().ok_or_else(malformed)??);
                }
                if values.next().is_some() {
                    return Err(malformed());
                }
                Ok(Self {
                    elements,
                    _lease: lease,
                    account: account.cloned(),
                })
            }

            pub(crate) fn copy(&self) -> Result<Self, TerminalInterpretError>
            where
                Element: Clone,
            {
                Self::collect(
                    u64::try_from(self.elements.len()).map_err(|_| malformed())?,
                    self.elements.iter().cloned().map(Ok),
                    self.account.as_ref(),
                )
            }

            pub(crate) fn elements(&self) -> &[Element] {
                &self.elements
            }

            pub(crate) fn elements_mut(&mut self) -> &mut [Element] {
                &mut self.elements
            }
        }

        impl ScalarArrayStorage<TerminalScalarValue> {
            pub(crate) fn into_cells(
                self,
            ) -> Result<ScalarArrayStorage<ScalarCell>, TerminalInterpretError> {
                let requested_bytes = self
                    .elements
                    .len()
                    .checked_mul(std::mem::size_of::<ScalarCell>())
                    .ok_or_else(malformed)?;
                let mut cells = Vec::new();
                cells
                    .try_reserve_exact(self.elements.len())
                    .map_err(|reason| TerminalInterpretError::StorageAllocationFailed {
                        requested_bytes,
                        reason: reason.to_string(),
                    })?;
                for value in self.elements {
                    cells.push(ScalarCell::Local(value));
                }
                Ok(ScalarArrayStorage {
                    elements: cells,
                    _lease: self._lease,
                    account: self.account,
                })
            }
        }

        impl ScalarArrayStorage<ScalarCell> {
            pub(crate) fn observe(
                &self,
            ) -> Result<ScalarArrayStorage<TerminalScalarValue>, TerminalInterpretError>
            {
                ScalarArrayStorage::collect(
                    u64::try_from(self.elements.len()).map_err(|_| malformed())?,
                    self.elements.iter().map(ScalarCell::read),
                    self.account.as_ref(),
                )
            }

            pub(crate) fn copy_for_mutation(&self) -> Result<Self, TerminalInterpretError> {
                self.copy()
            }

            pub(crate) fn cells(&self) -> &[ScalarCell] {
                self.elements()
            }
            pub(crate) fn cells_mut(&mut self) -> &mut [ScalarCell] {
                self.elements_mut()
            }
        }

        impl<Element: PartialEq> PartialEq for ScalarArrayStorage<Element> {
            fn eq(&self, other: &Self) -> bool {
                self.elements == other.elements
            }
        }
        impl<Element: Eq> Eq for ScalarArrayStorage<Element> {}
        impl<Element: std::fmt::Debug> std::fmt::Debug for ScalarArrayStorage<Element> {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.elements.fmt(formatter)
            }
        }
    }
    pub(crate) use bindings::{PackedHomeRetirement, ScalarArrayBindings};
    pub(crate) use loans::borrowed_array_memberships_match;
    use std::rc::Rc;
    use std::sync::Arc;
    use storage::ScalarArrayStorage;
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalScalarArrayValue {
        pub(crate) storage_identity: Option<u64>,
        pub qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        pub structural_type: StructuralTypeId,
        elements: Arc<ScalarArrayStorage<TerminalScalarValue>>,
    }

    impl TerminalScalarArrayValue {
        pub fn elements(&self) -> &[TerminalScalarValue] {
            self.elements.elements()
        }
        pub fn len(&self) -> usize {
            self.elements().len()
        }
        pub fn is_empty(&self) -> bool {
            self.elements().is_empty()
        }

        fn collect(
            structural_type: StructuralTypeId,
            qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
            count: u64,
            values: impl IntoIterator<Item = Result<TerminalScalarValue, TerminalInterpretError>>,
            account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
        ) -> Result<Self, TerminalInterpretError> {
            Ok(Self {
                storage_identity: None,
                qualifications,
                structural_type,
                elements: Arc::new(ScalarArrayStorage::collect(count, values, account)?),
            })
        }

        fn into_runtime(self) -> Result<PackedScalarArrayValue, TerminalInterpretError> {
            let storage = match Arc::try_unwrap(self.elements) {
                Ok(storage) => storage,
                Err(shared) => shared.copy()?,
            };
            Ok(PackedScalarArrayValue {
                storage_identity: self.storage_identity,
                qualifications: self.qualifications,
                structural_type: self.structural_type,
                elements: Rc::new(storage.into_cells()?),
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PackedScalarArrayValue {
        pub(crate) storage_identity: Option<u64>,
        pub qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        pub structural_type: StructuralTypeId,
        elements: Rc<ScalarArrayStorage<ScalarCell>>,
    }

    impl PackedScalarArrayValue {
        pub(crate) fn from_bytes(
            structural_type: StructuralTypeId,
            bytes: &[u8],
            account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
        ) -> Result<Self, TerminalInterpretError> {
            let scalar_type = semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                8,
            )
            .map_err(|_| malformed())?;
            Self::collect(
                structural_type,
                Vec::new(),
                u64::try_from(bytes.len()).map_err(|_| malformed())?,
                bytes.iter().map(|byte| {
                    Ok(TerminalScalarValue::Integer {
                        scalar_type,
                        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*byte)),
                    })
                }),
                account,
            )
        }
        #[cfg(test)]
        pub fn elements(&self) -> Result<Vec<TerminalScalarValue>, TerminalInterpretError> {
            self.cells().iter().map(ScalarCell::read).collect()
        }

        pub fn len(&self) -> usize {
            self.cells().len()
        }

        pub fn is_empty(&self) -> bool {
            self.cells().is_empty()
        }

        pub fn element(
            &self,
            index: usize,
        ) -> Result<Option<TerminalScalarValue>, TerminalInterpretError> {
            self.cell(index).map(ScalarCell::read).transpose()
        }

        pub(crate) fn cells(&self) -> &[ScalarCell] {
            self.elements.cells()
        }

        pub(crate) fn cell(&self, index: usize) -> Option<&ScalarCell> {
            self.cells().get(index)
        }

        pub(crate) fn set_element(
            &mut self,
            index: usize,
            value: TerminalScalarValue,
        ) -> Result<(), TerminalInterpretError> {
            if self.cell(index).ok_or_else(malformed)?.scalar_type() != value.scalar_type()
                || !crate::engine::scalar_operations::terminal_scalar_belongs_to_type(value)
            {
                return Err(malformed());
            }
            self.mutable_storage()?.cells_mut()[index].write(value)
        }

        pub(crate) fn prepare_resident_install(
            &mut self,
            index: usize,
            resident: &ScalarCell,
        ) -> Result<(), TerminalInterpretError> {
            let previous = self.cell(index).ok_or_else(malformed)?;
            if !matches!(resident, ScalarCell::Resident { .. })
                || previous.scalar_type() != resident.scalar_type()
                || previous.read()? != resident.read()?
                || (matches!(previous, ScalarCell::Resident { .. }) && previous != resident)
            {
                return Err(malformed());
            }
            self.mutable_storage()?;
            Ok(())
        }

        pub(crate) fn install_resident(
            &mut self,
            index: usize,
            resident: ScalarCell,
        ) -> Result<(), TerminalInterpretError> {
            self.prepare_resident_install(index, &resident)?;
            Rc::get_mut(&mut self.elements)
                .ok_or_else(malformed)?
                .cells_mut()[index] = resident;
            Ok(())
        }

        fn collect(
            structural_type: StructuralTypeId,
            qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
            count: u64,
            values: impl IntoIterator<Item = Result<TerminalScalarValue, TerminalInterpretError>>,
            account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
        ) -> Result<Self, TerminalInterpretError> {
            Ok(Self {
                storage_identity: None,
                qualifications,
                structural_type,
                elements: Rc::new(ScalarArrayStorage::collect(
                    count,
                    values.into_iter().map(|value| value.map(ScalarCell::Local)),
                    account,
                )?),
            })
        }

        pub(crate) fn snapshot(&self) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
            Ok(TerminalScalarArrayValue {
                storage_identity: None,
                qualifications: self.qualifications.clone(),
                structural_type: self.structural_type,
                elements: Arc::new(self.elements.observe()?),
            })
        }

        fn mutable_storage(
            &mut self,
        ) -> Result<&mut ScalarArrayStorage<ScalarCell>, TerminalInterpretError> {
            if Rc::get_mut(&mut self.elements).is_none() {
                self.elements = Rc::new(self.elements.copy_for_mutation()?);
            }
            Rc::get_mut(&mut self.elements).ok_or_else(malformed)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalScalarArrayResult {
        pub value: TerminalScalarArrayValue,
    }

    impl TerminalExecution<'_> {
        pub(crate) fn prepare_scalar_array_argument(
            &self,
            machine: &TerminalMachine,
            parameter: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
        ) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
            if parameter.access != StructuralAccess::Owned
                || argument.access != StructuralAccess::Owned
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.projected_qualifications.is_empty()
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == parameter.place)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place)
            {
                return Err(malformed());
            }
            let mut snapshot = self.scalar_array_snapshot(argument, parameter.structural_type)?;
            self.validate_scalar_array_qualifications(&snapshot, &parameter.qualifications)?;
            snapshot.qualifications = parameter.qualifications.clone();
            Ok(snapshot)
        }

        pub(crate) fn validate_scalar_array_qualifications(
            &self,
            snapshot: &TerminalScalarArrayValue,
            qualifications: &[semantic_vocabulary::StructuralDomainId],
        ) -> Result<(), TerminalInterpretError> {
            self.validate_scalar_array_contents_qualifications(
                snapshot.structural_type,
                snapshot.elements().iter().map(|value| value.scalar_type()),
                qualifications,
            )
        }

        pub(crate) fn validate_scalar_array_contents_qualifications(
            &self,
            structural_type: StructuralTypeId,
            mut elements: impl ExactSizeIterator<Item = ScalarType>,
            qualifications: &[semantic_vocabulary::StructuralDomainId],
        ) -> Result<(), TerminalInterpretError> {
            let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
                self.structural_types().values(),
                structural_type,
            )
            .ok_or_else(malformed)?;
            if u64::try_from(elements.len()).ok() != Some(count)
                || elements.any(|element_type| element_type != scalar_type)
                || qualifications.iter().any(|domain| {
                    !self
                        .program
                        .catalogs
                        .structural_domains
                        .iter()
                        .any(|declaration| {
                            declaration.id == *domain && declaration.carrier == structural_type
                        })
                })
            {
                return Err(malformed());
            }
            Ok(())
        }

        pub(crate) fn scalar_array_snapshot(
            &self,
            argument: &StructuralArgument,
            structural_type: StructuralTypeId,
        ) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
            let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
                self.structural_types().values(),
                structural_type,
            )
            .ok_or_else(malformed)?;
            let Some(value) = argument
                .path
                .is_empty()
                .then(|| self.scalar_array_values.get(&argument.place))
                .flatten()
            else {
                return self.stored_scalar_array_copy(
                    argument,
                    structural_type,
                    scalar_type,
                    count,
                );
            };
            if value.structural_type != structural_type
                || u64::try_from(value.len()).ok() != Some(count)
                || value
                    .cells()
                    .iter()
                    .any(|cell| cell.scalar_type() != scalar_type)
                || crate::engine::custody::place_is_claimed(&self.live_claims, argument.place)
                || self.structural_values.contains_key(&argument.place)
            {
                return Err(malformed());
            }
            value.snapshot()
        }

        fn stored_scalar_array_copy(
            &self,
            argument: &StructuralArgument,
            structural_type: StructuralTypeId,
            scalar_type: ScalarType,
            count: u64,
        ) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
            let types = self.structural_types();
            let root_type = self.live_structural_type(argument.place)?;
            if crate::engine::custody::resolve_structural_path_type(
                &types,
                root_type,
                &argument.path,
            )? != structural_type
            {
                return Err(malformed());
            }
            if count == 0 {
                let initialized = if let Some(array) = self.scalar_array_values.get(&argument.place)
                {
                    array.structural_type == root_type
                        && terminal_semantics::scalar_array_leaf_shape(types.values(), root_type)
                            .is_some_and(|(_, count)| count == 0 && array.is_empty())
                } else {
                    let selected = self.resolve_live_storage_projection(
                        argument.place,
                        &argument.path,
                        StructuralAccess::SharedBorrow,
                    )?;
                    if let Some(loan) = selected.array_loan {
                        loan.initialized_empty_scalar_array(self, &[], structural_type)?
                    } else {
                        if selected.value.structural_type != structural_type {
                            return Err(malformed());
                        }
                        self.initialized_empty_scalar_array(
                            &crate::boundary::values::StructuralRuntimePlace::from(&selected.value),
                            structural_type,
                        )?
                    }
                };
                if !initialized {
                    return Err(malformed());
                }
            }
            let mut dimensions = Vec::new();
            let mut current = structural_type;
            while let Some(StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            }) = types.get(&current).map(|declaration| &declaration.shape)
            {
                dimensions.push(*length);
                current = *element;
            }
            let mut path = argument.path.clone();
            let prefix = path.len();
            let elements = (0..count).map(|leaf| {
                path.truncate(prefix);
                let mut remainder = leaf;
                let mut stride = count;
                for length in &dimensions {
                    stride /= length;
                    path.push(StructuralPathSegment::FixedIndex(remainder / stride));
                    remainder %= stride;
                }
                self.read_primitive_place(argument.place, &path, scalar_type)
            });
            TerminalScalarArrayValue::collect(
                structural_type,
                Vec::new(),
                count,
                elements,
                self.account.as_ref(),
            )
        }

        pub(crate) fn initialized_empty_scalar_array(
            &self,
            home: &crate::boundary::values::StructuralRuntimePlace,
            structural_type: StructuralTypeId,
        ) -> Result<bool, TerminalInterpretError> {
            self.empty_array_storage.initialized(
                &self.structural_types(),
                &self.structural_byte_storage,
                home,
                structural_type,
            )
        }

        pub(crate) fn execute_scalar_array_establishment(
            &mut self,
            operation: &terminal_psi::Operation,
            elements: &[ValueId],
        ) -> Result<(), TerminalInterpretError> {
            let result = operation.result.structural().ok_or_else(malformed)?;
            let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
                self.structural_types().values(),
                result.structural_type,
            )
            .ok_or_else(malformed)?;
            if result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !result.qualification_establishments.is_empty()
                || u64::try_from(elements.len()).ok() != Some(count)
                || self.structural_values.contains_key(&result.place)
                || self.scalar_array_values.contains_key(&result.place)
            {
                return Err(malformed());
            }
            let contents = elements.iter().map(|element| {
                let value = self.scalar_value(*element)?;
                if value.scalar_type() != scalar_type {
                    return Err(malformed());
                }
                Ok(value)
            });
            let snapshot = TerminalScalarArrayValue::collect(
                result.structural_type,
                result.qualifications.clone(),
                count,
                contents,
                self.account.as_ref(),
            )?;
            self.validate_scalar_array_qualifications(&snapshot, &result.qualifications)?;
            self.scalar_array_values.publish(result.place, snapshot)?;
            Ok(())
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use crate::boundary::effects::TerminalEffectRejection;
        use compiler_storage::{RecordLimits, decode, encode};
        use evaluation_accounting::{
            BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationAccountingError,
            EvaluationMeasurementIdentity, EvaluationResource, TerminalStorageScheduleIdentity,
        };
        use semantic_vocabulary::IntegerType;

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
        fn payload_is_funded_before_reading_elements_and_refusal_preserves_inspection() {
            let sponsor = sponsor(2);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let mut value = PackedScalarArrayValue::collect(
                StructuralTypeId::new(1).unwrap(),
                Vec::new(),
                2,
                [TerminalScalarValue::Boolean(false); 2].map(Ok),
                Some(&account),
            )
            .unwrap();
            let inspected = value.clone();
            assert_eq!(sponsor.live_storage_cells(), 2);
            assert!(matches!(
                value.set_element(0, TerminalScalarValue::Boolean(true)),
                Err(TerminalInterpretError::Storage(
                    EvaluationAccountingError::BudgetExhausted(EvaluationResource::LiveStorage)
                ))
            ));
            assert_eq!(value.elements().unwrap(), inspected.elements().unwrap());
            assert_eq!(sponsor.live_storage_cells(), 2);
            assert!(value.snapshot().is_err());
            let consumed = std::cell::Cell::new(false);
            assert!(
                ScalarArrayStorage::collect(
                    1,
                    std::iter::once_with(|| {
                        consumed.set(true);
                        Ok(TerminalScalarValue::Boolean(true))
                    }),
                    Some(&account)
                )
                .is_err()
            );
            assert!(
                !consumed.get(),
                "refusal must precede payload evaluation/allocation"
            );
            drop(inspected);
            value
                .set_element(0, TerminalScalarValue::Boolean(true))
                .unwrap();
            assert_eq!(sponsor.live_storage_cells(), 2, "unique writes do not copy");
            drop(value);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }

        #[test]
        fn snapshots_and_detached_writes_own_their_overlap_until_last_observation() {
            let sponsor = sponsor(4);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let mut value = PackedScalarArrayValue::collect(
                StructuralTypeId::new(1).unwrap(),
                Vec::new(),
                2,
                [TerminalScalarValue::Boolean(false); 2].map(Ok),
                Some(&account),
            )
            .unwrap();
            value.storage_identity = Some(7);
            let copied = value.snapshot().unwrap();
            assert_eq!(copied.storage_identity, None);
            assert_eq!(copied.elements(), value.elements().unwrap());
            assert_eq!(sponsor.live_storage_cells(), 4);
            drop(copied);
            let inspected = value.clone();
            value
                .set_element(0, TerminalScalarValue::Boolean(true))
                .unwrap();
            assert_eq!(value.storage_identity, Some(7));
            assert_eq!(
                inspected.elements().unwrap(),
                [TerminalScalarValue::Boolean(false); 2]
            );
            assert_eq!(sponsor.live_storage_cells(), 4);
            assert_eq!(sponsor.peak_live_cells(), 4);
            drop(value);
            assert_eq!(sponsor.live_storage_cells(), 2);
            drop(inspected);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }

        #[test]
        fn empty_and_incomplete_payloads_do_not_leak_reservations() {
            let sponsor = sponsor(2);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let empty =
                ScalarArrayStorage::<TerminalScalarValue>::collect(0, [], Some(&account)).unwrap();
            assert!(empty.elements().is_empty());
            assert_eq!(sponsor.live_storage_cells(), 0);
            assert!(
                ScalarArrayStorage::<TerminalScalarValue>::collect(2, [], Some(&account)).is_err()
            );
            assert_eq!(sponsor.live_storage_cells(), 0);
            assert!(
                ScalarArrayStorage::collect(
                    0,
                    [Ok(TerminalScalarValue::Boolean(false))],
                    Some(&account)
                )
                .is_err()
            );
            assert_eq!(sponsor.live_storage_cells(), 0);
        }

        #[derive(Debug)]
        struct OriginalCell {
            value: std::cell::Cell<TerminalScalarValue>,
            faulted: std::cell::Cell<bool>,
        }

        impl crate::storage::scalar_storage::TerminalScalarBacking for OriginalCell {
            fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
                if self.faulted.get() {
                    return Err(TerminalEffectRejection::new("original cell unavailable"));
                }
                Ok(self.value.get())
            }

            fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
                if self.faulted.get() {
                    return Err(TerminalEffectRejection::new("original cell unavailable"));
                }
                self.value.set(value);
                Ok(())
            }
        }

        fn byte(value: u8) -> TerminalScalarValue {
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                    .unwrap(),
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(value)),
            }
        }

        fn original(value: u8) -> std::rc::Rc<OriginalCell> {
            std::rc::Rc::new(OriginalCell {
                value: std::cell::Cell::new(byte(value)),
                faulted: std::cell::Cell::new(false),
            })
        }

        fn array() -> PackedScalarArrayValue {
            PackedScalarArrayValue::collect(
                StructuralTypeId::new(1).unwrap(),
                Vec::new(),
                2,
                [Ok(byte(7)), Ok(byte(1))],
                None,
            )
            .unwrap()
        }

        #[test]
        fn resident_mutation_detaches_local_cells_but_preserves_the_original_endpoint() {
            let mut value = array();
            let home = original(7);
            value
                .install_resident(0, ScalarCell::resident(byte(7), home.clone()).unwrap())
                .unwrap();
            value.storage_identity = Some(19);
            let snapshot = value.snapshot().unwrap();
            let inspected = value.clone();
            value.set_element(0, byte(4)).unwrap();
            value.set_element(1, byte(9)).unwrap();
            assert_eq!(value.storage_identity, Some(19));
            assert_eq!(snapshot.storage_identity, None);
            assert_eq!(snapshot.elements(), [byte(7), byte(1)]);
            assert_eq!(inspected.elements().unwrap(), [byte(4), byte(1)]);
            assert_eq!(value.elements().unwrap(), [byte(4), byte(9)]);
            assert_eq!(value.cell(0), inspected.cell(0));
            home.value.set(byte(23));
            assert_eq!(value.element(0).unwrap(), Some(byte(23)));
            assert_eq!(inspected.element(0).unwrap(), Some(byte(23)));
            assert_eq!(snapshot.elements()[0], byte(7));
            assert_eq!(value.element(2).unwrap(), None);
            assert!(value.set_element(2, byte(1)).is_err());
            assert!(
                value
                    .set_element(0, TerminalScalarValue::Boolean(true))
                    .is_err()
            );
            assert_eq!(home.value.get(), byte(23));
        }

        #[test]
        fn resident_installation_cannot_redirect_or_change_an_established_cell() {
            let mut value = array();
            let home = original(7);
            assert!(
                value
                    .install_resident(0, ScalarCell::Local(byte(7)))
                    .is_err()
            );
            assert!(
                value
                    .install_resident(0, ScalarCell::resident(byte(9), original(9)).unwrap())
                    .is_err()
            );
            let resident = ScalarCell::resident(byte(7), home.clone()).unwrap();
            value.install_resident(0, resident.clone()).unwrap();
            value.install_resident(0, resident).unwrap();
            assert!(
                value
                    .install_resident(0, ScalarCell::resident(byte(7), original(7)).unwrap())
                    .is_err()
            );
            value.set_element(0, byte(31)).unwrap();
            assert_eq!(home.value.get(), byte(31));
        }

        #[test]
        fn resident_preflight_funds_detachment_without_publishing_contents() {
            let sponsor = sponsor(4);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let mut value = PackedScalarArrayValue::collect(
                StructuralTypeId::new(1).unwrap(),
                Vec::new(),
                2,
                [Ok(byte(7)), Ok(byte(1))],
                Some(&account),
            )
            .unwrap();
            let inspected = value.clone();
            let home = original(7);
            let resident = ScalarCell::resident(byte(7), home.clone()).unwrap();
            value.prepare_resident_install(0, &resident).unwrap();
            assert_eq!(sponsor.live_storage_cells(), 4);
            assert!(matches!(value.cell(0), Some(ScalarCell::Local(_))));
            assert!(matches!(inspected.cell(0), Some(ScalarCell::Local(_))));
            assert_eq!(value.elements().unwrap(), inspected.elements().unwrap());
            value.install_resident(0, resident).unwrap();
            assert_eq!(sponsor.live_storage_cells(), 4);
            home.value.set(byte(47));
            assert_eq!(value.element(0).unwrap(), Some(byte(47)));
            assert_eq!(inspected.element(0).unwrap(), Some(byte(7)));
            let further_inspection = value.clone();
            let same_resident = value.cell(0).unwrap().clone();
            assert!(matches!(
                value.prepare_resident_install(0, &same_resident),
                Err(TerminalInterpretError::Storage(
                    EvaluationAccountingError::BudgetExhausted(EvaluationResource::LiveStorage)
                ))
            ));
            assert_eq!(value.cell(0), further_inspection.cell(0));
            assert_eq!(value.element(0).unwrap(), Some(byte(47)));
            assert_eq!(sponsor.live_storage_cells(), 4);
            drop(further_inspection);
            drop(inspected);
            drop(value);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }

        #[test]
        fn retained_packed_cells_keep_the_scalar_wire_format_and_decode_as_local_snapshots() {
            let mut value = array();
            let home = original(7);
            value
                .install_resident(0, ScalarCell::resident(byte(7), home.clone()).unwrap())
                .unwrap();
            home.value.set(byte(37));
            let limits = RecordLimits::default();
            let portable = value.snapshot().unwrap();
            let encoded = encode(portable.elements.as_ref(), limits).unwrap();
            assert_eq!(encoded, encode(&vec![byte(37), byte(1)], limits).unwrap());
            let restored: ScalarArrayStorage<TerminalScalarValue> =
                decode(&encoded, limits).unwrap();
            home.value.set(byte(41));
            assert_eq!(restored.elements(), [byte(37), byte(1)]);
            assert_eq!(value.element(0).unwrap(), Some(byte(41)));
            home.faulted.set(true);
            assert!(value.elements().is_err());
            assert!(value.element(0).is_err());
            assert!(value.snapshot().is_err());
        }

        #[test]
        fn byte_observation_borrows_live_packed_cells_and_propagates_original_read_failure() {
            let mut value = array();
            let home = original(7);
            value
                .install_resident(0, ScalarCell::resident(byte(7), home.clone()).unwrap())
                .unwrap();
            let bytes = crate::TerminalByteRead::cells(value.cells()).unwrap();
            assert_eq!(bytes.len(), 2);
            assert_eq!(bytes.read(0).unwrap(), 7);
            home.value.set(byte(43));
            assert_eq!(bytes.read(0).unwrap(), 43);
            let mut output = [0; 2];
            bytes.copy_to(&mut output).unwrap();
            assert_eq!(output, [43, 1]);
            home.faulted.set(true);
            assert!(bytes.read(0).is_err());
            assert!(bytes.copy_to(&mut output).is_err());
            assert!(crate::TerminalByteRead::cells(value.cells()).is_err());
            assert_eq!(bytes.len(), 2);
        }

        #[test]
        fn portable_array_transfer_reuses_its_lease_and_runtime_snapshot_is_send() {
            fn assert_send_sync<Value: Send + Sync>() {}
            assert_send_sync::<TerminalScalarArrayValue>();
            assert_send_sync::<TerminalScalarArrayResult>();
            let sponsor = sponsor(4);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let owned = TerminalScalarArrayValue::collect(
                StructuralTypeId::new(1).unwrap(),
                Vec::new(),
                2,
                [Ok(byte(7)), Ok(byte(1))],
                Some(&account),
            )
            .unwrap();
            assert_eq!(sponsor.live_storage_cells(), 2);
            let mut runtime = owned.into_runtime().unwrap();
            assert_eq!(
                sponsor.live_storage_cells(),
                2,
                "unique transfer retains its lease"
            );
            let home = original(7);
            runtime
                .install_resident(0, ScalarCell::resident(byte(7), home.clone()).unwrap())
                .unwrap();
            home.value.set(byte(59));
            runtime.storage_identity = Some(29);
            let portable = runtime.snapshot().unwrap();
            assert_eq!(sponsor.live_storage_cells(), 4);
            assert_eq!(portable.storage_identity, None);
            assert_eq!(portable.elements(), [byte(59), byte(1)]);
            let bytes = encode(&portable, RecordLimits::default()).unwrap();
            let restored: TerminalScalarArrayValue =
                decode(&bytes, RecordLimits::default()).unwrap();
            home.value.set(byte(61));
            assert_eq!(portable.elements(), restored.elements());
            assert_eq!(restored.elements()[0], byte(59));
            assert_eq!(runtime.element(0).unwrap(), Some(byte(61)));
            drop(runtime);
            assert_eq!(sponsor.live_storage_cells(), 2);
            drop(portable);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }
    }
}

pub(crate) mod structural_storage {
    use crate::boundary::values::{
        StructuralByteSequenceRuntimeField, StructuralRuntimePlace, StructuralScalarRuntimeField,
    };
    use semantic_vocabulary::StructuralFieldId;
    use std::collections::BTreeMap;
    use std::ops::Bound::{Excluded, Included, Unbounded};

    pub(crate) mod contents {
        use crate::boundary::effects::scalar_loans::{
            captured_scalar_restoration_owner, satisfies_source_restriction,
        };
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::{
            StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
            TerminalStructuralValue,
        };
        use crate::engine::execution::TerminalExecution;
        use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
        use crate::storage::primitive_storage::{
            PrimitiveStorage, PrimitiveStorageEndpoint, scalar_fits_storage,
        };
        use semantic_vocabulary::{
            AuthoredFieldStorage, CanonicalStructuralPathSegment as Path, PlaceId, ScalarType,
            StructuralCaseId, StructuralTypeId,
        };
        use terminal_psi::{
            StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
            StructuralPathSegment, StructuralTypeShape,
        };
        use terminal_semantics::scalar_array_leaf_shape;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum TerminalStorageContents {
            Scalar {
                value: TerminalScalarValue,
                stored_scalar: ScalarType,
            },
            Case(StructuralCaseId),
            /// Plain immutable contents; the receiving declaration still owns
            /// the view's access and predicates. This supplies no qualification.
            Bytes(std::sync::Arc<[u8]>),
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct TerminalBoundaryStorageValue {
            pub path: Vec<Path>,
            pub contents: TerminalStorageContents,
        }

        pub(crate) struct StorageLeaf {
            pub path: Vec<Path>,
            pub kind: StorageLeafKind,
        }

        #[derive(Default)]
        pub(crate) struct StorageDescription {
            pub leaves: Vec<StorageLeaf>,
            pub requires_erased_establishment: bool,
            pub requires_atomic_storage: bool,
            // Read-only selection retains the actual backing root and the
            // logical prefix crossed to reach it. Capture must not replay a
            // stored-length index as inline storage beneath the outer record.
            observation_root: Option<(TerminalStructuralValue, usize)>,
        }

        impl StorageLeaf {
            pub(crate) fn validate_value(
                &self,
                value: &TerminalBoundaryStorageValue,
            ) -> Result<(), &'static str> {
                if self.path != value.path {
                    return Err("contents do not name the complete active declaration paths");
                }
                match (&self.kind, value.contents.clone()) {
                    (
                        StorageLeafKind::Scalar {
                            scalar,
                            restriction,
                            stored_scalar,
                            ..
                        },
                        TerminalStorageContents::Scalar {
                            value,
                            stored_scalar: actual,
                        },
                    ) => {
                        if value.scalar_type() != *scalar || !terminal_scalar_belongs_to_type(value)
                        {
                            return Err("scalar contents differ from the declared logical carrier");
                        }
                        if actual != *stored_scalar || !scalar_fits_storage(actual, value) {
                            return Err("scalar contents differ from the declared stored carrier");
                        }
                        if restriction
                            .as_ref()
                            .is_some_and(|field| !value.satisfies_field(field))
                        {
                            return Err(
                                "scalar contents do not satisfy the declaration restriction",
                            );
                        }
                        Ok(())
                    }
                    (StorageLeafKind::Case { case, .. }, TerminalStorageContents::Case(actual))
                        if actual == *case =>
                    {
                        Ok(())
                    }
                    _ => Err("contents kind differs from the active scalar or case declaration"),
                }
            }
        }

        pub(crate) enum StorageLeafKind {
            Scalar {
                destination: PrimitiveStorageEndpoint,
                scalar: ScalarType,
                restriction: Option<StructuralFieldType>,
                stored_scalar: ScalarType,
            },
            Case {
                destination: StructuralRuntimePlace,
                structural_type: StructuralTypeId,
                case: StructuralCaseId,
            },
            Opaque {
                destination: StructuralRuntimePlace,
                declaration: StructuralTypeId,
            },
            ByteField {
                parent: StructuralRuntimePlace,
                field: semantic_vocabulary::StructuralFieldId,
                declaration: StructuralFieldType,
            },
            ByteArray {
                destination: StructuralRuntimePlace,
                structural_type: StructuralTypeId,
                length: u64,
            },
            EmptyScalarArray {
                destination: StructuralRuntimePlace,
                structural_type: StructuralTypeId,
            },
        }

        pub(crate) struct PinnedScalar {
            pub path: Vec<Path>,
            pub storage: PrimitiveStorage,
            pub scalar: ScalarType,
            pub shared: bool,
            pub original: TerminalScalarValue,
            pub restriction: Option<StructuralFieldType>,
            pub restoration_owner: StructuralRuntimePlace,
        }

        pub(crate) fn describe(
            execution: &TerminalExecution,
            referent: &TerminalStructuralValue,
            access: StructuralAccess,
            replacement: Option<&[TerminalBoundaryStorageValue]>,
        ) -> Result<StorageDescription, TerminalInterpretError> {
            let mut leaves = StorageDescription::default();
            collect(
                execution,
                referent.structural_type,
                &StructuralRuntimePlace::from(referent),
                access,
                replacement,
                None,
                &mut Vec::new(),
                &mut leaves,
            )?;
            Ok(leaves)
        }

        pub(crate) fn select(
            execution: &TerminalExecution,
            referent: &TerminalStructuralValue,
            selected_path: &[Path],
        ) -> Result<Option<(TerminalStructuralValue, StorageLeaf)>, TerminalInterpretError>
        {
            let mut leaves = StorageDescription {
                observation_root: Some((referent.clone(), 0)),
                ..Default::default()
            };
            collect(
                execution,
                referent.structural_type,
                &StructuralRuntimePlace::from(referent),
                StructuralAccess::SharedBorrow,
                None,
                Some(selected_path),
                &mut Vec::new(),
                &mut leaves,
            )?;
            let (root, prefix) = leaves.observation_root.ok_or_else(malformed)?;
            let Some(mut leaf) = leaves
                .leaves
                .into_iter()
                .find(|leaf| leaf.path == selected_path)
            else {
                return Ok(None);
            };
            if prefix > leaf.path.len() {
                return Err(malformed());
            }
            drop(leaf.path.drain(..prefix));
            Ok(Some((root, leaf)))
        }

        fn collect(
            execution: &TerminalExecution,
            root: StructuralTypeId,
            destination: &StructuralRuntimePlace,
            access: StructuralAccess,
            replacement: Option<&[TerminalBoundaryStorageValue]>,
            selected_path: Option<&[Path]>,
            path: &mut Vec<Path>,
            leaves: &mut StorageDescription,
        ) -> Result<(), TerminalInterpretError> {
            let declaration = execution.structural_declaration(root)?;
            match &declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => leaves.leaves.push(StorageLeaf {
                    path: path.clone(),
                    kind: StorageLeafKind::Scalar {
                        destination: PrimitiveStorageEndpoint::Scalar(destination.clone()),
                        scalar: *scalar,
                        restriction: declaration.scalar_restriction.clone(),
                        stored_scalar: *scalar,
                    },
                }),
                StructuralTypeShape::AtomicCell(cell) => {
                    leaves.requires_atomic_storage = true;
                    if access == StructuralAccess::MutableBorrow {
                        leaves.leaves.push(StorageLeaf {
                            path: path.clone(),
                            kind: StorageLeafKind::Scalar {
                                destination: PrimitiveStorageEndpoint::Scalar(destination.clone()),
                                scalar: cell.scalar_type(),
                                restriction: Some((*cell).into()),
                                stored_scalar: cell.scalar_type(),
                            },
                        });
                    }
                }
                StructuralTypeShape::Record { fields, .. } => collect_fields(
                    execution,
                    fields,
                    destination,
                    access,
                    replacement,
                    selected_path,
                    path,
                    leaves,
                    false,
                )?,
                StructuralTypeShape::ElementView { element, .. } => {
                    let selected =
                        selected_path.and_then(|selected| selected.strip_prefix(path.as_slice()));
                    if let Some((Path::FixedIndex(index), _)) =
                        selected.and_then(|selected| selected.split_first())
                    {
                        if access != StructuralAccess::SharedBorrow || replacement.is_some() {
                            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                                "typed sequence snapshots require read-only established backing",
                            ));
                        }
                        let descriptor = TerminalStructuralValue {
                            opaque_identity: destination.opaque_identity,
                            structural_type: root,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            path: destination.path.clone(),
                        };
                        let view = execution.element_view_at_resolved_home(&descriptor)?;
                        let index_usize = usize::try_from(*index).map_err(|_| malformed())?;
                        if index_usize >= view.len() {
                            return Err(malformed());
                        }
                        let (selected, suffix) = view.structural_projection(
                            &execution.structural_types(),
                            &[StructuralPathSegment::FixedIndex(*index)],
                            false,
                        )?;
                        let tip = terminal_semantics::runtime_structural_path_tip(
                            execution.structural_types().values(),
                            selected.structural_type,
                            &suffix,
                        )
                        .ok_or_else(malformed)?;
                        if tip.id != *element {
                            return Err(malformed());
                        }
                        let mut child = StructuralRuntimePlace::from(&selected);
                        child.path.extend_from_slice(&suffix);
                        path.push(Path::FixedIndex(*index));
                        let actual = crate::engine::custody::project_structural_value(
                            &execution.structural_types(),
                            &selected,
                            &suffix,
                        )
                        .map_err(|error| {
                            error.in_argument_preparation("retain selected backing source")
                        })?;
                        leaves.observation_root = Some((actual, path.len()));
                        collect(
                            execution,
                            *element,
                            &child,
                            access,
                            None,
                            selected_path,
                            path,
                            leaves,
                        )?;
                        path.pop();
                    } else {
                        leaves.leaves.push(StorageLeaf {
                            path: path.clone(),
                            kind: StorageLeafKind::Opaque {
                                destination: destination.clone(),
                                declaration: root,
                            },
                        });
                    }
                }
                StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Application(_),
                    ..
                } => return Err(TerminalInterpretError::UnclosedArrayExtent(root)),
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    if selected_path.is_none_or(|selected| selected == path.as_slice())
                        && scalar_array_leaf_shape(execution.structural_types().values(), root)
                            .is_some_and(|(_, count)| count == 0)
                    {
                        leaves.leaves.push(StorageLeaf {
                            path: path.clone(),
                            kind: StorageLeafKind::EmptyScalarArray {
                                destination: destination.clone(),
                                structural_type: root,
                            },
                        });
                        return Ok(());
                    }
                    if execution.structural_types().get(element).is_some_and(|row| matches!(row.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(carrier)) if carrier.bits() == 8 && !carrier.is_address() && carrier.sign() == semantic_vocabulary::IntegerSign::Unsigned)) {
                leaves.leaves.push(StorageLeaf { path: path.clone(), kind: StorageLeafKind::ByteArray { destination: destination.clone(), structural_type: root, length: *length } });
            }
                    let elements = match selected_path {
                        None => 0..*length,
                        Some(selected) => match selected.get(path.len()) {
                            Some(Path::FixedIndex(index)) if index < length => *index..index + 1,
                            _ => 0..0,
                        },
                    };
                    for element_index in elements {
                        path.push(Path::FixedIndex(element_index));
                        let mut child = destination.clone();
                        child
                            .path
                            .push(StructuralPathSegment::FixedIndex(element_index));
                        collect(
                            execution,
                            *element,
                            &child,
                            access,
                            replacement,
                            selected_path,
                            path,
                            leaves,
                        )?;
                        path.pop();
                    }
                }
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    let case = if let Some(values) = replacement {
                        let position = values
                            .binary_search_by(|value| value.path.cmp(path))
                            .map_err(|_| malformed())?;
                        let value = &values[position];
                        let TerminalStorageContents::Case(case) = value.contents.clone() else {
                            return Err(malformed());
                        };
                        case
                    } else {
                        let contents = execution.structural_cases.require(destination)?;
                        if contents.structural_type != root {
                            return Err(malformed());
                        }
                        contents.case
                    };
                    let selected = cases
                        .iter()
                        .find(|selected| selected.id == case)
                        .ok_or_else(malformed)?;
                    leaves.leaves.push(StorageLeaf {
                        path: path.clone(),
                        kind: StorageLeafKind::Case {
                            destination: destination.clone(),
                            structural_type: root,
                            case,
                        },
                    });
                    if let StructuralTypeShape::Mixed { fields, .. } = &declaration.shape {
                        collect_fields(
                            execution,
                            fields,
                            destination,
                            access,
                            replacement,
                            selected_path,
                            path,
                            leaves,
                            false,
                        )?;
                    }
                    if selected_path
                        .is_some_and(|selected| selected.get(path.len()) != Some(&Path::Case(case)))
                    {
                        return Ok(());
                    }
                    path.push(Path::Case(case));
                    collect_fields(
                        execution,
                        &selected.fields,
                        destination,
                        access,
                        replacement,
                        selected_path,
                        path,
                        leaves,
                        true,
                    )?;
                    path.pop();
                }
                _ => leaves.leaves.push(StorageLeaf {
                    path: path.clone(),
                    kind: StorageLeafKind::Opaque {
                        destination: destination.clone(),
                        declaration: root,
                    },
                }),
            }
            Ok(())
        }

        fn collect_fields(
            execution: &TerminalExecution,
            fields: &[StructuralFieldDeclaration],
            parent: &StructuralRuntimePlace,
            access: StructuralAccess,
            replacement: Option<&[TerminalBoundaryStorageValue]>,
            selected_path: Option<&[Path]>,
            path: &mut Vec<Path>,
            leaves: &mut StorageDescription,
            payload: bool,
        ) -> Result<(), TerminalInterpretError> {
            for field in fields {
                if selected_path.is_some_and(|selected| {
                    selected.get(path.len()) != Some(&Path::Field(field.id))
                }) {
                    continue;
                }
                if field.relevance.is_erased()
                    || matches!(field.field_type, StructuralFieldType::Erased { .. })
                {
                    leaves.requires_erased_establishment = true;
                    continue;
                }
                let atomic = matches!(
                    field.field_type,
                    StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
                );
                leaves.requires_atomic_storage |= atomic;
                if atomic && access != StructuralAccess::MutableBorrow {
                    continue;
                }
                path.push(Path::Field(field.id));
                if let Some(scalar) = field.field_type.scalar_type() {
                    if matches!(field.storage, Some(AuthoredFieldStorage::Fragmented(_))) {
                        return Err(malformed());
                    }
                    let destination = if payload {
                        StructuralScalarRuntimeField::case_payload(parent.clone(), field.id)
                    } else {
                        StructuralScalarRuntimeField::member(parent.clone(), field.id)
                    };
                    leaves.leaves.push(StorageLeaf {
                        path: path.clone(),
                        kind: StorageLeafKind::Scalar {
                            destination: PrimitiveStorageEndpoint::ScalarField(destination),
                            scalar,
                            restriction: Some(field.field_type.clone()),
                            stored_scalar: field
                                .storage
                                .as_ref()
                                .and_then(|storage| storage.stored_scalar())
                                .unwrap_or(scalar),
                        },
                    });
                } else if let Some(child_type) = field.field_type.as_structural() {
                    let mut child = parent.clone();
                    if payload {
                        let Some(Path::Case(case)) = path.iter().rev().nth(1) else {
                            return Err(malformed());
                        };
                        child.path.push(StructuralPathSegment::Case(*case));
                    }
                    child
                        .path
                        .push(StructuralPathSegment::Field(field.identity));
                    collect(
                        execution,
                        child_type,
                        &child,
                        access,
                        replacement,
                        selected_path,
                        path,
                        leaves,
                    )?;
                } else {
                    leaves.leaves.push(StorageLeaf {
                        path: path.clone(),
                        kind: StorageLeafKind::ByteField {
                            parent: parent.clone(),
                            field: field.id,
                            declaration: field.field_type.clone(),
                        },
                    });
                }
                path.pop();
            }
            Ok(())
        }

        pub(crate) fn scalar_storage(
            execution: &TerminalExecution,
            referent: &TerminalStructuralValue,
            array_loan: Option<&crate::storage::element_views::ElementView>,
            aggregate: Option<&crate::storage::reference::AggregateBacking>,
            access: StructuralAccess,
            leaf: &StorageLeaf,
        ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
            let StorageLeafKind::Scalar { scalar, .. } = leaf.kind else {
                return Err(malformed());
            };
            if let Some(backing) = aggregate {
                if backing.placed.is_some() || array_loan.is_some() {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "host aggregate storage has no resident codec for placed cells",
                    ));
                }
                let path = terminal_semantics::canonical_structural_storage_path(
                    execution.structural_types().values(),
                    referent.structural_type,
                    &leaf.path,
                )
                .ok_or_else(malformed)?;
                if let Some(region) = backing.region {
                    execution.aggregate_view_primitive_access(
                        &backing.value,
                        crate::storage::reference::AggregateViewTip::Aggregate {
                            view: region,
                            backing: backing.value.structural_type,
                        },
                        &path,
                        backing.collection_home.as_ref(),
                        matches!(access, terminal_psi::exclusive_borrow_pair_pattern!()),
                    )
                } else {
                    let region = backing.byte_region.as_ref().ok_or_else(malformed)?;
                    let (storage, scalar) =
                        execution.captured_byte_region_access(&backing.value, region, &path)?;
                    Ok((storage, scalar, access == StructuralAccess::SharedBorrow))
                }
            } else if let Some(loan) = array_loan {
                let path = leaf
                    .path
                    .iter()
                    .map(|step| match step {
                        Path::FixedIndex(index) => Ok(StructuralPathSegment::FixedIndex(*index)),
                        _ => Err(malformed()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                loan.scalar_storage(execution, &path, access == StructuralAccess::MutableBorrow)
            } else {
                execution.boundary_scalar_storage(referent, &leaf.path, scalar, access)
            }
        }

        pub(crate) fn capture(
            execution: &TerminalExecution,
            source: PlaceId,
            referent: &TerminalStructuralValue,
            array_loan: Option<&crate::storage::element_views::ElementView>,
            aggregate: Option<&crate::storage::reference::AggregateBacking>,
            access: StructuralAccess,
            leaves: &[StorageLeaf],
            normal_qualifications: &[crate::boundary::effects::normal_qualifications::CapturedNormalQualification],
            source_owner: Option<&TerminalStructuralValue>,
        ) -> Result<(Vec<TerminalBoundaryStorageValue>, Vec<PinnedScalar>), TerminalInterpretError>
        {
            let mut values = Vec::new();
            let mut endpoints = Vec::new();
            for leaf in leaves {
                if aggregate.is_some()
                    && !matches!(
                        leaf.kind,
                        StorageLeafKind::Scalar { .. } | StorageLeafKind::ByteArray { .. }
                    )
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "host aggregate re-view requires independently represented scalar leaves",
                    ));
                }
                let contents = match &leaf.kind {
                    StorageLeafKind::Case { case, .. } => TerminalStorageContents::Case(*case),
                    StorageLeafKind::Scalar {
                        scalar,
                        restriction,
                        stored_scalar,
                        ..
                    } => {
                        let (storage, logical, shared) = scalar_storage(
                            execution, referent, array_loan, aggregate, access, leaf,
                        )
                        .map_err(|error| {
                            error.in_argument_preparation("project captured host scalar")
                        })?;
                        if logical != *scalar {
                            return Err(malformed());
                        }
                        let presentation = StructuralRuntimePlace::from(referent);
                        let restoration_owner = if access == StructuralAccess::MutableBorrow {
                            captured_scalar_restoration_owner(&storage, presentation)?
                        } else {
                            presentation
                        };
                        if access == StructuralAccess::MutableBorrow {
                            execution
                                .validate_scalar_loan_restoration(
                                    &storage,
                                    &restoration_owner,
                                    normal_qualifications,
                                )
                                .map_err(|error| {
                                    error.in_argument_preparation(
                                        "preserve captured scalar source owner",
                                    )
                                })?;
                        }
                        let value = execution.read_primitive_storage(
                            source,
                            storage.clone(),
                            logical,
                            logical,
                            shared,
                        )?;
                        if restriction
                            .as_ref()
                            .is_some_and(|field| !value.satisfies_field(field))
                        {
                            return Err(malformed());
                        }
                        let original_restriction = execution
                            .captured_scalar_restriction(&storage, access, source_owner)
                            .map_err(|error| {
                                error.in_argument_preparation("retain captured scalar restriction")
                            })?;
                        if original_restriction.as_ref().is_some_and(|field| {
                            !satisfies_source_restriction(value, field, storage.encoding)
                        }) {
                            return Err(malformed());
                        }
                        endpoints.push(PinnedScalar {
                            path: leaf.path.clone(),
                            storage: storage.clone(),
                            scalar: logical,
                            shared,
                            original: value,
                            restriction: original_restriction,
                            restoration_owner,
                        });
                        TerminalStorageContents::Scalar {
                            value,
                            stored_scalar: storage.storage_scalar().unwrap_or(*stored_scalar),
                        }
                    }
                    StorageLeafKind::EmptyScalarArray {
                        destination,
                        structural_type,
                    } => {
                        let initialized = if let Some(loan) = array_loan {
                            let selected = terminal_semantics::canonical_structural_storage_path(
                                execution.structural_types().values(),
                                referent.structural_type,
                                &leaf.path,
                            )
                            .ok_or_else(malformed)?;
                            loan.initialized_empty_scalar_array(
                                execution,
                                &selected,
                                *structural_type,
                            )?
                        } else {
                            execution
                                .initialized_empty_scalar_array(destination, *structural_type)?
                        };
                        if !initialized {
                            return Err(malformed());
                        }
                        continue;
                    }
                    StorageLeafKind::Opaque { .. }
                    | StorageLeafKind::ByteField { .. }
                    | StorageLeafKind::ByteArray { .. } => continue,
                };
                values.push(TerminalBoundaryStorageValue {
                    path: leaf.path.clone(),
                    contents,
                });
            }
            Ok((values, endpoints))
        }
    }
    pub(crate) mod retirement {
        use crate::boundary::results::TerminalExecutionResult;
        use crate::boundary::structural_inputs::byte_arrays::storage::{
            ArrayStorage, ArrayStorageHome,
        };
        use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
        use crate::engine::execution::{
            CleanupContinuation, SuspendedCallResult, TerminalExecution,
        };
        use crate::engine::terminators::block_bindings::BlockBindings;
        use crate::storage::byte_sequences::binding::ByteSequenceBinding;
        use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
        use std::ops::Range;
        use terminal_psi::{ReferenceOriginProjection, StructuralPathSegment};

        #[derive(Clone, PartialEq, Eq)]
        enum Region {
            Subtree(StructuralRuntimePlace),
            Projection {
                root: StructuralRuntimePlace,
                path: Vec<ReferenceOriginProjection>,
            },
            Elements {
                collection: StructuralRuntimePlace,
                window: Range<usize>,
                path: Vec<ReferenceOriginProjection>,
            },
        }

        impl Region {
            fn root(&self) -> &StructuralRuntimePlace {
                match self {
                    Self::Subtree(root) | Self::Projection { root, .. } => root,
                    Self::Elements { collection, .. } => collection,
                }
            }

            fn may_retain_field_parent(&self, parent: &StructuralRuntimePlace) -> bool {
                let root = self.root();
                root.opaque_identity == parent.opaque_identity
                    && (root.path.starts_with(&parent.path)
                        || self.selected_suffix(parent).is_some())
            }

            fn selected_suffix<'region>(
                &'region self,
                place: &StructuralRuntimePlace,
            ) -> Option<&'region [ReferenceOriginProjection]> {
                let root = self.root();
                if place.opaque_identity != root.opaque_identity {
                    return None;
                }
                let mut storage = place.path.strip_prefix(root.path.as_slice())?;
                let path = match self {
                    Self::Subtree(_) => return Some(&[]),
                    Self::Projection { path, .. } => path,
                    Self::Elements { window, path, .. } => {
                        let (StructuralPathSegment::FixedIndex(index), suffix) =
                            storage.split_first()?
                        else {
                            return None;
                        };
                        if !usize::try_from(*index).is_ok_and(|index| window.contains(&index)) {
                            return None;
                        }
                        storage = suffix;
                        path
                    }
                };
                let mut projection = path.as_slice();
                loop {
                    let Some((selected, remaining)) = projection.split_first() else {
                        return Some(&[]);
                    };
                    let Some((actual, suffix)) = storage.split_first() else {
                        return Some(projection);
                    };
                    let matches = match selected {
                        ReferenceOriginProjection::Exact(selected) => selected == actual,
                        ReferenceOriginProjection::Element => {
                            matches!(actual, StructuralPathSegment::FixedIndex(_))
                        }
                    };
                    if !matches {
                        return None;
                    }
                    projection = remaining;
                    storage = suffix;
                }
            }
        }

        #[derive(Default)]
        pub(crate) struct RetainedStorage {
            regions: Vec<Region>,
            frame_arrays: Vec<(usize, semantic_vocabulary::PlaceId)>,
        }

        impl RetainedStorage {
            pub(crate) fn frame_array(
                &mut self,
                depth: usize,
                place: semantic_vocabulary::PlaceId,
            ) {
                if !self.frame_arrays.contains(&(depth, place)) {
                    self.frame_arrays.push((depth, place));
                }
            }

            pub(crate) fn primitive_home(
                &mut self,
                home: crate::storage::primitive_storage::PrimitiveStorageHome,
            ) {
                let (depth, place, _) = home.scalar_array_frame();
                self.frame_array(depth, place);
            }

            pub(crate) fn array_storage(&mut self, array: &ArrayStorage) {
                if let ArrayStorageHome::Frame { depth, place, .. } = array.home {
                    self.frame_array(depth, place);
                }
            }

            pub(crate) fn primitive_endpoint(&mut self, endpoint: &PrimitiveStorageEndpoint) {
                use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
                match endpoint {
                    PrimitiveStorageEndpoint::RepresentationByte(byte) => {
                        byte.view.retain_storage(self)
                    }
                    PrimitiveStorageEndpoint::Projection { fragments, .. } => {
                        for fragment in fragments {
                            self.primitive_endpoint(&fragment.storage.endpoint);
                        }
                    }
                    PrimitiveStorageEndpoint::OwnedArrayElement { home, .. } => {
                        self.primitive_home(*home)
                    }
                    PrimitiveStorageEndpoint::ArrayElement { array, .. } => {
                        self.array_storage(array)
                    }
                    _ => {}
                }
            }

            pub(crate) fn byte_binding(&mut self, binding: &ByteSequenceBinding) {
                use crate::storage::byte_sequences::binding::ByteSequenceBinding;
                match binding {
                    ByteSequenceBinding::MutableArray { array, .. } => self.array_storage(array),
                    ByteSequenceBinding::Representation(view) => view.retain_storage(self),
                    ByteSequenceBinding::Immutable(bytes) => {
                        if let Some(original) = bytes.original_array() {
                            self.array_storage(&original.array);
                        }
                    }
                    _ => {}
                }
            }

            fn insert(&mut self, region: Region) {
                if !self.regions.contains(&region) {
                    self.regions.push(region);
                }
            }

            pub(crate) fn value(&mut self, value: &TerminalStructuralValue) {
                self.place(StructuralRuntimePlace::from(value));
            }

            fn place(&mut self, place: StructuralRuntimePlace) {
                self.insert(Region::Subtree(place));
            }

            pub(crate) fn projection(
                &mut self,
                value: &TerminalStructuralValue,
                path: &[ReferenceOriginProjection],
            ) {
                if path.is_empty() {
                    self.value(value);
                } else {
                    self.insert(Region::Projection {
                        root: StructuralRuntimePlace::from(value),
                        path: path.to_vec(),
                    });
                }
            }

            pub(crate) fn elements(
                &mut self,
                collection: &TerminalStructuralValue,
                window: Range<usize>,
                path: &[ReferenceOriginProjection],
            ) {
                self.insert(Region::Elements {
                    collection: StructuralRuntimePlace::from(collection),
                    window,
                    path: path.to_vec(),
                });
            }

            pub(crate) fn aggregate_backing(
                &mut self,
                backing: &crate::storage::reference::AggregateBacking,
            ) {
                self.value(&backing.value);
                if let Some(home) = &backing.collection_home {
                    self.primitive_home(home.storage);
                }
                if let Some(loan) = &backing.array_loan {
                    loan.retain_storage(self);
                }
                if let Some(region) = &backing.byte_region {
                    self.primitive_endpoint(&region.endpoint);
                }
            }

            pub(crate) fn reference(
                &mut self,
                reference: &crate::storage::reference::ReferenceReferent,
            ) {
                self.value(&reference.view);
                if let Some(loan) = &reference.primitive_loan {
                    self.primitive_endpoint(&loan.storage.endpoint);
                }
                if let Some(region) = &reference.byte_region {
                    self.primitive_endpoint(&region.endpoint);
                }
                if let Some(binding) = &reference.byte_owner {
                    self.byte_binding(binding);
                }
                if let Some(view) = &reference.collection_loan {
                    view.retain_storage(self);
                }
            }

            fn continuation(&mut self, continuation: &CleanupContinuation) {
                match continuation {
                    CleanupContinuation::Return(returned) => {
                        if let TerminalExecutionResult::Structural(value) = &returned.value {
                            self.value(&value.value);
                        }
                        if let Some(binding) = &returned.byte_sequence {
                            self.byte_binding(binding);
                        }
                        if let Some(view) = &returned.element_view {
                            view.retain_storage(self);
                        }
                    }
                    CleanupContinuation::Successor { bindings, .. } => {
                        bindings.retain_storage(self)
                    }
                }
            }

            fn reference_is_retained(&self, place: &StructuralRuntimePlace) -> bool {
                self.regions
                    .iter()
                    .any(|region| region.selected_suffix(place).is_some_and(<[_]>::is_empty))
            }

            fn view_is_retained(&self, place: &StructuralRuntimePlace) -> bool {
                self.regions.iter().any(|region| {
                    region.selected_suffix(place).is_some_and(|suffix| {
                        suffix.is_empty()
                            || matches!(
                                suffix.first(),
                                Some(
                                    ReferenceOriginProjection::Element
                                        | ReferenceOriginProjection::Exact(
                                            StructuralPathSegment::FixedIndex(_)
                                        )
                                )
                            )
                    })
                })
            }
        }

        impl TerminalExecution<'_> {
            fn descriptor_cells_exist(&self, root: &StructuralRuntimePlace) -> bool {
                super::cells(&self.reference_referents, root)
                    .next()
                    .is_some()
                    || super::cells(&self.structural_element_view_fields, root)
                        .next()
                        .is_some()
            }

            pub(crate) fn queue_retired_descriptor_home(&mut self, root: StructuralRuntimePlace) {
                if self.descriptor_cells_exist(&root)
                    && !self.retired_descriptor_homes.contains(&root)
                {
                    self.retired_descriptor_homes.push(root);
                }
            }

            pub(crate) fn collect_retired_descriptor_storage(
                &mut self,
                continuation: Option<&CleanupContinuation>,
            ) {
                self.collect_retired_storage(continuation, None);
            }

            pub(crate) fn collect_retired_successor_storage(&mut self, bindings: &BlockBindings) {
                self.collect_retired_storage(None, Some(bindings));
            }

            fn collect_retired_storage(
                &mut self,
                continuation: Option<&CleanupContinuation>,
                bindings: Option<&BlockBindings>,
            ) {
                let has_retired_arrays = self.scalar_array_values.has_retired()
                    || self
                        .call_stack
                        .iter()
                        .any(|frame| frame.scalar_array_values.has_retired());
                if self.retired_descriptor_homes.is_empty() && !has_retired_arrays {
                    return;
                }
                self.retired_descriptor_homes = std::mem::take(&mut self.retired_descriptor_homes)
                    .into_iter()
                    .filter(|root| self.descriptor_cells_exist(root))
                    .collect();
                if self.retired_descriptor_homes.is_empty() && !has_retired_arrays {
                    return;
                }
                let mut retained = RetainedStorage::default();
                for value in self
                    .structural_values
                    .values()
                    .chain(self.linear_homes.values())
                {
                    retained.value(value);
                }
                for (place, backing) in &self.aggregate_argument_backings {
                    if self.structural_values.contains_key(place) {
                        retained.aggregate_backing(backing);
                    }
                }
                for occurrence in self.access_view_occurrences.values() {
                    retained.place(occurrence.referent.clone());
                }
                for (place, view) in &self.element_view_values {
                    if self.structural_values.contains_key(place) {
                        view.retain_storage(&mut retained);
                    }
                }
                for (place, loan) in &self.primitive_loan_values {
                    if self.structural_values.contains_key(place) {
                        retained.primitive_endpoint(&loan.storage.endpoint);
                    }
                }
                for (place, binding) in &self.byte_sequence_values {
                    if self.structural_values.contains_key(place) {
                        retained.byte_binding(binding);
                    }
                }
                for original in &self.private_entry_ram {
                    retained.value(&original.allocation);
                }
                for frame in &self.call_stack {
                    for value in frame
                        .structural_values
                        .values()
                        .chain(frame.linear_homes.values())
                    {
                        retained.value(value);
                    }
                    for (place, backing) in &frame.aggregate_argument_backings {
                        if frame.structural_values.contains_key(place) {
                            retained.aggregate_backing(backing);
                        }
                    }
                    for occurrence in frame.access_view_occurrences.values() {
                        retained.place(occurrence.referent.clone());
                    }
                    for (place, view) in &frame.element_view_values {
                        if frame.structural_values.contains_key(place) {
                            view.retain_storage(&mut retained);
                        }
                    }
                    for (place, loan) in &frame.primitive_loan_values {
                        if frame.structural_values.contains_key(place) {
                            retained.primitive_endpoint(&loan.storage.endpoint);
                        }
                    }
                    for (place, binding) in &frame.byte_sequence_values {
                        if frame.structural_values.contains_key(place) {
                            retained.byte_binding(binding);
                        }
                    }
                    match &frame.result {
                        SuspendedCallResult::Structural {
                            expected_reference_backings,
                            ..
                        } => {
                            for backing in expected_reference_backings.iter().flatten() {
                                backing.retain_storage(&mut retained);
                            }
                        }
                        SuspendedCallResult::NominalCleanups {
                            completed,
                            continuation,
                            ..
                        } => {
                            retained.value(&completed.1);
                            retained.continuation(continuation);
                        }
                        _ => {}
                    }
                }
                if let Some(continuation) = continuation {
                    retained.continuation(continuation);
                }
                if let Some(bindings) = bindings {
                    bindings.retain_storage(&mut retained);
                }
                if let Some(TerminalExecutionResult::Structural(value)) = &self.result {
                    retained.value(&value.value);
                }

                let mut cursor = 0;
                while let Some(region) = retained.regions.get(cursor).cloned() {
                    cursor += 1;
                    for (carrier, reference) in
                        super::cells(&self.reference_referents, region.root())
                    {
                        if region.selected_suffix(carrier).is_some_and(<[_]>::is_empty) {
                            retained.reference(reference);
                        }
                    }
                    let field_namespace =
                        <StructuralRuntimePlace as super::StorageKey>::identity_start(
                            region.root().opaque_identity,
                        );
                    for (field, binding) in
                        super::cells(&self.structural_byte_sequence_fields, &field_namespace)
                    {
                        if region.may_retain_field_parent(&field.parent) {
                            retained.byte_binding(binding);
                        }
                    }
                    for (carrier, binding) in
                        super::cells(&self.structural_byte_descriptors, region.root())
                    {
                        if region.selected_suffix(carrier).is_some_and(<[_]>::is_empty) {
                            retained.byte_binding(binding);
                        }
                    }
                    for (carrier, view) in
                        super::cells(&self.structural_element_view_fields, region.root())
                    {
                        if let Some(suffix) = region.selected_suffix(carrier) {
                            view.retain_projection_storage(&mut retained, suffix);
                        }
                    }
                }
                let current_depth = self.call_stack.len();
                self.scalar_array_values.collect_retired(|place| {
                    retained.frame_arrays.contains(&(current_depth, place))
                });
                for (depth, frame) in self.call_stack.iter_mut().enumerate() {
                    frame
                        .scalar_array_values
                        .collect_retired(|place| retained.frame_arrays.contains(&(depth, place)));
                }
                let candidates = std::mem::take(&mut self.retired_descriptor_homes);
                for root in candidates {
                    let references: Vec<_> = super::cells(&self.reference_referents, &root)
                        .filter(|(carrier, _)| !retained.reference_is_retained(carrier))
                        .map(|(carrier, _)| carrier.clone())
                        .collect();
                    let views: Vec<_> = super::cells(&self.structural_element_view_fields, &root)
                        .filter(|(carrier, _)| !retained.view_is_retained(carrier))
                        .map(|(carrier, _)| carrier.clone())
                        .collect();
                    for carrier in references {
                        self.reference_referents.remove(&carrier);
                    }
                    for carrier in views {
                        self.structural_element_view_fields.remove(&carrier);
                    }
                    if self.descriptor_cells_exist(&root) {
                        self.retired_descriptor_homes.push(root);
                    }
                }
            }
        }
    }
    pub(crate) use retirement::RetainedStorage;
    pub(crate) mod original_allocation {
        use crate::{
            boundary::errors::{TerminalInterpretError, malformed},
            boundary::values::TerminalStructuralValue,
            engine::execution::TerminalExecution,
        };
        use semantic_vocabulary::{MachineId, PlaceId};
        use terminal_psi::StructuralAccess;

        pub(crate) struct OriginalAllocation {
            pub(crate) depth: usize,
            pub(crate) machine: MachineId,
            pub(crate) place: PlaceId,
            pub(crate) value: TerminalStructuralValue,
        }

        impl TerminalExecution<'_> {
            pub(crate) fn original_owned_allocation(
                &self,
                source: &TerminalStructuralValue,
            ) -> Result<OriginalAllocation, TerminalInterpretError> {
                let homes = self
                    .call_stack
                    .iter()
                    .enumerate()
                    .map(|(depth, frame)| (depth, frame.current_machine, &frame.structural_values))
                    .chain(std::iter::once((
                        self.call_stack.len(),
                        self.current_machine,
                        &self.structural_values,
                    )));
                let mut selected: Option<OriginalAllocation> = None;
                let mut competing = false;
                for (depth, machine_id, values) in homes {
                    let machine = self.program.machine(machine_id)?;
                    for (place, allocation) in values {
                        if allocation.opaque_identity != source.opaque_identity
                            || !source.path.starts_with(&allocation.path)
                        {
                            continue;
                        }
                        let owned = machine
                            .structural_parameter(*place)
                            .is_some_and(|parameter| {
                                parameter.access == StructuralAccess::Owned
                                    && parameter.structural_type == allocation.structural_type
                            })
                            || machine.producing_operation(*place).is_some_and(
                                |(operation, ty)| {
                                    operation.result.structural().is_some_and(|result| {
                                        result.structural_type == ty
                                            && ty == allocation.structural_type
                                    })
                                },
                            );
                        if !owned {
                            continue;
                        }
                        if let Some(previous) = &selected {
                            match allocation.path.len().cmp(&previous.value.path.len()) {
                                std::cmp::Ordering::Greater => continue,
                                std::cmp::Ordering::Equal => {
                                    competing = true;
                                    continue;
                                }
                                std::cmp::Ordering::Less => {}
                            }
                        }
                        selected = Some(OriginalAllocation {
                            depth,
                            machine: machine_id,
                            place: *place,
                            value: allocation.clone(),
                        });
                        competing = false;
                    }
                }
                if competing {
                    return Err(malformed());
                }
                selected.ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                    "storage source has no live supplying allocation",
                ))
            }
        }
    }

    pub(crate) trait StorageKey: Ord + Clone {
        fn identity_start(identity: u64) -> Self;
        fn subtree_start(root: &StructuralRuntimePlace) -> Self;
        fn parent(&self) -> &StructuralRuntimePlace;
    }
    impl StorageKey for StructuralRuntimePlace {
        fn identity_start(opaque_identity: u64) -> Self {
            Self {
                opaque_identity,
                path: Vec::new(),
            }
        }
        fn subtree_start(root: &StructuralRuntimePlace) -> Self {
            root.clone()
        }
        fn parent(&self) -> &StructuralRuntimePlace {
            self
        }
    }
    impl StorageKey for StructuralScalarRuntimeField {
        fn identity_start(identity: u64) -> Self {
            Self::subtree_start(&StructuralRuntimePlace::identity_start(identity))
        }
        fn subtree_start(root: &StructuralRuntimePlace) -> Self {
            Self::member(root.clone(), StructuralFieldId::new(1).unwrap())
        }
        fn parent(&self) -> &StructuralRuntimePlace {
            &self.parent
        }
    }
    impl StorageKey for StructuralByteSequenceRuntimeField {
        fn identity_start(identity: u64) -> Self {
            Self::subtree_start(&StructuralRuntimePlace::identity_start(identity))
        }
        fn subtree_start(root: &StructuralRuntimePlace) -> Self {
            Self {
                parent: root.clone(),
                namespace: crate::boundary::values::StructuralFieldNamespace::Member,
                field: StructuralFieldId::new(1).unwrap(),
            }
        }
        fn parent(&self) -> &StructuralRuntimePlace {
            &self.parent
        }
    }
    pub(crate) fn cells<'a, K: StorageKey + 'a, V: 'a>(
        map: &'a impl std::borrow::Borrow<BTreeMap<K, V>>,
        root: &StructuralRuntimePlace,
    ) -> impl Iterator<Item = (&'a K, &'a V)> {
        let lower = Included(K::subtree_start(root));
        let upper = root
            .opaque_identity
            .checked_add(1)
            .map_or(Unbounded, |identity| Excluded(K::identity_start(identity)));
        map.borrow()
            .range((lower, upper))
            .take_while(move |(key, _)| key.parent().path.starts_with(&root.path))
    }
    #[cfg(test)]
    fn prune<K: StorageKey, V>(map: &mut BTreeMap<K, V>, root: &StructuralRuntimePlace) {
        let keys: Vec<_> = cells(map, root).map(|(key, _)| key.clone()).collect();
        for key in keys {
            map.remove(&key);
        }
    }

    pub(crate) fn prune_rows<K: StorageKey, V>(
        map: &mut crate::storage::runtime_rows::RuntimeRows<K, V>,
        root: &StructuralRuntimePlace,
    ) {
        let keys: Vec<_> = cells(map, root).map(|(key, _)| key.clone()).collect();
        for key in keys {
            map.remove(&key);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::boundary::values::StructuralFieldNamespace;
        use terminal_psi::StructuralPathSegment;
        #[test]
        fn byte_cells_keep_both_namespaces_under_empty_root() {
            let root = StructuralRuntimePlace::identity_start(u64::MAX);
            let mut map = BTreeMap::new();
            for namespace in [
                StructuralFieldNamespace::Member,
                StructuralFieldNamespace::CasePayload,
            ] {
                map.insert(
                    StructuralByteSequenceRuntimeField {
                        parent: root.clone(),
                        namespace,
                        field: StructuralFieldId::new(1).unwrap(),
                    },
                    namespace,
                );
            }
            assert_eq!(cells(&map, &root).count(), 2);
            prune(&mut map, &root);
            assert!(map.is_empty());
        }
        #[test]
        fn selects_and_prunes_only_the_exact_subtree_at_identity_extremes() {
            let field = StructuralFieldId::new(1).unwrap();
            for identity in [0, u64::MAX] {
                let root = StructuralRuntimePlace {
                    opaque_identity: identity,
                    path: vec![StructuralPathSegment::FixedIndex(0)],
                };
                let sibling = StructuralRuntimePlace {
                    opaque_identity: identity,
                    path: vec![],
                };
                let unrelated = StructuralRuntimePlace {
                    opaque_identity: if identity == 0 { 1 } else { u64::MAX - 1 },
                    path: root.path.clone(),
                };
                let mut map = BTreeMap::new();
                map.insert(StructuralScalarRuntimeField::member(root.clone(), field), 1);
                map.insert(
                    StructuralScalarRuntimeField {
                        parent: root.clone(),
                        namespace: StructuralFieldNamespace::CasePayload,
                        field,
                    },
                    2,
                );
                map.insert(StructuralScalarRuntimeField::member(sibling, field), 3);
                map.insert(StructuralScalarRuntimeField::member(unrelated, field), 4);
                assert_eq!(
                    cells(&map, &root)
                        .map(|(_, value)| *value)
                        .collect::<Vec<_>>(),
                    vec![1, 2]
                );
                prune(&mut map, &root);
                assert_eq!(
                    map.values()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>(),
                    [3, 4].into_iter().collect()
                );
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum HostStructuralPayload {
        Opaque(u64),
        Materialized(u64),
        Modified,
    }

    impl crate::engine::execution::TerminalExecution<'_> {
        pub(crate) fn require_guest_storage_mutation(
            &self,
            changed: &StructuralRuntimePlace,
        ) -> Result<(), crate::boundary::errors::TerminalInterpretError> {
            if self.component_lent_host_atoms.keys().any(|lender| {
                lender.opaque_identity == changed.opaque_identity
                    && (changed.path.starts_with(&lender.path)
                        || lender.path.starts_with(&changed.path))
            }) {
                return Err(
                    crate::boundary::errors::TerminalInterpretError::UnsupportedSemanticVariant(
                        "opaque component loan lacks original guest storage for mutation",
                    ),
                );
            }
            Ok(())
        }

        pub(crate) fn invalidate_stored_provenance(&mut self, changed: &StructuralRuntimePlace) {
            self.provider_storage.completed_profiles.retain(|home, _| {
                home.opaque_identity != changed.opaque_identity
                    || (!changed.path.starts_with(&home.path)
                        && !home.path.starts_with(&changed.path))
            });
            for (home, payload) in self.structural_host_atoms.iter_mut() {
                if home.opaque_identity == changed.opaque_identity
                    && changed.path.starts_with(&home.path)
                {
                    *payload = HostStructuralPayload::Modified;
                }
            }
        }

        pub(crate) fn export_host_atom(
            &self,
            value: &mut crate::boundary::values::TerminalStructuralValue,
        ) -> Result<(), crate::boundary::errors::TerminalInterpretError> {
            let home = StructuralRuntimePlace::from(&*value);
            if let Some(
                HostStructuralPayload::Opaque(identity)
                | HostStructuralPayload::Materialized(identity),
            ) = self.structural_host_atoms.get(&home)
            {
                value.opaque_identity = *identity;
                value.path.clear();
                return Ok(());
            }
            if self.structural_host_atoms.iter().any(|(parent, payload)| {
                parent.opaque_identity == home.opaque_identity
                    && home.path.starts_with(&parent.path)
                    && matches!(payload, HostStructuralPayload::Modified)
            }) {
                return Err(
                    crate::boundary::errors::TerminalInterpretError::UnsupportedSemanticVariant(
                        "modified host structural payload requires materialized host export",
                    ),
                );
            }
            Ok(())
        }

        pub(crate) fn forward_host_atom(
            &self,
            value: &mut crate::boundary::values::TerminalStructuralValue,
        ) -> Option<StructuralRuntimePlace> {
            let home = StructuralRuntimePlace::from(&*value);
            if let Some(HostStructuralPayload::Opaque(identity)) =
                self.structural_host_atoms.get(&home)
            {
                value.opaque_identity = *identity;
                value.path.clear();
                Some(home)
            } else {
                None
            }
        }

        pub(crate) fn reclaim_orphan_scalar_records(&mut self) {
            use crate::engine::execution::SuspendedCallResult;
            use std::collections::BTreeSet;
            let Some(cursor) = self.local_structural_identities.cursor() else {
                return;
            };
            let collect_scalars = cursor >= self.next_scalar_collection_identity;
            let collect_atoms = self
                .structural_host_atoms
                .len()
                .saturating_add(self.provider_storage.completed_profiles.len())
                >= self.next_host_atom_collection_size;
            if !collect_scalars && !collect_atoms {
                return;
            }
            if collect_scalars {
                self.next_scalar_collection_identity = cursor.saturating_add(1024);
            }
            let hidden_backings = !self.primitive_loan_values.is_empty()
                || !self.byte_sequence_values.is_empty()
                || !self.element_view_values.is_empty()
                || !self.structural_element_view_fields.is_empty()
                || !self.structural_byte_descriptors.is_empty()
                || self
                    .aggregate_argument_backings
                    .values()
                    .any(|backing| backing.byte_region.is_some())
                || self.reference_referents.values().any(|referent| {
                    referent.primitive_loan.is_some() || referent.byte_region.is_some()
                })
                || self.call_stack.iter().any(|frame| {
                    !frame.primitive_loan_values.is_empty()
                        || frame
                            .aggregate_argument_backings
                            .values()
                            .any(|backing| backing.byte_region.is_some())
                        || !frame.byte_sequence_values.is_empty()
                        || !frame.element_view_values.is_empty()
                        || matches!(frame.result, SuspendedCallResult::NominalCleanups { .. })
                });
            let defer_scalars = !collect_scalars || self.structural_scalar_fields.len() < 4096;
            if hidden_backings || (defer_scalars && !collect_atoms) {
                return;
            }
            let mut roots = BTreeSet::new();
            roots.extend(
                self.borrowed_entry_storage
                    .iter()
                    .map(|(_, _, value)| value.opaque_identity),
            );
            for occurrence in self.access_view_occurrences.values() {
                roots.insert(occurrence.referent.opaque_identity);
            }
            for value in self.structural_values.values().chain(
                self.aggregate_argument_backings
                    .values()
                    .map(|backing| &backing.value),
            ) {
                roots.insert(value.opaque_identity);
            }
            for frame in &self.call_stack {
                for occurrence in frame.access_view_occurrences.values() {
                    roots.insert(occurrence.referent.opaque_identity);
                }
                for value in frame.structural_values.values().chain(
                    frame
                        .aggregate_argument_backings
                        .values()
                        .map(|backing| &backing.value),
                ) {
                    roots.insert(value.opaque_identity);
                }
                if let SuspendedCallResult::Structural {
                    expected_reference_backings,
                    ..
                } = &frame.result
                {
                    for value in expected_reference_backings.iter().flatten() {
                        roots.insert(value.value.opaque_identity);
                    }
                }
            }
            if let Some(crate::boundary::results::TerminalExecutionResult::Structural(result)) =
                &self.result
            {
                roots.insert(
                    result
                        .observation_home
                        .as_ref()
                        .map_or(result.value.opaque_identity, |home| home.opaque_identity),
                );
            }
            for referent in self.reference_referents.values() {
                roots.insert(referent.view.opaque_identity);
            }
            self.structural_host_atoms
                .retain(|home, _| roots.contains(&home.opaque_identity));
            self.provider_storage
                .completed_profiles
                .retain(|home, _| roots.contains(&home.opaque_identity));
            self.next_host_atom_collection_size = self
                .structural_host_atoms
                .len()
                .saturating_add(self.provider_storage.completed_profiles.len())
                .saturating_add(1024);
            if defer_scalars {
                return;
            }
            for key in self
                .reference_referents
                .keys()
                .chain(self.structural_primitive_storage.keys())
                .chain(self.structural_primitive_entry_places.values())
                .chain(self.structural_byte_storage.keys())
                .chain(self.empty_array_storage.rows.keys())
                .chain(self.structural_byte_descriptors.keys())
                .chain(self.structural_cases.keys())
            {
                roots.insert(key.opaque_identity);
            }
            for key in self.structural_byte_sequence_fields.keys() {
                roots.insert(key.parent.opaque_identity);
            }
            roots.extend(
                self.private_entry_ram
                    .iter()
                    .map(|original| original.allocation.opaque_identity),
            );
            self.structural_scalar_fields
                .retain(|key, _| roots.contains(&key.parent.opaque_identity));
        }
    }
}
