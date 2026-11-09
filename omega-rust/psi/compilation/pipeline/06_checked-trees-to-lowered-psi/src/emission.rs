/*
Shared operation and store emission across Unit and scalar bodies: boolean control and short-circuit
staging, case payload dispatch, claim bindings, descriptor residences, primitive and structural
stores, and runtime-indexed element writes.
*/

pub(crate) use crate::emission_cleanup::{
    affine_cleanup, borrowed_window, continuation_specialization,
};
pub(crate) use crate::operation_emission;

pub(crate) use checked_trees::types::PrimitiveType;
pub(crate) use checked_trees::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitStructuralPathSegment,
};
pub(crate) use semantic_vocabulary::{
    BlockId, EdgeId, PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};
pub(crate) use terminal_psi::{
    OperationKind, OperationResult, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, ValueDeclaration,
};

pub(crate) use crate::emission::expression_validation::{
    direct_expression_contains_short_circuit, validate_direct_parameter_types,
};
pub(crate) use crate::emission::operation_emission::buffer::scalar_result_declaration;
pub(crate) use crate::emission::operation_emission::emit_direct_expression;
pub(crate) use crate::emission::scalar_types::terminal_scalar_type;
pub(crate) use crate::shared::lowering_error::{LoweringError, unsupported};
pub(crate) use crate::shared::terminal_identities::{
    allocate_dense, obligation_id, place_id, structural_type_id, value_id,
};

pub(crate) mod call_qualifications {

    use crate::emission::reference_formations::ReferenceFormations;
    use checked_trees::CheckedTrees;
    use symbols::SymbolHandle;

    use crate::emission::operation_emission::buffer::SourceCallCoordinate;
    use crate::shared::lowering_error::LoweringError;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DeclarationRepresentation {
        Complete,
        RequiresPublication(DeclarationRepresentationRefusal),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DeclarationRepresentationRefusal {
        NotCallEnsures,
        NotDomainMembership,
        NoStoragePlace,
        NoRootType,
        NoLeafType,
        NoExactDeclaredDomain {
            domain: SymbolHandle,
            instance: language_semantics::SemanticDomainId,
            same_symbol: bool,
            same_instance: bool,
        },
        DuplicateDeclaredDomain,
        NoDomainInstance,
        IncompletePredicateTheory,
        EstablishmentRoute,
        RequiredSubjectMembership,
        ContentCustody,
        NotDeclaredByteField,
        UnknownDeclaredBytePredicate,
        MissingDeclaredBytePredicate,
        NotScalarPredicate,
        NoScalarRestriction,
        NoScalarDomainPredicate,
        ScalarPredicateNotEntailed,
    }

    impl DeclarationRepresentation {
        pub(crate) fn is_complete(self) -> bool {
            matches!(self, Self::Complete)
        }
    }

    pub(crate) fn for_call<'a>(
        checked: &'a CheckedTrees,
        machine: SymbolHandle,
        coordinate: SourceCallCoordinate,
        mut has_home: impl FnMut(facts::PlaceRoot) -> bool + 'a,
    ) -> impl Iterator<Item = &'a facts::Fact> {
        let point = facts::ProgramPoint::CallEnsures {
            machine_symbol: machine,
            state_symbol: coordinate.state,
            statement_index: coordinate.statement_index,
            call_ordinal: coordinate.call_ordinal,
        };
        checked
            .facts
            .semantic
            .facts
            .at_point(point)
            .into_iter()
            .filter(move |fact| {
                fact.origin == facts::FactOrigin::CallEnsures
                    && fact.point == point
                    && fact.place.as_place().is_some_and(|place| {
                        has_home(checked.facts.semantic.places.get(place).root)
                    })
                    && fact.payload.domain_membership().is_some()
            })
    }

    pub(crate) fn declaration_represents(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        fact: &facts::Fact,
    ) -> Result<DeclarationRepresentation, LoweringError> {
        use DeclarationRepresentation::{Complete, RequiresPublication};
        use DeclarationRepresentationRefusal as Refusal;
        let program = &checked.typed;
        let facts::ProgramPoint::CallEnsures {
            state_symbol,
            statement_index,
            ..
        } = fact.point
        else {
            return Ok(RequiresPublication(Refusal::NotCallEnsures));
        };
        let Some((_, _, domain_symbol, semantic_domain)) = fact.payload.domain_membership() else {
            return Ok(RequiresPublication(Refusal::NotDomainMembership));
        };
        let Some(place) = fact.place.as_place() else {
            return Ok(RequiresPublication(Refusal::NoStoragePlace));
        };
        let place = checked.facts.semantic.places.get(place);
        let segments = checked
            .facts
            .semantic
            .place_segments
            .span_or_empty(place.segments);
        let Some(root_type) = validation::canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        ) else {
            return Ok(RequiresPublication(Refusal::NoRootType));
        };
        let Some(reference) =
            validation::qualification_storage_type_reference(program, root_type, segments)
        else {
            return Ok(RequiresPublication(Refusal::NoLeafType));
        };
        let constraints =
            validation::field_domains::declared_domain_constraint_records(program, reference);
        let mut same_symbol = false;
        let mut same_instance = false;
        let mut selected = None;
        for constraint in constraints {
            same_symbol |= constraint.symbol == domain_symbol;
            same_instance |= constraint.semantic_id == semantic_domain;
            if constraint.symbol == domain_symbol
                && constraint.semantic_id == semantic_domain
                && selected.replace(constraint).is_some()
            {
                return Ok(RequiresPublication(Refusal::DuplicateDeclaredDomain));
            }
        }
        let Some(constraint) = selected else {
            return Ok(RequiresPublication(Refusal::NoExactDeclaredDomain {
                domain: domain_symbol,
                instance: semantic_domain,
                same_symbol,
                same_instance,
            }));
        };
        let Some(definition) =
            program.domain_definition_for_instance(domain_symbol, semantic_domain)
        else {
            return Ok(RequiresPublication(Refusal::NoDomainInstance));
        };
        if !validation::field_domains::domain_predicate_theory_is_complete(program, definition) {
            return Ok(RequiresPublication(Refusal::IncompletePredicateTheory));
        }
        if !definition.establishment_routes.is_empty() {
            return Ok(RequiresPublication(Refusal::EstablishmentRoute));
        }
        if !typed_trees::domain::required_subject_memberships(program, definition)
            .is_some_and(|required| required.is_empty())
        {
            return Ok(RequiresPublication(Refusal::RequiredSubjectMembership));
        }
        if checked
            .facts
            .qualifications
            .content
            .for_semantic_domain(semantic_domain)
            .is_some()
        {
            return Ok(RequiresPublication(Refusal::ContentCustody));
        }
        if let Some(predicate) =
            validation::field_domains::domain_byte_predicate(program, domain_symbol)
        {
            if !matches!(segments.last(), Some(facts::PlaceSegment::Field { .. }))
                || !matches!(
                    constraint.subject,
                    typed_trees::types::DomainConstraintSubject::Declared
                )
                || !matches!(
                    program.type_reference_table.type_reference(
                        validation::peel_declared_constraints(program, reference)
                    ),
                    typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
                        if checked.primitive_type_reference(*element_type)
                            == Some(typed_trees::types::PrimitiveType::U8)
                )
                || validation::stores_reference_shape(program, reference, &[])
            {
                return Ok(RequiresPublication(Refusal::NotDeclaredByteField));
            }
            let declared =
                typed_trees::byte_predicates::type_reference_domain_predicates(program, reference);
            if !declared.iter().all(|(_, predicate)| predicate.is_some()) {
                return Ok(RequiresPublication(Refusal::UnknownDeclaredBytePredicate));
            }
            return Ok(
                if declared
                    .iter()
                    .any(|(_, candidate)| *candidate == Some(predicate))
                {
                    Complete
                } else {
                    RequiresPublication(Refusal::MissingDeclaredBytePredicate)
                },
            );
        }
        let Some(primitive) = checked.primitive_type_reference(reference) else {
            return Ok(RequiresPublication(Refusal::NotScalarPredicate));
        };
        let Some(restriction) =
            crate::expression_preparation::qualifications::retained_scalar_restriction(
                checked, reference,
            )
        else {
            return Ok(RequiresPublication(Refusal::NoScalarRestriction));
        };
        let Some(predicate) = validation::scalar_domain_value_predicate(
            program,
            constraint,
            checked_trees::CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: primitive,
            },
        ) else {
            return Ok(RequiresPublication(Refusal::NoScalarDomainPredicate));
        };
        let required = crate::shared::structural_types::lower_field_type(
            identities,
            checked,
            &checked_trees::CheckedUnitStructuralFieldType::RefinedScalar {
                primitive,
                predicate,
            },
            &[],
        )?;
        let declared = crate::shared::structural_types::lower_field_type(
            identities,
            checked,
            restriction,
            &[],
        )?;
        Ok(
            if terminal_semantics::scalar_field_entails(&declared, &required) {
                Complete
            } else {
                RequiresPublication(Refusal::ScalarPredicateNotEntailed)
            },
        )
    }

    pub(crate) struct CallSourceRequirements<'checked> {
        sources: Vec<(facts::PlaceRoot, &'checked [facts::PlaceSegment], usize)>,
    }

    impl<'checked> CallSourceRequirements<'checked> {
        pub(crate) fn prepare(
            checked: &'checked CheckedTrees,
            machine: SymbolHandle,
            coordinate: SourceCallCoordinate,
            formations: &ReferenceFormations,
        ) -> Self {
            let borrow = &checked.facts.borrow;
            let mut sources = Vec::new();
            if let Some(call) = borrow.call_at(
                machine,
                coordinate.state,
                coordinate.statement_index,
                coordinate.call_ordinal,
            ) {
                let call = borrow.calls.get(call);
                let retained = call
                    .input_reference_loans
                    .map_or(&[][..], |span| borrow.loan_suppliers.span_or_empty(span));
                for access in borrow.argument_accesses.span_or_empty(call.accesses) {
                    if let Some(index) =
                        formations.find(coordinate.statement_index, access.expression)
                    {
                        sources.push((
                            access.root,
                            borrow.access_segments.span_or_empty(access.segments),
                            index,
                        ));
                    }
                }
                for loan in retained {
                    let loan = borrow.loans.get(*loan);
                    if let Some(index) =
                        formations.find(loan.statement_index, loan.formation_expression)
                    {
                        sources.push((
                            loan.root,
                            borrow.access_segments.span_or_empty(loan.segments),
                            index,
                        ));
                    }
                }
            }
            Self { sources }
        }

        pub(crate) fn represents(
            &self,
            checked: &CheckedTrees,
            formations: &ReferenceFormations,
            fact: &facts::Fact,
        ) -> bool {
            let Some((_, _, _, domain)) = fact.payload.domain_membership() else {
                return false;
            };
            let Some(place) = fact.place.as_place() else {
                return false;
            };
            let place = checked.facts.semantic.places.get(place);
            let segments = checked
                .facts
                .semantic
                .place_segments
                .span_or_empty(place.segments);
            self.sources.iter().any(|(root, prefix, formation)| {
                if *root != place.root || !segments.starts_with(prefix) {
                    return false;
                }
                let selected = &segments[prefix.len()..];
                formations
                    .requirements(*formation)
                    .iter()
                    .any(|requirement| {
                        requirement.semantic_domain == domain
                            && requirement.path.len() == selected.len()
                            && requirement
                                .path
                                .iter()
                                .zip(selected)
                                .all(|(required, actual)| {
                                    required == actual
                                        || (*required
                                            == validation::field_domains::WHOLE_ELEMENT_EXTENT
                                            && matches!(
                                                actual,
                                                facts::PlaceSegment::FixedIndex { .. }
                                                    | facts::PlaceSegment::Index { .. }
                                            ))
                                })
                    })
            })
        }
    }
}
pub(crate) mod claim_bindings {
    use crate::shared::checked_facts::flow_facts;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::{allocate_dense, claim_id, lookup_claim_id};
    use language_semantics::PermissionClaimIdentity;
    use semantic_vocabulary::ClaimId;

    pub(crate) mod selections {

        use super::{ClaimBindings, ClaimEdgeBinding};
        use crate::shared::checked_facts::flow_facts;
        use crate::shared::lowering_error::{LoweringError, unsupported};
        use crate::shared::terminal_identities::lookup_claim_id;
        use language_semantics::PermissionClaimIdentity;
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;

        #[derive(Clone, Copy, Debug)]
        pub(crate) struct SelectedDecision {
            pub(crate) expression: ExpressionHandle,
            pub(crate) decision: usize,
            pub(crate) selected: bool,
        }

        pub(crate) fn reserve(
            checked: &checked_trees::CheckedTrees,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: u32,
            expression: ExpressionHandle,
            decision: usize,
            claims: &mut ClaimBindings<'_>,
        ) -> Result<Vec<PermissionClaimIdentity>, LoweringError> {
            let destinations =
                destinations(checked, machine, state, statement, expression, decision)?;
            for destination in &destinations {
                claims.reserve_checked_binding(*destination)?;
            }
            Ok(destinations)
        }

        pub(crate) fn destinations(
            checked: &checked_trees::CheckedTrees,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: u32,
            expression: ExpressionHandle,
            decision: usize,
        ) -> Result<Vec<PermissionClaimIdentity>, LoweringError> {
            let mut destinations = Vec::new();
            for (_, binding) in checked.facts.flow.ownership.selection_place_bindings.iter() {
                if binding.machine != machine
                    || binding.state != state
                    || binding.statement_ordinal != statement as usize
                    || binding.expression != expression
                    || binding.decision != decision
                    || binding.selected == binding.remaining
                {
                    continue;
                }
                destinations.push(binding.destination.claim_identity);
            }
            Ok(destinations)
        }

        pub(crate) fn edge_bindings(
            checked: &checked_trees::CheckedTrees,
            destinations: &[PermissionClaimIdentity],
            route: &[SelectedDecision],
            edge: semantic_vocabulary::EdgeId,
            claims: &ClaimBindings<'_>,
        ) -> Result<Vec<ClaimEdgeBinding>, LoweringError> {
            let mut bindings = Vec::new();
            for destination in destinations {
                let mut incoming = *destination;
                let mut visited = Vec::new();
                loop {
                    if visited.contains(&incoming) {
                        return unsupported("selected claim arrivals form a cycle");
                    }
                    visited.push(incoming);
                    let arrival = flow_facts(checked)
                        .ownership
                        .selection_place_bindings
                        .iter()
                        .find_map(|(_, binding)| {
                            if binding.selected == binding.remaining
                                || binding.destination.claim_identity != incoming
                            {
                                return None;
                            }
                            route
                                .iter()
                                .find(|decision| {
                                    decision.expression == binding.expression
                                        && decision.decision == binding.decision
                                })
                                .map(|decision| (binding, decision.selected))
                        });
                    let Some((binding, selected)) = arrival else {
                        break;
                    };
                    incoming = if selected {
                        binding.selected.claim_identity
                    } else {
                        binding.remaining.claim_identity
                    };
                }
                if incoming == *destination {
                    return unsupported("selection edge has no checked incoming claim alternative");
                }
                bindings.push(ClaimEdgeBinding {
                    edge,
                    destination: lookup_claim_id(claims.bindings(), *destination)?,
                    source: lookup_claim_id(claims.bindings(), incoming)?,
                });
            }
            Ok(bindings)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ClaimIntroduction {
        pub(crate) operation: semantic_vocabulary::OperationId,
        pub(crate) claim: ClaimId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ClaimEdgeBinding {
        pub(crate) edge: semantic_vocabulary::EdgeId,
        pub(crate) destination: ClaimId,
        pub(crate) source: ClaimId,
    }

    pub(crate) enum ClaimBindings<'f> {
        Growable {
            bindings: Vec<(PermissionClaimIdentity, ClaimId)>,
            next_claim: u64,
            completed_values: Vec<CompletedValueClaim>,
        },
        Fixed(PublishedClaims<'f>),
    }

    #[derive(Clone, Copy)]
    pub(crate) struct PublishedClaims<'f> {
        pub(crate) bindings: &'f [(PermissionClaimIdentity, ClaimId)],
        pub(crate) unreceived_results: &'f [UnreceivedResultClaim],
        pub(crate) completed_values: &'f [CompletedValueClaim],
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct UnreceivedResultClaim {
        pub(crate) state: symbols::SymbolHandle,
        pub(crate) coordinate: checked_trees::CheckedUnitCallCoordinate,
        pub(crate) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        pub(crate) claim: ClaimId,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CompletedValueClaim {
        pub(crate) value: checked_trees::CheckedStructuralValueHandle,
        pub(crate) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        pub(crate) claim: ClaimId,
    }

    pub(crate) fn completed_value_paths(
        checked: &checked_trees::CheckedTrees,
        value: checked_trees::CheckedStructuralValueHandle,
    ) -> Result<Vec<Vec<checked_trees::CheckedUnitStructuralPathSegment>>, LoweringError> {
        let node = checked.facts.values.structural_values.nodes.get(value);
        validation::linear_claim_frontier(&checked.typed, node.type_reference)
            .iter()
            .map(|claim| {
                validation::structural_claim_path(&checked.typed, node.type_reference, &claim.path)
                    .map_err(LoweringError::Unsupported)
            })
            .collect()
    }

    pub(crate) fn seal_block_entry_claims(
        claims: &mut [terminal_psi::EntryClaim],
    ) -> Result<(), LoweringError> {
        claims.sort_by_key(|binding| binding.claim);
        if claims.windows(2).any(|pair| pair[0].claim == pair[1].claim) {
            return unsupported("block arrival namespace repeats a claim name");
        }
        Ok(())
    }

    pub(crate) fn seal_receiving_claims(
        operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
        place: semantic_vocabulary::PlaceId,
        claims: &mut [terminal_psi::StructuralResultClaimBinding],
        common: Option<&[terminal_psi::StructuralResultClaimBinding]>,
    ) -> Result<(), LoweringError> {
        if !claims.is_empty() && common.is_none() {
            return unsupported("owned join has no completed predecessor custody");
        }
        for claim in claims.iter_mut() {
            if let Some(incoming) = common
                .unwrap_or(&[])
                .iter()
                .find(|incoming| incoming.path == claim.path)
                && incoming.claim != claim.claim
            {
                let old = claim.claim;
                if operations
                    .block_entry_claims
                    .iter()
                    .filter(|row| row.claim == old && row.input == place && row.path == claim.path)
                    .count()
                    != 1
                {
                    return unsupported("completed value name has no unique reserved join slot");
                }
                operations.block_entry_claims.retain(|row| {
                    !(row.claim == old && row.input == place && row.path == claim.path)
                });
                claim.claim = incoming.claim;
            }
        }
        claims.sort();
        Ok(())
    }

    impl ClaimBindings<'_> {
        pub(crate) fn new(
            bindings: Vec<(PermissionClaimIdentity, ClaimId)>,
            next_claim: u64,
        ) -> Self {
            Self::Growable {
                bindings,
                next_claim,
                completed_values: Vec::new(),
            }
        }

        pub(crate) fn bindings(&self) -> &[(PermissionClaimIdentity, ClaimId)] {
            match self {
                Self::Growable { bindings, .. } => bindings,
                Self::Fixed(published) => published.bindings,
            }
        }

        pub(crate) fn receive_completed_value_claim(
            &mut self,
            value: checked_trees::CheckedStructuralValueHandle,
            path: &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> Result<ClaimId, LoweringError> {
            let (rows, next_claim) = match self {
                Self::Growable {
                    completed_values,
                    next_claim,
                    ..
                } => (completed_values, next_claim),
                Self::Fixed(published) => {
                    return published
                        .completed_values
                        .iter()
                        .find(|row| row.value == value && row.path == path)
                        .map(|row| row.claim)
                        .ok_or(LoweringError::Unsupported(
                            "published graph omits completed value claim slot",
                        ));
                }
            };
            if let Some(row) = rows
                .iter()
                .find(|row| row.value == value && row.path == path)
            {
                return Ok(row.claim);
            }
            let claim = claim_id(allocate_dense(next_claim)?);
            rows.push(CompletedValueClaim {
                value,
                path: path.to_vec(),
                claim,
            });
            Ok(claim)
        }

        pub(crate) fn receive_loan_completion_claim(
            &mut self,
            identity: PermissionClaimIdentity,
        ) -> Result<ClaimId, LoweringError> {
            self.reserve_checked_binding(identity)
        }

        pub(crate) fn reserve_checked_binding(
            &mut self,
            identity: PermissionClaimIdentity,
        ) -> Result<ClaimId, LoweringError> {
            if identity == PermissionClaimIdentity::Unknown {
                return unsupported("receiving binding has no checked claim identity");
            }
            match lookup_claim_id(self.bindings(), identity) {
                Ok(claim) => Ok(claim),
                Err(_) => self.mint(identity),
            }
        }

        pub(crate) fn receive_call_result_claim(
            &mut self,
            state: symbols::SymbolHandle,
            coordinate: checked_trees::CheckedUnitCallCoordinate,
            received: &checked_trees::CheckedStructuralResultClaimPlan,
        ) -> Result<ClaimId, LoweringError> {
            if received.claim == PermissionClaimIdentity::Unknown {
                return match self {
                    Self::Growable { next_claim, .. } => Ok(claim_id(allocate_dense(next_claim)?)),
                    Self::Fixed(published) => {
                        let mut matching = published.unreceived_results.iter().filter(|claim| {
                            claim.state == state
                                && claim.coordinate == coordinate
                                && claim.path == received.path
                        });
                        let claim = matching.next().ok_or(LoweringError::Unsupported(
                            "published graph omits an unreceived call result claim",
                        ))?;
                        if matching.next().is_some() {
                            return unsupported(
                                "published graph repeats an unreceived call result claim",
                            );
                        }
                        Ok(claim.claim)
                    }
                };
            }
            match lookup_claim_id(self.bindings(), received.claim) {
                Ok(bound) => Ok(bound),
                Err(_) => self.mint(received.claim),
            }
        }

        fn mint(&mut self, identity: PermissionClaimIdentity) -> Result<ClaimId, LoweringError> {
            let Self::Growable {
                bindings,
                next_claim,
                ..
            } = self
            else {
                return unsupported("published graph cannot mint another claim binding");
            };
            let claim = claim_id(allocate_dense(next_claim)?);
            bindings.push((identity, claim));
            Ok(claim)
        }
    }

    pub(crate) fn emitted_claim_transfers(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
        claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
        parameters: &[terminal_psi::StructuralParameterDeclaration],
        types: &[terminal_psi::StructuralTypeDeclaration],
        arguments: &[terminal_psi::StructuralArgument],
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<Vec<terminal_psi::ClaimTransfer>, LoweringError> {
        let prepared =
            prepare_claim_transfers(identities, transfers, types, |_identities, index| {
                parameters
                    .get(index as usize)
                    .map(|parameter| parameter.structural_type)
                    .ok_or(LoweringError::Unsupported(
                        "claim transfer has no structural argument type",
                    ))
            })?;
        emit_prepared_claim_transfers(&prepared, claim_bindings, arguments, current_claims)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PreparedClaimTransfer {
        pub(crate) identity: PermissionClaimIdentity,
        pub(crate) argument_index: u32,
        pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
    }

    pub(crate) fn emit_completion_receipts(
        prepared: &[PreparedClaimTransfer],
        bindings: &[(PermissionClaimIdentity, ClaimId)],
        arguments: &[terminal_psi::StructuralArgument],
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<Vec<terminal_psi::CompletionReceipt>, LoweringError> {
        let mut receipts = prepared
            .iter()
            .map(|receipt| {
                Ok(terminal_psi::CompletionReceipt {
                    claim: resolve_argument_claim(receipt, bindings, arguments, current_claims)?,
                    argument_index: receipt.argument_index,
                    path: receipt.path.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        receipts.sort();
        Ok(receipts)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PreparedResultClaim {
        received: checked_trees::CheckedStructuralResultClaimPlan,
        path: Vec<terminal_psi::StructuralPathSegment>,
    }

    pub(crate) fn prepare_call_result_claims(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        received: &[checked_trees::CheckedStructuralResultClaimPlan],
        structural_type: semantic_vocabulary::StructuralTypeId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<Vec<PreparedResultClaim>, LoweringError> {
        received.iter().map(|received| {
            Ok(PreparedResultClaim {
                received: received.clone(),
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(identities, &received.path, structural_type, types)?,
            })
        }).collect()
    }

    pub(crate) fn emit_call_result_claims(
        prepared: &[PreparedResultClaim],
        state: symbols::SymbolHandle,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        bindings: &mut ClaimBindings<'_>,
    ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
        let mut received = prepared
            .iter()
            .map(|claim| {
                Ok(terminal_psi::StructuralResultClaimBinding {
                    claim: bindings.receive_call_result_claim(
                        state,
                        coordinate,
                        &claim.received,
                    )?,
                    path: claim.path.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        received.sort();
        Ok(received)
    }

    pub(crate) fn prepare_checked_claim_transfers(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
        parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<Vec<PreparedClaimTransfer>, LoweringError> {
        prepare_claim_transfers(identities, transfers, types, |identities, index| {
            let parameter = parameters
                .get(index as usize)
                .ok_or(LoweringError::Unsupported(
                    "claim transfer has no checked structural parameter",
                ))?;
            let identity = checked
                .type_identity_text(parameter.type_identity)
                .unwrap_or_default();
            let mut declarations = types
                .iter()
                .filter(|declaration| declaration.identity == identities.intern(&identity));
            let declaration = declarations.next().ok_or(LoweringError::Unsupported(
                "claim transfer has no lowered parameter type",
            ))?;
            if declarations.next().is_some() {
                return unsupported("claim transfer has ambiguous parameter type");
            }
            Ok(declaration.id)
        })
    }

    pub(crate) fn prepare_claim_transfers(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
        types: &[terminal_psi::StructuralTypeDeclaration],
        mut argument_type: impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            u32,
        )
            -> Result<semantic_vocabulary::StructuralTypeId, LoweringError>,
    ) -> Result<Vec<PreparedClaimTransfer>, LoweringError> {
        transfers.iter().map(|transfer| {
            let root = argument_type(identities, transfer.argument_index)?;
            Ok(PreparedClaimTransfer {
                identity: transfer.claim_identity,
                argument_index: transfer.argument_index,
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(identities, &transfer.path, root, types)?,
            })
        }).collect()
    }

    pub(crate) fn emit_prepared_claim_transfers(
        transfers: &[PreparedClaimTransfer],
        claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
        arguments: &[terminal_psi::StructuralArgument],
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<Vec<terminal_psi::ClaimTransfer>, LoweringError> {
        let mut emitted = transfers
            .iter()
            .map(|transfer| {
                Ok(terminal_psi::ClaimTransfer {
                    claim: resolve_argument_claim(
                        transfer,
                        claim_bindings,
                        arguments,
                        current_claims,
                    )?,
                    argument_index: transfer.argument_index,
                    path: transfer.path.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        emitted.sort();
        Ok(emitted)
    }

    pub(crate) fn resolve_consumed_claim(
        identity: PermissionClaimIdentity,
        path: &[terminal_psi::StructuralPathSegment],
        bindings: &[(PermissionClaimIdentity, ClaimId)],
        completed: Option<&[terminal_psi::StructuralResultClaimBinding]>,
    ) -> Result<ClaimId, LoweringError> {
        if identity != PermissionClaimIdentity::Unknown {
            return lookup_claim_id(bindings, identity);
        }
        let mut matching = completed
            .into_iter()
            .flatten()
            .filter(|claim| claim.path == path);
        let selected = matching.next().ok_or(LoweringError::Unsupported(
            "selected transfer has no materialized source claim",
        ))?;
        if matching.next().is_some() {
            return unsupported("selected transfer has ambiguous materialized source claim");
        }
        Ok(selected.claim)
    }

    fn resolve_argument_claim(
        transfer: &PreparedClaimTransfer,
        bindings: &[(PermissionClaimIdentity, ClaimId)],
        arguments: &[terminal_psi::StructuralArgument],
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<ClaimId, LoweringError> {
        let argument =
            arguments
                .get(transfer.argument_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "selected transfer has no actual argument",
                ))?;
        if argument.access != terminal_psi::StructuralAccess::Owned {
            return unsupported("selected transfer requires owned argument custody");
        }
        let completed = current_claims.source(argument)?;
        resolve_consumed_claim(
            transfer.identity,
            &transfer.path,
            bindings,
            completed.as_deref(),
        )
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PreparedClaim {
        pub(crate) identity: PermissionClaimIdentity,
        pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
        provenance: language_semantics::PermissionProvenance,
        fresh: bool,
        direct_establishment: bool,
    }

    fn receiving_claim_places(
        checked: &checked_trees::CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement: usize,
        root: facts::PlaceRoot,
    ) -> Result<Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>, LoweringError> {
        let facts::PlaceRoot::Expression(expression) = root else {
            return Ok(Vec::new());
        };
        let flow = &checked.facts.flow.control;
        let semantic = &checked.facts.semantic;
        let mut roots = Vec::new();
        for (_, scope) in flow
            .states
            .iter()
            .filter(|(_, scope)| scope.machine_symbol == machine && scope.state_symbol == state)
        {
            for row in flow
                .statements
                .span_or_empty(scope.statements)
                .iter()
                .filter(|row| {
                    row.statement_index == statement
                        && flow.operands.is_valid(row.stored_value)
                        && semantic.places.is_valid(row.stored_destination)
                })
            {
                let destination = semantic.places.get(row.stored_destination);
                roots.push((
                    row.stored_value,
                    destination.root,
                    semantic
                        .place_segments
                        .span_or_empty(destination.segments)
                        .to_vec(),
                ));
            }
        }
        let mut established_constructions = Vec::new();
        let state_permissions = flow_facts(checked)
            .ownership
            .with_state_permissions(state, |handles| handles.to_vec());
        for handle in state_permissions {
            let event = checked.facts.flow.ownership.permissions.get(handle);
            let facts::PlaceRoot::Expression(construction) = event.root else {
                continue;
            };
            if event.machine_symbol == machine
                && event.state_symbol == state
                && event.expression == construction
                && event.kind == language_semantics::PermissionEventKind::Establish
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == language_semantics::Multiplicity::Linear
                && event.obligation_live
                && event.claim_identity != PermissionClaimIdentity::Unknown
                && matches!(event.source,
                    language_semantics::PermissionEventSource::Statement { statement_index }
                    | language_semantics::PermissionEventSource::Call { statement_index, .. }
                    if statement_index == statement)
                && !established_constructions.contains(&construction)
            {
                established_constructions.push(construction);
            }
        }
        if !established_constructions.is_empty() {
            for (handle, operand) in flow.operands.iter().filter(|(_, operand)| {
                operand.state_symbol == state
                    && operand.statement_index == statement
                    && operand.expression != expression
                    && !operand.constructed_children().is_empty()
                    && established_constructions.contains(&operand.expression)
            }) {
                roots.push((
                    handle,
                    facts::PlaceRoot::Expression(operand.expression),
                    Vec::new(),
                ));
            }
        }
        let mut receiving = Vec::new();
        for (handle, destination, path) in roots {
            let mut pending = vec![(handle, path)];
            while let Some((handle, path)) = pending.pop() {
                let operand = flow.operands.get(handle);
                if operand.state_symbol != state || operand.statement_index != statement {
                    return unsupported(
                        "stored claim operand has a different evaluation coordinate",
                    );
                }
                if operand.expression == expression {
                    let candidate = (destination, path);
                    if !receiving.contains(&candidate) {
                        receiving.push(candidate);
                    }
                    continue;
                }
                for child in flow
                    .operand_children
                    .span_or_empty(operand.constructed_children())
                {
                    let mut child_path = path.clone();
                    child_path
                        .extend_from_slice(flow.operand_child_paths.span_or_empty(child.path));
                    pending.push((child.value, child_path));
                }
            }
        }
        Ok(receiving)
    }

    pub(crate) fn prepare_establishment_claims(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement: usize,
        root: facts::PlaceRoot,
        type_reference: typed_trees::types::TypeReferenceHandle,
        structural_type: semantic_vocabulary::StructuralTypeId,
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<Vec<PreparedClaim>, LoweringError> {
        let completed_expression = root.as_expression();
        let receiving = receiving_claim_places(checked, machine, state, statement, root)?;
        let mut coordinates = vec![(root, Vec::new())];
        for coordinate in receiving {
            if !coordinates.contains(&coordinate) {
                coordinates.push(coordinate);
            }
        }
        let mut prepared: Vec<PreparedClaim> = Vec::new();
        let state_permissions = flow_facts(checked)
            .ownership
            .with_state_permissions(state, |handles| handles.to_vec());
        for (root, receiving_path) in coordinates {
            let mut completed_evaluation = None;
            if completed_expression.is_some() {
                if let Some(construction) = root.as_expression() {
                    for &handle in &state_permissions {
                        let event = checked.facts.flow.ownership.permissions.get(handle);
                        if event.machine_symbol != machine
                            || event.state_symbol != state
                            || event.root != root
                            || event.expression != construction
                            || event.kind != language_semantics::PermissionEventKind::Establish
                            || !matches!(event.source,
                                language_semantics::PermissionEventSource::Statement { statement_index }
                                | language_semantics::PermissionEventSource::Call { statement_index, .. }
                                if statement_index == statement)
                        {
                            continue;
                        }
                        if event.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
                            || completed_evaluation.is_some_and(|lane| lane != event.evaluation)
                        {
                            return unsupported(
                                "completed construction has ambiguous evaluation custody",
                            );
                        }
                        completed_evaluation = Some(event.evaluation);
                    }
                } else {
                    completed_evaluation = Some(checked_trees::FlowPermissionEvaluation::Statement);
                }
            }
            let at_completion = |event: &checked_trees::FlowPermissionEventFact| {
                completed_expression.is_none_or(|_| {
                    completed_evaluation == Some(event.evaluation)
                        && root
                            .as_expression()
                            .is_none_or(|owner| event.expression == owner)
                })
            };
            for &handle in &state_permissions {
                let event = checked.facts.flow.ownership.permissions.get(handle);
                if !(event.machine_symbol == machine
                    && event.state_symbol == state
                    && at_completion(event)
                    && event.root == root
                    && checked
                        .facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .starts_with(&receiving_path)
                    && event.kind == language_semantics::PermissionEventKind::Establish
                    && event.claim_identity != PermissionClaimIdentity::Unknown
                    && event.access == language_semantics::PermissionAccess::Owned
                    && event.multiplicity == language_semantics::Multiplicity::Linear
                    && event.obligation_live
                    && matches!(event.source, language_semantics::PermissionEventSource::Statement { statement_index }
                        | language_semantics::PermissionEventSource::Call { statement_index, .. } if statement_index == statement))
                {
                    continue;
                }
                let established_here = state_permissions.iter().copied().any(|source_handle| {
                    let source = checked.facts.flow.ownership.permissions.get(source_handle);
                    source.evaluation == event.evaluation
                        && source.claim_identity == event.claim_identity
                        && claim_established_here(machine, state, statement, source)
                });
                let path = validation::structural_claim_path(
                    &checked.typed,
                    type_reference,
                    &flow_facts(checked)
                        .ownership
                        .segments
                        .span_or_empty(event.segments)[receiving_path.len()..],
                )
                .map_err(LoweringError::Unsupported)?;
                let claim = PreparedClaim {
                    identity: event.claim_identity,
                    path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                        identities, &path, structural_type, structural_types)?,
                    provenance: event.provenance,
                    fresh: established_here,
                    direct_establishment: matches!(event.source, language_semantics::PermissionEventSource::Statement { .. })
                        && event.provenance == language_semantics::PermissionProvenance::Established {
                            machine_symbol: machine, state_symbol: state, source: event.source,
                        }
                        && matches!(event.claim_identity, PermissionClaimIdentity::Established { machine_symbol, state_symbol, source, .. }
                            if machine_symbol == machine && state_symbol == state && source == event.source),
                };
                if let Some(previous) = prepared.iter().find(|previous| previous.path == claim.path)
                {
                    if previous != &claim {
                        return unsupported(
                            "completed construction has conflicting claim correspondences",
                        );
                    }
                } else {
                    prepared.push(claim);
                }
            }
        }
        Ok(prepared)
    }

    pub(crate) fn emit_establishment_claims(
        claims: &[PreparedClaim],
        bindings: &mut ClaimBindings<'_>,
    ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
        let mut emitted = claims
            .iter()
            .map(|claim| {
                let resolved = match lookup_claim_id(bindings.bindings(), claim.identity) {
                    Ok(resolved) => resolved,
                    Err(_) if claim.fresh => bindings.mint(claim.identity)?,
                    Err(error) => return Err(error),
                };
                Ok(terminal_psi::StructuralResultClaimBinding {
                    claim: resolved,
                    path: claim.path.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        emitted.sort();
        Ok(emitted)
    }

    pub(crate) fn claim_established_here(
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: usize,
        event: &checked_trees::FlowPermissionEventFact,
    ) -> bool {
        event.machine_symbol == machine
            && event.state_symbol == state
            && matches!(event.source, language_semantics::PermissionEventSource::Statement { statement_index: index }
                | language_semantics::PermissionEventSource::Call { statement_index: index, .. } if index == statement_index)
            && matches!(
                event.kind,
                language_semantics::PermissionEventKind::Establish
                    | language_semantics::PermissionEventKind::Restore
            )
            && event.access == language_semantics::PermissionAccess::Owned
            && event.multiplicity == language_semantics::Multiplicity::Linear
            && event.obligation_live
            && matches!(event.claim_identity, PermissionClaimIdentity::Established { machine_symbol, state_symbol, source, .. }
                if machine_symbol == machine && state_symbol == state && source == event.source)
            && (event.provenance
                == language_semantics::PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: event.source,
                }
                || matches!((event.provenance, event.claim_identity),
                    (language_semantics::PermissionProvenance::Joined {
                        machine_symbol, state_symbol,
                        source: language_semantics::PermissionEventSource::Call { statement_index: received_at, .. },
                        ordinal,
                    }, PermissionClaimIdentity::Established { ordinal: identity_ordinal, .. })
                        if machine_symbol == machine && state_symbol == state
                            && received_at == statement_index && ordinal == identity_ordinal))
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ClaimEstablishmentKind {
        Construction,
        DetachedValue,
    }

    pub(crate) fn direct_establishment_claims(
        prepared: &[PreparedClaim],
        bindings: &ClaimBindings<'_>,
    ) -> Result<Vec<ClaimId>, LoweringError> {
        prepared
            .iter()
            .filter(|claim| claim.direct_establishment)
            .map(|claim| lookup_claim_id(bindings.bindings(), claim.identity))
            .collect()
    }

    pub(crate) fn record_prepared_introductions(
        operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
        place: semantic_vocabulary::PlaceId,
        prepared: &[PreparedClaim],
        bindings: &ClaimBindings<'_>,
        kind: ClaimEstablishmentKind,
    ) -> Result<(), LoweringError> {
        let claims = prepared
            .iter()
            .filter(|claim| {
                claim.direct_establishment
                    && (matches!(kind, ClaimEstablishmentKind::DetachedValue)
                        || claim.path.is_empty())
            })
            .map(|claim| lookup_claim_id(bindings.bindings(), claim.identity))
            .collect::<Result<Vec<_>, _>>()?;
        if claims.is_empty() {
            return Ok(());
        }
        let mut producers = operations.operations.iter().filter(|operation| {
            operation
                .result
                .as_structural()
                .is_some_and(|result| result.place == place)
        });
        let producer = producers.next().ok_or(LoweringError::Unsupported(
            "claim introduction lost its exact producer",
        ))?;
        if producers.next().is_some() {
            return unsupported("claim introduction has ambiguous producer");
        }
        let operation = producer.id;
        if claims.iter().any(|claim| {
            !producer
                .result
                .as_structural()
                .expect("matched structural result")
                .claims
                .iter()
                .any(|binding| binding.claim == *claim)
        }) {
            return unsupported("claim introduction is absent from its completed result");
        }
        for claim in claims {
            operations
                .claim_introductions
                .push(ClaimIntroduction { operation, claim });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_owned_operand_claims(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: u32,
        expression: typed_trees::expression::ExpressionHandle,
        expected_evaluation: Option<checked_trees::FlowPermissionEvaluation>,
        expected_source: Option<language_semantics::PermissionEventSource>,
        bindings: &[(PermissionClaimIdentity, ClaimId)],
        completed_claims: Option<&[terminal_psi::StructuralResultClaimBinding]>,
        structural_type: semantic_vocabulary::StructuralTypeId,
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
        if !expression.is_valid() {
            return unsupported("owned operand has no exact consuming expression");
        }
        let statement = usize::try_from(statement_index).map_err(|_| {
            LoweringError::Unsupported("structural return coordinate exceeds usize")
        })?;
        let expression =
            validation::ownership_source_expression(&checked.typed, state, statement, expression);
        let ownership = &flow_facts(checked).ownership;
        let evaluated_transfers = ownership
            .owned_transfer_paths
            .iter()
            .filter_map(|(_, transfer)| {
                let event = ownership.permissions.get(transfer.event);
                (event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.expression == expression
                    && transfer.placement != checked_trees::FlowOwnedTransferPlacement::OtherRoot)
                    .then_some((event, transfer.placement))
            })
            .collect::<Vec<_>>();
        if evaluated_transfers.is_empty() {
            return Ok(Vec::new());
        }
        let mut claims = Vec::new();
        let mut consumer = expected_source;
        let mut evaluation = expected_evaluation;
        for (event, placement) in &evaluated_transfers {
            if event.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
                || evaluation.is_some_and(|lane| lane != event.evaluation)
            {
                return unsupported("owned operand has ambiguous consuming evaluation");
            }
            evaluation = Some(event.evaluation);
            match event.source {
                language_semantics::PermissionEventSource::Statement { statement_index }
                | language_semantics::PermissionEventSource::Call {
                    statement_index, ..
                } if statement_index == statement => {}
                _ => return unsupported("owned operand transfer has another consumer statement"),
            }
            if consumer.is_some_and(|source| source != event.source) {
                return unsupported("owned operand has multiple consuming coordinates");
            }
            consumer = Some(event.source);
            let relative = match placement {
                checked_trees::FlowOwnedTransferPlacement::Within(path) => {
                    ownership.owned_transfer_path_segments.span_or_empty(*path)
                }
                checked_trees::FlowOwnedTransferPlacement::Outside
                | checked_trees::FlowOwnedTransferPlacement::OtherRoot => continue,
                checked_trees::FlowOwnedTransferPlacement::Unresolved => {
                    return unsupported("owned operand transfer has no checked placement");
                }
            };
            let path =
                crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities,
                    relative,
                    structural_type,
                    structural_types,
                )?;
            let claim = crate::emission::claim_bindings::resolve_consumed_claim(
                event.claim_identity,
                &path,
                bindings,
                completed_claims,
            )?;
            claims.push(terminal_psi::StructuralResultClaimBinding { claim, path });
        }
        claims.sort();
        if claims.iter().enumerate().any(|(index, claim)| {
            claims[..index]
                .iter()
                .any(|prior| prior.claim == claim.claim || prior.path == claim.path)
        }) {
            return unsupported("owned operand repeats a claim identity or path");
        }
        Ok(claims)
    }
}
pub(crate) mod completed_address {

    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::shared::lowering_error::LoweringError;
    use crate::shared::terminal_identities::{
        allocate_dense, block_id, edge_id, place_id, value_id,
    };
    use crate::unit::attached_unit::argument_evaluation::Evaluation;
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
    use terminal_psi::{
        Block, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
        Terminator, ValueDeclaration,
    };

    pub(crate) fn capture(
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        structural_type: StructuralTypeId,
        access: StructuralAccess,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        additional_input_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        evaluation: &mut Evaluation,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        next_place: &mut u64,
        places: &mut Vec<StructuralPlaceDeclaration>,
        operations: &mut OperationBuffer,
    ) -> Result<PlaceId, LoweringError> {
        let place = place_id(allocate_dense(next_place)?);
        capture_into(
            source,
            path,
            structural_type,
            access,
            qualifications,
            projected_qualifications,
            additional_input_qualifications,
            place,
            evaluation,
            values,
            next_value,
            next_block,
            next_edge,
            places,
            operations,
        )?;
        Ok(place)
    }

    pub(crate) fn capture_into(
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        structural_type: StructuralTypeId,
        access: StructuralAccess,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        additional_input_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        place: PlaceId,
        evaluation: &mut Evaluation,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        places: &mut Vec<StructuralPlaceDeclaration>,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let continuation = block_id(allocate_dense(next_block)?);
        let parameters = values
            .iter()
            .map(|value| {
                Ok(ValueDeclaration {
                    id: value_id(allocate_dense(next_value)?),
                    scalar_type: value.scalar_type,
                    qualifications: value.qualifications,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::BlockParameter {
                block: continuation,
                position: 0,
            },
        });
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: std::mem::take(&mut evaluation.parameters),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: std::mem::take(&mut evaluation.block_structural_parameters),
            operations: operations[evaluation.operation_start..].to_vec(),
            terminator: Terminator::Jump {
                edge: edge_id(allocate_dense(next_edge)?),
                target: continuation,
                arguments: values.iter().map(|value| value.id).collect(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: source,
                    path,
                    access,
                }],
                cleanup_actions: Vec::new(),
            },
        });
        evaluation.current = continuation;
        evaluation.operation_start = operations.len();
        operations.storage_observations.clear();
        *values = parameters.clone();
        evaluation.parameters = parameters;
        evaluation.block_structural_parameters = vec![StructuralParameterDeclaration {
            place,
            position: 0,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications,
            projected_qualifications,
            additional_input_qualifications,
        }];
        operations
            .captured_place_types
            .push((place, structural_type));
        Ok(())
    }
}
pub(crate) mod completed_place {

    use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

    #[derive(Clone, Copy)]
    pub(crate) enum EndpointUse {
        Storage,
        Value(StructuralTypeId),
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) enum Capture {
        Address(StructuralTypeId),
        DescriptorValue {
            shape: StructuralTypeShape,
            preferred: Option<StructuralTypeId>,
        },
    }

    #[derive(Clone)]
    pub(crate) enum Frontier {
        Capture { prefix: usize, capture: Capture },
        Selector { position: usize },
        Value { structural_type: StructuralTypeId },
    }

    pub(crate) fn plan(
        types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &[PreparedStructuralPathSegment],
        copy_value: bool,
        access: terminal_psi::StructuralAccess,
        endpoint: EndpointUse,
    ) -> Result<Vec<Frontier>, LoweringError> {
        let mut frontiers = Vec::new();
        let capture_address = access != terminal_psi::StructuralAccess::Owned || copy_value;
        let mut type_path = Vec::new();
        let root_declaration = types
            .iter()
            .find(|declaration| declaration.id == root)
            .ok_or(LoweringError::Unsupported(
                "completed place has no root declaration",
            ))?;
        let mut pending = matches!(
            root_declaration.shape,
            StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                )
        )
        .then_some((0, Capture::Address(root)));
        for (position, segment) in path.iter().enumerate() {
            let index = matches!(
                segment,
                PreparedStructuralPathSegment::ComputedIndex(_)
                    | PreparedStructuralPathSegment::EvaluatedFixedIndex { .. }
                    | PreparedStructuralPathSegment::RuntimeIndex(_)
                    | PreparedStructuralPathSegment::CapturedIndex(_)
                    | PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(_))
            );
            if index && let Some(shape) = descriptor_shape(types, root, &type_path) {
                let prefix = if position > 0
                    && matches!(
                        path[position - 1],
                        PreparedStructuralPathSegment::ViewExtent { .. }
                    ) {
                    position - 1
                } else {
                    position
                };
                pending = Some((
                    prefix,
                    Capture::DescriptorValue {
                        shape,
                        preferred: None,
                    },
                ));
            }
            if matches!(
                segment,
                PreparedStructuralPathSegment::ComputedIndex(_)
                    | PreparedStructuralPathSegment::EvaluatedFixedIndex { .. }
                    | PreparedStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::StorageRead { .. }
                            | checked_trees::CheckedRuntimeIndex::StructuralField(_)
                    )
            ) {
                if let Some((prefix, capture)) = pending.take().filter(|_| capture_address) {
                    frontiers.push(Frontier::Capture { prefix, capture });
                }
                frontiers.push(Frontier::Selector { position });
            }
            match segment {
                PreparedStructuralPathSegment::Static(segment) => {
                    type_path.push(segment.clone());
                    if *segment == StructuralPathSegment::Referent {
                        let tip = terminal_semantics::runtime_structural_path_tip(
                            types.iter(),
                            root,
                            &type_path,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "completed place lost its dereferenced carrier",
                        ))?;
                        pending = Some((position + 1, Capture::Address(tip.id)));
                    }
                }
                PreparedStructuralPathSegment::EvaluatedFixedIndex { index, .. } => {
                    type_path.push(StructuralPathSegment::FixedIndex(*index));
                }
                PreparedStructuralPathSegment::ViewExtent { .. } => {}
                _ => {
                    type_path.push(StructuralPathSegment::FixedIndex(0));
                }
            }
        }
        if !copy_value
            && let EndpointUse::Value(preferred) = endpoint
            && let Some(shape) = descriptor_shape(types, root, &type_path)
        {
            pending = Some((
                path.len(),
                Capture::DescriptorValue {
                    shape,
                    preferred: Some(preferred),
                },
            ));
        }
        if let Some((prefix, capture)) = pending.filter(|_| capture_address) {
            frontiers.push(Frontier::Capture { prefix, capture });
        }
        if copy_value {
            let structural_type = match endpoint {
                EndpointUse::Value(identity) => identity,
                EndpointUse::Storage => {
                    terminal_semantics::runtime_structural_path_tip(types.iter(), root, &type_path)
                        .ok_or(LoweringError::Unsupported(
                            "completed copy lost its selected carrier",
                        ))?
                        .id
                }
            };
            frontiers.push(Frontier::Value { structural_type });
        }
        if frontiers.windows(2).any(|pair| {
            matches!((&pair[0], &pair[1]),
                (Frontier::Capture { prefix: first, .. }, Frontier::Capture { prefix: second, .. })
                    if first > second)
        }) {
            return unsupported("completed place frontiers reverse their authored path");
        }
        Ok(frontiers)
    }

    pub(crate) fn endpoint_use(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        types: &[StructuralTypeDeclaration],
        parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    ) -> Result<EndpointUse, LoweringError> {
        let identity = checked.type_identity_text(parameter.type_identity).ok_or(
            LoweringError::Unsupported("completed formal has no checked type identity"),
        )?;
        let identity = identities.intern(&identity);
        let mut matches = types.iter().filter(|row| row.identity == identity);
        let Some(declaration) = matches.next() else {
            return Ok(EndpointUse::Storage);
        };
        if matches.next().is_some() {
            return unsupported("completed formal repeats its structural type identity");
        }
        Ok(
            if parameter.access == checked_trees::CheckedStructuralAccess::Owned
                || declaration.shape.is_canonical_view_descriptor()
            {
                EndpointUse::Value(declaration.id)
            } else {
                EndpointUse::Storage
            },
        )
    }

    pub(crate) fn descriptor_shape(
        types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeShape> {
        if let Some(field) = terminal_semantics::runtime_structural_field(types.iter(), root, path)
            && let Some(carrier) = field.field_type.byte_sequence_carrier()
            && matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            )
        {
            return Some(StructuralTypeShape::ByteSequence(carrier));
        }
        let shape =
            &terminal_semantics::runtime_structural_path_tip(types.iter(), root, path)?.shape;
        matches!(
            shape,
            StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                )
        )
        .then(|| shape.clone())
    }

    pub(crate) fn present_capture(
        capture: &Capture,
        access: terminal_psi::StructuralAccess,
        types: &[StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        whole: Vec<semantic_vocabulary::StructuralDomainId>,
        projected: Vec<terminal_psi::StructuralPathQualification>,
        additional: Vec<terminal_psi::StructuralPathQualification>,
    ) -> Result<
        (
            StructuralTypeId,
            Vec<semantic_vocabulary::StructuralDomainId>,
            Vec<terminal_psi::StructuralPathQualification>,
            Vec<terminal_psi::StructuralPathQualification>,
        ),
        LoweringError,
    > {
        let (shape, preferred) = match capture {
            Capture::Address(identity) => return Ok((*identity, whole, projected, additional)),
            Capture::DescriptorValue { shape, preferred } => (shape, preferred),
        };
        if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
            return unsupported("descriptor completion requires readable occurrence access");
        }
        let eligible = |row: &&StructuralTypeDeclaration| {
            row.shape.is_canonical_view_descriptor()
                && terminal_semantics::descriptor_reborrow_shapes_match(
                    shape,
                    access,
                    Some(access),
                    &row.shape,
                    access,
                )
        };
        let present =
            |identity, mut whole: Vec<_>, mut projected: Vec<_>, mut additional: Vec<_>| {
                terminal_semantics::present_structural_membership_contract(
                    types.iter(),
                    domains.iter(),
                    identity,
                    &mut whole,
                    &mut projected,
                    &mut additional,
                )
                .map_err(LoweringError::Unsupported)?;
                Ok((identity, whole, projected, additional))
            };
        if let Some(identity) = preferred {
            let mut matches = types.iter().filter(|row| row.id == *identity);
            let row = matches.next().ok_or(LoweringError::Unsupported(
                "descriptor completion lost its exact formal carrier",
            ))?;
            if matches.next().is_some() || !eligible(&row) {
                return unsupported("descriptor completion changes its formal geometry or access");
            }
            return present(*identity, whole, projected, additional);
        }
        let mut candidates = types.iter().filter(eligible).collect::<Vec<_>>();
        candidates.sort_by_key(|row| row.id);
        if candidates.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return unsupported("descriptor completion repeats a canonical carrier identity");
        }
        let mut failure = None;
        for candidate in candidates {
            match present(
                candidate.id,
                whole.clone(),
                projected.clone(),
                additional.clone(),
            ) {
                Ok(presentation) => return Ok(presentation),
                Err(error) => failure = Some(error),
            }
        }
        Err(failure.unwrap_or(LoweringError::Unsupported(
            "descriptor completion has no canonical membership-preserving carrier",
        )))
    }
}
pub(crate) mod control_flow {
    use std::collections::BTreeSet;

    pub(crate) fn block_dominators(
        entry: Option<usize>,
        successors: &[Vec<usize>],
    ) -> Vec<BTreeSet<usize>> {
        let count = successors.len();
        if count == 0 {
            return Vec::new();
        }
        let words = count.div_ceil(64);
        let mut universal_bits = vec![!0u64; words];
        let tail_bits = count % 64;
        if tail_bits != 0 {
            universal_bits[words - 1] = (1u64 << tail_bits) - 1;
        }
        let Some(entry) = entry else {
            let universal: BTreeSet<usize> = (0..count).collect();
            return vec![universal; count];
        };
        let mut predecessors = vec![Vec::new(); count];
        for (block, outs) in successors.iter().enumerate() {
            for &successor in outs {
                predecessors[successor].push(block);
            }
        }
        let mut dom_bits = vec![universal_bits.clone(); count];
        let mut entry_bits = vec![0u64; words];
        entry_bits[entry / 64] |= 1u64 << (entry % 64);
        dom_bits[entry] = entry_bits;
        let mut next_bits = vec![0u64; words];
        loop {
            let mut changed = false;
            for block in 0..count {
                if block == entry {
                    continue;
                }
                next_bits.copy_from_slice(&universal_bits);
                for &predecessor in &predecessors[block] {
                    for (word, pred_word) in next_bits.iter_mut().zip(&dom_bits[predecessor]) {
                        *word &= *pred_word;
                    }
                }
                next_bits[block / 64] |= 1u64 << (block % 64);
                if next_bits != dom_bits[block] {
                    dom_bits[block].copy_from_slice(&next_bits);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        dom_bits
            .into_iter()
            .map(|bits| {
                let mut set = BTreeSet::new();
                for (word_index, mut word) in bits.into_iter().enumerate() {
                    while word != 0 {
                        let tz = word.trailing_zeros() as usize;
                        set.insert(word_index * 64 + tz);
                        word &= word - 1;
                    }
                }
                set
            })
            .collect()
    }
}
pub(crate) mod current_claims {

    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::PlaceId;
    use terminal_psi::{StructuralArgument, StructuralPathSegment, StructuralResultClaimBinding};

    #[derive(Clone, Default)]
    pub(crate) struct CurrentClaimNames {
        homes: Vec<(PlaceId, Vec<StructuralResultClaimBinding>)>,
        unavailable: Vec<PlaceId>,
    }

    impl CurrentClaimNames {
        pub(crate) fn source(
            &self,
            argument: &StructuralArgument,
        ) -> Result<Option<Vec<StructuralResultClaimBinding>>, LoweringError> {
            if self.unavailable.contains(&argument.place) {
                return unsupported("current storage names require an unbound branch arrival");
            }
            if let Some((_, claims)) = self
                .homes
                .iter()
                .find(|(place, _)| *place == argument.place)
            {
                return Ok(Some(
                    claims
                        .iter()
                        .filter_map(|binding| {
                            binding
                                .path
                                .strip_prefix(argument.path.as_slice())
                                .map(|path| StructuralResultClaimBinding {
                                    claim: binding.claim,
                                    path: path.to_vec(),
                                })
                        })
                        .collect(),
                ));
            }
            Ok(None)
        }

        pub(crate) fn publish_operations(
            &mut self,
            operations: &[terminal_psi::Operation],
        ) -> Result<(), LoweringError> {
            for operation in operations {
                if let Some(result) = operation.result.structural() {
                    self.publish(result.place, result.claims.clone())?;
                } else if let terminal_psi::OperationKind::EstablishTrivialAffineLocal {
                    destination,
                }
                | terminal_psi::OperationKind::EstablishByteSequenceLiteral {
                    destination,
                    ..
                } = &operation.kind
                {
                    self.publish(*destination, Vec::new())?;
                }
            }
            Ok(())
        }

        pub(crate) fn contains(&self, place: PlaceId) -> bool {
            self.homes.iter().any(|(home, _)| *home == place) || self.unavailable.contains(&place)
        }

        pub(crate) fn publish(
            &mut self,
            place: PlaceId,
            mut claims: Vec<StructuralResultClaimBinding>,
        ) -> Result<(), LoweringError> {
            self.unavailable.retain(|home| *home != place);
            claims.sort();
            if claims.iter().enumerate().any(|(index, claim)| {
                claims[..index]
                    .iter()
                    .any(|previous| previous.claim == claim.claim || previous.path == claim.path)
            }) {
                return unsupported("current storage namespace repeats a claim or path");
            }
            if let Some((_, current)) = self.homes.iter_mut().find(|(home, _)| *home == place) {
                *current = claims;
            } else {
                self.homes.push((place, claims));
            }
            Ok(())
        }

        pub(crate) fn replace(
            &mut self,
            place: PlaceId,
            path: &[StructuralPathSegment],
            replacement: &[StructuralResultClaimBinding],
        ) -> Result<(), LoweringError> {
            let mut claims = self
                .source(&StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::Owned,
                })?
                .ok_or(LoweringError::Unsupported(
                    "storage update has no current whole-home namespace",
                ))?;
            claims.retain(|claim| !claim.path.starts_with(path));
            claims.extend(replacement.iter().map(|claim| {
                let mut destination = path.to_vec();
                destination.extend_from_slice(&claim.path);
                StructuralResultClaimBinding {
                    claim: claim.claim,
                    path: destination,
                }
            }));
            self.publish(place, claims)
        }

        pub(crate) fn with_edge_aliases(
            &self,
            aliases: &[super::claim_bindings::ClaimEdgeBinding],
        ) -> Result<Self, LoweringError> {
            let Some(first) = aliases.first() else {
                return Ok(self.clone());
            };
            if aliases.iter().any(|alias| alias.edge != first.edge) {
                return unsupported("selected storage aliases belong to different control edges");
            }
            for (position, alias) in aliases.iter().enumerate() {
                if aliases[..position].iter().any(|previous| {
                    (previous.destination == alias.destination && previous.source != alias.source)
                        || (previous.source == alias.source
                            && previous.destination != alias.destination)
                }) {
                    return unsupported(
                        "selected storage aliases have ambiguous source or destination names",
                    );
                }
            }
            let mut projected = self.clone();
            let mut observed = vec![false; aliases.len()];
            for (place, claims) in &self.homes {
                if self.unavailable.contains(place) {
                    continue;
                }
                let mut names = claims.clone();
                for (original, receiving) in claims.iter().zip(&mut names) {
                    for (position, alias) in aliases.iter().enumerate() {
                        if alias.source == original.claim {
                            receiving.claim = alias.destination;
                            observed[position] = true;
                        }
                    }
                }
                projected.publish(*place, names)?;
            }
            if observed.iter().any(|observed| !observed) {
                return unsupported("selected storage alias has no available current source name");
            }
            Ok(projected)
        }

        pub(crate) fn meet(&mut self, incoming: &Self) -> Result<(), LoweringError> {
            let mut homes = self
                .homes
                .iter()
                .chain(&incoming.homes)
                .map(|(place, _)| *place)
                .collect::<Vec<_>>();
            homes.extend(
                self.unavailable
                    .iter()
                    .chain(&incoming.unavailable)
                    .copied(),
            );
            homes.sort();
            homes.dedup();
            for place in homes {
                if self.unavailable.contains(&place) || incoming.unavailable.contains(&place) {
                    if !self.unavailable.contains(&place) {
                        self.unavailable.push(place);
                    }
                    continue;
                }
                let argument = StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::Owned,
                };
                let left = self.source(&argument)?;
                let right = incoming.source(&argument)?;
                match (left, right) {
                    (Some(left), Some(right)) if left == right => self.publish(place, left)?,
                    _ => self.unavailable.push(place),
                }
            }
            Ok(())
        }

        pub(crate) fn ingress(
            &mut self,
            parameters: impl IntoIterator<Item = PlaceId>,
            claims: &[terminal_psi::EntryClaim],
        ) -> Result<(), LoweringError> {
            for place in parameters {
                self.publish(
                    place,
                    claims
                        .iter()
                        .filter(|claim| claim.input == place)
                        .map(|claim| StructuralResultClaimBinding {
                            claim: claim.claim,
                            path: claim.path.clone(),
                        })
                        .collect(),
                )?;
            }
            Ok(())
        }
    }
}
pub(crate) mod descriptor_residences {

    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::{OperationId, PlaceId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        StructuralMultiplicity, TerminalAffineCleanupAction, TerminalMachine, TerminalModule,
    };
    use terminal_semantics::descriptor_lifetimes::{
        DescriptorLocation, DescriptorPosition, DescriptorResidenceRules,
        descriptor_residence_live_positions,
    };
    use terminal_semantics::descriptor_results::DescriptorAnalysis;

    pub(crate) mod normal_edges {

        use super::{LoweringError, Motion, PlaceId, Residence, unsupported};
        use semantic_vocabulary::StructuralTypeId;
        use std::collections::BTreeMap;
        use terminal_psi::{
            StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalMachine, Terminator,
        };
        use terminal_semantics::descriptor_lifetimes::{
            DescriptorFlowEdge, DescriptorFlowStep, DescriptorLocation, DescriptorPosition,
        };
        use terminal_semantics::descriptor_results::{DescriptorAnalysis, FlowFeasibility};

        pub(crate) struct Gap {
            pub(crate) position: DescriptorPosition,
            block: usize,
            roster: usize,
            ordinal: usize,
        }

        pub(crate) struct Action {
            pub(crate) position: DescriptorPosition,
            pub(crate) action: TerminalAffineCleanupAction,
            block: usize,
            roster: usize,
            ordinal: usize,
        }

        pub(crate) struct NormalEdges {
            pub(crate) flow: BTreeMap<DescriptorPosition, DescriptorFlowStep>,
            pub(crate) gaps: Vec<Gap>,
            pub(crate) actions: Vec<Action>,
            pub(crate) returned: BTreeMap<DescriptorPosition, PlaceId>,
        }

        impl NormalEdges {
            pub(crate) fn scheduled_flow(
                &self,
                motions: &[Motion],
            ) -> BTreeMap<DescriptorPosition, DescriptorFlowStep> {
                let mut flow = self.flow.clone();
                for action in &self.actions {
                    if let TerminalAffineCleanupAction::RetireHome(place) = action.action
                        && motions.iter().any(|motion| {
                            motion.place == place && !motion.retire.contains(&action.position)
                        })
                    {
                        flow.get_mut(&action.position).unwrap().inputs.clear();
                    }
                }
                for gap in &self.gaps {
                    flow.get_mut(&gap.position).unwrap().inputs.extend(
                        motions
                            .iter()
                            .filter(|motion| motion.retire.contains(&gap.position))
                            .map(|motion| DescriptorLocation::whole(motion.place)),
                    );
                }
                flow
            }

            pub(crate) fn new(
                machine: &TerminalMachine,
                types: &[StructuralTypeDeclaration],
                analysis: &mut DescriptorAnalysis<'_>,
                mut flow: BTreeMap<DescriptorPosition, DescriptorFlowStep>,
                residence_count: usize,
            ) -> Result<Self, LoweringError> {
                let mut gaps = Vec::new();
                let mut actions = Vec::new();
                let mut returned = BTreeMap::new();
                let feasibility = analysis.flow_feasibility(machine.id);
                for (block_position, block) in machine.blocks.iter().enumerate() {
                    let terminator = (
                        block.id,
                        u32::try_from(block.operations.len()).map_err(|_| {
                            LoweringError::Unsupported("descriptor residence block exceeds u32")
                        })?,
                    );
                    if feasibility.get(&terminator) == Some(&FlowFeasibility::ProvedImpossible) {
                        continue;
                    }
                    if let Terminator::ReturnStructural { source, .. } = &block.terminator {
                        returned.insert(terminator, *source);
                    }
                    if let Terminator::StructuralCase { source, path, .. } = &block.terminator {
                        let (location, opaque) = storage_observation(machine, types, *source, path);
                        let canonical = analysis.location(machine.id, terminator, &location);
                        let Some(step) = flow.get_mut(&terminator) else {
                            return unsupported("descriptor residence case has no terminator");
                        };
                        step.inputs.push(location);
                        step.inputs.extend(canonical);
                        step.may_escape |= opaque;
                    }
                    let rosters = block.terminator.cleanup_rosters().collect::<Vec<_>>();
                    let Some(original) = flow.get(&terminator) else {
                        return unsupported("descriptor residence graph lost a terminator");
                    };
                    let edges = original.successors.clone();
                    if !edges.is_empty() && edges.len() != rosters.len() {
                        return unsupported(
                            "descriptor residence cleanup and successor rosters disagree",
                        );
                    }
                    let mut next = terminator.1;
                    let mut starts = Vec::new();
                    for (roster, (_, cleanup)) in rosters.iter().enumerate() {
                        if !edges.is_empty()
                            && analysis.edge_feasibility(machine.id, terminator, roster)
                                == FlowFeasibility::ProvedImpossible
                        {
                            continue;
                        }
                        let mut positions = Vec::new();
                        for ordinal in 0..=cleanup.len() {
                            for _ in 0..residence_count {
                                let position = next_position(block.id, &mut next)?;
                                positions.push(position);
                                flow.insert(position, DescriptorFlowStep::default());
                                gaps.push(Gap {
                                    position,
                                    block: block_position,
                                    roster,
                                    ordinal,
                                });
                            }
                            if let Some(action) = cleanup.get(ordinal) {
                                let position = next_position(block.id, &mut next)?;
                                positions.push(position);
                                let mut step = DescriptorFlowStep::default();
                                let (location, opaque) = storage_observation(
                                    machine,
                                    types,
                                    action.place(),
                                    action.path(),
                                );
                                step.inputs.push(location.clone());
                                if let Some(canonical) =
                                    analysis.location(machine.id, terminator, &location)
                                    && canonical != location
                                {
                                    step.inputs.push(canonical);
                                }
                                step.may_escape = opaque
                                    || matches!(
                                        action,
                                        TerminalAffineCleanupAction::InvokeNominal(_)
                                    );
                                flow.insert(position, step);
                                actions.push(Action {
                                    position,
                                    action: action.clone(),
                                    block: block_position,
                                    roster,
                                    ordinal,
                                });
                            }
                        }
                        for pair in positions.windows(2) {
                            flow.get_mut(&pair[0])
                                .unwrap()
                                .successors
                                .push(plain_edge(pair[1]));
                        }
                        if let Some(edge) = edges.get(roster) {
                            flow.get_mut(positions.last().unwrap())
                                .unwrap()
                                .successors
                                .push(edge.clone());
                        }
                        starts.push(plain_edge(positions[0]));
                    }
                    if !rosters.is_empty() {
                        flow.get_mut(&terminator).unwrap().successors = starts;
                    }
                }
                Ok(Self {
                    flow,
                    gaps,
                    actions,
                    returned,
                })
            }

            pub(crate) fn install(
                &self,
                machine: &mut TerminalMachine,
                residences: &[Residence],
                motions: &[Motion],
            ) {
                for (block_position, block) in machine.blocks.iter_mut().enumerate() {
                    for (roster, actions) in cleanup_rosters_mut(&mut block.terminator)
                        .into_iter()
                        .enumerate()
                    {
                        let old = std::mem::take(actions);
                        for ordinal in 0..=old.len() {
                            for gap in self.gaps.iter().filter(|gap| {
                                gap.block == block_position
                                    && gap.roster == roster
                                    && gap.ordinal == ordinal
                            }) {
                                let mut additions = motions
                                    .iter()
                                    .filter(|motion| motion.retire.contains(&gap.position))
                                    .collect::<Vec<_>>();
                                additions.sort_by_key(|motion| {
                                    std::cmp::Reverse(
                                        residences
                                            .iter()
                                            .find(|home| home.place == motion.place)
                                            .unwrap()
                                            .producer,
                                    )
                                });
                                actions.extend(additions.into_iter().map(|motion| {
                                    TerminalAffineCleanupAction::RetireHome(motion.place)
                                }));
                            }
                            if let Some(action) = old.get(ordinal) {
                                let actual = self.actions.iter().find(|actual| {
                                    actual.block == block_position
                                        && actual.roster == roster
                                        && actual.ordinal == ordinal
                                });
                                let removed = actual.is_some_and(|actual| matches!(action, TerminalAffineCleanupAction::RetireHome(place)
                                    if motions.iter().any(|motion| motion.place == *place && !motion.retire.contains(&actual.position))));
                                if !removed {
                                    actions.push(action.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        fn next_position(
            block: semantic_vocabulary::BlockId,
            next: &mut u32,
        ) -> Result<DescriptorPosition, LoweringError> {
            *next = next.checked_add(1).ok_or(LoweringError::Unsupported(
                "descriptor residence cleanup coordinates exceed u32",
            ))?;
            Ok((block, *next))
        }

        fn plain_edge(target: DescriptorPosition) -> DescriptorFlowEdge {
            DescriptorFlowEdge {
                target,
                parameters: Vec::new(),
                bindings: Vec::new(),
            }
        }

        fn storage_observation(
            machine: &TerminalMachine,
            types: &[StructuralTypeDeclaration],
            place: PlaceId,
            path: &[terminal_psi::StructuralPathSegment],
        ) -> (DescriptorLocation, bool) {
            if let Some(ty) = place_type(machine, place)
                && let Some(location) =
                    terminal_semantics::descriptor_lifetimes::descriptor_location(
                        types, place, ty, path, None,
                    )
            {
                return (location, false);
            }
            (DescriptorLocation::whole(place), true)
        }

        fn place_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
            machine
                .structural_parameters
                .iter()
                .chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.structural_parameters),
                )
                .find(|parameter| parameter.place == place)
                .map(|parameter| parameter.structural_type)
                .or_else(|| {
                    machine
                        .structural_places
                        .iter()
                        .find(|declaration| declaration.id == place)
                        .and_then(|declaration| declaration.kind.produced_structural_type())
                })
        }

        fn cleanup_rosters_mut(
            terminator: &mut Terminator,
        ) -> Vec<&mut Vec<TerminalAffineCleanupAction>> {
            match terminator {
                Terminator::Jump {
                    cleanup_actions, ..
                }
                | Terminator::Return {
                    cleanup_actions, ..
                }
                | Terminator::ReturnUnit {
                    cleanup_actions, ..
                }
                | Terminator::ReturnStructural {
                    cleanup_actions, ..
                } => vec![cleanup_actions],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    &mut when_true.cleanup_actions,
                    &mut when_false.cleanup_actions,
                ],
                Terminator::StructuralCase { cases, .. } => cases
                    .iter_mut()
                    .map(|case| &mut case.cleanup_actions)
                    .collect(),
                Terminator::InvokeNoNormal { .. }
                | Terminator::Crash { .. }
                | Terminator::ExternalCompletion { .. } => Vec::new(),
            }
        }
    }

    use normal_edges::NormalEdges;

    pub(crate) struct Residence {
        place: PlaceId,
        producer: OperationId,
        birth: DescriptorPosition,
    }

    pub(crate) fn complete(module: &mut TerminalModule) -> Result<(), LoweringError> {
        let copyable_types =
            super::affine_cleanup::copyable_descriptor_type_ids(&module.structural_types);
        if copyable_types.is_empty() {
            return Ok(());
        }
        let machine_residences = module
            .machines
            .iter()
            .map(|machine| residences(machine, &copyable_types))
            .collect::<Result<Vec<_>, _>>()?;
        if machine_residences.iter().all(Vec::is_empty) {
            return Ok(());
        }
        let graph = terminal_semantics::descriptor_results::terminal_borrowed_storage_graph(module);
        let rules = module
            .machines
            .iter()
            .zip(&machine_residences)
            .map(|(machine, residences)| {
                if residences.is_empty() {
                    None
                } else {
                    terminal_semantics::descriptor_results::terminal_residence_rules(
                        module, machine,
                    )
                }
            })
            .collect::<Vec<_>>();
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
        for ((machine, residences), rules) in module
            .machines
            .iter_mut()
            .zip(machine_residences)
            .zip(rules)
        {
            if residences.is_empty() {
                continue;
            }
            let Some(mut rules) = rules else {
                continue;
            };
            let Some(flow) = analysis.residence_flow(machine.id, &mut rules) else {
                continue;
            };
            let normal = NormalEdges::new(
                machine,
                &module.structural_types,
                &mut analysis,
                flow,
                residences.len(),
            )?;
            let mut motions = Vec::<Motion>::new();
            loop {
                let scheduled = normal.scheduled_flow(&motions);
                let mut additions = Vec::new();
                for residence in &residences {
                    if let Some(motion) =
                        plan_residence(machine, &normal, &scheduled, &rules, residence, &motions)?
                    {
                        let previous = motions
                            .iter()
                            .find(|previous| previous.place == motion.place);
                        if previous.is_none_or(|previous| {
                            motion.live.is_subset(&previous.live) && motion.live != previous.live
                        }) {
                            additions.push(motion);
                        }
                    }
                }
                if additions.is_empty() {
                    break;
                }
                for motion in additions {
                    motions.retain(|previous| previous.place != motion.place);
                    motions.push(motion);
                }
            }
            normal.install(machine, &residences, &motions);
        }
        Ok(())
    }

    fn residences(
        machine: &TerminalMachine,
        copyable_types: &[semantic_vocabulary::StructuralTypeId],
    ) -> Result<Vec<Residence>, LoweringError> {
        let mut retire_only_places = BTreeMap::<PlaceId, bool>::new();
        for block in &machine.blocks {
            for (_, actions) in block.terminator.cleanup_rosters() {
                for action in actions {
                    let is_retire = matches!(action, TerminalAffineCleanupAction::RetireHome(_));
                    retire_only_places
                        .entry(action.place())
                        .and_modify(|all_retire| *all_retire &= is_retire)
                        .or_insert(is_retire);
                }
            }
        }
        if !retire_only_places.values().any(|&all_retire| all_retire) {
            return Ok(Vec::new());
        }
        let mut residences = Vec::new();
        for block in &machine.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                let Some(result) = operation.result.structural() else {
                    continue;
                };
                if result.multiplicity != StructuralMultiplicity::Unrestricted
                    || !result.claims.is_empty()
                    || !copyable_types.contains(&result.structural_type)
                {
                    continue;
                }
                if retire_only_places.get(&result.place) != Some(&true) {
                    continue;
                }
                if residences
                    .iter()
                    .any(|residence: &Residence| residence.place == result.place)
                {
                    return unsupported("descriptor residence has repeated operation births");
                }
                residences.push(Residence {
                    place: result.place,
                    producer: operation.id,
                    birth: (
                        block.id,
                        u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("descriptor residence ordinal exceeds u32")
                        })?,
                    ),
                });
            }
        }
        Ok(residences)
    }

    pub(crate) struct Motion {
        place: PlaceId,
        retire: BTreeSet<DescriptorPosition>,
        live: BTreeSet<DescriptorPosition>,
    }

    const UNBORN: u8 = 1;
    const LIVE: u8 = 2;
    const RETIRED: u8 = 4;

    fn plan_residence(
        machine: &TerminalMachine,
        normal: &NormalEdges,
        scheduled: &BTreeMap<
            DescriptorPosition,
            terminal_semantics::descriptor_lifetimes::DescriptorFlowStep,
        >,
        rules: &DescriptorResidenceRules,
        residence: &Residence,
        motions: &[Motion],
    ) -> Result<Option<Motion>, LoweringError> {
        let descriptor = DescriptorLocation::whole(residence.place);
        let previous = motions
            .iter()
            .find(|motion| motion.place == residence.place);
        let retirements = previous
            .map(|motion| motion.retire.clone())
            .unwrap_or_else(|| {
                normal
                    .actions
                    .iter()
                    .filter(|action| {
                        matches!(action.action,
                TerminalAffineCleanupAction::RetireHome(place) if place == residence.place)
                    })
                    .map(|action| action.position)
                    .collect()
            });
        let live = descriptor_residence_live_positions(
            scheduled,
            rules,
            residence.birth,
            &descriptor,
            |position, input| !(retirements.contains(&position) && input == &descriptor),
        );
        if live.contains(&residence.birth) {
            return Ok(None);
        }
        let baseline = residence_frontiers(machine, normal, residence, &BTreeSet::new())?;
        let cuts = normal
            .gaps
            .iter()
            .filter(|gap| {
                baseline.get(&gap.position) == Some(&LIVE) && !live.contains(&gap.position)
            })
            .map(|gap| gap.position)
            .collect::<BTreeSet<_>>();
        let planned = residence_frontiers(machine, normal, residence, &cuts)?;
        let mut retire = BTreeSet::new();
        for gap in &normal.gaps {
            let state = planned.get(&gap.position).copied().unwrap_or(0);
            if cuts.contains(&gap.position) {
                match state {
                    LIVE => {
                        retire.insert(gap.position);
                    }
                    UNBORN | RETIRED | 0 => {}
                    _ => {
                        return Ok(None);
                    }
                }
            }
        }
        for action in &normal.actions {
            if matches!(action.action, TerminalAffineCleanupAction::RetireHome(place) if place == residence.place)
            {
                match planned.get(&action.position).copied().unwrap_or(0) {
                    LIVE => {
                        retire.insert(action.position);
                    }
                    RETIRED => {}
                    _ => return Ok(None),
                }
            }
        }
        Ok(Some(Motion {
            place: residence.place,
            retire,
            live,
        }))
    }

    fn residence_frontiers(
        machine: &TerminalMachine,
        normal: &NormalEdges,
        residence: &Residence,
        cuts: &BTreeSet<DescriptorPosition>,
    ) -> Result<BTreeMap<DescriptorPosition, u8>, LoweringError> {
        let entry = (machine.entry, 0);
        let mut before = normal
            .flow
            .keys()
            .map(|position| (*position, 0))
            .collect::<BTreeMap<_, _>>();
        let Some(initial) = before.get_mut(&entry) else {
            return unsupported("descriptor residence graph has no entry");
        };
        *initial = UNBORN;
        let mut pending = vec![entry];
        while let Some(position) = pending.pop() {
            let step = &normal.flow[&position];
            let mut state = before[&position];
            if position == residence.birth {
                state = LIVE;
            }
            if cuts.contains(&position) {
                state = retire(state);
            }
            if normal.actions.iter().any(|action| {
                action.position == position
                    && matches!(action.action, TerminalAffineCleanupAction::RetireHome(place) if place == residence.place)
            }) || normal.returned.get(&position) == Some(&residence.place)
            {
                state = retire(state);
            }
            for edge in &step.successors {
                let Some(target) = before.get_mut(&edge.target) else {
                    return unsupported("descriptor residence graph lost a successor");
                };
                let joined = *target | state;
                if joined != *target {
                    *target = joined;
                    pending.push(edge.target);
                }
            }
        }
        Ok(before)
    }

    fn retire(state: u8) -> u8 {
        (state & !LIVE) | if state & LIVE != 0 { RETIRED } else { 0 }
    }
}
pub(crate) mod formation_memberships {

    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::shared::lowering_error::LoweringError;
    use semantic_vocabulary::{PlaceId, StructuralDomainId};

    pub(crate) fn publish(
        operations: &mut OperationBuffer,
        place: PlaceId,
        domains: Vec<StructuralDomainId>,
    ) -> Result<(), LoweringError> {
        let result = operations
            .operations
            .iter_mut()
            .find_map(|operation| match &mut operation.result {
                terminal_psi::OperationResult::Structural(result) if result.place == place => {
                    Some(result)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "fresh construction has no producer",
            ))?;
        result.qualifications.extend(domains);
        result.qualifications.sort();
        result.qualifications.dedup();
        Ok(())
    }
}
pub(crate) mod indexed_write {
    use super::{
        CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees, LoweringError,
        OperationKind, PlaceId, PrimitiveType, ScalarType, StructuralFieldId, StructuralFieldType,
        StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
        StructuralTypeId, StructuralTypeShape, ValueDeclaration, ValueId, allocate_dense,
        direct_expression_contains_short_circuit, emit_direct_expression, emit_u64_coordinate,
        obligation_id, terminal_scalar_type, unsupported, validate_direct_parameter_types,
        value_id,
    };
    use crate::emission::operation_emission::buffer::{OperationBuffer, scalar_result_declaration};
    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::expression_preparation::bindings::ScalarBindings;
    use checked_trees::{
        CheckedIndexedWriteDecisions, CheckedIntegerAssignmentDecision, CheckedScalarStoreValue,
        CheckedStoreLeaf, CheckedStorePlan,
    };
    use symbols::SymbolHandle;

    #[derive(Clone, Copy)]
    pub(crate) struct IndexedWriteLeaf<'a> {
        statement_index: u32,
        index: &'a CheckedScalarExpression,
        role: CheckedScalarExpressionRole,
        value: &'a CheckedScalarStoreValue,
        decisions: CheckedIndexedWriteDecisions,
        element: Option<(
            PrimitiveType,
            arena::Handle<CheckedIntegerAssignmentDecision>,
        )>,
    }

    #[derive(Clone, Copy)]
    pub(crate) enum IndexedWriteTarget<'a> {
        Bytes {
            store: &'a CheckedStorePlan,
            place: PlaceId,
            structural_type: StructuralTypeId,
        },
        ByteField(&'a CheckedStorePlan),
        ElementView {
            store: &'a CheckedStorePlan,
            place: PlaceId,
            structural_type: StructuralTypeId,
        },
    }

    pub(crate) struct IndexedWriteDecisions {
        pub(crate) index: bool,
        pub(crate) write: bool,
    }

    struct Wording {
        operands: &'static str,
    }

    const BYTE_VIEW: Wording = Wording {
        operands: "byte-view write requires exact mutable byte custody and scalar operands",
    };

    const BYTE_FIELD: Wording = Wording {
        operands: "byte replacement requires bounded bytes and exact branch-free operands",
    };

    const ELEMENT_VIEW: Wording = Wording {
        operands: "element-view write requires scalar operands",
    };

    impl<'a> IndexedWriteLeaf<'a> {
        pub(crate) fn source_decisions(self) -> IndexedWriteDecisions {
            IndexedWriteDecisions {
                index: self.decisions.index_in_bounds,
                write: self.decisions.predicates_preserved,
            }
        }

        pub(crate) fn statement_index(self) -> u32 {
            self.statement_index
        }

        pub(crate) fn index(self) -> &'a CheckedScalarExpression {
            self.index
        }

        pub(crate) fn value(self) -> &'a CheckedScalarStoreValue {
            self.value
        }

        pub(crate) fn value_type(self) -> PrimitiveType {
            self.element
                .map_or(PrimitiveType::U8, |(element, _)| element)
        }

        pub(crate) fn lower_value(
            self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            bindings: &ScalarBindings,
            checked: &CheckedTrees,
            state: SymbolHandle,
            values: &[ValueDeclaration],
        ) -> Result<LoweredDirectExpression, LoweringError> {
            crate::emission::structural_scalar_store_source::direct_value(
                identities,
                checked,
                state,
                self.statement_index,
                self.role,
                self.value,
                terminal_scalar_type(self.value_type())?,
                bindings,
                values,
            )
        }
    }

    pub(crate) struct Destination {
        place: PlaceId,
        pub(crate) descriptor_steps: Vec<super::runtime_elements::ProjectionStep>,
        value_type: ScalarType,
        shape: DestinationShape,
    }

    enum DestinationShape {
        ByteView,
        ByteField {
            path: Vec<StructuralPathSegment>,
            field: StructuralFieldId,
        },
        ElementView {
            descriptor: terminal_psi::StructuralArgument,
            path: Vec<StructuralPathSegment>,
            leaf: terminal_semantics::ElementViewLeaf,
        },
    }

    impl<'a> IndexedWriteTarget<'a> {
        pub(crate) fn store(self) -> &'a CheckedStorePlan {
            match self {
                Self::ByteField(store) => store,
                Self::Bytes { store, .. } | Self::ElementView { store, .. } => store,
            }
        }

        fn wording(self) -> &'static Wording {
            match self {
                Self::Bytes { .. } => &BYTE_VIEW,
                Self::ByteField(_) => &BYTE_FIELD,
                Self::ElementView { .. } => &ELEMENT_VIEW,
            }
        }

        pub(crate) fn leaf(self) -> Result<IndexedWriteLeaf<'a>, LoweringError> {
            let store = self.store();
            let Some((role, value)) = store.value.scalar() else {
                return unsupported("indexed write stores no scalar value");
            };
            let (index, decisions, element) = match &store.destination.leaf {
                CheckedStoreLeaf::ByteElement { index, decisions } => (index, *decisions, None),
                CheckedStoreLeaf::ViewElement {
                    index,
                    element,
                    range_decision,
                    decisions,
                    ..
                } => (index, *decisions, Some((*element, *range_decision))),
                _ => return unsupported("indexed write names no indexed element"),
            };
            Ok(IndexedWriteLeaf {
                statement_index: store.statement_index,
                index,
                role,
                value,
                decisions,
                element,
            })
        }

        pub(crate) fn destination(
            self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,

            parameters: &[(u32, StructuralParameterDeclaration)],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<Destination, LoweringError> {
            let operands = self.wording().operands;
            match self {
                Self::Bytes {
                    place,
                    structural_type,
                    ..
                } => {
                    if !structural_types.iter().any(|declaration| {
                        declaration.id == structural_type
                            && matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
                    }) {
                        return unsupported("byte write root has no byte carrier");
                    }
                    Ok(Destination {
                        place,
                        descriptor_steps: Vec::new(),
                        value_type: terminal_scalar_type(PrimitiveType::U8)?,
                        shape: DestinationShape::ByteView,
                    })
                }
                Self::ByteField(store) => {
                    let Some(position) = store.destination.root.parameter_position() else {
                        return unsupported("byte replacement destination has no parameter root");
                    };
                    let (carrier_path, field_identity) =
                        store.destination.field().ok_or(LoweringError::Unsupported(
                            "byte replacement destination names no byte field",
                        ))?;
                    let (_, parameter) = parameters
                        .iter()
                        .find(|(authored, _)| *authored == position)
                        .ok_or(LoweringError::Unsupported(
                            "byte replacement destination is absent",
                        ))?;
                    let (path, field) =
                        crate::emission::structural_scalar_store::lower_structural_field_place(
                            identities,
                            carrier_path,
                            field_identity,
                            parameter,
                            structural_types,
                        )?;
                    if !matches!(
                        field.field_type,
                        StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                | terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        ) | StructuralFieldType::PredicateByteSequence {
                            carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                            ..
                        }
                    ) {
                        return unsupported(operands);
                    }
                    Ok(Destination {
                        place: parameter.place,
                        descriptor_steps: Vec::new(),
                        value_type: terminal_scalar_type(PrimitiveType::U8)?,
                        shape: DestinationShape::ByteField {
                            path,
                            field: field.id,
                        },
                    })
                }
                Self::ElementView {
                    store,
                    place,
                    structural_type,
                } => {
                    let CheckedStoreLeaf::ViewElement { element_path, .. } =
                        &store.destination.leaf
                    else {
                        return unsupported("element-view write names no view element");
                    };
                    let (mut descriptor_steps, endpoint, _) =
                        super::structural_scalar_store::lower_window_path(
                            identities,
                            structural_type,
                            &store.destination.path,
                            structural_types,
                        )?;
                    let selected_type = match endpoint {
                        super::structural_scalar_store::WindowLeaf::Subtree { structural_type } => {
                            structural_type
                        }
                        super::structural_scalar_store::WindowLeaf::Field(field) => {
                            descriptor_steps.push(super::runtime_elements::ProjectionStep::Static(
                                StructuralPathSegment::Field(field.identity),
                            ));
                            field
                                .field_type
                                .as_structural()
                                .ok_or(LoweringError::Unsupported(
                                    "element-view destination field has no structural descriptor",
                                ))?
                        }
                    };
                    let element = structural_types
                        .iter()
                        .find_map(|declaration| match declaration.shape {
                            StructuralTypeShape::ElementView { element, .. }
                                if declaration.id == selected_type =>
                            {
                                Some(element)
                            }
                            _ => None,
                        })
                        .ok_or(LoweringError::Unsupported(
                            "element-view write destination is not an element view",
                        ))?;
                    let path = crate::expression_preparation::bindings::structural_paths::lower_structural_path(
                identities,element_path)?;
                    let leaf =
                        terminal_semantics::element_view_leaf(structural_types, element, &path)
                            .ok_or(LoweringError::Unsupported(
                                "element-view write path is not an admitted scalar leaf",
                            ))?;
                    Ok(Destination {
                        place,
                        descriptor_steps,
                        value_type: leaf.scalar_type,
                        shape: DestinationShape::ElementView {
                            descriptor: terminal_psi::StructuralArgument::whole(
                                place,
                                terminal_psi::StructuralAccess::MutableBorrow,
                            ),
                            path,
                            leaf,
                        },
                    })
                }
            }
        }
    }

    impl Destination {
        pub(crate) fn capture_descriptor(
            &mut self,
            source: PlaceId,
            mut path: Vec<StructuralPathSegment>,
        ) -> Result<(), LoweringError> {
            match &mut self.shape {
                DestinationShape::ElementView { descriptor, .. } => {
                    descriptor.place = source;
                    descriptor.path = path;
                }
                DestinationShape::ByteView if path.is_empty() => {}
                DestinationShape::ByteField { .. } if path.is_empty() => {
                    self.shape = DestinationShape::ByteView;
                }
                DestinationShape::ByteField { path: carrier, .. } => {
                    if !matches!(path.pop(), Some(StructuralPathSegment::Field(_))) {
                        return unsupported(
                            "captured byte field lost its declared field projection",
                        );
                    }
                    *carrier = path;
                }
                _ => return unsupported("captured byte descriptor is not a whole view"),
            }
            self.place = source;
            self.descriptor_steps.clear();
            Ok(())
        }

        pub(crate) fn set_descriptor_path(&mut self, path: Vec<StructuralPathSegment>) {
            if let DestinationShape::ElementView { descriptor, .. } = &mut self.shape {
                descriptor.path = path;
            }
        }

        pub(crate) fn source_decisions(
            &self,
            checked: &CheckedTrees,
            leaf: IndexedWriteLeaf<'_>,
        ) -> Result<IndexedWriteDecisions, LoweringError> {
            let mut decisions = leaf.source_decisions();
            if let (Some((element, range_decision)), DestinationShape::ElementView { leaf, .. }) =
                (leaf.element, &self.shape)
            {
                decisions.write = decisions.index
                    && match &leaf.restriction {
                        None => true,
                        Some(terminal_psi::StructuralFieldType::BoundedInteger(bounds)) => {
                            super::structural_scalar_store::source_range_decided(
                                checked,
                                element,
                                range_decision,
                                Some(*bounds),
                            )?
                        }
                        Some(_) => false,
                    };
            }
            Ok(decisions)
        }

        fn length(
            &self,
            next_value: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<ValueId, LoweringError> {
            let kind = match &self.shape {
                DestinationShape::ByteView => OperationKind::ByteSequenceLength {
                    source: terminal_psi::StructuralArgument::whole(
                        self.place,
                        terminal_psi::StructuralAccess::SharedBorrow,
                    ),
                },
                DestinationShape::ByteField { path, field } => {
                    if let Some(length) = operations
                        .storage_observations
                        .field_bytes
                        .iter()
                        .rev()
                        .find_map(|(place, carrier, named, value)| {
                            (*place == self.place && carrier == path && named == field)
                                .then_some(*value)
                        })
                    {
                        return Ok(length);
                    }
                    OperationKind::StructuralByteSequenceFieldLength {
                        source: self.place,
                        path: path.clone(),
                        field: *field,
                    }
                }
                DestinationShape::ElementView { descriptor, .. } => {
                    OperationKind::ElementViewLength {
                        source: descriptor.clone(),
                    }
                }
            };
            let length = value_id(allocate_dense(next_value)?);
            operations.push_leaf(
                scalar_result_declaration(length, terminal_scalar_type(PrimitiveType::U64)?),
                kind,
            );
            if let DestinationShape::ByteField { path, field } = &self.shape {
                operations.storage_observations.field_bytes.push((
                    self.place,
                    path.clone(),
                    *field,
                    length,
                ));
            }
            Ok(length)
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum IndexedWriteValue<'a> {
        Expression(&'a LoweredDirectExpression),
        Established(ValueDeclaration),
    }

    pub(crate) fn emit(
        target: IndexedWriteTarget<'_>,
        destination: Destination,
        decisions: IndexedWriteDecisions,
        index: &LoweredDirectExpression,
        value: IndexedWriteValue<'_>,
        values: &[ValueDeclaration],
        next_value: &mut u64,
        next_obligation: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<OperationKind, LoweringError> {
        let operands = target.wording().operands;
        let value_type = match value {
            IndexedWriteValue::Expression(expression) => expression.scalar_type(),
            IndexedWriteValue::Established(value) => value.scalar_type,
        };
        if !matches!(index.scalar_type(), ScalarType::Integer(_))
            || value_type != destination.value_type
            || direct_expression_contains_short_circuit(index)
            || matches!(value, IndexedWriteValue::Expression(expression)
                if direct_expression_contains_short_circuit(expression))
        {
            return unsupported(operands);
        }
        let types = values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        validate_direct_parameter_types(index, &types)?;
        if let IndexedWriteValue::Expression(expression) = value {
            validate_direct_parameter_types(expression, &types)?;
        }
        let index = emit_index_coordinate(
            index,
            false,
            values,
            next_value,
            next_obligation,
            operations,
        )?;
        let value = match value {
            IndexedWriteValue::Expression(expression) => {
                emit_direct_expression(expression, values, next_value, operations)
            }
            IndexedWriteValue::Established(value) => value.id,
        };
        let length = destination.length(next_value, operations)?;
        let obligation = obligation_id(allocate_dense(next_obligation)?);
        if decisions.write {
            operations.source_checked_obligations.push(obligation);
        }
        let place = destination.place;
        Ok(match destination.shape {
            DestinationShape::ByteView => OperationKind::ByteSequenceWrite {
                destination: place,
                index,
                value,
                length,
                obligation,
            },
            DestinationShape::ByteField { path, field } => {
                OperationKind::StructuralByteSequenceFieldByteStore {
                    destination: place,
                    path,
                    field,
                    index,
                    value,
                    length,
                    obligation,
                }
            }
            DestinationShape::ElementView {
                descriptor, path, ..
            } => OperationKind::ElementViewWrite {
                destination: descriptor,
                path,
                index,
                value,
                length,
                obligation,
            },
        })
    }

    pub(crate) fn emit_index_coordinate(
        expression: &LoweredDirectExpression,
        bound_decided: bool,
        values: &[ValueDeclaration],
        next_value: &mut u64,
        next_obligation: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<ValueId, LoweringError> {
        let operand = emit_direct_expression(expression, values, next_value, operations);
        emit_u64_coordinate(
            operand,
            expression.scalar_type(),
            bound_decided,
            next_value,
            next_obligation,
            operations,
        )
    }
}
pub(crate) mod loan_completions {

    use crate::emission::operation_emission::buffer::{
        OperationBuffer, SourceCallCoordinate, SourceInvocation,
    };
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::shared::checked_facts::flow_facts;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::lookup_domain_id_for_carrier;
    use checked_trees::CheckedTrees;
    use language_semantics::{PermissionClaimIdentity, PermissionEventKind, PermissionEventSource};
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use symbols::SymbolHandle;
    use terminal_psi::{
        LoanCompletion, StructuralArgument, StructuralMultiplicity, StructuralPathSegment,
        StructuralResultClaimBinding, StructuralTypeDeclaration,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct PreparedLoanCompletion {
        pub(crate) place: PlaceId,
        pub(crate) path: Vec<StructuralPathSegment>,
        pub(crate) multiplicity: StructuralMultiplicity,
        pub(crate) claims: Vec<(PermissionClaimIdentity, Vec<StructuralPathSegment>, bool)>,
        pub(crate) qualifications: Vec<(
            language_semantics::SemanticDomainId,
            Vec<StructuralPathSegment>,
            StructuralTypeId,
        )>,
    }

    pub(crate) fn qualification_facts<'a>(
        checked: &'a CheckedTrees,
        region: &'a checked_trees::FlowLoanCompletionFact,
    ) -> impl Iterator<Item = &'a facts::Fact> {
        checked
            .facts
            .semantic
            .facts
            .iter()
            .filter_map(move |(_, fact)| {
                let facts::ProgramPoint::CallEnsures {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    call_ordinal,
                } = fact.point
                else {
                    return None;
                };
                if fact.origin != facts::FactOrigin::CallEnsures
                    || machine_symbol != region.machine
                    || state_symbol != region.state
                    || !matches!(region.source, PermissionEventSource::Call {
                    statement_index: source_statement, call_ordinal: source_ordinal, ..
                } if source_statement == statement_index && source_ordinal == call_ordinal)
                    || fact.payload.domain_membership().is_none()
                {
                    return None;
                }
                let place = checked.facts.semantic.places.get(fact.place.as_place()?);
                let segments = checked
                    .facts
                    .semantic
                    .place_segments
                    .span_or_empty(place.segments);
                let region_segments = flow_facts(checked)
                    .ownership
                    .segments
                    .span_or_empty(region.segments);
                (place.root == region.root && segments.starts_with(region_segments)).then_some(fact)
            })
    }

    pub(crate) fn prepare(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: SymbolHandle,
        coordinate: SourceCallCoordinate,
        types: &[StructuralTypeDeclaration],
        mut owned_home: impl FnMut(
            facts::PlaceRoot,
        )
            -> Result<(StructuralArgument, StructuralTypeId), LoweringError>,
    ) -> Result<Vec<PreparedLoanCompletion>, LoweringError> {
        let mut completions = Vec::new();
        for (_, region) in checked.facts.flow.ownership.loan_completions.iter() {
            if region.machine != machine
                || region.state != coordinate.state
                || !matches!(region.source, PermissionEventSource::Call { statement_index, call_ordinal, .. }
                    if statement_index == coordinate.statement_index && call_ordinal == coordinate.call_ordinal)
            {
                continue;
            }
            let reference = validation::canonical_place_type_reference(
                &checked.typed,
                coordinate.state,
                coordinate.statement_index,
                &validation::CanonicalPlace {
                    root: region.root,
                    segments: Vec::new(),
                },
            )
            .ok_or(LoweringError::Unsupported(
                "loan completion lost its owned root type",
            ))?;
            let (owner, structural_type) = owned_home(region.root)?;
            if owner.access != terminal_psi::StructuralAccess::Owned {
                return unsupported("loan completion cannot establish borrowed ingress custody");
            }
            let canonical = flow_facts(checked)
                .ownership
                .segments
                .span_or_empty(region.segments);
            let (path, _) = validation::structural_owned_path(&checked.typed, reference, canonical)
                .map_err(LoweringError::Unsupported)?;
            let mut completed_path = owner.path.clone();
            completed_path.extend(
                crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities,
                    &path,
                    structural_type,
                    types,
                )?,
            );
            let mut claims = Vec::new();
            let state_permissions = flow_facts(checked)
                .ownership
                .with_state_permissions(coordinate.state, |handles| handles.to_vec());
            for handle in state_permissions {
                let event = checked.facts.flow.ownership.permissions.get(handle);
                let event_path = flow_facts(checked)
                    .ownership
                    .segments
                    .span_or_empty(event.segments);
                if event.machine_symbol != machine
                    || event.state_symbol != coordinate.state
                    || event.source != region.source
                    || event.root != region.root
                    || event.kind != PermissionEventKind::Restore
                    || !event_path.starts_with(canonical)
                {
                    continue;
                }
                let path = validation::structural_claim_path(&checked.typed, reference, event_path)
                    .map_err(LoweringError::Unsupported)?;
                let mut terminal_path = owner.path.clone();
                terminal_path.extend(crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities, &path, structural_type, types,
                )?);
                let introduces = crate::emission::claim_bindings::claim_established_here(
                    machine,
                    coordinate.state,
                    coordinate.statement_index,
                    event,
                );
                claims.push((event.claim_identity, terminal_path, introduces));
            }
            let mut qualifications = Vec::new();
            for fact in qualification_facts(checked, region) {
                let representation =
                    super::call_qualifications::declaration_represents(identities, checked, fact)?;
                if representation.is_complete() {
                    continue;
                }
                let Some(place_handle) = fact.place.as_place() else {
                    continue;
                };
                let place = checked.facts.semantic.places.get(place_handle);
                let segments = checked
                    .facts
                    .semantic
                    .place_segments
                    .span_or_empty(place.segments);
                let Some((_, _, domain_symbol, semantic_domain)) = fact.payload.domain_membership()
                else {
                    continue;
                };
                let mut fact_path =
                    validation::qualification_storage_path(&checked.typed, reference, segments)
                        .map_err(|reason| {
                            LoweringError::qualification_storage_path(
                                checked,
                                coordinate,
                                reference,
                                segments,
                                reason,
                                representation,
                            )
                        })?;
                let selected = validation::project_type_reference_from_segments(
                    &checked.typed,
                    reference,
                    segments,
                )
                .ok_or(LoweringError::Unsupported(
                    "loan completion qualification lost its selected source type",
                ))?;
                let (membership_path, _) =
                    validation::result_membership_carrier(&checked.typed, selected, domain_symbol)
                        .ok_or(LoweringError::Unsupported(
                            "loan completion qualification has no matching domain carrier",
                        ))?;
                fact_path.extend(membership_path);
                let mut full_path = owner.path.clone();
                full_path.extend(
                    crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                        identities,
                        &fact_path,
                        structural_type,
                        types,
                    )?,
                );
                let carrier = if full_path.is_empty() {
                    structural_type
                } else {
                    terminal_semantics::qualification_path_carrier(
                        types.iter(),
                        structural_type,
                        &full_path,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "loan completion qualification has no exact projected carrier",
                    ))?
                };
                qualifications.push((semantic_domain, full_path, carrier));
            }
            completions.push(PreparedLoanCompletion {
                place: owner.place,
                path: completed_path,
                claims,
                qualifications,
                multiplicity: match region.multiplicity {
                    language_semantics::Multiplicity::Unrestricted => {
                        StructuralMultiplicity::Unrestricted
                    }
                    language_semantics::Multiplicity::Affine => StructuralMultiplicity::Affine,
                    language_semantics::Multiplicity::Linear => StructuralMultiplicity::Linear,
                },
            });
        }
        Ok(completions)
    }

    pub(crate) fn emit(
        prepared: &[PreparedLoanCompletion],
        coordinate: impl Into<SourceInvocation>,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
        current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
    ) -> Result<(), LoweringError> {
        if prepared.is_empty() {
            return Ok(());
        }
        let coordinate = coordinate
            .into()
            .statement()
            .ok_or(LoweringError::Unsupported(
                "loan completion has no statement custody",
            ))?;
        let occurrence = operations
            .source_calls
            .iter()
            .find(|occurrence| {
                occurrence.matches_statement(
                    coordinate.state,
                    coordinate.statement_index,
                    coordinate.call_ordinal,
                )
            })
            .ok_or(LoweringError::Unsupported(
                "loan completion has no emitted call",
            ))?;
        let lowered_psi::LoweredSourceCallSite::Operation(operation) = occurrence.terminal_site
        else {
            return unsupported("loan completion has no returning operation");
        };
        for completion in prepared {
            let mut claims = completion
                .claims
                .iter()
                .map(|(identity, path, introduces)| {
                    let claim = calls.claims.receive_loan_completion_claim(*identity)?;
                    if *introduces {
                        operations.claim_introductions.push(
                            crate::emission::claim_bindings::ClaimIntroduction { operation, claim },
                        );
                    }
                    Ok(StructuralResultClaimBinding {
                        claim,
                        path: path.clone(),
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            claims.sort();
            let relative = claims
                .iter()
                .map(|claim| {
                    let path = claim.path.strip_prefix(completion.path.as_slice()).ok_or(
                        LoweringError::Unsupported(
                            "loan completion claim escapes its exact region",
                        ),
                    )?;
                    Ok(StructuralResultClaimBinding {
                        claim: claim.claim,
                        path: path.to_vec(),
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            current_claims.replace(completion.place, &completion.path, &relative)?;
            operations.loan_completions.push(LoanCompletion {
                operation,
                place: completion.place,
                path: completion.path.clone(),
                multiplicity: completion.multiplicity,
                claims,
            });

            let mut qualifications = Vec::new();
            let mut projected_qualifications = Vec::new();
            for (semantic_domain, path, carrier) in &completion.qualifications {
                let domain =
                    lookup_domain_id_for_carrier(calls.domain_ids, *semantic_domain, *carrier)?;
                if path.is_empty() {
                    qualifications.push(domain);
                } else {
                    projected_qualifications.push(terminal_psi::StructuralPathQualification {
                        path: path.clone(),
                        domain,
                    });
                }
            }
            qualifications.sort();
            qualifications.dedup();
            projected_qualifications.sort();
            projected_qualifications.dedup();
            if !qualifications.is_empty() || !projected_qualifications.is_empty() {
                operations
                    .record_call_memberships(
                        coordinate,
                        completion.place,
                        &qualifications,
                        &projected_qualifications,
                    )
                    .map_err(LoweringError::InvalidCallMembershipSource)?;
            }
        }
        operations.loan_completions.sort_by(|left, right| {
            (left.operation, left.place, &left.path).cmp(&(
                right.operation,
                right.place,
                &right.path,
            ))
        });
        Ok(())
    }
}
pub(crate) mod primitive_store {
    use super::{
        CheckedScalarExpressionRole, CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError,
        OperationKind, PlaceId, PrimitiveType, ScalarType, StructuralFieldType,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
        StructuralTypeId, StructuralTypeShape, ValueDeclaration, terminal_scalar_type, unsupported,
    };
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::emission::runtime_elements::ProjectionStep;
    use crate::shared::structural_types::unique_declaration;
    use terminal_psi::live_field;

    pub(crate) struct ReceivingPlace {
        pub(crate) root_type: StructuralTypeId,
        pub(crate) qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        pub(crate) projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        pub(crate) additional_input_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
    }

    pub(crate) struct Destination {
        pub(crate) logical_receiving: Option<ReceivingPlace>,
        pub(crate) root_type: StructuralTypeId,
        pub(crate) qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        pub(crate) projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        pub(crate) additional_input_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        pub(crate) place: PlaceId,
        pub(crate) path: Vec<ProjectionStep>,
        pub(crate) scalar_type: ScalarType,
    }

    pub(crate) fn parameter_destination(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        parameter: &StructuralParameterDeclaration,
        path: &[CheckedUnitStructuralPathSegment],
        types: &[StructuralTypeDeclaration],
        scalar_type: ScalarType,
    ) -> Result<Destination, LoweringError> {
        let writable = matches!(
            parameter.access,
            terminal_psi::exclusive_borrow_pair_pattern!()
        ) || (!path.is_empty()
            && parameter.access == super::StructuralAccess::Owned);
        if !writable
            || parameter.multiplicity == StructuralMultiplicity::Linear
            || (path.is_empty() && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
        {
            return unsupported("primitive store parameter lost its exclusive custody");
        }
        let (path, _) = lower_destination_path(identities, parameter.structural_type, path, types)?;
        Ok(Destination {
            logical_receiving: None,
            root_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            projected_qualifications: parameter.projected_qualifications.clone(),
            additional_input_qualifications: parameter.additional_input_qualifications.to_vec(),
            place: parameter.place,
            path,
            scalar_type,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_assignment(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: u32,
        destination: Destination,
        structural_types: &[StructuralTypeDeclaration],
        preserves_byte_memberships: bool,
        receiving_range: Option<&checked_trees::CheckedPrimitiveReceivingRange>,
        role: CheckedScalarExpressionRole,
        value: &checked_trees::CheckedScalarStoreValue,
        evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
        environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
    ) -> Result<OperationKind, LoweringError> {
        let call_result =
            if let checked_trees::CheckedScalarStoreValue::ScalarResult { position } = value {
                let position = usize::try_from(*position).map_err(|_| {
                    LoweringError::Unsupported("primitive store call-result position exceeds usize")
                })?;
                if position.checked_add(1) != Some(values.len())
                    || values[position].scalar_type != destination.scalar_type
                {
                    return unsupported("primitive store lost its exact established call result");
                }
                Some(position)
            } else {
                None
            };
        let indexes = super::runtime_elements::evaluate_selectors(
            identities,
            &destination.path,
            checked,
            machine,
            state,
            statement_index,
            evaluation,
            source_value_count,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
            environment,
        )?;
        let value = if let Some(position) = call_result {
            let value = *values.get(position).ok_or(LoweringError::Unsupported(
                "primitive store lost its call-result slot after selector evaluation",
            ))?;
            if value.scalar_type != destination.scalar_type {
                return unsupported(
                    "primitive store call-result carrier changed after selector evaluation",
                );
            }
            value
        } else {
            let value = match value {
                checked_trees::CheckedScalarStoreValue::Pure(expression) => {
                    checked_trees::CheckedCallScalarArgument::Pure(expression.clone())
                }
                checked_trees::CheckedScalarStoreValue::Computation(root) => {
                    checked_trees::CheckedCallScalarArgument::Computation(*root)
                }
                checked_trees::CheckedScalarStoreValue::ScalarResult { .. } => {
                    unreachable!("captured call result")
                }
            };
            evaluation.source_value(
                identities,
                checked,
                machine,
                state,
                statement_index,
                role,
                &value,
                source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
                environment,
            )?
        };
        let path =
            super::runtime_elements::complete_path(destination.path, indexes, calls, operations)?;
        let logical_receiving = destination.logical_receiving;
        let physical = ReceivingPlace {
            root_type: destination.root_type,
            qualifications: destination.qualifications,
            projected_qualifications: destination.projected_qualifications,
            additional_input_qualifications: destination.additional_input_qualifications,
            path: path.clone(),
        };
        let logical = logical_receiving.as_ref().unwrap_or(&physical);
        let receiving_type = terminal_semantics::runtime_structural_field(
            structural_types.iter(),
            logical.root_type,
            &logical.path,
        )
        .and_then(|field| {
            evaluation
                .shared_qualifications
                .catalog()
                .fields
                .iter()
                .find(|row| row.field == field.id)
                .map(|row| row.value_type)
        });
        let value = if let Some(receiving_type) = receiving_type {
            evaluation.receiving_value(
                identities,
                value,
                receiving_type,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
                environment,
            )?
        } else {
            value
        };
        let logical_scalar = declared_restriction(logical, structural_types);
        let logical_restriction = receiving_restriction(logical, calls)?;
        let byte_predicates = terminal_semantics::primitive_store_byte_predicates(
            calls.structural_types.iter(),
            calls.domain_declarations.iter(),
            logical.root_type,
            &logical.qualifications,
            &logical.projected_qualifications,
            &logical.additional_input_qualifications,
            &logical.path,
        )
        .ok_or(LoweringError::Unsupported(
            "primitive write lost its declared byte receiving component",
        ))?;
        let has_byte_predicates = byte_predicates.predicates().next().is_some();
        if has_byte_predicates
            && logical_restriction != logical_scalar
            && !preserves_byte_memberships
        {
            return unsupported(
                "primitive write lost its exact logical byte preservation judgment",
            );
        }
        if let Some(StructuralFieldType::BoundedInteger(integer)) = &logical_scalar
            && receiving_range.is_none_or(|range| range.integer != *integer)
        {
            return unsupported("primitive store lost its logical receiving-range judgment");
        }
        let logical_proven = receiving_range
            .map(|range| StructuralFieldType::BoundedInteger(range.integer))
            .or_else(|| {
                (has_byte_predicates && preserves_byte_memberships)
                    .then(|| logical_restriction.clone())
                    .flatten()
            });
        emit_completed_assignment(
            machine,
            state,
            statement_index,
            PhysicalDestination {
                place: destination.place,
                root_type: physical.root_type,
                qualifications: physical.qualifications,
                projected_qualifications: physical.projected_qualifications,
                additional_input_qualifications: physical.additional_input_qualifications,
                path,
                logical_receiving,
            },
            destination.scalar_type,
            value,
            logical_proven.as_ref(),
            structural_types,
            next_value,
            operations,
            calls,
        )
    }

    pub(crate) struct PhysicalDestination {
        pub(crate) place: PlaceId,
        pub(crate) root_type: StructuralTypeId,
        pub(crate) qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        pub(crate) projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        pub(crate) additional_input_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
        pub(crate) logical_receiving: Option<ReceivingPlace>,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_completed_assignment(
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: u32,
        destination: PhysicalDestination,
        scalar_type: ScalarType,
        value: ValueDeclaration,
        proven_restriction: Option<&StructuralFieldType>,
        structural_types: &[StructuralTypeDeclaration],
        next_value: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<OperationKind, LoweringError> {
        let physical = ReceivingPlace {
            root_type: destination.root_type,
            qualifications: destination.qualifications,
            projected_qualifications: destination.projected_qualifications,
            additional_input_qualifications: destination.additional_input_qualifications,
            path: destination.path.clone(),
        };
        let actual = receiving_scalar(&physical, structural_types);
        if actual != Some(scalar_type) {
            return Err(LoweringError::CapturedScalarStoreEndpoint {
                machine,
                state,
                statement_index,
                destination: destination.place,
                root_type: destination.root_type,
                path: destination.path,
                expected: scalar_type,
                actual,
            });
        }
        let restriction = receiving_restriction(&physical, calls)?;
        if let Some(logical) = &destination.logical_receiving {
            let logical_restriction = receiving_restriction(logical, calls)?;
            if let Some(required) = logical_restriction.as_ref()
                && restriction.as_ref() != Some(required)
                && !proven_restriction.is_some_and(|proven| receiving_entails(proven, required))
            {
                return unsupported("completed scalar store lost its logical receiving judgment");
            }
        }
        let range_obligation = if let Some(required) = restriction.as_ref() {
            let obligation =
                super::obligation_id(super::allocate_dense(&mut calls.next_obligation_identity)?);
            if proven_restriction.is_some_and(|proven| receiving_entails(proven, required)) {
                operations.source_checked_obligations.push(obligation);
            }
            Some(obligation)
        } else {
            None
        };
        super::runtime_elements::observe_element_extent(
            destination.place,
            destination.root_type,
            &destination.path,
            structural_types,
            next_value,
            operations,
        )?;
        if let Some(field) = terminal_semantics::runtime_structural_field(
            structural_types.iter(),
            destination.root_type,
            &destination.path,
        )
        .filter(|field| {
            matches!(
                field.field_type,
                StructuralFieldType::Scalar(ScalarType::Boolean | ScalarType::Integer(_))
                    | StructuralFieldType::BoundedInteger(_)
                    | StructuralFieldType::RefinedScalar(_)
            )
        }) {
            return Ok(OperationKind::StructuralScalarFieldStore {
                destination: destination.place,
                path: destination.path[..destination.path.len() - 1].to_vec(),
                field: field.id,
                value: value.id,
                range_obligation,
            });
        }
        Ok(OperationKind::WriteOnlyPrimitiveStore {
            range_obligation,
            destination: destination.place,
            path: destination.path,
            value: value.id,
        })
    }

    pub(crate) fn receiving_scalar(
        place: &ReceivingPlace,
        types: &[StructuralTypeDeclaration],
    ) -> Option<ScalarType> {
        terminal_semantics::runtime_structural_field(types.iter(), place.root_type, &place.path)
            .and_then(|field| field.field_type.scalar_type())
            .or_else(|| {
                terminal_semantics::primitive_projection_type(
                    types.iter(),
                    place.root_type,
                    &place.path,
                )
            })
    }

    fn declared_restriction(
        place: &ReceivingPlace,
        types: &[StructuralTypeDeclaration],
    ) -> Option<StructuralFieldType> {
        terminal_semantics::primitive_projection_restriction(
            types.iter(),
            place.root_type,
            &place.path,
        )
        .cloned()
        .or_else(|| {
            terminal_semantics::runtime_structural_field(types.iter(), place.root_type, &place.path)
                .filter(|field| {
                    terminal_semantics::scalar_field_requires_evidence(&field.field_type)
                })
                .map(|field| field.field_type.clone())
        })
    }

    pub(crate) fn receiving_restriction(
        place: &ReceivingPlace,
        calls: &CallEmissionContext<'_>,
    ) -> Result<Option<StructuralFieldType>, LoweringError> {
        let restriction = terminal_semantics::primitive_store_restriction(
            calls.structural_types.iter(),
            calls.domain_declarations.iter(),
            place.root_type,
            &place.qualifications,
            &place.projected_qualifications,
            &place.additional_input_qualifications,
            &place.path,
        )
        .ok_or(LoweringError::Unsupported(
            "primitive write does not preserve enclosing qualifications",
        ))?;
        let declared = declared_restriction(place, calls.structural_types);
        match (restriction, declared) {
            (None, declared) => Ok(declared),
            (Some(restriction), None) => Ok(Some(restriction)),
            (Some(restriction), Some(declared)) if restriction == declared => Ok(Some(restriction)),
            _ => unsupported("primitive write has incompatible receiving predicates"),
        }
    }

    fn receiving_entails(proven: &StructuralFieldType, required: &StructuralFieldType) -> bool {
        terminal_semantics::scalar_field_entails(proven, required)
    }

    pub(crate) fn lower_destination_path(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        structural_type: StructuralTypeId,
        path: &[CheckedUnitStructuralPathSegment],
        types: &[StructuralTypeDeclaration],
    ) -> Result<(Vec<ProjectionStep>, ScalarType), LoweringError> {
        let walked = walk_path(identities, structural_type, path, types)?;
        if walked.selected_case.is_some() {
            return unsupported("primitive projection ends at a case without its payload field");
        }
        let scalar_type = match walked.position {
            Walked::Declared(structural_type) => {
                let StructuralTypeShape::PrimitiveScalar(scalar_type) = unique_declaration(
                    types,
                    structural_type,
                    "primitive projection type is absent",
                    "primitive projection type is ambiguous",
                )?
                .shape
                else {
                    return unsupported("primitive projection does not end at a primitive scalar");
                };
                scalar_type
            }
            Walked::ByteElement => terminal_scalar_type(PrimitiveType::U8)?,
            Walked::ScalarField(scalar) => scalar,
        };
        Ok((walked.steps, scalar_type))
    }

    pub(crate) fn lower_path(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        structural_type: StructuralTypeId,
        path: &[CheckedUnitStructuralPathSegment],
        types: &[StructuralTypeDeclaration],
    ) -> Result<(Vec<terminal_psi::StructuralPathSegment>, ScalarType), LoweringError> {
        if let Some((CheckedUnitStructuralPathSegment::Field(identity), carrier)) =
            path.split_last()
        {
            let walked = walk_path(identities, structural_type, carrier, types)?;
            let mut path = super::runtime_elements::static_segments(walked.steps)?;
            path.push(terminal_psi::StructuralPathSegment::Field(
                identities.intern(&identity.spelling()),
            ));
            let scalar_type = terminal_semantics::primitive_read_projection_type(
                types.iter(),
                structural_type,
                &path,
            )
            .ok_or(LoweringError::Unsupported(
                "primitive observation has no readable scalar endpoint",
            ))?;
            return Ok((path, scalar_type));
        }
        let (steps, scalar_type) =
            lower_destination_path(identities, structural_type, path, types)?;
        Ok((
            super::runtime_elements::static_segments(steps)?,
            scalar_type,
        ))
    }

    pub(crate) fn indexed_element_scalar_type(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        element: StructuralTypeId,
        element_path: &[CheckedUnitStructuralPathSegment],
        types: &[StructuralTypeDeclaration],
    ) -> Result<ScalarType, LoweringError> {
        if let Some((CheckedUnitStructuralPathSegment::Field(identity), carrier)) =
            element_path.split_last()
        {
            let walked = walk_path(identities, element, carrier, types)?;
            let Walked::Declared(record) = walked.position else {
                return unsupported("indexed field carrier is not a record");
            };
            let declaration = unique_declaration(
                types,
                record,
                "primitive projection type is absent",
                "primitive projection type is ambiguous",
            )?;
            let fields = match walked.selected_case {
                Some(case) => case.fields.as_slice(),
                None => declaration
                    .shape
                    .direct_fields()
                    .ok_or(LoweringError::Unsupported(
                        "indexed field carrier has no directly addressed fields",
                    ))?,
            };
            let mut matching = fields
                .iter()
                .filter(|field| field.identity == identities.intern(&identity.spelling()));
            let (Some(field), None) = (matching.next(), matching.next()) else {
                return unsupported("indexed field is absent or ambiguous");
            };
            if field.relevance.is_erased() {
                return unsupported("indexed field is erased");
            }
            let scalar_type = match &field.field_type {
                declared if terminal_semantics::scalar_field_requires_evidence(declared) => {
                    declared.scalar_type().ok_or(LoweringError::Unsupported(
                        "indexed field invariant has no scalar carrier",
                    ))?
                }
                StructuralFieldType::Scalar(ScalarType::Boolean) => ScalarType::Boolean,
                StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                    ScalarType::Integer(*integer)
                }
                StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => *scalar,
                StructuralFieldType::IeeeFloat(format) => ScalarType::IeeeFloat(*format),
                _ => {
                    return unsupported(
                        "indexed element field is not an observable scalar declaration",
                    );
                }
            };
            return Ok(scalar_type);
        }
        let walked = walk_path(identities, element, element_path, types)?;
        if walked.selected_case.is_some() {
            return unsupported("indexed element path ends at a case without its payload field");
        }
        let scalar_type = match walked.position {
            Walked::ScalarField(scalar_type) => scalar_type,
            Walked::Declared(identity) => match unique_declaration(
                types,
                identity,
                "primitive projection type is absent",
                "primitive projection type is ambiguous",
            )?
            .shape
            {
                StructuralTypeShape::PrimitiveScalar(scalar_type) => scalar_type,
                _ => return unsupported("indexed element path does not end at a scalar"),
            },
            Walked::ByteElement => {
                return unsupported("indexed element path requires its byte carrier");
            }
        };
        match scalar_type {
            ScalarType::Boolean | ScalarType::IeeeFloat(_) => Ok(scalar_type),
            ScalarType::Integer(_) => Ok(scalar_type),
        }
    }

    type Walked = terminal_semantics::Position;

    struct WalkedPath<'types> {
        steps: Vec<ProjectionStep>,
        position: Walked,
        selected_case: Option<&'types terminal_psi::StructuralCaseDeclaration>,
    }

    fn walk_path<'types>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        structural_type: StructuralTypeId,
        path: &[CheckedUnitStructuralPathSegment],
        types: &'types [StructuralTypeDeclaration],
    ) -> Result<WalkedPath<'types>, LoweringError> {
        use terminal_psi::StructuralPathSegment as Segment;
        let mut walked = Walked::Declared(structural_type);
        let mut selected_case: Option<&terminal_psi::StructuralCaseDeclaration> = None;
        let mut result = Vec::with_capacity(path.len());
        for segment in path {
            let Walked::Declared(structural_type) = walked else {
                return unsupported("primitive projection descends below a byte element");
            };
            let declaration = unique_declaration(
                types,
                structural_type,
                "primitive projection type is absent",
                "primitive projection type is ambiguous",
            )?;
            if selected_case.is_some()
                && !matches!(segment, CheckedUnitStructuralPathSegment::Field(_))
            {
                return unsupported("primitive projection case must select its payload field");
            }
            walked = match (segment, &declaration.shape) {
                (
                    CheckedUnitStructuralPathSegment::Case(identity),
                    StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
                ) => {
                    let mut matching = cases
                        .iter()
                        .filter(|case| case.identity == identities.intern(&identity.spelling()));
                    let case = matching.next().ok_or(LoweringError::Unsupported(
                        "primitive projection case belongs to another source type",
                    ))?;
                    if matching.next().is_some() {
                        return unsupported("primitive projection case identity is ambiguous");
                    }
                    selected_case = Some(case);
                    result.push(ProjectionStep::Static(Segment::Case(case.id)));
                    Walked::Declared(structural_type)
                }
                (
                    CheckedUnitStructuralPathSegment::Referent,
                    StructuralTypeShape::Reference { referent, .. },
                ) => {
                    result.push(ProjectionStep::Static(Segment::Referent));
                    Walked::Declared(*referent)
                }
                (CheckedUnitStructuralPathSegment::Field(identity), shape) => {
                    let case = selected_case.take();
                    let fields = match case {
                        Some(case) => case.fields.as_slice(),
                        None => shape.direct_fields().ok_or(LoweringError::Unsupported(
                            "primitive projection field requires its source record or selected case",
                        ))?,
                    };
                    let Some(field) = live_field(fields, identities.intern(&identity.spelling()))
                    else {
                        return unsupported("primitive projection field is erased or ambiguous");
                    };
                    result.push(ProjectionStep::Static(Segment::Field(field.identity)));
                    match field.field_type {
                        StructuralFieldType::Structural(child) => Walked::Declared(child),
                        StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => {
                            Walked::ScalarField(scalar)
                        }
                        StructuralFieldType::IeeeFloat(format) => {
                            Walked::ScalarField(ScalarType::IeeeFloat(format))
                        }
                        _ => {
                            let mut selection = [
                                Segment::Field(field.identity),
                                Segment::Field(field.identity),
                            ];
                            let selection = if let Some(case) = case {
                                selection[0] = Segment::Case(case.id);
                                &selection[..]
                            } else {
                                &selection[1..]
                            };
                            Walked::Declared(
                                terminal_semantics::runtime_structural_path_tip(
                                    types.iter(),
                                    structural_type,
                                    selection,
                                )
                                .ok_or(LoweringError::Unsupported(
                                    "primitive projection field has no exact retained declaration",
                                ))?
                                .id,
                            )
                        }
                    }
                }
                (
                    segment @ (CheckedUnitStructuralPathSegment::FixedIndex(_)
                    | CheckedUnitStructuralPathSegment::RuntimeIndex(_)),
                    StructuralTypeShape::FixedArray { .. }
                    | StructuralTypeShape::ElementView { .. }
                    | StructuralTypeShape::ByteSequence(_),
                ) => {
                    let (step, element) = super::structural_scalar_store::element_selection(
                        declaration,
                        segment,
                        types,
                    )?;
                    result.push(step);
                    Walked::Declared(element)
                }
                _ => {
                    return unsupported(
                        "primitive projection has an incompatible or out-of-bounds path",
                    );
                }
            };
        }
        Ok(WalkedPath {
            steps: result,
            position: walked,
            selected_case,
        })
    }
}
pub(crate) mod private_borrow_transport {

    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::{allocate_dense, place_id};
    use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
    use terminal_psi::{
        Block, StructuralAccess, StructuralArgument, StructuralPlaceDeclaration,
        StructuralTypeDeclaration, TerminalAffineCleanupAction, Terminator,
    };

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum BorrowDemand {
        Unused,
        Shared,
        Exclusive,
    }

    impl BorrowDemand {
        fn for_access(access: StructuralAccess) -> Self {
            match access {
                StructuralAccess::SharedBorrow => Self::Shared,
                StructuralAccess::Owned
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow => Self::Exclusive,
            }
        }

        fn access(self) -> StructuralAccess {
            match self {
                Self::Shared => StructuralAccess::SharedBorrow,
                Self::Exclusive => StructuralAccess::MutableBorrow,
                Self::Unused => unreachable!("an unused private loan has no arrival"),
            }
        }
    }

    pub(crate) fn forward_private_state_borrows(
        root: BlockId,
        blocks: &mut [Block],
        structural_types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
        places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<(), LoweringError> {
        let roots = std::iter::once(root)
            .chain(
                blocks
                    .iter()
                    .filter(|block| {
                        block.id != root
                            && block.structural_parameters.iter().any(|parameter| {
                                matches!(
                                    parameter.access,
                                    StructuralAccess::MutableBorrow
                                        | StructuralAccess::WriteOnlyBorrow
                                )
                            })
                    })
                    .map(|block| block.id),
            )
            .collect::<Vec<_>>();
        for root in roots {
            forward_from(root, blocks, structural_types, next_place, places)?;
        }
        Ok(())
    }

    fn forward_from(
        root: BlockId,
        blocks: &mut [Block],
        structural_types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
        places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<(), LoweringError> {
        let source = blocks
            .iter()
            .find(|block| block.id == root)
            .ok_or(LoweringError::Unsupported(
                "state borrow transport has no root",
            ))?
            .structural_parameters
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ) && !has_explicit_transfer(blocks, parameter.place)
                    && structural_types.iter().any(|declaration| {
                        declaration.id == parameter.structural_type
                            && matches!(
                                declaration.shape,
                                terminal_psi::record_or_fixed_array_shape_pattern!()
                            )
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        if source.is_empty() || blocks.len() == 1 {
            return Ok(());
        }
        let mut needed = vec![vec![BorrowDemand::Unused; source.len()]; blocks.len()];
        let mut successors = Vec::with_capacity(blocks.len());
        let identities = blocks.iter().map(|block| block.id).collect::<Vec<_>>();
        for (position, block) in blocks.iter_mut().enumerate() {
            let mut observe = |place, access| {
                if let Some(ordinal) = source.iter().position(|parameter| parameter.place == place)
                {
                    needed[position][ordinal] =
                        needed[position][ordinal].max(BorrowDemand::for_access(access));
                }
                place
            };
            for operation in &mut block.operations {
                operation.kind.map_structural_uses_with_access(&mut observe);
            }
            let targets = match &block.terminator {
                Terminator::Jump {
                    target,
                    structural_arguments,
                    ..
                } => {
                    for argument in structural_arguments {
                        observe(argument.place, argument.access);
                    }
                    vec![*target]
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        for argument in &edge.structural_arguments {
                            observe(argument.place, argument.access);
                        }
                    }
                    vec![when_true.target, when_false.target]
                }
                Terminator::StructuralCase { source, cases, .. } => {
                    observe(*source, StructuralAccess::SharedBorrow);
                    for case in cases {
                        for argument in &case.structural_arguments {
                            observe(argument.place, argument.access);
                        }
                    }
                    cases.iter().map(|case| case.target).collect()
                }
                Terminator::ReturnStructural { source, .. } => {
                    observe(*source, StructuralAccess::Owned);
                    Vec::new()
                }
                _ => Vec::new(),
            };
            successors.push(
                targets
                    .into_iter()
                    .filter(|target| *target != root)
                    .filter_map(|target| identities.iter().position(|identity| *identity == target))
                    .collect::<Vec<_>>(),
            );
        }
        let mut reachable = vec![false; blocks.len()];
        let mut pending = vec![
            identities
                .iter()
                .position(|identity| *identity == root)
                .ok_or(LoweringError::Unsupported(
                    "private borrow definition is absent",
                ))?,
        ];
        while let Some(position) = pending.pop() {
            if std::mem::replace(&mut reachable[position], true) {
                continue;
            }
            pending.extend(successors[position].iter().copied());
        }
        loop {
            let mut changed = false;
            for (position, targets) in successors.iter().enumerate() {
                if !reachable[position] {
                    continue;
                }
                for target in targets {
                    for ordinal in 0..source.len() {
                        if needed[*target][ordinal] > needed[position][ordinal] {
                            needed[position][ordinal] = needed[*target][ordinal];
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let mut namespaces = Vec::with_capacity(blocks.len());
        for (position, block) in blocks.iter_mut().enumerate() {
            let mut names = Vec::with_capacity(source.len());
            if !reachable[position] {
                namespaces.push((block.id, names));
                continue;
            }
            for (ordinal, parameter) in source.iter().enumerate() {
                if needed[position][ordinal] == BorrowDemand::Unused {
                    continue;
                }
                let access = if block.id == root {
                    parameter.access
                } else if parameter.access == StructuralAccess::WriteOnlyBorrow {
                    if needed[position][ordinal] == BorrowDemand::Shared {
                        return unsupported("private write-only borrow cannot acquire read access");
                    }
                    StructuralAccess::WriteOnlyBorrow
                } else {
                    needed[position][ordinal].access()
                };
                let place = if block.id == root {
                    parameter.place
                } else {
                    let mut forwarded = parameter.clone();
                    forwarded.access = access;
                    forwarded.place = place_id(allocate_dense(next_place)?);
                    forwarded.position =
                        u32::try_from(block.structural_parameters.len()).map_err(|_| {
                            LoweringError::Unsupported("private borrow roster exceeds u32")
                        })?;
                    places.push(StructuralPlaceDeclaration {
                        id: forwarded.place,
                        kind: StructuralPlaceKind::BlockParameter {
                            block: block.id,
                            position: forwarded.position,
                        },
                    });
                    let place = forwarded.place;
                    block.structural_parameters.push(forwarded);
                    place
                };
                names.push((parameter.place, place, access));
            }
            namespaces.push((block.id, names));
        }
        for block in blocks {
            let names = &namespaces
                .iter()
                .find(|(identity, _)| *identity == block.id)
                .ok_or(LoweringError::Unsupported(
                    "private borrow namespace is absent",
                ))?
                .1;
            let mut remap = |place| mapped(names, place);
            for operation in &mut block.operations {
                operation.kind.map_structural_uses(&mut remap);
            }
            let forward = |target: BlockId, arguments: &mut Vec<StructuralArgument>| {
                for argument in arguments.iter_mut() {
                    argument.place = mapped(names, argument.place);
                }
                if target != root
                    && let Some((_, destination_names)) =
                        namespaces.iter().find(|(identity, _)| *identity == target)
                {
                    arguments.extend(destination_names.iter().map(|(source, _, access)| {
                        StructuralArgument {
                            place: mapped(names, *source),
                            path: Vec::new(),
                            access: *access,
                        }
                    }));
                }
            };
            match &mut block.terminator {
                Terminator::Jump {
                    target,
                    structural_arguments,
                    cleanup_actions,
                    ..
                } => {
                    forward(*target, structural_arguments);
                    remap_cleanup_actions(cleanup_actions, names);
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        forward(edge.target, &mut edge.structural_arguments);
                        remap_cleanup_actions(&mut edge.cleanup_actions, names);
                    }
                }
                Terminator::ReturnStructural {
                    source,
                    cleanup_actions,
                    ..
                } => {
                    *source = mapped(names, *source);
                    remap_cleanup_actions(cleanup_actions, names);
                }
                terminal_psi::terminator_return_pair_pattern!(cleanup_actions, ..) => {
                    remap_cleanup_actions(cleanup_actions, names);
                }
                Terminator::StructuralCase { source, cases, .. } => {
                    *source = mapped(names, *source);
                    for case in cases {
                        forward(case.target, &mut case.structural_arguments);
                        remap_cleanup_actions(&mut case.cleanup_actions, names);
                    }
                }
                Terminator::InvokeNoNormal {
                    structural_arguments,
                    ..
                }
                | Terminator::ExternalCompletion {
                    structural_arguments,
                    ..
                } => {
                    for argument in structural_arguments {
                        argument.place = mapped(names, argument.place);
                    }
                }
                Terminator::Crash { .. } => {}
            }
        }
        Ok(())
    }

    fn mapped(names: &[(PlaceId, PlaceId, StructuralAccess)], place: PlaceId) -> PlaceId {
        names
            .iter()
            .find(|(source, _, _)| *source == place)
            .map_or(place, |(_, target, _)| *target)
    }

    fn has_explicit_transfer(blocks: &[Block], place: PlaceId) -> bool {
        blocks.iter().any(|block| {
            block
                .terminator
                .successor_block_arguments()
                .iter()
                .any(|arguments| arguments.iter().any(|argument| argument.place == place))
        })
    }

    fn remap_cleanup_actions(
        actions: &mut [TerminalAffineCleanupAction],
        names: &[(PlaceId, PlaceId, StructuralAccess)],
    ) {
        for action in actions {
            let place = match action {
                TerminalAffineCleanupAction::DiscardRoot(place)
                | TerminalAffineCleanupAction::RetireHome(place) => place,
                TerminalAffineCleanupAction::DiscardResidual(discard) => &mut discard.place,
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => &mut cleanup.place,
            };
            *place = mapped(names, *place);
        }
    }
}
pub(crate) mod representation_view_store {
    use super::{
        CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError, OperationKind,
        PrimitiveType, ScalarType, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueDeclaration,
        ValueId, allocate_dense, emit_direct_expression, obligation_id, terminal_scalar_type,
        unsupported,
    };
    use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::emission::operation_emission::expressions::{
        LoweredDirectExpression, emit_scalar_leaf,
    };
    use crate::emission::operation_emission::integer::{
        LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
    };
    use crate::emission::runtime_elements::{self, ProjectionStep};
    use crate::expression_preparation::integer_bit_image::integer_bit_image;
    use crate::shared::structural_types::unique_declaration;
    use checked_trees::{CheckedRuntimeIndex, CheckedStorePlan, CheckedStoreRoot};
    use semantic_vocabulary::IntegerValue;
    use symbols::SymbolHandle;
    use terminal_psi::ByteSequenceCarrier;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        store: &CheckedStorePlan,
        view: PrimitiveType,
        captured: Option<super::primitive_store::PhysicalDestination>,
        checked: &CheckedTrees,
        machine: SymbolHandle,
        state: SymbolHandle,
        evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
        structural_types: &[StructuralTypeDeclaration],
        environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
    ) -> Result<Vec<OperationKind>, LoweringError> {
        let bindings = evaluation
            .scalar_bindings
            .clone()
            .with_resolved_structural_observations(
                &evaluation.structural_fields,
                &evaluation.structural_cases,
            );
        let CheckedStoreRoot::Parameter { position } = store.destination.root else {
            return unsupported("representation view store has no parameter root");
        };
        let Some((role, value)) = store.value.scalar() else {
            return unsupported("representation view store stores no scalar value");
        };
        let parameter = evaluation
            .structural_parameters
            .iter()
            .find(|(authored, _)| authored == &position)
            .map(|(_, parameter)| parameter)
            .ok_or(LoweringError::Unsupported(
                "representation view store parameter is absent",
            ))?;
        let (destination, root_type) = (parameter.place, parameter.structural_type);
        let qualifications = parameter.qualifications.clone();
        let projected_qualifications = parameter.projected_qualifications.clone();
        let additional_input_qualifications = parameter.additional_input_qualifications.clone();
        let view_scalar = terminal_scalar_type(view)?;
        let Some((leaf, prefix)) = store.destination.path.split_last() else {
            return unsupported("representation view store lost its leaf segment");
        };
        let mut kinds = Vec::new();
        match leaf {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                let physical = if let Some(captured) = captured {
                    captured
                } else {
                    let (steps, _) =
                        walk_carrier_prefix(identities, root_type, prefix, structural_types)?;
                    let indexes = runtime_elements::evaluate_selectors(
                        identities,
                        &steps,
                        checked,
                        machine,
                        state,
                        store.statement_index,
                        evaluation,
                        source_value_count,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        environment,
                    )?;
                    let mut path =
                        runtime_elements::complete_path(steps, indexes, calls, operations)?;
                    path.push(StructuralPathSegment::Field(
                        identities.intern(&identity.spelling()),
                    ));
                    super::primitive_store::PhysicalDestination {
                        place: destination,
                        root_type,
                        qualifications: qualifications.clone(),
                        projected_qualifications: projected_qualifications.clone(),
                        additional_input_qualifications: additional_input_qualifications.clone(),
                        path,
                        logical_receiving: None,
                    }
                };
                let leaf_scalar = terminal_semantics::primitive_read_projection_type(
                    structural_types.iter(),
                    physical.root_type,
                    &physical.path,
                )
                .ok_or(LoweringError::Unsupported(
                    "representation view store lost its physical scalar carrier",
                ))?;
                let operand = crate::emission::structural_scalar_store_source::direct_value(
                    identities,
                    checked,
                    state,
                    store.statement_index,
                    role,
                    value,
                    view_scalar,
                    &bindings,
                    values,
                )?;
                let casted = view_leaf_image(operand, view_scalar, leaf_scalar)?;
                let value = emit_direct_expression(&casted, values, next_value, operations);
                kinds.push(super::primitive_store::emit_completed_assignment(
                    machine,
                    state,
                    store.statement_index,
                    physical,
                    leaf_scalar,
                    ValueDeclaration {
                        id: value,
                        scalar_type: leaf_scalar,
                        qualifications: Default::default(),
                    },
                    None,
                    structural_types,
                    next_value,
                    operations,
                    calls,
                )?);
            }
            CheckedUnitStructuralPathSegment::FixedIndex(_)
            | CheckedUnitStructuralPathSegment::RuntimeIndex(_) => {
                let mut logical_receiving = None;
                let (destination, mut physical) = if let Some(mut captured) = captured {
                    let physical = super::primitive_store::ReceivingPlace {
                        root_type: captured.root_type,
                        qualifications: captured.qualifications.clone(),
                        projected_qualifications: captured.projected_qualifications.clone(),
                        additional_input_qualifications: captured
                            .additional_input_qualifications
                            .to_vec(),
                        path: Vec::new(),
                    };
                    logical_receiving = captured.logical_receiving.take();
                    (
                        captured_byte_destination(captured, structural_types)?,
                        physical,
                    )
                } else {
                    let physical = super::primitive_store::ReceivingPlace {
                        root_type,
                        qualifications: qualifications.clone(),
                        projected_qualifications: projected_qualifications.clone(),
                        additional_input_qualifications: additional_input_qualifications.clone(),
                        path: Vec::new(),
                    };
                    let destination = prepare_byte_destination(
                        identities,
                        destination,
                        root_type,
                        &store.destination.path,
                        None,
                        false,
                        checked,
                        machine,
                        state,
                        store.statement_index,
                        evaluation,
                        source_value_count,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        structural_types,
                        environment,
                    )?;
                    (destination, physical)
                };
                let ScalarType::Integer(integer) = view_scalar else {
                    return unsupported("byte representation store requires an integer carrier");
                };
                if !integer.bits().is_multiple_of(8) {
                    return unsupported("byte representation store requires a whole-byte encoding");
                }
                let byte_count = u64::from(integer.bits()) / 8;
                physical.path = destination.prefix_path.clone();
                require_unrestricted_byte_region(&physical, &destination.index, byte_count, calls)?;
                if let Some(logical) = logical_receiving {
                    require_unrestricted_logical_footprint(logical, byte_count, calls)?;
                }
                let operand = crate::emission::structural_scalar_store_source::direct_value(
                    identities,
                    checked,
                    state,
                    store.statement_index,
                    role,
                    value,
                    view_scalar,
                    &bindings,
                    values,
                )?;
                let value = ValueDeclaration {
                    qualifications: Default::default(),
                    id: emit_direct_expression(&operand, values, next_value, operations),
                    scalar_type: view_scalar,
                };
                kinds.extend(destination.emit(
                    value,
                    view_scalar,
                    0,
                    next_value,
                    operations,
                    calls,
                )?);
            }
            _ => {
                return unsupported("representation view store leaf path is incompatible");
            }
        }
        Ok(kinds)
    }

    pub(crate) struct ByteStoreDestination {
        destination: semantic_vocabulary::PlaceId,
        prefix_path: Vec<StructuralPathSegment>,
        index: ByteStoreIndex,
        capacity: u64,
        decided: bool,
    }

    enum ByteStoreIndex {
        Fixed(u64),
        Runtime(ValueId),
    }

    impl ByteStoreDestination {
        pub(crate) fn project_index(
            &mut self,
            index: ValueId,
            stride: u64,
            extent: u64,
            next_value: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<(), LoweringError> {
            let scalar_type = terminal_scalar_type(PrimitiveType::U64)?;
            let coordinate =
                crate::expression_preparation::bindings::place_observations::bounded_coordinate(
                    LoweredDirectExpression::Parameter {
                        position: 0,
                        scalar_type,
                    },
                    extent,
                )?;
            let scaled = LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactMultiply,
                scalar_type,
                left: Box::new(coordinate),
                right: Box::new(LoweredDirectExpression::IntegerLiteral {
                    value: IntegerValue::Unsigned(u128::from(stride)),
                    scalar_type,
                }),
                formation_decided: extent.checked_mul(stride).is_some(),
            };
            let index_value = ValueDeclaration {
                id: index,
                scalar_type,
                qualifications: Default::default(),
            };
            let scaled = emit_direct_expression(&scaled, &[index_value], next_value, operations);
            let base = match self.index {
                ByteStoreIndex::Fixed(index) => emit_scalar_leaf(
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(u128::from(index)),
                    },
                    scalar_type,
                    next_value,
                    operations,
                ),
                ByteStoreIndex::Runtime(index) => index,
            };
            let base_value = ValueDeclaration {
                id: base,
                ..index_value
            };
            let scaled_value = ValueDeclaration {
                id: scaled,
                ..index_value
            };
            let sum = LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactAdd,
                scalar_type,
                left: Box::new(LoweredDirectExpression::Parameter {
                    position: 0,
                    scalar_type,
                }),
                right: Box::new(LoweredDirectExpression::Parameter {
                    position: 1,
                    scalar_type,
                }),
                formation_decided: true,
            };
            self.index = ByteStoreIndex::Runtime(emit_direct_expression(
                &sum,
                &[base_value, scaled_value],
                next_value,
                operations,
            ));
            Ok(())
        }

        pub(crate) fn emit(
            self,
            value: ValueDeclaration,
            stored_type: ScalarType,
            field_offset: u64,
            next_value: &mut u64,
            operations: &mut OperationBuffer,
            calls: &mut CallEmissionContext<'_>,
        ) -> Result<Vec<OperationKind>, LoweringError> {
            let (ScalarType::Integer(source_integer), ScalarType::Integer(stored_integer)) =
                (value.scalar_type, stored_type)
            else {
                return unsupported("byte representation store requires an integer carrier");
            };
            if !stored_integer.bits().is_multiple_of(8) {
                return unsupported("byte representation store requires a whole-byte encoding");
            }
            let encoded = if value.scalar_type == stored_type {
                value
            } else {
                let kind = if source_integer.can_widen_to(stored_integer) {
                    OperationKind::IntegerWiden { operand: value.id }
                } else {
                    OperationKind::IntegerExactCast {
                        operand: value.id,
                        obligation: obligation_id(allocate_dense(
                            &mut calls.next_obligation_identity,
                        )?),
                    }
                };
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: emit_scalar_leaf(kind, stored_type, next_value, operations),
                    scalar_type: stored_type,
                }
            };
            let byte_scalar = terminal_scalar_type(PrimitiveType::U8)?;
            let count_scalar = terminal_scalar_type(PrimitiveType::U64)?;
            let byte_count = u64::from(stored_integer.bits()) / 8;
            let mut kinds = Vec::new();
            for byte_offset in 0..byte_count {
                let offset =
                    field_offset
                        .checked_add(byte_offset)
                        .ok_or(LoweringError::Unsupported(
                            "byte representation store offset overflows",
                        ))?;
                let mut path = self.prefix_path.clone();
                if let ByteStoreIndex::Runtime(base) = self.index {
                    let index = if offset == 0 {
                        base
                    } else {
                        let step = emit_scalar_leaf(
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(u128::from(offset)),
                            },
                            count_scalar,
                            next_value,
                            operations,
                        );
                        let add_obligation =
                            obligation_id(allocate_dense(&mut calls.next_obligation_identity)?);
                        if self.decided {
                            operations.source_checked_obligations.push(add_obligation);
                        }
                        emit_scalar_leaf(
                            OperationKind::ExactIntegerAdd {
                                left: base,
                                right: step,
                                obligation: add_obligation,
                            },
                            count_scalar,
                            next_value,
                            operations,
                        )
                    };
                    let obligation = calls.allocate_requirement()?;
                    if self.decided {
                        operations.source_checked_obligations.push(obligation);
                    }
                    path.push(StructuralPathSegment::RuntimeIndex { index, obligation });
                } else {
                    let ByteStoreIndex::Fixed(fixed_index) = self.index else {
                        return unsupported("byte representation store lost its coordinate");
                    };
                    let selected =
                        fixed_index
                            .checked_add(offset)
                            .ok_or(LoweringError::Unsupported(
                                "byte representation store index overflows",
                            ))?;
                    if selected >= self.capacity {
                        return unsupported(
                            "byte representation store exceeds its backing capacity",
                        );
                    }
                    path.push(StructuralPathSegment::FixedIndex(selected));
                }
                let operand = LoweredDirectExpression::Parameter {
                    position: 0,
                    scalar_type: stored_type,
                };
                let shifted = if byte_offset == 0 {
                    operand
                } else {
                    LoweredDirectExpression::IntegerBinary {
                        kind: LoweredIntegerBinaryKind::WrappingShiftRight,
                        scalar_type: stored_type,
                        left: Box::new(operand),
                        right: Box::new(LoweredDirectExpression::IntegerLiteral {
                            value: IntegerValue::Unsigned(u128::from(byte_offset * 8)),
                            scalar_type: stored_type,
                        }),
                        formation_decided: false,
                    }
                };
                let byte_expression = integer_bit_image(shifted, stored_integer, byte_scalar)?;
                let byte =
                    emit_direct_expression(&byte_expression, &[encoded], next_value, operations);
                kinds.push(OperationKind::WriteOnlyPrimitiveStore {
                    range_obligation: None,
                    destination: self.destination,
                    path,
                    value: byte,
                });
            }
            Ok(kinds)
        }
    }

    fn require_unrestricted_byte_region(
        region: &super::primitive_store::ReceivingPlace,
        index: &ByteStoreIndex,
        byte_count: u64,
        calls: &CallEmissionContext<'_>,
    ) -> Result<(), LoweringError> {
        let mut selected = super::primitive_store::ReceivingPlace {
            root_type: region.root_type,
            qualifications: region.qualifications.clone(),
            projected_qualifications: region.projected_qualifications.clone(),
            additional_input_qualifications: region.additional_input_qualifications.to_vec(),
            path: region.path.clone(),
        };
        match index {
            ByteStoreIndex::Fixed(base) => {
                for offset in 0..byte_count {
                    let coordinate = base.checked_add(offset).ok_or(LoweringError::Unsupported(
                        "byte receiving footprint overflows",
                    ))?;
                    selected.path.truncate(region.path.len());
                    selected
                        .path
                        .push(StructuralPathSegment::FixedIndex(coordinate));
                    require_unrestricted_receiving(&selected, calls)?;
                }
            }
            ByteStoreIndex::Runtime(_) => {
                if !terminal_semantics::structural_memberships_disjoint_from_write(
                    &region.qualifications,
                    &region.projected_qualifications,
                    &region.path,
                ) {
                    return unsupported(
                        "byte representation footprint overlaps a receiving qualification",
                    );
                }
                selected.path.push(StructuralPathSegment::FixedIndex(0));
                require_unrestricted_receiving(&selected, calls)?;
            }
        }
        Ok(())
    }

    fn require_unrestricted_logical_footprint(
        mut logical: super::primitive_store::ReceivingPlace,
        byte_count: u64,
        calls: &CallEmissionContext<'_>,
    ) -> Result<(), LoweringError> {
        let scalar = super::primitive_store::receiving_scalar(&logical, calls.structural_types)
            .ok_or(LoweringError::Unsupported(
                "logical byte footprint lost its scalar declaration",
            ))?;
        if scalar != terminal_scalar_type(PrimitiveType::U8)? {
            return require_unrestricted_receiving(&logical, calls);
        }
        let index = match logical.path.last() {
            Some(StructuralPathSegment::FixedIndex(index)) => Some(ByteStoreIndex::Fixed(*index)),
            Some(StructuralPathSegment::RuntimeIndex { index, .. }) => {
                Some(ByteStoreIndex::Runtime(*index))
            }
            _ => None,
        };
        if let Some(index) = index {
            logical.path.pop();
            require_unrestricted_byte_region(&logical, &index, byte_count, calls)
        } else {
            require_unrestricted_receiving(&logical, calls)
        }
    }

    fn require_unrestricted_receiving(
        place: &super::primitive_store::ReceivingPlace,
        calls: &CallEmissionContext<'_>,
    ) -> Result<(), LoweringError> {
        if super::primitive_store::receiving_restriction(place, calls)?.is_some() {
            return unsupported(
                "byte representation footprint requires an unproved receiving predicate",
            );
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_byte_destination(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        destination: semantic_vocabulary::PlaceId,
        root_type: StructuralTypeId,
        path: &[CheckedUnitStructuralPathSegment],
        captured_index: Option<ValueId>,
        decided: bool,
        checked: &CheckedTrees,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: u32,
        evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
        structural_types: &[StructuralTypeDeclaration],
        environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
    ) -> Result<ByteStoreDestination, LoweringError> {
        let Some((leaf, prefix)) = path.split_last() else {
            return unsupported("byte representation store has no backing coordinate");
        };
        let (mut steps, walked) =
            walk_carrier_prefix(identities, root_type, prefix, structural_types)?;
        let capacity = byte_capacity(walked, structural_types)?;
        let fixed_index = match leaf {
            CheckedUnitStructuralPathSegment::FixedIndex(index) => *index,
            CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
                if captured_index.is_none() {
                    steps.push(runtime_elements::selector_step(selector));
                }
                0
            }
            _ => return unsupported("byte representation store has no element coordinate"),
        };
        let mut indexes = runtime_elements::evaluate_selectors(
            identities,
            &steps,
            checked,
            machine,
            state,
            statement_index,
            evaluation,
            source_value_count,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
            environment,
        )?;
        if captured_index.is_some()
            && !matches!(leaf, CheckedUnitStructuralPathSegment::RuntimeIndex(_))
        {
            return unsupported("byte store snapshot replaced a fixed coordinate");
        }
        let runtime_index = if let Some(index) = captured_index {
            Some(index)
        } else if matches!(leaf, CheckedUnitStructuralPathSegment::RuntimeIndex(_)) {
            steps.pop();
            Some(
                indexes
                    .pop()
                    .ok_or(LoweringError::Unsupported(
                        "byte store lost its evaluated coordinate",
                    ))?
                    .index,
            )
        } else {
            None
        };
        let prefix_path = runtime_elements::complete_path(steps, indexes, calls, operations)?;
        Ok(ByteStoreDestination {
            destination,
            prefix_path,
            index: match runtime_index {
                Some(value) => ByteStoreIndex::Runtime(value),
                None => ByteStoreIndex::Fixed(fixed_index),
            },
            capacity,
            decided,
        })
    }

    fn captured_byte_destination(
        captured: super::primitive_store::PhysicalDestination,
        types: &[StructuralTypeDeclaration],
    ) -> Result<ByteStoreDestination, LoweringError> {
        let Some((leaf, prefix)) = captured.path.split_last() else {
            return unsupported("captured byte representation store lost its coordinate");
        };
        let carrier =
            terminal_semantics::runtime_structural_field(types.iter(), captured.root_type, prefix)
                .and_then(|field| field.field_type.byte_sequence_carrier());
        let declaration = terminal_semantics::runtime_structural_path_tip(
            types.iter(),
            captured.root_type,
            prefix,
        );
        let walked = if let Some(carrier) = carrier.as_ref() {
            Walked::Carrier(carrier)
        } else if let Some(declaration) = declaration {
            match &declaration.shape {
                StructuralTypeShape::ByteSequence(carrier) => Walked::Carrier(carrier),
                _ => Walked::Type(declaration.id),
            }
        } else {
            return unsupported("captured byte representation store lost its backing declaration");
        };
        let capacity = byte_capacity(walked, types)?;
        let index = match leaf {
            StructuralPathSegment::FixedIndex(index) if *index < capacity => {
                ByteStoreIndex::Fixed(*index)
            }
            StructuralPathSegment::RuntimeIndex { index, .. } => ByteStoreIndex::Runtime(*index),
            _ => {
                return unsupported(
                    "captured byte representation store has no bounded byte coordinate",
                );
            }
        };
        Ok(ByteStoreDestination {
            destination: captured.place,
            prefix_path: prefix.to_vec(),
            index,
            capacity,
            decided: false,
        })
    }

    fn byte_capacity(
        walked: Walked<'_>,
        types: &[StructuralTypeDeclaration],
    ) -> Result<u64, LoweringError> {
        match walked {
            Walked::Carrier(ByteSequenceCarrier::BoundedOwned { capacity, .. }) => Ok(*capacity),
            Walked::Type(array) => {
                let StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } = &unique_declaration(
                    types,
                    array,
                    "representation view store carrier type is absent",
                    "representation view store carrier type is ambiguous",
                )?
                .shape
                else {
                    return unsupported("byte representation store carrier is not a fixed array");
                };
                let StructuralTypeShape::PrimitiveScalar(scalar) = &unique_declaration(
                    types,
                    *element,
                    "representation view store carrier type is absent",
                    "representation view store carrier type is ambiguous",
                )?
                .shape
                else {
                    return unsupported("byte representation store carrier element is not scalar");
                };
                if *scalar != terminal_scalar_type(PrimitiveType::U8)? {
                    return unsupported("byte representation store carrier element is not a byte");
                }
                Ok(*length)
            }
            _ => unsupported("byte representation store requires a bounded backing"),
        }
    }

    enum Walked<'a> {
        Type(StructuralTypeId),
        Carrier(&'a ByteSequenceCarrier),
    }

    fn walk_carrier_prefix<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root_type: StructuralTypeId,
        prefix: &[CheckedUnitStructuralPathSegment],
        types: &'a [StructuralTypeDeclaration],
    ) -> Result<(Vec<ProjectionStep>, Walked<'a>), LoweringError> {
        let mut walked = Walked::Type(root_type);
        let mut steps = Vec::with_capacity(prefix.len());
        for segment in prefix {
            let Walked::Type(owner) = walked else {
                return unsupported("representation view store carrier path is incompatible");
            };
            let declaration = unique_declaration(
                types,
                owner,
                "representation view store carrier type is absent",
                "representation view store carrier type is ambiguous",
            )?;
            match segment {
                CheckedUnitStructuralPathSegment::Field(identity) => {
                    let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = &declaration.shape
                    else {
                        return unsupported("representation view store carrier is not a record");
                    };
                    let mut matching = fields.iter().filter(|field| {
                        field.identity == identities.intern(&identity.spelling())
                            && !field.relevance.is_erased()
                    });
                    let field = matching.next().ok_or(LoweringError::Unsupported(
                        "representation view store carrier field is absent",
                    ))?;
                    if matching.next().is_some() {
                        return unsupported("representation view store carrier field is ambiguous");
                    }
                    steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                        identities.intern(&identity.spelling()),
                    )));
                    walked = match &field.field_type {
                        StructuralFieldType::Structural(nested) => Walked::Type(*nested),
                        StructuralFieldType::ByteSequence(carrier)
                        | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
                            Walked::Carrier(carrier)
                        }
                        _ => {
                            return unsupported(
                                "representation view store carrier has a scalar member",
                            );
                        }
                    };
                }
                CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    let StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } = &declaration.shape
                    else {
                        return unsupported(
                            "representation view store fixed index requires a fixed array",
                        );
                    };
                    if *index >= *length {
                        return unsupported(
                            "representation view store fixed index is out of bounds",
                        );
                    }
                    steps.push(ProjectionStep::Static(StructuralPathSegment::FixedIndex(
                        *index,
                    )));
                    walked = Walked::Type(*element);
                }
                CheckedUnitStructuralPathSegment::RuntimeIndex(runtime) => {
                    let element = match &declaration.shape {
                        StructuralTypeShape::FixedArray { element, .. }
                        | StructuralTypeShape::ElementView { element, .. } => *element,
                        _ => {
                            return unsupported(
                                "representation view store runtime index requires an array",
                            );
                        }
                    };
                    let step = match runtime {
                        CheckedRuntimeIndex::AssignmentIndex { depth } => {
                            ProjectionStep::AssignmentIndex { depth: *depth }
                        }
                        CheckedRuntimeIndex::Parameter { position } => {
                            ProjectionStep::RuntimeIndexParameter {
                                position: *position,
                            }
                        }
                        _ => {
                            return unsupported(
                                "representation view store carrier index is unsupported",
                            );
                        }
                    };
                    steps.push(step);
                    walked = Walked::Type(element);
                }
                _ => return unsupported("representation view store carrier path is incompatible"),
            }
        }
        Ok((steps, walked))
    }

    fn view_leaf_image(
        operand: LoweredDirectExpression,
        view: ScalarType,
        leaf: ScalarType,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        match (view, leaf) {
            (ScalarType::Integer(view_integer), ScalarType::Integer(_)) => {
                integer_bit_image(operand, view_integer, leaf)
            }
            (ScalarType::Integer(_), ScalarType::Boolean) => {
                let zero = match view {
                    ScalarType::Integer(integer) => integer.zero(),
                    _ => unreachable!("integer view checked above"),
                };
                Ok(LoweredDirectExpression::Boolean {
                    expression: Box::new(LoweredBooleanReturnExpression::Not {
                        operand: Box::new(LoweredBooleanReturnExpression::IntegerComparison {
                            kind: LoweredIntegerComparisonKind::Equal,
                            left: Box::new(operand),
                            right: Box::new(LoweredDirectExpression::IntegerLiteral {
                                value: zero,
                                scalar_type: view,
                            }),
                        }),
                    }),
                })
            }
            (ScalarType::Boolean, ScalarType::Integer(_)) => match operand {
                LoweredDirectExpression::Boolean { expression } => {
                    let LoweredBooleanReturnExpression::Constant { value } = *expression else {
                        return unsupported(
                            "representation view store cannot land a runtime Boolean on an integer leaf",
                        );
                    };
                    let Some(carrier) = leaf.as_integer() else {
                        unreachable!("integer leaf checked above");
                    };
                    let value = IntegerValue::for_sign(carrier.sign(), i128::from(value))
                        .expect("a Boolean leaf fits every integer carrier");
                    Ok(LoweredDirectExpression::IntegerLiteral {
                        value,
                        scalar_type: leaf,
                    })
                }
                _ => unsupported(
                    "representation view store cannot land a runtime Boolean on an integer leaf",
                ),
            },
            _ => unsupported("representation view store pairs incompatible carriers"),
        }
    }
}
pub(crate) mod runtime_elements {
    use super::{
        CheckedScalarExpressionRole, CheckedTrees, LoweringError, ScalarType,
        StructuralPathSegment, ValueDeclaration, ValueId, unsupported,
    };
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::shared::checked_facts::value_facts;
    use crate::shared::terminal_identities::value_id;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum ProjectionStep {
        Static(StructuralPathSegment),
        RuntimeIndexLiteral { index: u64 },
        AssignmentIndex { depth: u32 },
        RuntimeIndexParameter { position: u32 },
        RuntimeIndexLocal { position: u32 },
        RuntimeIndexField(checked_trees::CheckedStructuralParameterField),
        RuntimeIndexStorage { symbol: symbols::SymbolHandle },
    }

    pub(crate) fn selector_step(runtime: &checked_trees::CheckedRuntimeIndex) -> ProjectionStep {
        match runtime {
            checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
                ProjectionStep::RuntimeIndexStorage { symbol: *symbol }
            }
            checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth } => {
                ProjectionStep::AssignmentIndex { depth: *depth }
            }
            checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                ProjectionStep::RuntimeIndexParameter {
                    position: *position,
                }
            }
            checked_trees::CheckedRuntimeIndex::Local { position } => {
                ProjectionStep::RuntimeIndexLocal {
                    position: *position,
                }
            }
            checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                ProjectionStep::RuntimeIndexField(field.clone())
            }
        }
    }

    pub(crate) fn static_segments(
        steps: Vec<ProjectionStep>,
    ) -> Result<Vec<StructuralPathSegment>, LoweringError> {
        steps
            .into_iter()
            .map(|step| match step {
                ProjectionStep::Static(segment) => Ok(segment),
                ProjectionStep::AssignmentIndex { .. }
                | ProjectionStep::RuntimeIndexLiteral { .. }
                | ProjectionStep::RuntimeIndexParameter { .. }
                | ProjectionStep::RuntimeIndexLocal { .. }
                | ProjectionStep::RuntimeIndexField(_)
                | ProjectionStep::RuntimeIndexStorage { .. } => {
                    unsupported("this route evaluates no assignment runtime element")
                }
            })
            .collect()
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct EvaluatedSelector {
        pub(crate) index: ValueId,
        decided: bool,
        coordinate: Option<u64>,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn evaluate_selectors(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        steps: &[ProjectionStep],
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: u32,
        evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
        environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
    ) -> Result<Vec<EvaluatedSelector>, LoweringError> {
        let mut indexes = Vec::new();
        for step in steps {
            let mut decided = false;
            let mut coordinate = None;
            let (index, scalar_type) = match step {
                ProjectionStep::AssignmentIndex { depth } => {
                    let role = CheckedScalarExpressionRole::AssignmentIndex { depth: *depth };
                    coordinate =
                        checked
                            .facts
                            .assignment_index_coordinate(state, statement_index, *depth);
                    let source = selector_source(checked, state, statement_index, role)?;
                    decided =
                        checked
                            .facts
                            .assignment_index_is_proven(state, statement_index, *depth);

                    let index = evaluation.source_value(
                        identities,
                        checked,
                        machine,
                        state,
                        statement_index,
                        role,
                        &source,
                        source_value_count,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        calls,
                        environment,
                    )?;
                    (index.id, index.scalar_type)
                }
                ProjectionStep::RuntimeIndexParameter { position }
                | ProjectionStep::RuntimeIndexLocal { position } => {
                    let Some(index) = values.get(*position as usize) else {
                        return unsupported(
                            "store runtime index position exceeds the scalar source frame",
                        );
                    };
                    (index.id, index.scalar_type)
                }
                ProjectionStep::RuntimeIndexLiteral { index } => {
                    coordinate = Some(*index);
                    let scalar_type =
                        terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
                    let value = super::emit_direct_expression(
                        &LoweredDirectExpression::IntegerLiteral {
                            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*index)),
                            scalar_type,
                        },
                        values,
                        next_value,
                        operations,
                    );
                    (value, scalar_type)
                }
                ProjectionStep::RuntimeIndexStorage { symbol } => {
                    let observation = evaluation
                        .contextual_bindings()
                        .runtime_index_storage(*symbol)?;
                    let scalar_type = observation.scalar_type();
                    let index =
                        super::emit_direct_expression(&observation, values, next_value, operations);
                    (index, scalar_type)
                }
                ProjectionStep::RuntimeIndexField(field) => {
                    let observation =
                        crate::expression_preparation::bindings::structural_fields::resolve_typed(
                            identities,
                            &evaluation.structural_fields,
                            field.parameter_position,
                            &field.path,
                        )?;
                    let scalar_type = observation.scalar_type();
                    if !matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address())
                    {
                        return unsupported(
                            "store runtime index field selector is not an integer read",
                        );
                    }
                    let index =
                        super::emit_direct_expression(&observation, values, next_value, operations);
                    (index, scalar_type)
                }
                _ => continue,
            };
            let index = super::emit_u64_coordinate(
                index,
                scalar_type,
                decided,
                next_value,
                &mut calls.next_obligation_identity,
                operations,
            )?;
            indexes.push(EvaluatedSelector {
                index,
                decided,
                coordinate,
            });
        }
        Ok(indexes)
    }

    pub(crate) fn complete_path(
        steps: Vec<ProjectionStep>,
        indexes: Vec<EvaluatedSelector>,
        calls: &mut CallEmissionContext<'_>,
        operations: &mut OperationBuffer,
    ) -> Result<Vec<StructuralPathSegment>, LoweringError> {
        let mut indexes = indexes.into_iter();
        let path = steps
            .into_iter()
            .map(|step| match step {
                ProjectionStep::Static(segment) => Ok(segment),
                ProjectionStep::AssignmentIndex { .. }
                | ProjectionStep::RuntimeIndexLiteral { .. }
                | ProjectionStep::RuntimeIndexParameter { .. }
                | ProjectionStep::RuntimeIndexLocal { .. }
                | ProjectionStep::RuntimeIndexField(_)
                | ProjectionStep::RuntimeIndexStorage { .. } => {
                    let selector = indexes.next().ok_or(LoweringError::Unsupported(
                        "store path lost an evaluated runtime element",
                    ))?;
                    let obligation = calls.allocate_requirement()?;
                    if let Some(coordinate) = selector.coordinate {
                        operations.scalar_index_coordinates.push((
                            obligation,
                            selector.index,
                            coordinate,
                        ));
                    }
                    if selector.decided {
                        operations.source_checked_obligations.push(obligation);
                    }
                    Ok(StructuralPathSegment::RuntimeIndex {
                        index: selector.index,
                        obligation,
                    })
                }
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        if indexes.next().is_some() {
            return unsupported("store path evaluated an extra runtime element");
        }
        Ok(path)
    }

    pub(crate) fn observe_element_extent(
        destination: semantic_vocabulary::PlaceId,
        structural_type: semantic_vocabulary::StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &[terminal_psi::StructuralTypeDeclaration],
        next_value: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        for (position, segment) in path.iter().enumerate() {
            if !matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
                continue;
            }
            let prefix = &path[..position];
            let owner = terminal_semantics::runtime_structural_path_tip(
                declarations.iter(),
                structural_type,
                prefix,
            )
            .ok_or(LoweringError::Unsupported(
                "runtime element extent has an unresolved structural prefix",
            ))?;
            let source = terminal_psi::StructuralArgument {
                place: destination,
                path: prefix.to_vec(),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            };
            let kind = match owner.shape {
                terminal_psi::StructuralTypeShape::ElementView { .. } => {
                    terminal_psi::OperationKind::ElementViewLength { source }
                }
                terminal_psi::StructuralTypeShape::ByteSequence(_) => {
                    terminal_psi::OperationKind::ByteSequenceLength { source }
                }
                _ => continue,
            };
            let length = value_id(crate::shared::terminal_identities::allocate_dense(
                next_value,
            )?);
            operations.push_leaf(
                crate::emission::operation_emission::buffer::scalar_result_declaration(
                    length,
                    terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?,
                ),
                kind,
            );
        }
        Ok(())
    }

    fn selector_source(
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        statement_index: u32,
        role: CheckedScalarExpressionRole,
    ) -> Result<checked_trees::CheckedCallScalarArgument, LoweringError> {
        if let Some((_, expression)) = value_facts(checked).scalar_expressions.bound_expression_at(
            state,
            statement_index,
            role,
        ) {
            return Ok(checked_trees::CheckedCallScalarArgument::Pure(
                expression.clone(),
            ));
        }
        let root = value_facts(checked)
            .scalar_computations
            .root_at(state, statement_index, role)
            .ok_or(LoweringError::Unsupported(
                "store runtime element lost its retained selector",
            ))?;
        Ok(checked_trees::CheckedCallScalarArgument::Computation(
            root.root,
        ))
    }
}
pub(crate) mod selected_continuations {

    use std::sync::Arc;

    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::{BlockId, EdgeId, PlaceId, StructuralPlaceKind, StructuralTypeId};
    use terminal_psi::{
        Block, OperationKind, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
        StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
        TerminalAffineCleanupAction, Terminator,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum CleanupRequirement {
        WholeValue,
        ResidentHome,
    }

    #[derive(Clone, Debug)]
    pub(crate) struct SelectedCleanupGroup {
        pub(crate) edge: EdgeId,
        pub(crate) region: StructuralArgument,
        pub(crate) requirement: CleanupRequirement,
        pub(crate) bypass: BlockId,
    }

    /// The checked partition has finished these value obligations. This records
    /// availability only; real cleanup actions, homes, claims and loans stay in
    /// their existing schedules and namespaces.
    #[derive(Clone, Debug)]
    pub(crate) struct CompletedCleanupFrontier {
        pub(crate) edge: EdgeId,
        pub(crate) region: StructuralArgument,
        pub(crate) kind: CompletedCleanupKind,
    }

    #[derive(Clone, Debug)]
    pub(crate) enum CompletedCleanupKind {
        Value,
        CasePayloads(Vec<semantic_vocabulary::StructuralCaseId>),
    }

    pub(crate) struct CleanupCompletionIndex<'rows> {
        rows: semantic_vocabulary::IdTable<EdgeId, Vec<&'rows CompletedCleanupFrontier>>,
    }

    impl<'rows> CleanupCompletionIndex<'rows> {
        pub(crate) fn new(rows: &'rows [CompletedCleanupFrontier]) -> Result<Self, LoweringError> {
            let mut ordered = rows.iter().collect::<Vec<_>>();
            ordered.sort_by_key(|row| row.edge);
            let mut grouped = Vec::<(EdgeId, Vec<&CompletedCleanupFrontier>)>::new();
            for row in ordered {
                if let Some((edge, entries)) = grouped.last_mut()
                    && *edge == row.edge
                {
                    entries.push(row);
                } else {
                    grouped.push((row.edge, vec![row]));
                }
            }
            let rows = semantic_vocabulary::IdTable::from_rows_with_gaps(grouped, usize::MAX)
                .map_err(LoweringError::InvalidCleanupCompletionIndex)?;
            Ok(Self { rows })
        }

        pub(crate) fn at(
            &self,
            edge: EdgeId,
        ) -> impl Iterator<Item = &'rows CompletedCleanupFrontier> + '_ {
            self.rows.get(edge).into_iter().flatten().copied()
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TrackedRegion {
        region: StructuralArgument,
        requirement: CleanupRequirement,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub(crate) enum RegionState {
        Absent,
        Live,
        // State copies share unchanged lists and paths; updates copy only the list.
        Partial(Arc<Vec<Arc<[StructuralPathSegment]>>>),
    }

    #[derive(Clone, Debug)]
    pub(crate) struct Continuation {
        pub(crate) original_block: usize,
        pub(crate) incoming: Vec<RegionState>,
        pub(crate) successors: Vec<(EdgeId, usize)>,
    }

    pub(crate) struct Specialization {
        pub(crate) entry: usize,
        pub(crate) continuations: Vec<Continuation>,
    }

    struct ContinuationCandidates {
        by_block: Vec<symbols::SymbolKeyMap<u64, Vec<usize>>>,
    }

    impl ContinuationCandidates {
        fn new(block_count: usize) -> Self {
            Self {
                by_block: vec![symbols::SymbolKeyMap::default(); block_count],
            }
        }

        fn find(
            &self,
            block: usize,
            signature: u64,
            incoming: &[RegionState],
            continuations: &[Continuation],
        ) -> Option<usize> {
            self.by_block[block]
                .get(&signature)?
                .iter()
                .copied()
                .find(|&position| continuations[position].incoming == incoming)
        }

        fn insert(&mut self, block: usize, signature: u64, position: usize) {
            self.by_block[block]
                .entry(signature)
                .or_default()
                .push(position);
        }

        fn release(&mut self, block: usize, continuations: &mut [Continuation]) {
            for position in self.by_block[block].values().flatten() {
                continuations[*position].incoming = Vec::new();
            }
            self.by_block[block] = Default::default();
        }
    }

    // Incoming states are lookup keys only while another predecessor can arrive.
    // Keep the emitted topology, but release processed keys after their last reader.
    pub(crate) struct IncomingKeyLifetimes {
        successors: Vec<Vec<usize>>,
        predecessors: Vec<usize>,
        pending: Vec<usize>,
        closed: Vec<bool>,
    }

    impl IncomingKeyLifetimes {
        pub(crate) fn for_emitted_blocks(blocks: &[Block]) -> Option<Self> {
            let positions = BlockPositions::new(blocks.iter().map(|block| block.id));
            Self::for_blocks(blocks, &positions, &[])
        }

        fn for_blocks(
            blocks: &[Block],
            positions: &BlockPositions,
            groups: &[SelectedCleanupGroup],
        ) -> Option<Self> {
            let mut bypasses = groups
                .iter()
                .map(|group| (group.edge, group.bypass))
                .collect::<Vec<_>>();
            bypasses.sort_unstable_by_key(|(edge, _)| *edge);
            let mut successors = Vec::with_capacity(blocks.len());
            for (position, block) in blocks.iter().enumerate() {
                // Unknown or repeated identities leave the original lazy checks in
                // charge. They do not establish that a lookup key is dead.
                if positions.position(block.id).ok()? != position {
                    return None;
                }
                let mut targets = Vec::new();
                for (edge, target) in block.terminator.successor_edges() {
                    targets.push(positions.position(target).ok()?);
                    let first = bypasses.partition_point(|(candidate, _)| *candidate < edge);
                    for (_, bypass) in bypasses[first..]
                        .iter()
                        .take_while(|(candidate, _)| *candidate == edge)
                    {
                        targets.push(positions.position(*bypass).ok()?);
                    }
                }
                targets.sort_unstable();
                targets.dedup();
                successors.push(targets);
            }
            Self::from_successors(successors)
        }

        fn from_successors(successors: Vec<Vec<usize>>) -> Option<Self> {
            let mut predecessors = vec![0_usize; successors.len()];
            for targets in &successors {
                for target in targets {
                    let count = predecessors.get_mut(*target)?;
                    *count = count.checked_add(1)?;
                }
            }
            let pending = vec![0; successors.len()];
            let closed = vec![false; successors.len()];
            Some(Self {
                successors,
                predecessors,
                pending,
                closed,
            })
        }

        pub(crate) fn discover(&mut self, block: usize) {
            // Every arrival's source is still pending, so it cannot have closed
            // this destination's complete predecessor set.
            debug_assert!(!self.closed[block]);
            self.pending[block] += 1;
        }

        pub(crate) fn processed(&mut self, block: usize) -> Vec<usize> {
            self.pending[block] -= 1;
            self.close_ready(vec![block])
        }

        pub(crate) fn close_unused_roots(&mut self) {
            let roots = (0..self.successors.len())
                .filter(|block| self.predecessors[*block] == 0 && self.pending[*block] == 0)
                .collect();
            self.close_ready(roots);
        }

        fn close_ready(&mut self, mut ready: Vec<usize>) -> Vec<usize> {
            let mut closed = Vec::new();
            while let Some(block) = ready.pop() {
                if self.closed[block] || self.predecessors[block] != 0 || self.pending[block] != 0 {
                    continue;
                }
                self.closed[block] = true;
                closed.push(block);
                for target in &self.successors[block] {
                    self.predecessors[*target] -= 1;
                    ready.push(*target);
                }
            }
            closed
        }
    }

    fn incoming_signature(incoming: &[RegionState]) -> u64 {
        use std::hash::BuildHasher;
        symbols::BuildSymbolHasher.hash_one(incoming)
    }

    // A missing key is absent; None marks a repeated identity. Recording either
    // condition is harmless until the original query order requests that block.
    struct BlockPositions(symbols::SymbolKeyMap<BlockId, Option<usize>>);

    impl BlockPositions {
        const ABSENT_ERROR: LoweringError =
            LoweringError::Unsupported("selected continuation names an absent block");
        const REPEATED_ERROR: LoweringError =
            LoweringError::Unsupported("selected continuation repeats a block identity");

        fn new(blocks: impl Iterator<Item = BlockId>) -> Self {
            let mut positions = symbols::SymbolKeyMap::default();
            for (position, block) in blocks.enumerate() {
                positions
                    .entry(block)
                    .and_modify(|entry| *entry = None)
                    .or_insert(Some(position));
            }
            Self(positions)
        }

        fn position(&self, target: BlockId) -> Result<usize, LoweringError> {
            self.0
                .get(&target)
                .copied()
                .ok_or(Self::ABSENT_ERROR)?
                .ok_or(Self::REPEATED_ERROR)
        }
    }

    pub(crate) fn prepare(
        machine: semantic_vocabulary::MachineId,
        entry: BlockId,
        parameters: &[StructuralParameterDeclaration],
        places: &[StructuralPlaceDeclaration],
        blocks: &[Block],
        types: &[StructuralTypeDeclaration],
        groups: &[SelectedCleanupGroup],
        completed_cleanups: &[CompletedCleanupFrontier],
        has_claim_bindings: bool,
    ) -> Result<Option<Specialization>, LoweringError> {
        // Record identities once, but report absent or repeated identities only
        // when the original validation order asks for that block.
        let positions = BlockPositions::new(blocks.iter().map(|block| block.id));
        let completions = CleanupCompletionIndex::new(completed_cleanups)?;
        let mut regions = Vec::new();
        for group in groups {
            if group.region.access != StructuralAccess::Owned {
                return unsupported("selected cleanup does not name an owned occurrence");
            }
            let tracked = TrackedRegion {
                region: group.region.clone(),
                requirement: group.requirement,
            };
            if !regions.contains(&tracked) {
                regions.push(tracked);
            }
            if group.requirement == CleanupRequirement::ResidentHome
                && !group.region.path.is_empty()
            {
                return unsupported("selected home retirement has a projected value receiver");
            }
            validate_group(machine, blocks, &positions, group)?;
        }
        let extractions =
            super::affine_cleanup::owned_extractions(parameters, places, blocks, types)?;
        let copied_roots = super::affine_cleanup::copied_operand_roots(parameters, blocks);
        let projected_roots =
            super::affine_cleanup::partitioned_roots(blocks, &extractions, &copied_roots);
        if !projected_roots.is_empty() {
            for &place in &projected_roots {
                let root = StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                };
                let tracked = TrackedRegion {
                    region: root,
                    requirement: CleanupRequirement::WholeValue,
                };
                if !regions.contains(&tracked) {
                    regions.push(tracked);
                }
            }
        }
        if regions.is_empty() && !has_claim_bindings {
            return Ok(None);
        }
        let entry = positions.position(entry)?;
        let mut initial = vec![RegionState::Absent; regions.len()];
        for parameter in parameters {
            establish_parameter(&regions, &mut initial, parameter, &projected_roots);
        }
        let mut continuations = vec![Continuation {
            original_block: entry,
            incoming: initial,
            successors: Vec::new(),
        }];
        let mut key_lifetimes = IncomingKeyLifetimes::for_blocks(blocks, &positions, groups);
        if let Some(lifetimes) = &mut key_lifetimes {
            lifetimes.discover(entry);
            lifetimes.close_unused_roots();
        }
        // Only continuations of the same original block can share an incoming
        // state. Keep their discovery order and compare the complete state.
        let mut candidates = ContinuationCandidates::new(blocks.len());
        candidates.insert(entry, incoming_signature(&continuations[0].incoming), 0);
        let mut position = 0;
        while position < continuations.len() {
            let block = &blocks[continuations[position].original_block];
            let mut frontier = continuations[position].incoming.clone();
            for parameter in &block.structural_parameters {
                establish_parameter(&regions, &mut frontier, parameter, &projected_roots);
            }
            for operation in &block.operations {
                for argument in super::affine_cleanup::structural_operands(&operation.kind) {
                    consume_argument(&regions, &mut frontier, argument, &copied_roots);
                }
                for (_, extraction) in extractions.iter().filter(|(id, _)| *id == operation.id) {
                    extract(&regions, &mut frontier, extraction);
                }
                if let OperationKind::ReleaseReference { source, path } = &operation.kind {
                    consume(&regions, &mut frontier, *source, path);
                }
                if let OperationKind::StoreStructuralField {
                    destination,
                    path,
                    field,
                    ..
                } = &operation.kind
                    && regions
                        .iter()
                        .any(|tracked| tracked.region.place == *destination)
                {
                    let root = root_type(*destination, parameters, places, blocks)?;
                    let restored = terminal_semantics::structural_window_projection(
                        types.iter(),
                        root,
                        path,
                        *field,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "selected owner restoration has no exact structural projection",
                    ))?;
                    restore(&regions, &mut frontier, *destination, &restored.path);
                }
                if let Some(result) = operation.result.structural() {
                    establish(&regions, &mut frontier, result.place);
                } else if let OperationKind::EstablishTrivialAffineLocal { destination }
                | OperationKind::EstablishByteSequenceLiteral { destination, .. }
                | OperationKind::EstablishElementView { destination, .. }
                | OperationKind::EstablishMutableElementView { destination, .. } =
                    &operation.kind
                {
                    establish(&regions, &mut frontier, *destination);
                }
            }
            if let Terminator::ReturnStructural { source, .. } = &block.terminator {
                consume(&regions, &mut frontier, *source, &[]);
            }
            let edges = block.terminator.successor_edges();
            let arguments = block.terminator.successor_block_arguments();
            if edges.len() != arguments.len() {
                return unsupported("selected continuation lost its successor argument roster");
            }
            let mut successors = Vec::with_capacity(edges.len());
            for ((edge, original_target), arguments) in edges.into_iter().zip(arguments) {
                let mut outgoing = frontier.clone();
                for argument in arguments {
                    consume_argument(&regions, &mut outgoing, argument, &copied_roots);
                }
                let mut target = original_target;
                if let Some(group) = groups.iter().find(|group| group.edge == edge) {
                    let selected = regions
                        .iter()
                        .position(|tracked| {
                            tracked.region == group.region
                                && tracked.requirement == group.requirement
                        })
                        .ok_or(LoweringError::Unsupported(
                            "selected death lost its occurrence",
                        ))?;
                    match &outgoing[selected] {
                        RegionState::Live => {}
                        RegionState::Absent => target = group.bypass,
                        RegionState::Partial(missing)
                            if transfers_complete_custody(
                                &group.region,
                                missing,
                                parameters,
                                places,
                                blocks,
                                types,
                            )? =>
                        {
                            target = group.bypass;
                        }
                        RegionState::Partial(_) => {
                            return unsupported(
                                "selected whole-owner cleanup has a partially transferred occurrence",
                            );
                        }
                    }
                }
                if let Some((_, actions)) = block
                    .terminator
                    .cleanup_rosters()
                    .find(|(id, _)| *id == edge)
                {
                    for action in actions {
                        dispose(&regions, &mut outgoing, action);
                    }
                }
                for completion in completions.at(edge) {
                    let paths = match &completion.kind {
                        CompletedCleanupKind::Value => vec![completion.region.path.clone()],
                        CompletedCleanupKind::CasePayloads(cases) => cases
                            .iter()
                            .map(|case| {
                                let mut path = completion.region.path.clone();
                                path.push(StructuralPathSegment::Case(*case));
                                path
                            })
                            .collect(),
                    };
                    for path in paths {
                        for (tracked, state) in regions.iter().zip(&mut outgoing) {
                            if tracked.requirement == CleanupRequirement::WholeValue {
                                consume_region(
                                    &tracked.region,
                                    state,
                                    completion.region.place,
                                    &path,
                                );
                            }
                        }
                    }
                }
                let original_block = positions.position(target)?;
                let signature = incoming_signature(&outgoing);
                let successor = if let Some(successor) =
                    candidates.find(original_block, signature, &outgoing, &continuations)
                {
                    successor
                } else {
                    if let Some(lifetimes) = &mut key_lifetimes {
                        lifetimes.discover(original_block);
                    }
                    let successor = continuations.len();
                    continuations.push(Continuation {
                        original_block,
                        incoming: outgoing,
                        successors: Vec::new(),
                    });
                    candidates.insert(original_block, signature, successor);
                    successor
                };
                successors.push((edge, successor));
            }
            continuations[position].successors = successors;
            if let Some(lifetimes) = &mut key_lifetimes {
                for block in lifetimes.processed(continuations[position].original_block) {
                    candidates.release(block, &mut continuations);
                }
            }
            position += 1;
        }
        Ok(Some(Specialization {
            entry: 0,
            continuations,
        }))
    }

    fn establish_parameter(
        regions: &[TrackedRegion],
        frontier: &mut [RegionState],
        parameter: &StructuralParameterDeclaration,
        projected_roots: &std::collections::BTreeSet<PlaceId>,
    ) {
        for (tracked, state) in regions.iter().zip(frontier) {
            if tracked.region.place == parameter.place
                && (tracked.requirement == CleanupRequirement::ResidentHome
                    || parameter.access == StructuralAccess::Owned
                    || projected_roots.contains(&parameter.place))
            {
                *state = RegionState::Live;
            }
        }
    }

    fn establish(regions: &[TrackedRegion], frontier: &mut [RegionState], place: PlaceId) {
        for (region, state) in regions.iter().map(|tracked| &tracked.region).zip(frontier) {
            if region.place == place {
                *state = RegionState::Live;
            }
        }
    }

    fn consume_argument(
        regions: &[TrackedRegion],
        frontier: &mut [RegionState],
        argument: &StructuralArgument,
        copied_roots: &std::collections::BTreeSet<PlaceId>,
    ) {
        if super::affine_cleanup::transfers_custody(argument, copied_roots) {
            consume(regions, frontier, argument.place, &argument.path);
        }
    }

    fn extract(
        regions: &[TrackedRegion],
        frontier: &mut [RegionState],
        extraction: &StructuralArgument,
    ) {
        if !extraction.path.is_empty() {
            consume(regions, frontier, extraction.place, &extraction.path);
            return;
        }
        for (tracked, state) in regions.iter().zip(frontier) {
            if tracked.requirement == CleanupRequirement::ResidentHome {
                continue;
            }
            let region = &tracked.region;
            if region.place == extraction.place && *state != RegionState::Absent {
                *state = if region.path.is_empty() {
                    RegionState::Partial(Arc::new(vec![Arc::from([])]))
                } else {
                    RegionState::Absent
                };
            }
        }
    }

    fn consume(
        regions: &[TrackedRegion],
        frontier: &mut [RegionState],
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) {
        for (tracked, state) in regions.iter().zip(frontier) {
            if tracked.requirement == CleanupRequirement::ResidentHome {
                if tracked.region.place == place && path.is_empty() {
                    *state = RegionState::Absent;
                }
            } else {
                consume_region(&tracked.region, state, place, path);
            }
        }
    }

    fn consume_region(
        region: &StructuralArgument,
        state: &mut RegionState,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) {
        if region.place != place || *state == RegionState::Absent {
            return;
        }
        if contains_path(path, &region.path) {
            *state = RegionState::Absent;
        } else if terminal_semantics::structural_paths_may_overlap(&region.path, path) {
            match state {
                RegionState::Absent => {}
                RegionState::Live => {
                    *state = RegionState::Partial(Arc::new(vec![Arc::from(path)]));
                }
                RegionState::Partial(missing) => {
                    if !missing.iter().any(|prior| contains_path(prior, path)) {
                        let missing = Arc::make_mut(missing);
                        missing.retain(|prior| !contains_path(path, prior));
                        missing.push(Arc::from(path));
                        missing.sort();
                    }
                }
            }
        }
    }

    fn contains_path(prefix: &[StructuralPathSegment], path: &[StructuralPathSegment]) -> bool {
        prefix.len() <= path.len()
            && terminal_semantics::structural_paths_select_same_location(
                prefix,
                &path[..prefix.len()],
            )
    }

    fn restore(
        regions: &[TrackedRegion],
        frontier: &mut [RegionState],
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) {
        for (tracked, state) in regions.iter().zip(frontier) {
            if tracked.requirement == CleanupRequirement::ResidentHome {
                continue;
            }
            let region = &tracked.region;
            if region.place != place {
                continue;
            }
            if contains_path(path, &region.path) {
                *state = RegionState::Live;
            } else if let RegionState::Partial(missing) = state {
                if missing.iter().any(|hole| contains_path(path, hole)) {
                    Arc::make_mut(missing).retain(|hole| !contains_path(path, hole));
                }
                if missing.is_empty() {
                    *state = RegionState::Live;
                }
            }
        }
    }

    fn transfers_complete_custody(
        region: &StructuralArgument,
        missing: &[Arc<[StructuralPathSegment]>],
        parameters: &[StructuralParameterDeclaration],
        places: &[StructuralPlaceDeclaration],
        blocks: &[Block],
        types: &[StructuralTypeDeclaration],
    ) -> Result<bool, LoweringError> {
        if !region.path.is_empty() {
            return Ok(false);
        }
        let parent_hook = blocks.iter().any(|block| {
            block.terminator.cleanup_rosters().any(|(_, actions)| {
                actions.iter().any(|action| {
                    matches!(
                        action,
                        TerminalAffineCleanupAction::InvokeNominal(cleanup)
                            if cleanup.place == region.place && cleanup.path.is_empty()
                    )
                })
            })
        });
        if parent_hook {
            return Ok(false);
        }
        let root = root_type(region.place, parameters, places, blocks)?;
        let moved = missing.iter().map(|path| path.to_vec()).collect();
        Ok(
            terminal_semantics::partial_affine_residuals(types, root, &moved, 1)
                .is_some_and(|residuals| residuals.is_empty()),
        )
    }

    fn root_type(
        place: PlaceId,
        parameters: &[StructuralParameterDeclaration],
        places: &[StructuralPlaceDeclaration],
        blocks: &[Block],
    ) -> Result<StructuralTypeId, LoweringError> {
        let mut declarations = parameters
            .iter()
            .chain(blocks.iter().flat_map(|block| &block.structural_parameters))
            .filter(|parameter| parameter.place == place)
            .map(|parameter| parameter.structural_type)
            .chain(
                blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| operation.result.structural())
                    .filter(|result| result.place == place)
                    .map(|result| result.structural_type),
            )
            .chain(places.iter().filter_map(|declaration| {
                if declaration.id != place {
                    return None;
                }
                match declaration.kind {
                    StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    } => Some(structural_type),
                    _ => None,
                }
            }));
        let root = declarations.next().ok_or(LoweringError::Unsupported(
            "selected owner restoration has no structural declaration",
        ))?;
        if declarations.any(|other| other != root) {
            return unsupported(
                "selected owner restoration has inconsistent structural declarations",
            );
        }
        Ok(root)
    }

    fn dispose(
        regions: &[TrackedRegion],
        frontier: &mut [RegionState],
        action: &TerminalAffineCleanupAction,
    ) {
        match action {
            TerminalAffineCleanupAction::DiscardRoot(place)
            | TerminalAffineCleanupAction::RetireHome(place) => {
                consume(regions, frontier, *place, &[]);
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                for (tracked, state) in regions.iter().zip(frontier) {
                    if tracked.requirement == CleanupRequirement::WholeValue {
                        consume_region(&tracked.region, state, discard.place, &discard.path);
                    }
                }
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) if !cleanup.retains_descendants => {
                consume(regions, frontier, cleanup.place, &cleanup.path);
            }
            TerminalAffineCleanupAction::InvokeNominal(_) => {}
        }
    }

    fn validate_group(
        machine: semantic_vocabulary::MachineId,
        blocks: &[Block],
        positions: &BlockPositions,
        group: &SelectedCleanupGroup,
    ) -> Result<(), LoweringError> {
        let mut sites = blocks.iter().filter_map(|block| match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                structural_arguments,
                cleanup_actions,
                ..
            } if *edge == group.edge => Some((*target, structural_arguments, cleanup_actions)),
            _ => None,
        });
        let Some((target, arguments, actions)) = sites.next() else {
            return unsupported("selected cleanup group has no private entry jump");
        };
        if sites.next().is_some() || !arguments.is_empty() || !actions.is_empty() {
            let sites = blocks
                .iter()
                .filter_map(|block| match &block.terminator {
                    Terminator::Jump {
                        edge,
                        target,
                        structural_arguments,
                        cleanup_actions,
                        ..
                    } if *edge == group.edge => {
                        Some(crate::shared::lowering_error::SelectedCleanupEntrySite {
                            block: block.id,
                            target: *target,
                            structural_arguments: structural_arguments.clone(),
                            cleanup_actions: cleanup_actions.clone(),
                        })
                    }
                    _ => None,
                })
                .collect();
            return Err(LoweringError::InvalidSelectedCleanupEntry(Box::new(
                crate::shared::lowering_error::SelectedCleanupEntryMismatch {
                    machine,
                    edge: group.edge,
                    region: group.region.clone(),
                    sites,
                },
            )));
        }
        let entry = &blocks[positions.position(target)?];
        let bypass = &blocks[positions.position(group.bypass)?];
        if !entry.structural_parameters.is_empty()
            || !bypass.structural_parameters.is_empty()
            || entry
                .parameters
                .iter()
                .map(|value| value.value_type())
                .ne(bypass.parameters.iter().map(|value| value.value_type()))
            || entry
                .erased_scalar_formals
                .iter()
                .map(|value| value.value_type())
                .ne(bypass
                    .erased_scalar_formals
                    .iter()
                    .map(|value| value.value_type()))
            || entry.erased_proof_formals != bypass.erased_proof_formals
        {
            return unsupported("selected cleanup bypass changes the captured continuation frame");
        }
        Ok(())
    }

    #[cfg(test)]
    mod continuation_candidate_tests {
        use super::*;
        use semantic_vocabulary::{ObligationId, ScalarType, StructuralCaseId, ValueId};

        #[test]
        fn incoming_keys_wait_for_all_predecessors_and_pending_work() {
            let mut lifetimes = IncomingKeyLifetimes::from_successors(vec![
                vec![1, 2],
                vec![3],
                vec![4],
                vec![],
                vec![3],
                vec![3],
            ])
            .unwrap();
            lifetimes.discover(0);
            lifetimes.close_unused_roots();
            assert!(lifetimes.closed[5]);
            lifetimes.discover(1);
            lifetimes.discover(2);
            assert_eq!(lifetimes.processed(0), vec![0]);
            lifetimes.discover(3);
            assert_eq!(lifetimes.processed(1), vec![1]);
            // The first arrival can finish before another predecessor arrives.
            assert!(lifetimes.processed(3).is_empty());
            assert!(!lifetimes.closed[3]);
            lifetimes.discover(4);
            assert_eq!(lifetimes.processed(2), vec![2]);
            assert!(!lifetimes.closed[3]);
            lifetimes.discover(3);
            assert_eq!(lifetimes.processed(4), vec![4]);
            assert!(!lifetimes.closed[3]);
            assert_eq!(lifetimes.processed(3), vec![3]);

            let mut cycle =
                IncomingKeyLifetimes::from_successors(vec![vec![1], vec![1, 2], vec![]]).unwrap();
            cycle.discover(0);
            cycle.close_unused_roots();
            cycle.discover(1);
            assert_eq!(cycle.processed(0), vec![0]);
            cycle.discover(2);
            assert!(cycle.processed(1).is_empty());
            assert!(cycle.processed(2).is_empty());
            assert!(!cycle.closed[1] && !cycle.closed[2]);
            assert!(IncomingKeyLifetimes::from_successors(vec![vec![1]]).is_none());
        }

        #[test]
        fn block_positions_preserve_lazy_missing_and_duplicate_queries() {
            let id = |value| BlockId::new(value).unwrap();
            let blocks = [id(1), id(2), id(4), id(2)];
            let positions = BlockPositions::new(blocks.into_iter());
            for target in [id(1), id(4), id(2), id(3), id(1)] {
                let direct = {
                    let mut matches = blocks
                        .iter()
                        .enumerate()
                        .filter(|(_, block)| **block == target);
                    match matches.next() {
                        None => Err(BlockPositions::ABSENT_ERROR),
                        Some((_, _)) if matches.next().is_some() => {
                            Err(BlockPositions::REPEATED_ERROR)
                        }
                        Some((position, _)) => Ok(position),
                    }
                };
                let indexed = positions.position(target);
                assert_eq!(indexed, direct);
            }
        }

        #[test]
        fn signature_collisions_keep_full_states_and_first_discovery() {
            let segments = vec![
                StructuralPathSegment::AllElements,
                StructuralPathSegment::Case(StructuralCaseId::new(1).unwrap()),
                StructuralPathSegment::Case(StructuralCaseId::new(2).unwrap()),
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(1)),
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(2)),
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::FixedIndex(2),
                StructuralPathSegment::FixedRange { start: 0, end: 3 },
                StructuralPathSegment::FixedRange { start: 1, end: 3 },
                StructuralPathSegment::FixedRange { start: 1, end: 4 },
                StructuralPathSegment::RuntimeIndex {
                    index: ValueId::new(1).unwrap(),
                    obligation: ObligationId::new(1).unwrap(),
                },
                StructuralPathSegment::RuntimeIndex {
                    index: ValueId::new(1).unwrap(),
                    obligation: ObligationId::new(2).unwrap(),
                },
                StructuralPathSegment::RuntimeIndex {
                    index: ValueId::new(2).unwrap(),
                    obligation: ObligationId::new(1).unwrap(),
                },
                StructuralPathSegment::RepresentationView {
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    stored_scalar: None,
                },
                StructuralPathSegment::RepresentationView {
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    stored_scalar: Some(ScalarType::Boolean),
                },
                StructuralPathSegment::RepresentationView {
                    structural_type: StructuralTypeId::new(2).unwrap(),
                    stored_scalar: None,
                },
                StructuralPathSegment::Referent,
            ];
            let mut states = vec![
                Vec::new(),
                vec![RegionState::Absent],
                vec![RegionState::Live],
                vec![RegionState::Partial(Arc::new(Vec::new()))],
                vec![RegionState::Partial(Arc::new(vec![Arc::from([])]))],
                vec![RegionState::Absent, RegionState::Live],
                vec![RegionState::Live, RegionState::Absent],
                vec![RegionState::Partial(Arc::new(vec![
                    segments.clone().into(),
                ]))],
                vec![RegionState::Partial(Arc::new(vec![
                    segments.iter().rev().cloned().collect::<Vec<_>>().into(),
                ]))],
            ];
            states.extend(
                segments.into_iter().map(|segment| {
                    vec![RegionState::Partial(Arc::new(vec![Arc::from([segment])]))]
                }),
            );
            let mut index = ContinuationCandidates::new(2);
            let mut continuations: Vec<Continuation> = Vec::new();
            // Every distinct state collides. Replay both blocks and duplicates;
            // the direct old search decides each first representative.
            for block in [0, 1, 0, 1] {
                for incoming in &states {
                    let direct = continuations.iter().position(|candidate| {
                        candidate.original_block == block && candidate.incoming == *incoming
                    });
                    let found = index.find(block, 0, incoming, &continuations);
                    assert_eq!(found, direct);
                    if found.is_none() {
                        let position = continuations.len();
                        continuations.push(Continuation {
                            original_block: block,
                            incoming: incoming.clone(),
                            successors: Vec::new(),
                        });
                        index.insert(block, 0, position);
                    }
                }
            }
            assert_eq!(continuations.len(), states.len() * 2);
        }
    }
}
pub(crate) mod structural_byte_sequence_store {
    use super::{
        CheckedUnitStructuralPathSegment, LoweringError, OperationKind, OperationResult,
        PrimitiveType, StructuralParameterDeclaration, StructuralPlaceDeclaration,
        StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
        allocate_dense, obligation_id, place_id, structural_type_id, terminal_scalar_type,
        unsupported, value_id,
    };
    use crate::emission::operation_emission::buffer::{
        OperationBuffer, StructuralBinding, scalar_result_declaration,
    };

    use crate::emission::operation_emission::place_declarations::operation_result_place;
    use crate::emission::structural_scalar_store::lower_structural_field_path;
    use crate::emission::structural_scalar_store::lower_structural_field_place;
    use checked_trees::{CheckedByteSequenceValue as ByteValue, CheckedByteSequenceValuePlan};
    use terminal_psi::control_flow::ByteSequenceOperand;

    const LITERAL_VIEW_IDENTITY: &str = "compiler(byte-sequence-literal-view)";

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ByteLiteral {
        pub(crate) place: semantic_vocabulary::PlaceId,
        pub(crate) structural_type: StructuralTypeId,
        pub(crate) bytes: Vec<u8>,
        storage: LiteralStorage,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum LiteralStorage {
        Borrowed,
        Owned {
            source: semantic_vocabulary::PlaceId,
            empty: semantic_vocabulary::PlaceId,
            view_type: StructuralTypeId,
        },
    }

    impl ByteLiteral {
        pub(crate) fn reserve(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            bytes: &[u8],
            structural_type: StructuralTypeId,
            types: &[StructuralTypeDeclaration],
            next_place: &mut u64,
        ) -> Result<Self, LoweringError> {
            let storage = match terminal_psi::find_structural_type(types, structural_type)
                .map(|declaration| &declaration.shape)
            {
                Some(StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                )) => LiteralStorage::Borrowed,
                Some(StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                )) => {
                    if u64::try_from(bytes.len()).map_or(true, |length| length > *capacity) {
                        return unsupported("byte literal exceeds its declared capacity");
                    }
                    LiteralStorage::Owned {
                        source: place_id(allocate_dense(next_place)?),
                        empty: place_id(allocate_dense(next_place)?),
                        view_type: existing_literal_view_type(identities, types)?,
                    }
                }
                _ => {
                    return unsupported("byte literal has neither an owned nor a borrowed carrier");
                }
            };
            Ok(Self {
                place: place_id(allocate_dense(next_place)?),
                structural_type,
                bytes: bytes.to_vec(),
                storage,
            })
        }

        fn declare_views(
            &self,
            places: &mut Vec<StructuralPlaceDeclaration>,
        ) -> Result<(), LoweringError> {
            let (views, structural_type): (&[_], _) = match self.storage {
                LiteralStorage::Borrowed => (&[self.place], self.structural_type),
                LiteralStorage::Owned {
                    source,
                    empty,
                    view_type,
                } => (&[source, empty], view_type),
            };
            for &place in views {
                let declaration_ordinal = super::next_byte_sequence_literal_ordinal(places)?;
                places.push(StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal,
                        structural_type,
                    },
                });
            }
            Ok(())
        }

        pub(crate) fn emit(
            &self,
            qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
            next_value: &mut u64,
            next_obligation: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<(semantic_vocabulary::OperationId, OperationResult), LoweringError> {
            let LiteralStorage::Owned {
                source,
                empty,
                view_type,
            } = self.storage
            else {
                let id = emit_literal(
                    self.place,
                    self.structural_type,
                    &self.bytes,
                    qualifications,
                    operations,
                );
                return Ok((id, OperationResult::Unit));
            };
            let mut operand = |place, bytes: &[u8]| -> Result<ByteSequenceOperand, LoweringError> {
                emit_literal(place, view_type, bytes, Vec::new(), operations);
                let length = value_id(allocate_dense(next_value)?);
                operations.push_leaf(
                    scalar_result_declaration(length, terminal_scalar_type(PrimitiveType::U64)?),
                    OperationKind::ByteSequenceLength {
                        source: terminal_psi::StructuralArgument::whole(
                            place,
                            terminal_psi::StructuralAccess::SharedBorrow,
                        ),
                    },
                );
                Ok(whole_operand(place, length))
            };
            let left = operand(source, &self.bytes)?;
            let right = operand(empty, &[])?;
            let (producer, result) = emit_concatenation(
                self.place,
                left,
                right,
                self.structural_type,
                qualifications,
                next_obligation,
                operations,
            )?;
            Ok((producer, OperationResult::Structural(result)))
        }
    }

    pub(crate) enum StoreRoot<'a> {
        Parameter(&'a StructuralParameterDeclaration),
        Local {
            place: semantic_vocabulary::PlaceId,
            structural_type: StructuralTypeId,
            multiplicity: terminal_psi::StructuralMultiplicity,
            access: terminal_psi::StructuralAccess,
        },
        DescriptorHome {
            place: semantic_vocabulary::PlaceId,
            structural_type: StructuralTypeId,
            qualified: bool,
        },
    }

    pub(crate) fn existing_literal_view_type(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        types: &[StructuralTypeDeclaration],
    ) -> Result<StructuralTypeId, LoweringError> {
        let declaration = terminal_psi::structural_type_by_identity(
            types,
            identities.intern(LITERAL_VIEW_IDENTITY),
        )
        .ok_or(LoweringError::Unsupported(
            "composed callable produced a type absent from the shared catalog",
        ))?;
        if !declaration.shape.is_borrowed_byte_view() {
            return unsupported("generated literal-view identity has a different carrier");
        }
        Ok(declaration.id)
    }

    pub(crate) fn completed_call_result(
        operations: &OperationBuffer,
        binding_ordinal: u32,
    ) -> Result<&terminal_psi::StructuralOperationResult, LoweringError> {
        let (_, binding) = operations
            .structural_values
            .iter()
            .find(|(ordinal, _)| *ordinal == binding_ordinal)
            .ok_or(LoweringError::Unsupported(
                "byte call result has no completed binding",
            ))?;
        binding.operation_result()
    }

    pub(crate) fn prepare_types(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: &CheckedByteSequenceValuePlan,
        types: &mut Vec<StructuralTypeDeclaration>,
    ) -> Result<(), LoweringError> {
        literal_view_type(identities, types)?;
        if value
            .nodes
            .iter()
            .any(|(_, node)| matches!(node, ByteValue::Descriptor { .. }))
        {
            descriptor_value_type(identities, types)?;
        }
        for (identity, node) in value.nodes.iter() {
            let capacity = match node {
                ByteValue::Concatenation { capacity, .. } => Some(capacity),
                ByteValue::Field { capacity, .. } | ByteValue::Whole { capacity, .. }
                    if identity == value.root =>
                {
                    Some(capacity)
                }
                _ => None,
            };
            if let Some(capacity) = capacity {
                let identity = format!("compiler(byte-sequence-owned:{capacity})");
                let shape = StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned {
                        capacity: *capacity,
                    },
                );
                if let Some(existing) =
                    terminal_psi::structural_type_by_identity(types, identities.intern(&identity))
                {
                    if existing.shape != shape {
                        return unsupported("generated owned byte carrier has a different shape");
                    }
                } else {
                    let ordinal = types
                        .iter()
                        .map(|candidate| candidate.id.get())
                        .max()
                        .unwrap_or(0)
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "owned byte type identity overflow",
                        ))?;
                    types.push(StructuralTypeDeclaration {
                        quotient_carrier: None,
                        owns_linear_claim: false,
                        no_code_affine_cleanup: false,
                        copyable: false,
                        descriptor_cleanup: false,
                        scalar_restriction: None,
                        id: structural_type_id(ordinal),
                        identity: identities.intern(&identity),
                        shape,
                    });
                }
            }
        }
        Ok(())
    }

    fn whole_operand(
        source: semantic_vocabulary::PlaceId,
        length: semantic_vocabulary::ValueId,
    ) -> ByteSequenceOperand {
        ByteSequenceOperand::Whole {
            source: terminal_psi::StructuralArgument::whole(
                source,
                terminal_psi::StructuralAccess::SharedBorrow,
            ),
            length,
        }
    }

    pub(crate) fn literal_view_type(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        types: &mut Vec<StructuralTypeDeclaration>,
    ) -> Result<StructuralTypeId, LoweringError> {
        let shape =
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            });
        if let Some(declaration) = terminal_psi::structural_type_by_identity(
            types,
            identities.intern(LITERAL_VIEW_IDENTITY),
        ) {
            return if declaration.shape == shape {
                Ok(declaration.id)
            } else {
                unsupported("generated literal-view identity has a different carrier")
            };
        }
        let identity = types
            .iter()
            .map(|declaration| declaration.id.get())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "literal-view type identity overflow",
            ))?;
        let id = structural_type_id(identity);
        types.push(StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id,
            identity: identities.intern(LITERAL_VIEW_IDENTITY),
            shape,
        });
        Ok(id)
    }

    pub(crate) fn descriptor_value_type(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        types: &mut Vec<StructuralTypeDeclaration>,
    ) -> Result<StructuralTypeId, LoweringError> {
        const IDENTITY: &str = "compiler(shared-byte-descriptor)";
        if let Some(existing) = descriptor_value_declaration(types) {
            return Ok(existing.id);
        }
        let shape =
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: None,
            });
        if types
            .iter()
            .any(|declaration| declaration.identity == identities.intern(IDENTITY))
        {
            return unsupported("descriptor carrier identity changed shape");
        }
        let id = structural_type_id(
            types
                .iter()
                .map(|declaration| declaration.id.get())
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "descriptor carrier identity overflow",
                ))?,
        );
        types.push(StructuralTypeDeclaration {
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id,
            identity: identities.intern(IDENTITY),
            shape,
        });
        Ok(id)
    }

    pub(crate) fn existing_descriptor_value_type(
        types: &[StructuralTypeDeclaration],
    ) -> Result<StructuralTypeId, LoweringError> {
        descriptor_value_declaration(types)
            .map(|declaration| declaration.id)
            .ok_or(LoweringError::Unsupported(
                "descriptor value type was not prepared",
            ))
    }

    fn descriptor_value_declaration(
        types: &[StructuralTypeDeclaration],
    ) -> Option<&StructuralTypeDeclaration> {
        types.iter().find(|declaration| {
            declaration.shape
                == StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { access: None },
                )
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_content_store(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        path: &[CheckedUnitStructuralPathSegment],
        value: &CheckedByteSequenceValuePlan,
        root: StoreRoot<'_>,
        parameters: &[(u32, StructuralParameterDeclaration)],
        locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
        views: &[crate::expression_preparation::bindings::view_locals::ViewLocalBinding],
        structural_types: &[StructuralTypeDeclaration],
        literal_places: &mut Vec<StructuralPlaceDeclaration>,
        next_place: &mut u64,
        next_value: &mut u64,
        next_obligation: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<OperationKind, LoweringError> {
        let destination = match &root {
            StoreRoot::Parameter(parameter) => parameter.place,
            StoreRoot::Local { place, .. } => *place,
            StoreRoot::DescriptorHome { .. } => {
                return unsupported("a descriptor home holds no owned byte contents");
            }
        };
        let field_destination = match path.split_last() {
            None => {
                let (structural_type, multiplicity, access) = match &root {
                    StoreRoot::Parameter(parameter) => (
                        parameter.structural_type,
                        parameter.multiplicity,
                        parameter.access,
                    ),
                    StoreRoot::Local {
                        structural_type,
                        multiplicity,
                        access,
                        ..
                    } => (*structural_type, *multiplicity, *access),
                    StoreRoot::DescriptorHome { .. } => {
                        return unsupported("descriptor storage holds no owned byte contents");
                    }
                };
                if multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !matches!(
                        access,
                        terminal_psi::StructuralAccess::Owned
                            | terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::WriteOnlyBorrow
                    )
                    || !terminal_psi::find_structural_type(structural_types, structural_type)
                        .is_some_and(|declaration| {
                            declaration.shape.is_bounded_owned_byte_sequence()
                        })
                {
                    return unsupported("whole byte store requires a writable bounded owner");
                }
                None
            }
            Some((CheckedUnitStructuralPathSegment::Field(field_identity), carrier_path)) => {
                let (path, field) = match &root {
                    StoreRoot::Parameter(parameter) => lower_structural_field_place(
                        identities,
                        carrier_path,
                        field_identity,
                        parameter,
                        structural_types,
                    )?,
                    StoreRoot::DescriptorHome { .. } => {
                        return unsupported("a descriptor home has no byte field");
                    }
                    StoreRoot::Local {
                        structural_type,
                        multiplicity,
                        access,
                        ..
                    } => {
                        let (path, field) = lower_structural_field_path(
                            identities,
                            *structural_type,
                            carrier_path,
                            field_identity,
                            structural_types,
                        )?;
                        if !terminal_semantics::reference_source_projection_accessible(
                            structural_types.iter(),
                            *structural_type,
                            *multiplicity,
                            *access,
                            &path,
                            terminal_psi::StructuralAccess::WriteOnlyBorrow,
                        ) {
                            return unsupported(
                                "byte-field local projection has no write authority",
                            );
                        }
                        (path, field)
                    }
                };
                if !matches!(
                    field.field_type.byte_sequence_carrier(),
                    Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
                ) {
                    return unsupported("byte-field store destination is not bounded owned bytes");
                }
                Some((path, field.id))
            }
            Some(_) => {
                return unsupported("byte contents store does not end at a byte field");
            }
        };
        let mut emitter = ValueEmitter {
            plan: value,
            parameters,
            locals,
            views,
            emission: ByteEmission {
                structural_types,
                places: literal_places,
                next_place,
                next_value,
                next_obligation,
                operations,
            },
        };
        let (operand, capacity) = emitter.emit(identities, value.root, 0)?;
        let (source, length) = match operand {
            ByteSequenceOperand::Whole { source, length }
                if source.path.is_empty()
                    && !matches!(
                        value.nodes.get(value.root),
                        ByteValue::Whole {
                            root: checked_trees::CheckedStoreRoot::Local { .. },
                            ..
                        }
                    ) =>
            {
                (source.place, length)
            }
            source => {
                let (empty, _) = emitter.emission.literal(identities, &[])?;
                emitter.owned(identities, source, empty, capacity)?
            }
        };
        let obligation = obligation_id(allocate_dense(next_obligation)?);
        match field_destination {
            Some((path, field)) => {
                operations.invalidate_field_lengths_under(destination, &path);
                Ok(OperationKind::StructuralByteSequenceFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    length,
                    obligation,
                })
            }
            None => {
                operations.invalidate_field_lengths_under(destination, &[]);
                Ok(OperationKind::ByteSequenceStore {
                    destination,
                    source,
                    length,
                    obligation,
                })
            }
        }
    }

    pub(crate) struct DescriptorStore {
        pub(crate) destination: semantic_vocabulary::PlaceId,
        pub(crate) structural_type: StructuralTypeId,
        pub(crate) steps: Vec<crate::emission::runtime_elements::ProjectionStep>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        requires_predicate_obligation: bool,
    }

    pub(crate) fn prepare_descriptor_store(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root: &StoreRoot<'_>,
        path: &[CheckedUnitStructuralPathSegment],
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<DescriptorStore, LoweringError> {
        let shared_view = Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        });
        let Some((field_identity, carrier_path)) = path.split_last() else {
            let StoreRoot::DescriptorHome {
                place,
                structural_type,
                qualified,
            } = root
            else {
                return unsupported("whole descriptor store requires a declared descriptor home");
            };
            let declaration =
                terminal_psi::find_structural_type(structural_types, *structural_type).ok_or(
                    LoweringError::Unsupported("whole descriptor store home type was not declared"),
                )?;
            if !matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                    }
                )
            ) {
                return unsupported("whole descriptor store destination is not a shared view home");
            }
            return Ok(DescriptorStore {
                destination: *place,
                structural_type: *structural_type,
                steps: Vec::new(),
                field: None,
                requires_predicate_obligation: *qualified,
            });
        };
        let CheckedUnitStructuralPathSegment::Field(field_identity) = field_identity else {
            return unsupported("view-field store destination does not end at a field");
        };
        let (destination, structural_type, (steps, field)) = match root {
            StoreRoot::Parameter(parameter) => (
                parameter.place,
                parameter.structural_type,
                crate::emission::structural_scalar_store::lower_structural_field_place_steps(
                    identities,
                    carrier_path,
                    field_identity,
                    parameter,
                    structural_types,
                )?,
            ),
            StoreRoot::Local {
                place,
                structural_type,
                ..
            } => (
                *place,
                *structural_type,
                crate::emission::structural_scalar_store::lower_structural_field_steps(
                    identities,
                    *structural_type,
                    carrier_path,
                    field_identity,
                    structural_types,
                )?,
            ),
            StoreRoot::DescriptorHome { .. } => {
                return unsupported("a descriptor home has no record field slots");
            }
        };
        if field.field_type.byte_sequence_carrier() != shared_view {
            return unsupported("view-field store destination is not a shared view field");
        }
        Ok(DescriptorStore {
            destination,
            structural_type,
            steps,
            field: Some(field.id),
            requires_predicate_obligation: !field.field_type.byte_predicates().is_empty(),
        })
    }

    pub(crate) fn emit_descriptor_store(
        destination: DescriptorStore,
        path: Vec<terminal_psi::StructuralPathSegment>,
        source: semantic_vocabulary::PlaceId,
        next_obligation: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<OperationKind, LoweringError> {
        let predicate_obligation = if destination.requires_predicate_obligation {
            Some(obligation_id(allocate_dense(next_obligation)?))
        } else {
            None
        };
        operations.invalidate_field_lengths_under(destination.destination, &path);
        if destination.field.is_none() {
            operations
                .storage_observations
                .bytes
                .retain(|(place, _)| *place != destination.destination);
        }
        Ok(OperationKind::StructuralByteViewFieldStore {
            destination: destination.destination,
            path,
            field: destination.field,
            source,
            predicate_obligation,
        })
    }

    pub(crate) fn emit_literal_view(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        bytes: &[u8],
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        structural_types: &[StructuralTypeDeclaration],
        places: &mut Vec<StructuralPlaceDeclaration>,
        next_place: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<semantic_vocabulary::PlaceId, LoweringError> {
        let structural_type = existing_literal_view_type(identities, structural_types)?;
        let source = place_id(allocate_dense(next_place)?);
        let declaration_ordinal = super::next_byte_sequence_literal_ordinal(places)?;
        places.push(StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            },
        });
        emit_literal(source, structural_type, bytes, qualifications, operations);
        Ok(source)
    }

    fn emit_literal(
        destination: semantic_vocabulary::PlaceId,
        structural_type: StructuralTypeId,
        bytes: &[u8],
        mut qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        operations: &mut OperationBuffer,
    ) -> semantic_vocabulary::OperationId {
        operations
            .literal_types
            .push((destination, structural_type));
        qualifications.sort();
        qualifications.dedup();
        operations.push_leaf(
            OperationResult::Unit,
            OperationKind::EstablishByteSequenceLiteral {
                destination,
                bytes: bytes.to_vec(),
                qualifications,
            },
        )
    }

    pub(crate) fn retain_literal_declarations(
        operations: &OperationBuffer,
        declarations: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<(), LoweringError> {
        for &(place, structural_type) in &operations.literal_types {
            if declarations
                .iter()
                .any(|declaration| declaration.id == place)
            {
                continue;
            }
            let kind = StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: super::next_byte_sequence_literal_ordinal(declarations)?,
                structural_type,
            };
            declarations.push(StructuralPlaceDeclaration { id: place, kind });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_owned_value(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: &CheckedByteSequenceValuePlan,
        structural_type: StructuralTypeId,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        parameters: &[(u32, StructuralParameterDeclaration)],
        locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
        views: &[crate::expression_preparation::bindings::view_locals::ViewLocalBinding],
        structural_types: &[StructuralTypeDeclaration],
        places: &mut Vec<StructuralPlaceDeclaration>,
        next_place: &mut u64,
        next_value: &mut u64,
        next_obligation: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<
        (
            StructuralPlaceDeclaration,
            terminal_psi::StructuralOperationResult,
        ),
        LoweringError,
    > {
        let Some(StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
        )) = terminal_psi::find_structural_type(structural_types, structural_type)
            .map(|row| &row.shape)
        else {
            return unsupported("owned byte local has no bounded carrier");
        };
        let mut emitter = ValueEmitter {
            plan: value,
            parameters,
            locals,
            views,
            emission: ByteEmission {
                structural_types,
                places,
                next_place,
                next_value,
                next_obligation,
                operations,
            },
        };
        let (operand, maximum_length) = emitter.emit(identities, value.root, 0)?;
        if maximum_length > *capacity {
            return unsupported("owned byte initializer exceeds its declared capacity");
        }
        let (empty, _) = emitter.emission.literal(identities, &[])?;
        emitter
            .emission
            .concatenate(operand, empty, structural_type, qualifications)
    }

    pub(crate) fn emit_owned_literal(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        bytes: &[u8],
        structural_type: StructuralTypeId,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        structural_types: &[StructuralTypeDeclaration],
        literal_places: &mut Vec<StructuralPlaceDeclaration>,
        next_place: &mut u64,
        next_value: &mut u64,
        next_obligation: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<
        (
            StructuralPlaceDeclaration,
            terminal_psi::StructuralOperationResult,
        ),
        LoweringError,
    > {
        let literal = ByteLiteral::reserve(
            identities,
            bytes,
            structural_type,
            structural_types,
            next_place,
        )?;
        if !matches!(literal.storage, LiteralStorage::Owned { .. }) {
            return unsupported("owned literal local carrier is not a bounded byte owner");
        }
        literal.declare_views(literal_places)?;
        let (producer, result) =
            literal.emit(qualifications, next_value, next_obligation, operations)?;
        let OperationResult::Structural(result) = result else {
            return unsupported("owned literal materialization has no owned result");
        };
        Ok((
            operation_result_place(literal.place, producer, structural_type),
            result,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_call_literal(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        bytes: &[u8],
        structural_type: StructuralTypeId,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        structural_types: &[StructuralTypeDeclaration],
        places: &mut Vec<StructuralPlaceDeclaration>,
        next_place: &mut u64,
        next_value: &mut u64,
        next_obligation: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<StructuralPlaceDeclaration, LoweringError> {
        let literal = ByteLiteral::reserve(
            identities,
            bytes,
            structural_type,
            structural_types,
            next_place,
        )?;
        let declaration_ordinal = super::next_byte_sequence_literal_ordinal(places)?;
        literal.declare_views(places)?;
        let (producer, result) =
            literal.emit(qualifications, next_value, next_obligation, operations)?;
        let kind = if matches!(result, OperationResult::Structural(_)) {
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            }
        } else {
            StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            }
        };
        let declaration = StructuralPlaceDeclaration {
            id: literal.place,
            kind,
        };
        if matches!(result, OperationResult::Structural(_)) {
            places.push(declaration);
        }
        Ok(declaration)
    }

    struct ByteEmission<'input> {
        structural_types: &'input [StructuralTypeDeclaration],
        places: &'input mut Vec<StructuralPlaceDeclaration>,
        next_place: &'input mut u64,
        next_value: &'input mut u64,
        next_obligation: &'input mut u64,
        operations: &'input mut OperationBuffer,
    }

    impl ByteEmission<'_> {
        fn length(
            &mut self,
            kind: OperationKind,
        ) -> Result<semantic_vocabulary::ValueId, LoweringError> {
            let length = value_id(allocate_dense(self.next_value)?);
            self.operations.push_leaf(
                scalar_result_declaration(length, terminal_scalar_type(PrimitiveType::U64)?),
                kind,
            );
            Ok(length)
        }

        fn shared_whole_length(
            &mut self,
            source: semantic_vocabulary::PlaceId,
        ) -> Result<semantic_vocabulary::ValueId, LoweringError> {
            self.length(OperationKind::ByteSequenceLength {
                source: terminal_psi::StructuralArgument::whole(
                    source,
                    terminal_psi::StructuralAccess::SharedBorrow,
                ),
            })
        }

        fn literal(
            &mut self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            bytes: &[u8],
        ) -> Result<(ByteSequenceOperand, u64), LoweringError> {
            let source = emit_literal_view(
                identities,
                bytes,
                Vec::new(),
                self.structural_types,
                self.places,
                self.next_place,
                self.operations,
            )?;
            let capacity = u64::try_from(bytes.len())
                .map_err(|_| LoweringError::Unsupported("byte literal length overflow"))?;
            let length = self.shared_whole_length(source)?;
            Ok((whole_operand(source, length), capacity))
        }

        fn concatenate(
            &mut self,
            left: ByteSequenceOperand,
            right: ByteSequenceOperand,
            structural_type: StructuralTypeId,
            qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        ) -> Result<
            (
                StructuralPlaceDeclaration,
                terminal_psi::StructuralOperationResult,
            ),
            LoweringError,
        > {
            let source = place_id(allocate_dense(self.next_place)?);
            let (id, result) = emit_concatenation(
                source,
                left,
                right,
                structural_type,
                qualifications,
                self.next_obligation,
                self.operations,
            )?;
            let declaration = operation_result_place(source, id, structural_type);
            Ok((declaration, result))
        }
    }

    fn emit_concatenation(
        place: semantic_vocabulary::PlaceId,
        left: ByteSequenceOperand,
        right: ByteSequenceOperand,
        structural_type: StructuralTypeId,
        mut qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        next_obligation: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<
        (
            semantic_vocabulary::OperationId,
            terminal_psi::StructuralOperationResult,
        ),
        LoweringError,
    > {
        qualifications.sort();
        qualifications.dedup();
        let result = terminal_psi::StructuralOperationResult {
            place,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications,
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        let id = operations.push_leaf(
            OperationResult::Structural(result.clone()),
            OperationKind::EstablishByteSequenceConcatenation {
                left,
                right,
                capacity_obligation: obligation_id(allocate_dense(next_obligation)?),
            },
        );
        Ok((id, result))
    }

    struct ValueEmitter<'input> {
        plan: &'input CheckedByteSequenceValuePlan,
        parameters: &'input [(u32, StructuralParameterDeclaration)],
        locals: &'input [crate::expression_preparation::bindings::StructuralLocalBinding],
        views: &'input [crate::expression_preparation::bindings::view_locals::ViewLocalBinding],
        emission: ByteEmission<'input>,
    }

    impl ValueEmitter<'_> {
        fn local_source(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Result<&terminal_psi::StructuralOperationResult, LoweringError> {
            let mut locals = self
                .locals
                .iter()
                .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol));
            let crate::expression_preparation::bindings::StructuralLocalBinding {
                argument: local,
                ..
            } = locals.next().ok_or(LoweringError::Unsupported(
                "byte value local has no established home",
            ))?;
            if locals.next().is_some()
                || local.access != terminal_psi::StructuralAccess::Owned
                || !local.path.is_empty()
            {
                return unsupported("byte value local lost its exact whole owned home");
            }
            self.emission
                .operations
                .operations
                .iter()
                .filter_map(|operation| operation.result.structural())
                .find(|result| result.place == local.place)
                .or_else(|| {
                    self.emission
                        .operations
                        .structural_values
                        .iter()
                        .filter_map(|(_, binding)| match binding {
                            StructuralBinding::OperationResult(result) => Some(result),
                            _ => None,
                        })
                        .find(|result| result.place == local.place)
                })
                .ok_or(LoweringError::Unsupported(
                    "byte value local has no completed producer",
                ))
        }

        fn field_source(
            &self,
            root: checked_trees::CheckedStoreRoot,
        ) -> Result<(semantic_vocabulary::PlaceId, StructuralTypeId), LoweringError> {
            match root {
                checked_trees::CheckedStoreRoot::Parameter { position } => {
                    let (_, parameter) = self
                        .parameters
                        .iter()
                        .find(|(authored, _)| *authored == position)
                        .ok_or(LoweringError::Unsupported("byte value parameter is absent"))?;
                    if !matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::Owned
                            | terminal_psi::StructuralAccess::SharedBorrow
                            | terminal_psi::StructuralAccess::MutableBorrow
                    ) || !matches!(
                        parameter.multiplicity,
                        terminal_psi::StructuralMultiplicity::Unrestricted
                            | terminal_psi::StructuralMultiplicity::Affine
                    ) || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                    {
                        return unsupported("byte value parameter has no ordinary read authority");
                    }
                    Ok((parameter.place, parameter.structural_type))
                }
                checked_trees::CheckedStoreRoot::Local { symbol } => {
                    let produced = self.local_source(symbol)?;
                    if !matches!(
                        produced.multiplicity,
                        terminal_psi::StructuralMultiplicity::Unrestricted
                            | terminal_psi::StructuralMultiplicity::Affine
                    ) || !produced.qualifications.is_empty()
                        || !produced.projected_qualifications.is_empty()
                        || !produced.claims.is_empty()
                    {
                        return unsupported("byte value local lost its ordinary read custody");
                    }
                    Ok((produced.place, produced.structural_type))
                }
            }
        }

        fn owned(
            &mut self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            left: ByteSequenceOperand,
            right: ByteSequenceOperand,
            capacity: u64,
        ) -> Result<(semantic_vocabulary::PlaceId, semantic_vocabulary::ValueId), LoweringError>
        {
            let identity = format!("compiler(byte-sequence-owned:{capacity})");
            let structural_type = self
                .emission
                .structural_types
                .iter()
                .find(|candidate| {
                    candidate.identity == identities.intern(&identity)
                        && candidate.shape
                            == StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                            )
                })
                .ok_or(LoweringError::Unsupported(
                    "owned byte carrier absent from shared catalog",
                ))?
                .id;
            let (declaration, result) =
                self.emission
                    .concatenate(left, right, structural_type, Vec::new())?;
            self.emission.places.push(declaration);
            let length = self.emission.shared_whole_length(result.place)?;
            Ok((result.place, length))
        }

        fn emit(
            &mut self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,

            node: arena::Handle<ByteValue>,
            depth: usize,
        ) -> Result<(ByteSequenceOperand, u64), LoweringError> {
            if depth > 128 || !self.plan.nodes.iter().any(|(identity, _)| identity == node) {
                return unsupported("byte value plan has an invalid child");
            }
            match self.plan.nodes.get(node) {
                ByteValue::Literal(bytes) => self.emission.literal(identities, bytes),
                ByteValue::Descriptor { .. } => {
                    unsupported("descriptor value is not an owned byte-content snapshot")
                }
                ByteValue::CallResult {
                    binding_ordinal,
                    capacity,
                    ..
                } => {
                    let produced =
                        completed_call_result(self.emission.operations, *binding_ordinal)?;
                    if produced.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                        || !produced.projected_qualifications.is_empty()
                        || !produced.claims.is_empty()
                        || !self.emission.structural_types.iter().any(|declaration| {
                            declaration.id == produced.structural_type
                                && declaration.shape
                                    == StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                            capacity: *capacity,
                                        },
                                    )
                        })
                    {
                        return unsupported("byte call result lost exact completed owned custody");
                    }
                    let source = produced.place;
                    let length = self.emission.shared_whole_length(source)?;
                    Ok((whole_operand(source, length), *capacity))
                }
                ByteValue::Whole { root, capacity } => {
                    let (source, structural_type) = match *root {
                        checked_trees::CheckedStoreRoot::Parameter { position } => {
                            let (_, parameter) = self
                                .parameters
                                .iter()
                                .find(|(authored, _)| *authored == position)
                                .ok_or(LoweringError::Unsupported(
                                    "whole byte value parameter is absent",
                                ))?;
                            if !matches!(
                                parameter.access,
                                terminal_psi::StructuralAccess::Owned
                                    | terminal_psi::StructuralAccess::SharedBorrow
                                    | terminal_psi::StructuralAccess::MutableBorrow
                            ) || parameter.multiplicity
                                != terminal_psi::StructuralMultiplicity::Unrestricted
                                || !parameter.projected_qualifications.is_empty()
                            {
                                return unsupported(
                                    "whole byte parameter has no ordinary read authority",
                                );
                            }
                            (parameter.place, parameter.structural_type)
                        }
                        checked_trees::CheckedStoreRoot::Local { symbol } => {
                            let mut views = self.views.iter().filter(|view| view.symbol == symbol);
                            if let Some(view) = views.next() {
                                if views.next().is_some()
                                    || !matches!(
                                        view.access,
                                        terminal_psi::StructuralAccess::SharedBorrow
                                            | terminal_psi::StructuralAccess::MutableBorrow
                                    )
                                {
                                    return unsupported(
                                        "byte value view local has no unique readable descriptor",
                                    );
                                }
                                (view.place, view.structural_type)
                            } else {
                                let produced = self.local_source(symbol)?;
                                if produced.multiplicity
                                    != terminal_psi::StructuralMultiplicity::Unrestricted
                                    || !produced.projected_qualifications.is_empty()
                                    || !produced.claims.is_empty()
                                {
                                    return unsupported(
                                        "whole byte local has no ordinary read custody",
                                    );
                                }
                                (produced.place, produced.structural_type)
                            }
                        }
                    };
                    if !self.emission.structural_types.iter().any(|row| {
                        row.id == structural_type
                            && readable_byte_source_bound(&row.shape, *capacity)
                    }) {
                        return unsupported("whole byte source has a different capacity");
                    }
                    let length = self.emission.shared_whole_length(source)?;
                    Ok((whole_operand(source, length), *capacity))
                }
                ByteValue::Field {
                    root,
                    carrier_path,
                    field_identity,
                    capacity,
                } => {
                    let (source_place, source_type) = self.field_source(*root)?;
                    let (path, field) = lower_structural_field_path(
                        identities,
                        source_type,
                        carrier_path,
                        field_identity,
                        self.emission.structural_types,
                    )?;
                    if !field
                        .field_type
                        .byte_sequence_carrier()
                        .is_some_and(|carrier| {
                            readable_byte_source_bound(
                                &StructuralTypeShape::ByteSequence(carrier),
                                *capacity,
                            )
                        })
                    {
                        return unsupported("byte value field has a different carrier");
                    }
                    let source = terminal_psi::StructuralArgument {
                        place: source_place,
                        path: path.clone(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    };
                    let field = field.id;
                    let length =
                        self.emission
                            .length(OperationKind::StructuralByteSequenceFieldLength {
                                source: source.place,
                                path,
                                field,
                            })?;
                    Ok((
                        ByteSequenceOperand::Field {
                            source,
                            field,
                            length,
                        },
                        *capacity,
                    ))
                }
                ByteValue::Concatenation {
                    left,
                    right,
                    capacity,
                } => {
                    let (left, left_capacity) = self.emit(identities, *left, depth + 1)?;
                    let (right, right_capacity) = self.emit(identities, *right, depth + 1)?;
                    if left_capacity.checked_add(right_capacity) != Some(*capacity) {
                        return unsupported(
                            "byte concatenation capacity differs from operand carriers",
                        );
                    }
                    let (source, length) = self.owned(identities, left, right, *capacity)?;
                    Ok((whole_operand(source, length), *capacity))
                }
            }
        }
    }

    fn readable_byte_source_bound(shape: &StructuralTypeShape, maximum_length: u64) -> bool {
        match shape {
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
            ) => *capacity == maximum_length,
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedView {
                capacity,
                access,
            }) => {
                *capacity == maximum_length
                    && matches!(
                        access,
                        None | Some(
                            terminal_psi::StructuralAccess::SharedBorrow
                                | terminal_psi::StructuralAccess::MutableBorrow
                        )
                    )
            }
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access },
            ) => matches!(
                access,
                None | Some(
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                )
            ),
            _ => false,
        }
    }
}
pub(crate) mod structural_observations {

    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use checked_trees::CheckedUnitStructuralPathSegment;
    use semantic_vocabulary::CanonicalStructuralPathSegment;
    use semantic_vocabulary::{ScalarReadProjection, ScalarType, StructuralTypeId};
    use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

    pub(crate) struct ObservationPath<'declaration> {
        pub(crate) segments: Vec<CheckedUnitStructuralPathSegment>,
        pub(crate) declaration: Option<&'declaration StructuralTypeDeclaration>,
        pub(crate) scalar_field: Option<&'declaration terminal_psi::StructuralFieldDeclaration>,
        pub(crate) canonical: Option<Vec<CanonicalStructuralPathSegment>>,
        pub(crate) observation: Option<Vec<ScalarReadProjection>>,
    }

    impl ObservationPath<'_> {
        pub(crate) fn static_observation_path(
            &self,
        ) -> Option<Vec<CanonicalStructuralPathSegment>> {
            self.observation
                .as_ref()?
                .iter()
                .map(|projection| match projection {
                    ScalarReadProjection::Referent => {
                        Some(CanonicalStructuralPathSegment::Referent)
                    }
                    ScalarReadProjection::Static(segment) => Some(*segment),
                    ScalarReadProjection::Index(_) | ScalarReadProjection::Extent => None,
                })
                .collect()
        }

        pub(crate) fn field_type(&self) -> Result<StructuralFieldType, LoweringError> {
            if let Some(field) = self.scalar_field {
                return Ok(field.field_type.clone());
            }
            let declaration = self.declaration.ok_or(LoweringError::Unsupported(
                "structural observation has no declared endpoint",
            ))?;
            Ok(match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => StructuralFieldType::Scalar(scalar),
                _ => StructuralFieldType::Structural(declaration.id),
            })
        }

        pub(crate) fn scalar_type(&self) -> Option<ScalarType> {
            if let Some(field) = self.scalar_field {
                return field.field_type.scalar_type();
            }
            match &self.declaration?.shape {
                StructuralTypeShape::PrimitiveScalar(scalar_type) => Some(*scalar_type),
                _ => None,
            }
        }
    }

    fn carrier_declaration(
        selected: StructuralTypeId,
        declarations: &[StructuralTypeDeclaration],
    ) -> Result<&StructuralTypeDeclaration, LoweringError> {
        let mut matching = declarations.iter().filter(|row| row.id == selected);
        let (Some(declaration), None) = (matching.next(), matching.next()) else {
            return unsupported("field observation requires one carrier declaration");
        };
        Ok(declaration)
    }

    pub(crate) fn readable_carrier(
        mut selected: StructuralTypeId,
        declarations: &[StructuralTypeDeclaration],
    ) -> Result<(&StructuralTypeDeclaration, usize), LoweringError> {
        let mut references = Vec::new();
        loop {
            let declaration = carrier_declaration(selected, declarations)?;
            let StructuralTypeShape::Reference { referent, access } = &declaration.shape else {
                return Ok((declaration, references.len()));
            };
            if !matches!(access, terminal_psi::reading_borrow_pair_pattern!())
                || references.contains(&selected)
            {
                return unsupported("field observation crosses an unreadable or cyclic reference");
            }
            references.push(selected);
            selected = *referent;
        }
    }

    pub(crate) struct DeclaredPredicatePath {
        pub(crate) path: Vec<CanonicalStructuralPathSegment>,
        pub(crate) field_type: StructuralFieldType,
    }

    #[derive(Clone, Copy)]
    enum PathPurpose {
        ExecutableObservation,
        StoredValue,
        DeclaredPredicate,
    }

    pub(crate) fn explicit_path<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        structural_type: StructuralTypeId,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        declarations: &'a [StructuralTypeDeclaration],
    ) -> Result<ObservationPath<'a>, LoweringError> {
        resolve_path(
            identities,
            structural_type,
            path,
            declarations,
            PathPurpose::ExecutableObservation,
        )
    }

    pub(crate) fn stored_value_path<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        structural_type: StructuralTypeId,
        path: &[CheckedUnitStructuralPathSegment],
        declarations: &'a [StructuralTypeDeclaration],
    ) -> Result<ObservationPath<'a>, LoweringError> {
        use checked_trees::CheckedStructuralPredicatePathSegment as Source;
        let path = path
            .iter()
            .map(|segment| match segment {
                CheckedUnitStructuralPathSegment::Referent => Ok(Source::Referent),
                CheckedUnitStructuralPathSegment::Case(identity) => {
                    Ok(Source::Case(identity.clone()))
                }
                CheckedUnitStructuralPathSegment::Field(identity) => {
                    Ok(Source::Field(identity.clone()))
                }
                CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    Ok(Source::FixedIndex(*index))
                }
                _ => unsupported("stored value path requires static declared coordinates"),
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        resolve_path(
            identities,
            structural_type,
            &path,
            declarations,
            PathPurpose::StoredValue,
        )
    }

    pub(crate) fn declared_predicate_path(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        structural_type: StructuralTypeId,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        declarations: &[StructuralTypeDeclaration],
    ) -> Result<DeclaredPredicatePath, LoweringError> {
        let selected = resolve_path(
            identities,
            structural_type,
            path,
            declarations,
            PathPurpose::DeclaredPredicate,
        )?;
        let path = selected
            .static_observation_path()
            .ok_or(LoweringError::Unsupported(
                "declared predicate path requires occurrence-bound selector evidence",
            ))?;
        Ok(DeclaredPredicatePath {
            path,
            field_type: selected.field_type()?,
        })
    }

    fn resolve_path<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        structural_type: StructuralTypeId,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        declarations: &'a [StructuralTypeDeclaration],
        purpose: PathPurpose,
    ) -> Result<ObservationPath<'a>, LoweringError> {
        use checked_trees::CheckedStructuralPredicatePathSegment as Source;
        let mut selected = Some(structural_type);
        let mut selected_case_fields: Option<&[terminal_psi::StructuralFieldDeclaration]> = None;
        let mut result = Vec::with_capacity(path.len());
        let mut canonical = Some(Vec::with_capacity(path.len()));
        let mut observation = Some(Vec::with_capacity(path.len()));
        let mut scalar_field: Option<&terminal_psi::StructuralFieldDeclaration> = None;
        for segment in path {
            if matches!(segment, Source::Referent) {
                if selected_case_fields.is_some() {
                    return unsupported("reference observation has an unfinished case path");
                }
                let receiver = selected.ok_or(LoweringError::Unsupported(
                    "reference observation has no structural receiver",
                ))?;
                let declaration = carrier_declaration(receiver, declarations)?;
                let StructuralTypeShape::Reference { referent, access } = declaration.shape else {
                    return unsupported("reference observation requires a reference carrier");
                };
                if !matches!(purpose, PathPurpose::DeclaredPredicate)
                    && !matches!(access, terminal_psi::reading_borrow_pair_pattern!())
                {
                    return unsupported("reference observation requires readable access");
                }
                result.push(CheckedUnitStructuralPathSegment::Referent);
                canonical = None;
                if let Some(observation) = &mut observation {
                    observation.push(ScalarReadProjection::Referent);
                }
                selected = Some(referent);
                scalar_field = None;
                continue;
            }
            let (declaration, crossings) = match selected {
                Some(selected) => {
                    let (declaration, crossings) = match purpose {
                        PathPurpose::ExecutableObservation | PathPurpose::StoredValue => {
                            readable_carrier(selected, declarations)?
                        }
                        PathPurpose::DeclaredPredicate => {
                            (carrier_declaration(selected, declarations)?, 0)
                        }
                    };
                    (Some(declaration), crossings)
                }
                None => (None, 0),
            };
            result.extend(std::iter::repeat_n(
                CheckedUnitStructuralPathSegment::Referent,
                crossings,
            ));
            if crossings != 0 {
                canonical = None;
                if let Some(observation) = &mut observation {
                    observation.extend(std::iter::repeat_n(
                        ScalarReadProjection::Referent,
                        crossings,
                    ));
                }
            }
            let byte_carrier = declaration
                .and_then(|declaration| match declaration.shape {
                    StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                    _ => None,
                })
                .or_else(|| {
                    scalar_field.and_then(|field| field.field_type.byte_sequence_carrier())
                });
            if let Some(carrier) = byte_carrier {
                match segment {
                    Source::FixedIndex(index) => {
                        if matches!(
                            carrier,
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }
                                | terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. }
                                if *index >= capacity
                        ) {
                            return unsupported(
                                "byte observation index exceeds its storage capacity",
                            );
                        }
                        result.push(CheckedUnitStructuralPathSegment::FixedIndex(*index));
                        if let Some(observation) = &mut observation {
                            observation.push(ScalarReadProjection::Static(
                                CanonicalStructuralPathSegment::FixedIndex(*index),
                            ));
                        }
                    }
                    Source::RuntimeIndex(index) => {
                        observation = None;
                        result.push(CheckedUnitStructuralPathSegment::RuntimeIndex(
                            index.clone(),
                        ));
                    }
                    _ => return unsupported("byte observation requires an element selector"),
                }
                let scalar_type = terminal_scalar_type(typed_trees::types::PrimitiveType::U8)?;
                let element = StructuralFieldType::Scalar(scalar_type)
                    .leaf_declaration(declarations.iter())
                    .ok_or(LoweringError::Unsupported(
                        "byte observation has no unique retained element declaration",
                    ))?;
                selected = Some(element.id);
                scalar_field = None;
                canonical = None;
                continue;
            }
            let declaration = declaration.ok_or(LoweringError::Unsupported(
                "field observation descends below a scalar",
            ))?;
            selected = match (segment, &declaration.shape) {
                (
                    Source::Case(identity),
                    StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
                ) if selected_case_fields.is_none() => {
                    let mut matching = cases
                        .iter()
                        .filter(|case| case.identity == identities.intern(&identity.spelling()));
                    let (Some(case), None) = (matching.next(), matching.next()) else {
                        return unsupported("field observation requires one declared case");
                    };
                    selected_case_fields = Some(&case.fields);
                    result.push(CheckedUnitStructuralPathSegment::Case(identity.clone()));
                    if let Some(observation) = &mut observation {
                        observation.push(ScalarReadProjection::Static(
                            CanonicalStructuralPathSegment::Case(case.id),
                        ));
                    }
                    if let Some(canonical) = &mut canonical {
                        canonical.push(CanonicalStructuralPathSegment::Case(case.id));
                    }
                    Some(declaration.id)
                }
                (Source::Field(identity), _) => {
                    let fields = selected_case_fields
                        .take()
                        .or_else(|| declaration.shape.direct_fields())
                        .ok_or(LoweringError::Unsupported(
                            "field observation requires a field carrier",
                        ))?;
                    let mut matching = fields
                        .iter()
                        .filter(|field| field.identity == identities.intern(&identity.spelling()));
                    let (Some(field), None) = (matching.next(), matching.next()) else {
                        return unsupported("field observation requires one declared field");
                    };
                    if field.relevance.is_erased() {
                        return unsupported("field observation cannot read an erased field");
                    }
                    result.push(CheckedUnitStructuralPathSegment::Field(identity.clone()));
                    if let Some(observation) = &mut observation {
                        observation.push(ScalarReadProjection::Static(
                            CanonicalStructuralPathSegment::Field(field.id),
                        ));
                    }
                    if let Some(canonical) = &mut canonical {
                        canonical.push(CanonicalStructuralPathSegment::Field(field.id));
                    }
                    match &field.field_type {
                        StructuralFieldType::Structural(child) => Some(*child),
                        _ => {
                            scalar_field = Some(field);
                            None
                        }
                    }
                }
                (
                    Source::FixedIndex(index),
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    },
                ) if index < length => {
                    result.push(CheckedUnitStructuralPathSegment::FixedIndex(*index));
                    if let Some(observation) = &mut observation {
                        observation.push(ScalarReadProjection::Static(
                            CanonicalStructuralPathSegment::FixedIndex(*index),
                        ));
                    }
                    if let Some(canonical) = &mut canonical {
                        canonical.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                    }
                    Some(*element)
                }
                (Source::FixedIndex(index), StructuralTypeShape::ElementView { element, .. }) => {
                    result.push(CheckedUnitStructuralPathSegment::FixedIndex(*index));
                    if let Some(observation) = &mut observation {
                        observation.push(ScalarReadProjection::Static(
                            CanonicalStructuralPathSegment::FixedIndex(*index),
                        ));
                    }
                    canonical = None;
                    Some(*element)
                }
                (
                    Source::RuntimeIndex(index),
                    StructuralTypeShape::FixedArray { element, .. }
                    | StructuralTypeShape::ElementView { element, .. },
                ) => {
                    observation = None;
                    result.push(CheckedUnitStructuralPathSegment::RuntimeIndex(
                        index.clone(),
                    ));
                    canonical = None;
                    Some(*element)
                }
                _ => {
                    return unsupported(
                        "field observation has an incompatible declared projection",
                    );
                }
            };
        }
        if selected_case_fields.is_some() {
            return unsupported("field observation case selection has no payload field");
        }
        let declaration = match selected {
            Some(selected) => {
                let (declaration, crossings) =
                    if !matches!(purpose, PathPurpose::ExecutableObservation)
                        || matches!(path.last(), Some(Source::Referent))
                    {
                        (carrier_declaration(selected, declarations)?, 0)
                    } else {
                        readable_carrier(selected, declarations)?
                    };
                result.extend(std::iter::repeat_n(
                    CheckedUnitStructuralPathSegment::Referent,
                    crossings,
                ));
                if crossings != 0 {
                    canonical = None;
                    if let Some(observation) = &mut observation {
                        observation.extend(std::iter::repeat_n(
                            ScalarReadProjection::Referent,
                            crossings,
                        ));
                    }
                }
                Some(declaration)
            }
            None => None,
        };
        Ok(ObservationPath {
            segments: result,
            declaration,
            scalar_field,
            canonical,
            observation,
        })
    }
}
pub(crate) mod structural_scalar_store {

    use super::{
        CheckedUnitStructuralPathSegment, LoweringError, OperationKind, PlaceId, ScalarType,
        StructuralFieldId, StructuralFieldType, StructuralParameterDeclaration,
        StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
        ValueId, allocate_dense, obligation_id, terminal_scalar_type, unsupported,
    };
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::emission::runtime_elements::ProjectionStep;
    use crate::emission::runtime_elements::selector_step;
    use crate::emission::runtime_elements::static_segments;

    pub(crate) struct LoweredStructuralScalarStore {
        pub steps: Vec<ProjectionStep>,
        pub field: StructuralFieldId,
        pub requires_range_obligation: bool,
        bounded_integer: Option<semantic_vocabulary::BoundedIntegerType>,
        pub range_decided: bool,
    }

    impl LoweredStructuralScalarStore {
        fn at_field(
            scalar_type: ScalarType,
            steps: Vec<ProjectionStep>,
            field: &terminal_psi::StructuralFieldDeclaration,
        ) -> Result<Self, LoweringError> {
            if field.field_type.scalar_type() != Some(scalar_type) {
                return unsupported("structural scalar store field has a different type");
            }
            Ok(Self {
                bounded_integer: match &field.field_type {
                    StructuralFieldType::BoundedInteger(integer) => Some(*integer),
                    _ => None,
                },
                range_decided: false,
                steps,
                field: field.id,
                requires_range_obligation: terminal_semantics::scalar_field_requires_evidence(
                    &field.field_type,
                ),
            })
        }

        pub(crate) fn bind_range_decision(
            &mut self,
            checked: &checked_trees::CheckedTrees,
            primitive_type: checked_trees::types::PrimitiveType,
            range_decision: arena::Handle<checked_trees::CheckedIntegerAssignmentDecision>,
        ) -> Result<(), LoweringError> {
            self.range_decided = source_range_decided(
                checked,
                primitive_type,
                range_decision,
                self.bounded_integer,
            )?;
            Ok(())
        }

        pub(crate) fn into_operation(
            self,
            destination: PlaceId,
            path: Vec<StructuralPathSegment>,
            value: ValueId,
            calls: &mut CallEmissionContext<'_>,
            operations: &mut OperationBuffer,
        ) -> Result<OperationKind, LoweringError> {
            let range_obligation = if self.requires_range_obligation {
                Some(obligation_id(allocate_dense(
                    &mut calls.next_obligation_identity,
                )?))
            } else {
                None
            };
            if self.range_decided
                && let Some(obligation) = range_obligation
            {
                operations.source_checked_obligations.push(obligation);
            }
            Ok(OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field: self.field,
                value,
                range_obligation,
            })
        }
    }

    pub(crate) fn source_range_decided(
        checked: &checked_trees::CheckedTrees,
        primitive_type: checked_trees::types::PrimitiveType,
        range_decision: arena::Handle<checked_trees::CheckedIntegerAssignmentDecision>,
        bounded_integer: Option<semantic_vocabulary::BoundedIntegerType>,
    ) -> Result<bool, LoweringError> {
        receiving_range_decided(
            checked,
            primitive_type,
            range_decision,
            bounded_integer,
            false,
        )
    }

    pub(crate) fn atomic_source_range_decided(
        checked: &checked_trees::CheckedTrees,
        primitive_type: checked_trees::types::PrimitiveType,
        range_decision: arena::Handle<checked_trees::CheckedIntegerAssignmentDecision>,
        bounded_integer: Option<semantic_vocabulary::BoundedIntegerType>,
    ) -> Result<bool, LoweringError> {
        receiving_range_decided(
            checked,
            primitive_type,
            range_decision,
            bounded_integer,
            true,
        )
    }

    fn receiving_range_decided(
        checked: &checked_trees::CheckedTrees,
        primitive_type: checked_trees::types::PrimitiveType,
        range_decision: arena::Handle<checked_trees::CheckedIntegerAssignmentDecision>,
        bounded_integer: Option<semantic_vocabulary::BoundedIntegerType>,
        atomic_resident: bool,
    ) -> Result<bool, LoweringError> {
        if !range_decision.is_valid() {
            return Ok(false);
        }
        let decision = checked.facts.proof.integer_assignments.get(range_decision);
        if decision.atomic_resident != atomic_resident {
            return Ok(false);
        }
        let Some(integer) = bounded_integer else {
            return Ok(false);
        };
        let mathematical = |value| match value {
            semantic_vocabulary::IntegerValue::Signed(value) => {
                numerics::bignum::BigInt::from_i128(value)
            }
            semantic_vocabulary::IntegerValue::Unsigned(value) => {
                numerics::bignum::BigInt::from_u128(value)
            }
        };
        let decided = terminal_scalar_type(primitive_type)?
            == ScalarType::Integer(integer.integer_type())
            && decision.minimum == mathematical(integer.minimum())
            && decision.maximum == mathematical(integer.maximum());
        Ok(decided)
    }

    pub(crate) fn lower_structural_scalar_store_place(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        primitive_type: checked_trees::types::PrimitiveType,
        carrier_path: &[CheckedUnitStructuralPathSegment],
        field_identity: &checked_trees::CheckedMemberIdentity,
        parameter: &StructuralParameterDeclaration,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<LoweredStructuralScalarStore, LoweringError> {
        let scalar_type = terminal_scalar_type(primitive_type)?;
        validate_storage_access(parameter)?;
        let (steps, field) = lower_structural_field_steps(
            identities,
            parameter.structural_type,
            carrier_path,
            field_identity,
            structural_types,
        )?;
        LoweredStructuralScalarStore::at_field(scalar_type, steps, field)
    }

    pub(crate) fn lower_structural_scalar_store_local(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        primitive_type: checked_trees::types::PrimitiveType,
        carrier_path: &[CheckedUnitStructuralPathSegment],
        field_identity: &checked_trees::CheckedMemberIdentity,
        root_type: StructuralTypeId,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<LoweredStructuralScalarStore, LoweringError> {
        let scalar_type = terminal_scalar_type(primitive_type)?;
        let (steps, field) = lower_structural_field_steps(
            identities,
            root_type,
            carrier_path,
            field_identity,
            structural_types,
        )?;
        LoweredStructuralScalarStore::at_field(scalar_type, steps, field)
    }

    pub(crate) fn lower_structural_field_place<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        carrier_path: &[CheckedUnitStructuralPathSegment],
        field_identity: &checked_trees::CheckedMemberIdentity,
        parameter: &StructuralParameterDeclaration,
        structural_types: &'a [StructuralTypeDeclaration],
    ) -> Result<
        (
            Vec<StructuralPathSegment>,
            &'a terminal_psi::StructuralFieldDeclaration,
        ),
        LoweringError,
    > {
        let (steps, field) = lower_structural_field_place_steps(
            identities,
            carrier_path,
            field_identity,
            parameter,
            structural_types,
        )?;
        Ok((static_segments(steps)?, field))
    }

    pub(crate) fn lower_structural_field_place_steps<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        carrier_path: &[CheckedUnitStructuralPathSegment],
        field_identity: &checked_trees::CheckedMemberIdentity,
        parameter: &StructuralParameterDeclaration,
        structural_types: &'a [StructuralTypeDeclaration],
    ) -> Result<
        (
            Vec<ProjectionStep>,
            &'a terminal_psi::StructuralFieldDeclaration,
        ),
        LoweringError,
    > {
        validate_storage_access(parameter)?;
        lower_structural_field_steps(
            identities,
            parameter.structural_type,
            carrier_path,
            field_identity,
            structural_types,
        )
    }

    pub(crate) fn validate_root_access(
        parameter: &StructuralParameterDeclaration,
    ) -> Result<(), LoweringError> {
        validate_storage_access(parameter)?;
        if !parameter.qualifications.is_empty() || !parameter.projected_qualifications.is_empty() {
            return unsupported("structural store requires qualification preservation");
        }
        Ok(())
    }

    fn validate_storage_access(
        parameter: &StructuralParameterDeclaration,
    ) -> Result<(), LoweringError> {
        let access_matches = matches!(
            parameter.access,
            terminal_psi::ownership::StructuralAccess::Owned
                | terminal_psi::exclusive_borrow_pair_pattern!()
        );
        if !access_matches {
            return unsupported("structural scalar store lost exact exclusive custody");
        }
        Ok(())
    }

    pub(crate) fn lower_structural_field_path<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root_type: StructuralTypeId,
        carrier_path: &[CheckedUnitStructuralPathSegment],
        field_identity: &checked_trees::CheckedMemberIdentity,
        structural_types: &'a [StructuralTypeDeclaration],
    ) -> Result<
        (
            Vec<StructuralPathSegment>,
            &'a terminal_psi::StructuralFieldDeclaration,
        ),
        LoweringError,
    > {
        let (steps, field) = lower_structural_field_steps(
            identities,
            root_type,
            carrier_path,
            field_identity,
            structural_types,
        )?;
        Ok((static_segments(steps)?, field))
    }

    fn structural_field_root(
        root_type: StructuralTypeId,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<(&StructuralTypeDeclaration, Vec<ProjectionStep>), LoweringError> {
        let declaration = terminal_psi::find_structural_type(structural_types, root_type).ok_or(
            LoweringError::Unsupported("structural scalar store root type is absent"),
        )?;
        let mut field_owner = declaration;
        let mut steps = Vec::new();
        while let StructuralTypeShape::Reference { referent, .. } = &field_owner.shape {
            if steps.len() >= structural_types.len() {
                return unsupported("field geometry contains a cyclic reference path");
            }
            field_owner = terminal_psi::find_structural_type(structural_types, *referent).ok_or(
                LoweringError::Unsupported("structural scalar store referent type is absent"),
            )?;
            steps.push(ProjectionStep::Static(StructuralPathSegment::Referent));
        }
        Ok((field_owner, steps))
    }

    pub(crate) fn lower_structural_field_steps<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root_type: StructuralTypeId,
        carrier_path: &[CheckedUnitStructuralPathSegment],
        field_identity: &checked_trees::CheckedMemberIdentity,
        structural_types: &'a [StructuralTypeDeclaration],
    ) -> Result<
        (
            Vec<ProjectionStep>,
            &'a terminal_psi::StructuralFieldDeclaration,
        ),
        LoweringError,
    > {
        let mut field_owner = terminal_psi::find_structural_type(structural_types, root_type)
            .ok_or(LoweringError::Unsupported(
                "structural scalar store root type is absent",
            ))?;
        let mut steps = Vec::new();
        let mut payload_fields = None;
        for segment in carrier_path {
            if matches!(segment, CheckedUnitStructuralPathSegment::Referent) {
                if payload_fields.is_some() {
                    return unsupported("field path case must select a payload field");
                }
                let StructuralTypeShape::Reference { referent, .. } = &field_owner.shape else {
                    return unsupported("field path reference crossing has no reference carrier");
                };
                field_owner = terminal_psi::find_structural_type(structural_types, *referent)
                    .ok_or(LoweringError::Unsupported(
                        "field path referent type is absent",
                    ))?;
                steps.push(ProjectionStep::Static(StructuralPathSegment::Referent));
                continue;
            }
            let (owner, crossings) = structural_field_root(field_owner.id, structural_types)?;
            field_owner = owner;
            steps.extend(crossings);
            if let CheckedUnitStructuralPathSegment::Case(identity) = segment {
                if payload_fields.is_some() {
                    return unsupported("field path repeats an unconsumed case namespace");
                }
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &field_owner.shape
                else {
                    return unsupported("field path case owner is not a sum");
                };
                let mut matching = cases
                    .iter()
                    .filter(|case| case.identity == identities.intern(&identity.spelling()));
                let selected = matching
                    .next()
                    .ok_or(LoweringError::Unsupported("field path case is absent"))?;
                if matching.next().is_some() {
                    return unsupported("field path case is ambiguous");
                }
                payload_fields = Some(selected.fields.as_slice());
                steps.push(ProjectionStep::Static(StructuralPathSegment::Case(
                    selected.id,
                )));
                continue;
            }
            if payload_fields.is_some()
                && !matches!(segment, CheckedUnitStructuralPathSegment::Field(_))
            {
                return unsupported("field path case must select a payload field");
            }
            let nested = match segment {
                CheckedUnitStructuralPathSegment::Field(identity) if identity.is_valid() => {
                    let fields =
                        match payload_fields.take() {
                            Some(fields) => fields,
                            None => field_owner.shape.direct_fields().ok_or(
                                LoweringError::Unsupported(
                                    "structural scalar store carrier has no direct fields",
                                ),
                            )?,
                        };
                    let carriers = fields
                        .iter()
                        .filter(|field| {
                            field.identity == identities.intern(&identity.spelling())
                                && !field.relevance.is_erased()
                                && matches!(field.field_type, StructuralFieldType::Structural(_))
                        })
                        .collect::<Vec<_>>();
                    let [carrier] = carriers.as_slice() else {
                        return unsupported(
                            "structural scalar store carrier is absent or ambiguous",
                        );
                    };
                    let Some(nested) = carrier.field_type.as_structural() else {
                        unreachable!("carrier shape was checked above")
                    };
                    steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                        identities.intern(&identity.spelling()),
                    )));
                    nested
                }
                CheckedUnitStructuralPathSegment::FixedIndex(_)
                | CheckedUnitStructuralPathSegment::RuntimeIndex(_) => {
                    let (step, element) =
                        element_selection(field_owner, segment, structural_types)?;
                    if matches!(step, ProjectionStep::AssignmentIndex { .. })
                        && steps.contains(&step)
                    {
                        return unsupported("structural scalar store carrier path is unsupported");
                    }
                    steps.push(step);
                    element
                }
                _ => return unsupported("structural scalar store carrier path is unsupported"),
            };
            field_owner = terminal_psi::find_structural_type(structural_types, nested).ok_or(
                LoweringError::Unsupported("structural scalar store carrier type is absent"),
            )?;
        }
        let (owner, crossings) = structural_field_root(field_owner.id, structural_types)?;
        field_owner = owner;
        steps.extend(crossings);
        let Some(fields) = payload_fields.or_else(|| field_owner.shape.direct_fields()) else {
            return unsupported("structural scalar store field owner is not a record");
        };
        let matching = fields
            .iter()
            .filter(|field| {
                field.identity == identities.intern(&field_identity.spelling())
                    && !field.relevance.is_erased()
            })
            .collect::<Vec<_>>();
        let [field] = matching.as_slice() else {
            return unsupported("structural scalar store field is absent or ambiguous");
        };
        Ok((steps, field))
    }

    pub(crate) fn element_selection(
        owner: &StructuralTypeDeclaration,
        segment: &CheckedUnitStructuralPathSegment,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<(ProjectionStep, StructuralTypeId), LoweringError> {
        let (element, view) = match &owner.shape {
            StructuralTypeShape::FixedArray { element, length } => {
                if let terminal_psi::ArrayExtent::Literal(length) = length
                    && matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(index) if index >= length)
                {
                    return unsupported("structural element index is out of bounds");
                }
                (*element, false)
            }
            StructuralTypeShape::ElementView { element, .. } => (*element, true),
            StructuralTypeShape::ByteSequence(_) => {
                let element = StructuralFieldType::Scalar(terminal_scalar_type(
                    checked_trees::types::PrimitiveType::U8,
                )?)
                .leaf_declaration(structural_types.iter())
                .ok_or(LoweringError::Unsupported(
                    "byte element has no unique retained scalar declaration",
                ))?;
                (element.id, true)
            }
            _ => return unsupported("structural element carrier is not a fixed array or view"),
        };
        let step = match segment {
            CheckedUnitStructuralPathSegment::FixedIndex(index) if view => {
                ProjectionStep::RuntimeIndexLiteral { index: *index }
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                ProjectionStep::Static(StructuralPathSegment::FixedIndex(*index))
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => selector_step(selector),
            _ => return unsupported("structural element selector is unsupported"),
        };
        Ok((step, element))
    }

    pub(crate) enum WindowLeaf<'a> {
        Field(&'a terminal_psi::StructuralFieldDeclaration),
        Subtree { structural_type: StructuralTypeId },
    }

    pub(crate) fn lower_window_path<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root_type: StructuralTypeId,
        path: &[CheckedUnitStructuralPathSegment],
        structural_types: &'a [StructuralTypeDeclaration],
    ) -> Result<(Vec<ProjectionStep>, WindowLeaf<'a>, bool), LoweringError> {
        let mut field_owner = terminal_psi::find_structural_type(structural_types, root_type)
            .ok_or(LoweringError::Unsupported(
                "structural window root type is absent",
            ))?;
        let mut steps = Vec::new();
        let mut borrowed_backing = false;
        if path.is_empty() {
            let (owner, crossings) = structural_field_root(root_type, structural_types)?;
            field_owner = owner;
            borrowed_backing = !crossings.is_empty();
            steps = crossings;
        }
        let mut cursor = path;
        let mut payload: Option<(
            semantic_vocabulary::StructuralCaseId,
            &'a [terminal_psi::StructuralFieldDeclaration],
        )> = None;
        while let Some((segment, rest)) = cursor.split_first() {
            cursor = rest;
            if payload.is_some() && !matches!(segment, CheckedUnitStructuralPathSegment::Field(_)) {
                return unsupported("structural window case must select a payload field");
            }
            if !matches!(segment, CheckedUnitStructuralPathSegment::Referent) && payload.is_none() {
                let (owner, crossings) = structural_field_root(field_owner.id, structural_types)?;
                field_owner = owner;
                borrowed_backing |= !crossings.is_empty();
                steps.extend(crossings);
            }
            match segment {
                CheckedUnitStructuralPathSegment::Case(identity) => {
                    let (StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. }) = &field_owner.shape
                    else {
                        return unsupported("structural window case owner is not a sum");
                    };
                    let mut matching = cases
                        .iter()
                        .filter(|case| case.identity == identities.intern(&identity.spelling()));
                    let selected = matching.next().ok_or(LoweringError::Unsupported(
                        "structural window case is absent",
                    ))?;
                    if matching.next().is_some() {
                        return unsupported("structural window case is ambiguous");
                    }
                    payload = Some((selected.id, selected.fields.as_slice()));
                    steps.push(ProjectionStep::Static(StructuralPathSegment::Case(
                        selected.id,
                    )));
                }
                CheckedUnitStructuralPathSegment::Field(identity) if identity.is_valid() => {
                    let selected_case = payload.map(|(case, _)| case);
                    let fields = match payload.take() {
                        Some((_, fields)) => fields,
                        None => {
                            let (terminal_psi::record_or_mixed_shape_pattern!(fields)) =
                                &field_owner.shape
                            else {
                                return unsupported("structural window carrier is not a record");
                            };
                            fields.as_slice()
                        }
                    };
                    if rest.is_empty() {
                        let mut leafs = fields.iter().filter(|field| {
                            field.identity == identities.intern(&identity.spelling())
                                && !field.relevance.is_erased()
                        });
                        let leaf = leafs.next();
                        if leaf.is_none() || leafs.next().is_some() {
                            return unsupported("structural window leaf is absent or ambiguous");
                        }
                        return Ok((
                            steps,
                            WindowLeaf::Field(leaf.expect("leaf checked above")),
                            borrowed_backing,
                        ));
                    }
                    let mut carriers = fields.iter().filter(|field| {
                        field.identity == identities.intern(&identity.spelling())
                            && !field.relevance.is_erased()
                    });
                    let carrier = carriers.next();
                    if carrier.is_none() || carriers.next().is_some() {
                        return unsupported("structural window carrier is absent or ambiguous");
                    }
                    let carrier = carrier.expect("carrier presence was checked above");
                    steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                        identities.intern(&identity.spelling()),
                    )));
                    let selection = selected_case
                        .map(StructuralPathSegment::Case)
                        .into_iter()
                        .chain([StructuralPathSegment::Field(carrier.identity)])
                        .collect::<Vec<_>>();
                    let nested = terminal_semantics::runtime_structural_path_tip(
                        structural_types.iter(),
                        field_owner.id,
                        &selection,
                    )
                    .map(|row| row.id)
                    .ok_or(LoweringError::Unsupported(
                        "structural window carrier has no exact retained declaration",
                    ))?;
                    field_owner = terminal_psi::find_structural_type(structural_types, nested)
                        .ok_or(LoweringError::Unsupported(
                            "structural window nested carrier type is absent",
                        ))?;
                }
                CheckedUnitStructuralPathSegment::FixedIndex(_)
                | CheckedUnitStructuralPathSegment::RuntimeIndex(_) => {
                    borrowed_backing |= matches!(
                        field_owner.shape,
                        StructuralTypeShape::ElementView { .. }
                            | StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                            )
                    );
                    let (step, element) =
                        element_selection(field_owner, segment, structural_types)?;
                    if !(matches!(step, ProjectionStep::AssignmentIndex { .. })
                        && steps.contains(&step))
                    {
                        steps.push(step);
                    }
                    field_owner = terminal_psi::find_structural_type(structural_types, element)
                        .ok_or(LoweringError::Unsupported(
                            "structural window element type is absent",
                        ))?;
                    if rest.is_empty() {
                        return Ok((
                            steps,
                            WindowLeaf::Subtree {
                                structural_type: element,
                            },
                            borrowed_backing,
                        ));
                    }
                }
                CheckedUnitStructuralPathSegment::Referent => {
                    borrowed_backing = true;
                    let StructuralTypeShape::Reference { referent, .. } = field_owner.shape else {
                        return unsupported(
                            "structural window referent crossing names no reference",
                        );
                    };
                    field_owner = terminal_psi::find_structural_type(structural_types, referent)
                        .ok_or(LoweringError::Unsupported(
                            "structural window referent type is absent",
                        ))?;
                    steps.push(ProjectionStep::Static(StructuralPathSegment::Referent));
                }
                _ => return unsupported("structural window carrier path is unsupported"),
            }
        }
        Ok((
            steps,
            WindowLeaf::Subtree {
                structural_type: field_owner.id,
            },
            borrowed_backing,
        ))
    }
}
pub(crate) mod structural_scalar_store_source {
    use super::{
        CheckedScalarExpressionRole, CheckedTrees, LoweringError, ScalarType, unsupported,
    };
    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::expression_preparation::bindings::ScalarBindings;
    use terminal_psi::ValueDeclaration;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn direct_value(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        statement_index: u32,
        role: CheckedScalarExpressionRole,
        value: &checked_trees::CheckedScalarStoreValue,
        scalar_type: ScalarType,
        bindings: &ScalarBindings,
        values: &[ValueDeclaration],
    ) -> Result<LoweredDirectExpression, LoweringError> {
        match value {
            checked_trees::CheckedScalarStoreValue::Pure(_) => {
                bindings.expression_at(identities, checked, state, statement_index, role)
            }
            checked_trees::CheckedScalarStoreValue::ScalarResult { position } => {
                let position = usize::try_from(*position).map_err(|_| {
                    LoweringError::Unsupported("store call-result position exceeds usize")
                })?;
                if position.checked_add(1) != Some(values.len()) {
                    return unsupported(
                        "store call result is not this statement's established scalar result",
                    );
                }
                Ok(LoweredDirectExpression::Local {
                    position,
                    scalar_type,
                })
            }
            checked_trees::CheckedScalarStoreValue::Computation(_) => {
                unsupported("a computed store value has no direct expression")
            }
        }
    }

    pub(crate) fn computation_root(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: u32,
        value: &checked_trees::CheckedScalarStoreValue,
        field_identity: Option<&checked_trees::CheckedMemberIdentity>,
    ) -> Result<
        Option<(
            checked_trees::CheckedScalarComputationHandle,
            CheckedScalarExpressionRole,
        )>,
        LoweringError,
    > {
        let role = CheckedScalarExpressionRole::AssignmentValue;
        let plans = &checked.facts.values.scalar_computations;
        let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
            root.state == state && root.statement_ordinal == statement_index && root.role == role
        });
        let root = roots.next();
        if roots.next().is_some() {
            return unsupported("field assignment has duplicate computation roots");
        }
        let checked_trees::CheckedScalarStoreValue::Computation(handle) = value else {
            return if root.is_none() {
                Ok(None)
            } else {
                unsupported("field assignment replaced its computation with a pure value")
            };
        };
        let handle = *handle;
        let root = match root {
            Some(root) => root,
            None => {
                let mut record_roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
                    root.state == state
                        && root.statement_ordinal == statement_index
                        && root.root == handle
                        && matches!(root.role, CheckedScalarExpressionRole::RecordField { .. })
                });
                let root = record_roots.next().ok_or(LoweringError::Unsupported(
                    "field assignment has no computation root",
                ))?;
                if record_roots.next().is_some() {
                    return unsupported("field assignment has duplicate record computation roots");
                }
                root
            }
        };
        if root.machine != machine || root.root != handle || !plans.nodes.is_valid(handle) {
            return unsupported("field assignment computation root has different custody");
        }
        if matches!(root.role, CheckedScalarExpressionRole::RecordField { .. })
            && field_identity.is_none()
        {
            return unsupported("record store field has no field identity");
        }
        Ok(Some((handle, root.role)))
    }
}

pub(crate) mod expression_validation {
    use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
    use crate::emission::operation_emission::expressions::{
        LoweredDirectExpression, LoweredElementPathSegment,
    };
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::ScalarType;

    pub(crate) fn validate_boolean_parameter_types(
        expression: &LoweredBooleanReturnExpression,
        parameter_types: &[ScalarType],
    ) -> Result<(), LoweringError> {
        fn validate_lowered_operand_path(
            path: &[LoweredElementPathSegment],
            parameter_types: &[ScalarType],
        ) -> Result<(), LoweringError> {
            for segment in path {
                if let LoweredElementPathSegment::RuntimeIndex {
                    selector, extent, ..
                } = segment
                {
                    validate_direct_parameter_types(selector, parameter_types)?;
                    if let Some(extent) = extent {
                        validate_direct_parameter_types(extent, parameter_types)?;
                    }
                }
            }
            Ok(())
        }
        match expression {
            LoweredBooleanReturnExpression::ScalarObservation { operand } => {
                if operand.scalar_type() != ScalarType::Boolean {
                    return unsupported("Boolean observation lost its scalar type");
                }
                validate_direct_parameter_types(operand, parameter_types)
            }
            LoweredBooleanReturnExpression::Constant { .. } => Ok(()),
            LoweredBooleanReturnExpression::StructuralField { .. }
            | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
            | LoweredBooleanReturnExpression::PrimitiveRead { .. } => Ok(()),
            LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
                unsupported("unresolved structural field crossed Boolean type validation")
            }
            LoweredBooleanReturnExpression::Parameter { position }
            | LoweredBooleanReturnExpression::Local { position } => {
                if parameter_types.get(*position) == Some(&ScalarType::Boolean) {
                    Ok(())
                } else {
                    unsupported("scalar graph guard parameters must be Boolean")
                }
            }
            LoweredBooleanReturnExpression::Not { operand } => {
                validate_boolean_parameter_types(operand, parameter_types)
            }
            LoweredBooleanReturnExpression::Equal { left, right }
            | LoweredBooleanReturnExpression::And { left, right }
            | LoweredBooleanReturnExpression::Or { left, right } => {
                validate_boolean_parameter_types(left, parameter_types)?;
                validate_boolean_parameter_types(right, parameter_types)
            }
            LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
                validate_direct_parameter_types(left, parameter_types)?;
                validate_direct_parameter_types(right, parameter_types)
            }
            LoweredBooleanReturnExpression::IndexedCaseMembership { index, .. } => {
                validate_direct_parameter_types(index, parameter_types)
            }
            LoweredBooleanReturnExpression::ByteSequencePredicate { path, .. } => {
                validate_lowered_operand_path(path, parameter_types)
            }
            LoweredBooleanReturnExpression::ByteSequenceEqual { left, right, .. } => {
                validate_lowered_operand_path(&left.path, parameter_types)?;
                validate_lowered_operand_path(&right.path, parameter_types)
            }
        }
    }

    pub(crate) fn validate_direct_parameter_types(
        expression: &LoweredDirectExpression,
        parameter_types: &[ScalarType],
    ) -> Result<(), LoweringError> {
        match expression {
            LoweredDirectExpression::ErasedParameter { .. } => {
                unsupported("erased formal is proof-only and has no runtime parameter type")
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type,
            }
            | LoweredDirectExpression::Local {
                position,
                scalar_type,
            } => {
                if parameter_types.get(*position) == Some(scalar_type) {
                    Ok(())
                } else {
                    unsupported("scalar graph integer guard parameter type does not match")
                }
            }
            LoweredDirectExpression::IntegerLiteral { .. }
            | LoweredDirectExpression::ScalarApplication { .. }
            | LoweredDirectExpression::PrimitiveRead { .. }
            | LoweredDirectExpression::StructuralField { .. }
            | LoweredDirectExpression::AtomicLoad { .. }
            | LoweredDirectExpression::IeeeFloatLiteral { .. }
            | LoweredDirectExpression::ImplementationSelection { .. } => Ok(()),
            LoweredDirectExpression::IndexedPrimitiveRead {
                index,
                extent,
                element_path,
                ..
            } => {
                if let Some(extent) = extent {
                    validate_direct_parameter_types(extent, parameter_types)?;
                }
                validate_direct_parameter_types(index, parameter_types)?;
                validate_path_parameter_types(element_path, parameter_types)
            }
            LoweredDirectExpression::ByteSequenceLength { path, .. }
            | LoweredDirectExpression::ByteSequenceFieldLength { path, .. }
            | LoweredDirectExpression::ElementViewLength { path, .. } => {
                validate_path_parameter_types(path, parameter_types)
            }
            LoweredDirectExpression::ByteSequenceFieldRead { path, index, .. } => {
                validate_path_parameter_types(path, parameter_types)?;
                validate_direct_parameter_types(index, parameter_types)
            }
            LoweredDirectExpression::ElementViewRead { index, path, .. } => {
                validate_direct_parameter_types(index, parameter_types)?;
                validate_path_parameter_types(path, parameter_types)
            }
            LoweredDirectExpression::IntegerBinary { left, right, .. }
            | LoweredDirectExpression::SaturatingShiftLeft { left, right, .. } => {
                validate_direct_parameter_types(left, parameter_types)?;
                validate_direct_parameter_types(right, parameter_types)
            }
            LoweredDirectExpression::IntegerBitwiseNot { operand, .. } => {
                validate_direct_parameter_types(operand, parameter_types)
            }
            LoweredDirectExpression::IntegerWiden { operand, .. } => {
                validate_direct_parameter_types(operand, parameter_types)
            }
            LoweredDirectExpression::IntegerExactCast { operand, .. }
            | LoweredDirectExpression::RepresentationView { operand, .. } => {
                validate_direct_parameter_types(operand, parameter_types)
            }
            LoweredDirectExpression::Boolean { expression } => {
                validate_boolean_parameter_types(expression, parameter_types)
            }
        }
    }

    fn validate_path_parameter_types(
        path: &[crate::emission::operation_emission::expressions::LoweredElementPathSegment],
        parameter_types: &[ScalarType],
    ) -> Result<(), LoweringError> {
        use crate::emission::operation_emission::expressions::{
            LoweredElementPathSegment, TERMINAL_NESTED_SELECTOR_LIMIT,
        };
        let selectors = path.iter().filter_map(|segment| match segment {
            LoweredElementPathSegment::RuntimeIndex {
                selector, extent, ..
            } => Some((selector, extent)),
            _ => None,
        });
        if selectors.clone().count() > TERMINAL_NESTED_SELECTOR_LIMIT {
            return unsupported("field observation exceeds its selector namespace");
        }
        for (selector, extent) in selectors {
            if let Some(extent) = extent {
                validate_direct_parameter_types(extent, parameter_types)?;
            }
            validate_direct_parameter_types(selector, parameter_types)?;
            if !matches!(selector.scalar_type(), ScalarType::Integer(integer)
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64 && !integer.is_address())
            {
                return unsupported("field observation selector requires an exact u64 value");
            }
        }
        Ok(())
    }

    pub(crate) fn contains_short_circuit(expression: &LoweredBooleanReturnExpression) -> bool {
        match expression {
            LoweredBooleanReturnExpression::ScalarObservation { operand } => {
                direct_expression_contains_short_circuit(operand)
            }
            LoweredBooleanReturnExpression::Constant { .. }
            | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
            | LoweredBooleanReturnExpression::IndexedCaseMembership { .. }
            | LoweredBooleanReturnExpression::PrimitiveRead { .. }
            | LoweredBooleanReturnExpression::Parameter { .. }
            | LoweredBooleanReturnExpression::Local { .. }
            | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
            | LoweredBooleanReturnExpression::StructuralField { .. }
            | LoweredBooleanReturnExpression::ByteSequencePredicate { .. }
            | LoweredBooleanReturnExpression::ByteSequenceEqual { .. }
            | LoweredBooleanReturnExpression::IntegerComparison { .. } => false,
            LoweredBooleanReturnExpression::Not { operand } => contains_short_circuit(operand),
            LoweredBooleanReturnExpression::Equal { left, right } => {
                contains_short_circuit(left) || contains_short_circuit(right)
            }
            LoweredBooleanReturnExpression::And { .. }
            | LoweredBooleanReturnExpression::Or { .. } => true,
        }
    }

    pub(crate) fn direct_expression_contains_short_circuit(
        expression: &LoweredDirectExpression,
    ) -> bool {
        matches!(
            expression,
            LoweredDirectExpression::Boolean { expression }
                if contains_short_circuit(expression)
        )
    }

    pub(crate) fn validate_short_circuit_expression(
        expression: &LoweredBooleanReturnExpression,
    ) -> Result<(), LoweringError> {
        match expression {
            LoweredBooleanReturnExpression::ScalarObservation { operand } => {
                if operand.scalar_type() != ScalarType::Boolean
                    || direct_expression_contains_short_circuit(operand)
                {
                    return unsupported(
                        "Boolean scalar observation has an invalid carrier or evaluation shape",
                    );
                }
                Ok(())
            }
            LoweredBooleanReturnExpression::Constant { .. }
            | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
            | LoweredBooleanReturnExpression::IndexedCaseMembership { .. }
            | LoweredBooleanReturnExpression::PrimitiveRead { .. }
            | LoweredBooleanReturnExpression::Parameter { .. }
            | LoweredBooleanReturnExpression::StructuralField { .. }
            | LoweredBooleanReturnExpression::Local { .. }
            | LoweredBooleanReturnExpression::ByteSequencePredicate { .. }
            | LoweredBooleanReturnExpression::ByteSequenceEqual { .. }
            | LoweredBooleanReturnExpression::IntegerComparison { .. } => Ok(()),
            LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
                unsupported("unresolved structural field crossed Boolean validation")
            }
            LoweredBooleanReturnExpression::Not { operand } => {
                validate_short_circuit_expression(operand)
            }
            LoweredBooleanReturnExpression::Equal { left, right } => {
                validate_short_circuit_expression(left)?;
                validate_short_circuit_expression(right)
            }
            LoweredBooleanReturnExpression::And { left, right }
            | LoweredBooleanReturnExpression::Or { left, right } => {
                validate_short_circuit_expression(left)?;
                validate_short_circuit_expression(right)
            }
        }
    }
}
pub(crate) mod scalar_types {

    use crate::shared::lowering_error::{LoweringError, unsupported};
    use checked_trees::types::PrimitiveType;
    use semantic_vocabulary::{
        IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, ScalarType,
    };

    pub(crate) fn integer_scalar_type(
        primitive: PrimitiveType,
    ) -> Result<ScalarType, LoweringError> {
        if primitive == PrimitiveType::Addr {
            return Ok(ScalarType::Integer(
                IntegerType::supported_address_representation(),
            ));
        }
        let (sign, bits) = match primitive {
            PrimitiveType::Selector => (IntegerSign::Unsigned, 64),
            primitive => match primitive.integer_sign_bits() {
                Some(pair) => pair,
                None => return unsupported("only primitive integers are supported"),
            },
        };
        IntegerType::new(sign, bits)
            .map(ScalarType::Integer)
            .map_err(|_| LoweringError::InvalidPsiIntegerType)
    }

    pub(crate) fn selector_scalar_type() -> ScalarType {
        ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is a valid integer type"),
        )
    }

    pub(crate) fn terminal_scalar_type(
        primitive: PrimitiveType,
    ) -> Result<ScalarType, LoweringError> {
        match primitive {
            PrimitiveType::Bool => Ok(ScalarType::Boolean),
            PrimitiveType::F32 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
            PrimitiveType::F64 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
            primitive => integer_scalar_type(primitive),
        }
    }

    pub(crate) fn integer_landing_scalar_type(
        literal: &numerics::literals::IntegerLiteral,
    ) -> Result<ScalarType, LoweringError> {
        let primitive = PrimitiveType::from_landed_integer(
            literal
                .landing()
                .ok_or(LoweringError::UnlandedIntegerLiteral)?
                .landed_type,
        );
        integer_scalar_type(primitive)
    }

    pub(crate) fn integer_value(
        literal: &numerics::literals::IntegerLiteral,
        scalar_type: ScalarType,
    ) -> Result<IntegerValue, LoweringError> {
        let Some(integer_type) = scalar_type.as_integer() else {
            return Err(LoweringError::InvalidPsiIntegerType);
        };
        let landing = literal
            .landing()
            .ok_or(LoweringError::UnlandedIntegerLiteral)?;
        if landing.landed_type.bit_width() != u32::from(integer_type.bits())
            || landing.landed_type.is_signed() != (integer_type.sign() == IntegerSign::Signed)
        {
            return Err(LoweringError::IntegerLandingMismatch);
        }
        let value = match integer_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(
                literal
                    .value_i64()
                    .map(i128::from)
                    .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
            ),
            IntegerSign::Unsigned => IntegerValue::Unsigned(
                literal
                    .value_u64()
                    .map(u128::from)
                    .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
            ),
        };
        if !integer_type.admits(value) {
            return Err(LoweringError::IntegerLiteralOutsidePsiType);
        }
        Ok(value)
    }
}
pub(crate) mod store_destination {

    use crate::expression_preparation::bindings::structural_local_by_root;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::{PlaceId, StructuralTypeId};

    pub(crate) struct LocalStoreRoot<'a> {
        pub(crate) argument: &'a terminal_psi::StructuralArgument,
        pub(crate) structural_type: StructuralTypeId,
        pub(crate) multiplicity: terminal_psi::StructuralMultiplicity,
        pub(crate) selected_type: StructuralTypeId,
        pub(crate) qualifications: &'a [semantic_vocabulary::StructuralDomainId],
        pub(crate) projected_qualifications: &'a [terminal_psi::StructuralPathQualification],
        pub(crate) additional_input_qualifications:
            &'a [terminal_psi::StructuralPathQualification],
    }

    impl LocalStoreRoot<'_> {
        pub(crate) fn require_unqualified(&self) -> Result<(), LoweringError> {
            self.require_disjoint_from_write(&[])
        }

        pub(crate) fn require_disjoint_from_write(
            &self,
            written: &[terminal_psi::StructuralPathSegment],
        ) -> Result<(), LoweringError> {
            if !terminal_semantics::structural_memberships_disjoint_from_write(
                self.qualifications,
                self.projected_qualifications,
                written,
            ) {
                return unsupported(
                    "store local carries qualification evidence the replacement does not restore",
                );
            }
            Ok(())
        }
    }

    pub(crate) fn local_store_place<'a>(
        symbol: symbols::SymbolHandle,
        locals: &'a [crate::expression_preparation::bindings::StructuralLocalBinding],
        parameters: impl Iterator<Item = &'a terminal_psi::StructuralParameterDeclaration>,
        operations: &'a crate::emission::operation_emission::buffer::OperationBuffer,
    ) -> Result<LocalStoreRoot<'a>, LoweringError> {
        let crate::expression_preparation::bindings::StructuralLocalBinding {
            argument,
            structural_type: selected_type,
            ..
        } = structural_local_by_root(locals, facts::PlaceRoot::Symbol(symbol)).ok_or(
            LoweringError::Unsupported("store names an unknown structural local"),
        )?;
        if !matches!(
            argument.access,
            terminal_psi::mutating_access_pair_pattern!()
        ) {
            return unsupported("store local does not retain exclusive storage access");
        }
        let declaration = parameters
            .map(|parameter| {
                (
                    parameter.place,
                    parameter.structural_type,
                    parameter.qualifications.as_slice(),
                    parameter.projected_qualifications.as_slice(),
                    parameter.multiplicity,
                    parameter.additional_input_qualifications.as_slice(),
                )
            })
            .chain(operations.operations.iter().filter_map(|operation| {
                operation.result.structural().map(|result| {
                    (
                        result.place,
                        result.structural_type,
                        result.qualifications.as_slice(),
                        result.projected_qualifications.as_slice(),
                        result.multiplicity,
                        result.additional_input_qualifications.as_slice(),
                    )
                })
            }))
            .find(|(place, _, _, _, _, _)| *place == argument.place)
            .ok_or(LoweringError::Unsupported(
                "store local has no declared storage root",
            ))?;
        Ok(LocalStoreRoot {
            argument,
            structural_type: declaration.1,
            multiplicity: declaration.4,
            selected_type: *selected_type,
            qualifications: declaration.2,
            projected_qualifications: declaration.3,
            additional_input_qualifications: declaration.5,
        })
    }

    pub(crate) fn local_store_root<'a>(
        symbol: symbols::SymbolHandle,
        locals: &'a [crate::expression_preparation::bindings::StructuralLocalBinding],
        parameters: impl Iterator<Item = &'a terminal_psi::StructuralParameterDeclaration>,
        operations: &'a crate::emission::operation_emission::buffer::OperationBuffer,
    ) -> Result<LocalStoreRoot<'a>, LoweringError> {
        let root = local_store_place(symbol, locals, parameters, operations)?;
        if !root.argument.path.is_empty() {
            return unsupported("whole-home store names a projected local alias");
        }
        Ok(root)
    }

    pub(crate) fn unqualified_local_store_root<'a>(
        symbol: symbols::SymbolHandle,
        locals: &'a [crate::expression_preparation::bindings::StructuralLocalBinding],
        parameters: impl Iterator<Item = &'a terminal_psi::StructuralParameterDeclaration>,
        operations: &'a crate::emission::operation_emission::buffer::OperationBuffer,
    ) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
        let root = local_store_root(symbol, locals, parameters, operations)?;
        root.require_unqualified()?;
        Ok((root.argument.place, root.structural_type))
    }

    pub(crate) fn descriptor_home(
        symbol: symbols::SymbolHandle,
        locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
        operations: &crate::emission::operation_emission::buffer::OperationBuffer,
    ) -> Result<(PlaceId, StructuralTypeId, bool), LoweringError> {
        let crate::expression_preparation::bindings::StructuralLocalBinding { argument, .. } =
            structural_local_by_root(locals, facts::PlaceRoot::Symbol(symbol)).ok_or(
                LoweringError::Unsupported("descriptor store names an unknown structural local"),
            )?;
        if !argument.path.is_empty() {
            return unsupported("descriptor store local is not a whole home");
        }
        let producer = operations
            .operations
            .iter()
            .find(|operation| {
                operation
                    .result
                    .structural()
                    .is_some_and(|result| result.place == argument.place)
            })
            .ok_or(LoweringError::Unsupported(
                "descriptor store local has no produced home",
            ))?;
        let Some(produced) = producer.result.structural() else {
            return unsupported("descriptor store local has no produced home");
        };
        if !matches!(
            producer.kind,
            terminal_psi::OperationKind::EstablishStructuralLocal {}
        ) || !matches!(
            produced.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        ) || !produced.projected_qualifications.is_empty()
            || !produced.claims.is_empty()
        {
            return unsupported("descriptor store local is not a declared descriptor home");
        }
        Ok((
            argument.place,
            produced.structural_type,
            !produced.qualifications.is_empty(),
        ))
    }
}

pub(crate) mod boolean_control {
    use crate::emission::case_payload_dispatch::CaseDispatch;
    use crate::emission::expression_validation::contains_short_circuit;
    use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
    use crate::emission::operation_emission::boolean::emit_boolean_expression;
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::emission::value_successor;
    use crate::shared::terminal_identities::{block_id, edge_id, mint};
    use semantic_vocabulary::{BlockId, ValueId};
    use terminal_psi::{
        Block, StructuralCaseSuccessorEdge, SuccessorEdge, Terminator, ValueDeclaration,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum LoweredBooleanDecision {
        Value(LoweredBooleanReturnExpression),
        Test {
            condition: LoweredBooleanReturnExpression,
            when_true: Box<LoweredBooleanDecision>,
            when_false: Box<LoweredBooleanDecision>,
        },
        CaseDispatch {
            dispatch: CaseDispatch,
            when_true: Box<LoweredBooleanDecision>,
            when_false: Box<LoweredBooleanDecision>,
        },
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind_boolean_decision<F>(
        decision: LoweredBooleanDecision,
        continuation: &F,
    ) -> LoweredBooleanDecision
    where
        F: Fn(&LoweredBooleanReturnExpression) -> LoweredBooleanDecision,
    {
        match decision {
            LoweredBooleanDecision::Value(expression) => continuation(&expression),
            LoweredBooleanDecision::Test {
                condition,
                when_true,
                when_false,
            } => LoweredBooleanDecision::Test {
                condition,
                when_true: Box::new(bind_boolean_decision(*when_true, continuation)),
                when_false: Box::new(bind_boolean_decision(*when_false, continuation)),
            },
            LoweredBooleanDecision::CaseDispatch {
                dispatch,
                when_true,
                when_false,
            } => LoweredBooleanDecision::CaseDispatch {
                dispatch,
                when_true: Box::new(bind_boolean_decision(*when_true, continuation)),
                when_false: Box::new(bind_boolean_decision(*when_false, continuation)),
            },
        }
    }

    fn branch_boolean_decision(
        decision: LoweredBooleanDecision,
        when_true: LoweredBooleanDecision,
        when_false: LoweredBooleanDecision,
    ) -> LoweredBooleanDecision {
        match decision {
            LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value }) => {
                if value {
                    when_true
                } else {
                    when_false
                }
            }
            LoweredBooleanDecision::Value(condition) => LoweredBooleanDecision::Test {
                condition,
                when_true: Box::new(when_true),
                when_false: Box::new(when_false),
            },
            LoweredBooleanDecision::Test {
                condition,
                when_true: nested_true,
                when_false: nested_false,
            } => LoweredBooleanDecision::Test {
                condition,
                when_true: Box::new(branch_boolean_decision(
                    *nested_true,
                    when_true.clone(),
                    when_false.clone(),
                )),
                when_false: Box::new(branch_boolean_decision(
                    *nested_false,
                    when_true,
                    when_false,
                )),
            },
            LoweredBooleanDecision::CaseDispatch {
                dispatch,
                when_true: nested_true,
                when_false: nested_false,
            } => LoweredBooleanDecision::CaseDispatch {
                dispatch,
                when_true: Box::new(branch_boolean_decision(
                    *nested_true,
                    when_true.clone(),
                    when_false.clone(),
                )),
                when_false: Box::new(branch_boolean_decision(
                    *nested_false,
                    when_true,
                    when_false,
                )),
            },
        }
    }

    pub(crate) fn lower_boolean_control_decision(
        expression: &LoweredBooleanReturnExpression,
        when_true: LoweredBooleanDecision,
        when_false: LoweredBooleanDecision,
    ) -> LoweredBooleanDecision {
        match expression {
            LoweredBooleanReturnExpression::And { left, right } => {
                let right = lower_boolean_control_decision(right, when_true, when_false.clone());
                lower_boolean_control_decision(left, right, when_false)
            }
            LoweredBooleanReturnExpression::Or { left, right } => {
                let right = lower_boolean_control_decision(right, when_true.clone(), when_false);
                lower_boolean_control_decision(left, when_true, right)
            }
            LoweredBooleanReturnExpression::Not { operand } if contains_short_circuit(operand) => {
                lower_boolean_control_decision(operand, when_false, when_true)
            }
            expression if contains_short_circuit(expression) => branch_boolean_decision(
                lower_boolean_value_decision(expression),
                when_true,
                when_false,
            ),
            expression => LoweredBooleanDecision::Test {
                condition: expression.clone(),
                when_true: Box::new(when_true),
                when_false: Box::new(when_false),
            },
        }
    }

    pub(crate) fn lower_boolean_value_decision(
        expression: &LoweredBooleanReturnExpression,
    ) -> LoweredBooleanDecision {
        if !contains_short_circuit(expression) {
            return LoweredBooleanDecision::Value(expression.clone());
        }
        match expression {
            LoweredBooleanReturnExpression::And { .. }
            | LoweredBooleanReturnExpression::Or { .. } => lower_boolean_control_decision(
                expression,
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: true,
                }),
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: false,
                }),
            ),
            LoweredBooleanReturnExpression::Not { operand } => {
                bind_boolean_decision(lower_boolean_value_decision(operand), &|operand| {
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Not {
                        operand: Box::new(operand.clone()),
                    })
                })
            }
            LoweredBooleanReturnExpression::Equal { left, right } => {
                bind_boolean_decision(lower_boolean_value_decision(left), &|left| {
                    bind_boolean_decision(lower_boolean_value_decision(right), &|right| {
                        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Equal {
                            left: Box::new(left.clone()),
                            right: Box::new(right.clone()),
                        })
                    })
                })
            }
            LoweredBooleanReturnExpression::Constant { .. }
            | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
            | LoweredBooleanReturnExpression::IndexedCaseMembership { .. }
            | LoweredBooleanReturnExpression::ScalarObservation { .. }
            | LoweredBooleanReturnExpression::PrimitiveRead { .. }
            | LoweredBooleanReturnExpression::Parameter { .. }
            | LoweredBooleanReturnExpression::Local { .. }
            | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
            | LoweredBooleanReturnExpression::StructuralField { .. }
            | LoweredBooleanReturnExpression::ByteSequencePredicate { .. }
            | LoweredBooleanReturnExpression::ByteSequenceEqual { .. }
            | LoweredBooleanReturnExpression::IntegerComparison { .. } => {
                unreachable!("non-short-circuit expressions return above")
            }
        }
    }

    pub(crate) fn boolean_decision_block_count(decision: &LoweredBooleanDecision) -> usize {
        match decision {
            LoweredBooleanDecision::Value(_) => 1,
            LoweredBooleanDecision::Test {
                when_true,
                when_false,
                ..
            }
            | LoweredBooleanDecision::CaseDispatch {
                when_true,
                when_false,
                ..
            } => {
                1 + boolean_decision_block_count(when_true)
                    + boolean_decision_block_count(when_false)
            }
        }
    }

    pub(crate) fn boolean_guard_decision_block_count(decision: &LoweredBooleanDecision) -> usize {
        match decision {
            LoweredBooleanDecision::Value(_) => 0,
            LoweredBooleanDecision::Test {
                when_true,
                when_false,
                ..
            } => {
                1 + boolean_guard_decision_block_count(when_true)
                    + boolean_guard_decision_block_count(when_false)
            }
            LoweredBooleanDecision::CaseDispatch {
                dispatch,
                when_true,
                when_false,
            } => {
                let selected_entry = usize::from(matches!(
                    when_true.as_ref(),
                    LoweredBooleanDecision::Value(_)
                ));
                let unselected_entries = dispatch.cases.len().saturating_sub(1);
                1 + selected_entry
                    + unselected_entries
                    + boolean_guard_decision_block_count(when_true)
                    + boolean_guard_decision_block_count(when_false)
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LoweredBooleanDecisionTarget {
        pub(crate) block: BlockId,
        pub(crate) arguments: Vec<ValueId>,
    }

    fn reserved_guard_block(first_block_identity: u64, block_index: usize) -> BlockId {
        block_id(
            first_block_identity
                .checked_add(
                    u64::try_from(block_index)
                        .expect("reserved guard block count fits a semantic identity"),
                )
                .expect("reserved guard block identity advances"),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn reserve_case_entry(
        target: LoweredBooleanDecisionTarget,
        parameters: Vec<ValueDeclaration>,
        first_block_identity: u64,
        next_edge_identity: &mut u64,
        blocks: &mut Vec<Option<Block>>,
    ) -> LoweredBooleanDecisionTarget {
        let block_index = blocks.len();
        let block = reserved_guard_block(first_block_identity, block_index);
        let edge = mint(next_edge_identity, edge_id);
        blocks.push(Some(Block::plain(
            block,
            parameters,
            Terminator::Jump {
                edge,
                target: target.block,
                arguments: target.arguments,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            },
        )));
        LoweredBooleanDecisionTarget {
            block,
            arguments: Vec::new(),
        }
    }

    fn emit_reserved_boolean_guard_decision_blocks(
        decision: &LoweredBooleanDecision,
        parameters: &[ValueDeclaration],
        block_parameters: Vec<ValueDeclaration>,
        when_true_target: &LoweredBooleanDecisionTarget,
        when_false_target: &LoweredBooleanDecisionTarget,
        first_block_identity: u64,
        next_value_identity: &mut u64,
        next_edge_identity: &mut u64,
        all_operations: &mut OperationBuffer,
        blocks: &mut Vec<Option<Block>>,
    ) -> LoweredBooleanDecisionTarget {
        macro_rules! guard_blocks {
            ($decision:expr, $block_parameters:expr $(,)?) => {
                emit_reserved_boolean_guard_decision_blocks(
                    $decision,
                    parameters,
                    $block_parameters,
                    when_true_target,
                    when_false_target,
                    first_block_identity,
                    next_value_identity,
                    next_edge_identity,
                    all_operations,
                    blocks,
                )
            };
        }

        let incoming_observations = all_operations.storage_observations.clone();
        let target = match decision {
            LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                value: true,
            }) => when_true_target.clone(),
            LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                value: false,
            }) => when_false_target.clone(),
            LoweredBooleanDecision::Value(_) => {
                unreachable!("guard control decisions end in canonical Boolean choices")
            }
            LoweredBooleanDecision::Test {
                condition,
                when_true,
                when_false,
            } => {
                let block_index = blocks.len();
                let block = reserved_guard_block(first_block_identity, block_index);
                blocks.push(None);
                let operation_start = all_operations.len();
                let condition = emit_boolean_expression(
                    condition,
                    parameters,
                    next_value_identity,
                    all_operations,
                );
                let operation_end = all_operations.len();
                let true_edge = mint(next_edge_identity, edge_id);
                let false_edge = mint(next_edge_identity, edge_id);
                let when_true = guard_blocks!(when_true, Vec::new());
                let when_false = guard_blocks!(when_false, Vec::new());
                blocks[block_index] = Some(Block {
                    structural_parameters: Vec::new(),
                    id: block,
                    parameters: block_parameters,
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    operations: all_operations[operation_start..operation_end].to_vec(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: true_edge,
                            target: when_true.block,
                            arguments: when_true.arguments,
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: Vec::new(),
                            cleanup_actions: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: false_edge,
                            target: when_false.block,
                            arguments: when_false.arguments,
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: Vec::new(),
                            cleanup_actions: Vec::new(),
                        },
                    },
                });
                LoweredBooleanDecisionTarget {
                    block,
                    arguments: Vec::new(),
                }
            }
            LoweredBooleanDecision::CaseDispatch {
                dispatch,
                when_true,
                when_false,
            } => {
                let block_index = blocks.len();
                let block = reserved_guard_block(first_block_identity, block_index);
                blocks.push(None);
                let payload_parameters = dispatch
                    .payloads
                    .iter()
                    .map(|(_, value)| *value)
                    .collect::<Vec<_>>();
                let when_true = if matches!(when_true.as_ref(), LoweredBooleanDecision::Value(_)) {
                    let target = guard_blocks!(when_true, Vec::new());
                    reserve_case_entry(
                        target,
                        payload_parameters,
                        first_block_identity,
                        next_edge_identity,
                        blocks,
                    )
                } else {
                    guard_blocks!(when_true, payload_parameters)
                };
                let when_false = guard_blocks!(when_false, Vec::new());
                assert!(
                    when_true.arguments.is_empty() && when_false.arguments.is_empty(),
                    "case dispatch successors bind only payload parameters"
                );
                let cases = dispatch
                    .cases
                    .iter()
                    .map(|case| {
                        let selected = *case == dispatch.selected;
                        let target = if selected {
                            when_true.block
                        } else {
                            reserve_case_entry(
                                when_false.clone(),
                                Vec::new(),
                                first_block_identity,
                                next_edge_identity,
                                blocks,
                            )
                            .block
                        };
                        let edge = mint(next_edge_identity, edge_id);
                        StructuralCaseSuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge,
                            target,
                            case: *case,
                            payload_fields: if selected {
                                dispatch.payloads.iter().map(|(field, _)| *field).collect()
                            } else {
                                Vec::new()
                            },
                            cleanup_actions: Vec::new(),
                        }
                    })
                    .collect();
                blocks[block_index] = Some(Block::plain(
                    block,
                    block_parameters,
                    Terminator::StructuralCase {
                        source: dispatch.source,
                        path: Vec::new(),
                        cases,
                    },
                ));
                LoweredBooleanDecisionTarget {
                    block,
                    arguments: Vec::new(),
                }
            }
        };
        all_operations.storage_observations = incoming_observations;
        target
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_inlined_boolean_guard_blocks(
        decision: &LoweredBooleanDecision,
        parameters: &[ValueDeclaration],
        block_parameters: Vec<ValueDeclaration>,
        when_true_target: &LoweredBooleanDecisionTarget,
        when_false_target: &LoweredBooleanDecisionTarget,
        source_block: BlockId,
        first_synthetic_block: BlockId,
        next_value_identity: &mut u64,
        next_edge_identity: &mut u64,
        all_operations: &mut OperationBuffer,
    ) -> (Block, Vec<Block>) {
        let first_reserved_identity = first_synthetic_block
            .get()
            .checked_sub(1)
            .expect("synthetic Boolean blocks follow source blocks");
        let mut reserved = Vec::new();
        let entry = emit_reserved_boolean_guard_decision_blocks(
            decision,
            parameters,
            block_parameters,
            when_true_target,
            when_false_target,
            first_reserved_identity,
            next_value_identity,
            next_edge_identity,
            all_operations,
            &mut reserved,
        );
        assert_eq!(entry.block.get(), first_reserved_identity);
        assert!(entry.arguments.is_empty());
        let mut reserved = reserved
            .into_iter()
            .map(|block| block.expect("every inlined Boolean guard block is finalized"));
        let mut root = reserved
            .next()
            .expect("short-circuit Boolean guard has a decision root");
        root.id = source_block;
        (root, reserved.collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_reserved_boolean_tuple_stage_blocks(
        decision: &LoweredBooleanDecision,
        parameters: &[ValueDeclaration],
        block_parameters: Vec<ValueDeclaration>,
        next_stage: BlockId,
        carried_arguments: &[ValueId],
        first_block_identity: u64,
        next_value_identity: &mut u64,
        next_edge_identity: &mut u64,
        all_operations: &mut OperationBuffer,
        blocks: &mut Vec<Option<Block>>,
    ) -> BlockId {
        let incoming_observations = all_operations.storage_observations.clone();
        let block_index = blocks.len();
        let block = block_id(
            first_block_identity
                .checked_add(
                    u64::try_from(block_index)
                        .expect("reserved Boolean tuple block count fits a semantic identity"),
                )
                .expect("reserved Boolean tuple block identity advances"),
        );
        blocks.push(None);
        let operation_start = all_operations.len();
        let (terminator, operation_end) = match decision {
            LoweredBooleanDecision::Value(expression) => {
                let value = emit_boolean_expression(
                    expression,
                    parameters,
                    next_value_identity,
                    all_operations,
                );
                let edge = mint(next_edge_identity, edge_id);
                let mut arguments = carried_arguments.to_vec();
                arguments.push(value);
                (
                    Terminator::Jump {
                        structural_arguments: Vec::new(),
                        edge,
                        target: next_stage,
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        arguments,
                        cleanup_actions: Vec::new(),
                    },
                    all_operations.len(),
                )
            }
            LoweredBooleanDecision::CaseDispatch { .. } => {
                unreachable!("case dispatch is planned only for guard decisions")
            }
            LoweredBooleanDecision::Test {
                condition,
                when_true,
                when_false,
            } => {
                let condition = emit_boolean_expression(
                    condition,
                    parameters,
                    next_value_identity,
                    all_operations,
                );
                let operation_end = all_operations.len();
                let true_edge = mint(next_edge_identity, edge_id);
                let false_edge = mint(next_edge_identity, edge_id);
                let when_true = emit_reserved_boolean_tuple_stage_blocks(
                    when_true,
                    parameters,
                    Vec::new(),
                    next_stage,
                    carried_arguments,
                    first_block_identity,
                    next_value_identity,
                    next_edge_identity,
                    all_operations,
                    blocks,
                );
                let when_false = emit_reserved_boolean_tuple_stage_blocks(
                    when_false,
                    parameters,
                    Vec::new(),
                    next_stage,
                    carried_arguments,
                    first_block_identity,
                    next_value_identity,
                    next_edge_identity,
                    all_operations,
                    blocks,
                );
                (
                    Terminator::Conditional {
                        condition,
                        when_true: value_successor(true_edge, when_true),
                        when_false: value_successor(false_edge, when_false),
                    },
                    operation_end,
                )
            }
        };
        blocks[block_index] = Some(Block {
            structural_parameters: Vec::new(),
            id: block,
            parameters: block_parameters,
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            operations: all_operations[operation_start..operation_end].to_vec(),
            terminator,
        });
        all_operations.storage_observations = incoming_observations;
        block
    }
}
pub(crate) mod case_payload_dispatch {
    use crate::emission::boolean_control::LoweredBooleanDecision;
    use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
    use crate::emission::operation_emission::expressions::{
        LoweredDirectExpression, PrimitiveReadSource,
    };
    use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, PlaceId, StructuralCaseId, StructuralFieldId,
    };
    use terminal_psi::{StructuralCaseDeclaration, StructuralFieldType, ValueDeclaration};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CaseDispatch {
        pub(crate) source: PlaceId,
        pub(crate) selected: StructuralCaseId,
        pub(crate) cases: Vec<StructuralCaseId>,
        pub(crate) payloads: Vec<(StructuralFieldId, ValueDeclaration)>,
    }

    pub(crate) struct PlannedGuard {
        pub(crate) decision: LoweredBooleanDecision,
        pub(crate) established: Vec<EstablishedCasePayload>,
    }

    pub(crate) fn plan<'a>(
        decision: LoweredBooleanDecision,
        declared_cases: &impl Fn(PlaceId) -> Option<&'a [StructuralCaseDeclaration]>,
        qualifications: &terminal_psi::ScalarQualificationCatalog,
        namespace: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        outcome_reads_payloads: bool,
    ) -> Result<PlannedGuard, LoweringError> {
        let decision = plan_decision(
            decision,
            declared_cases,
            qualifications,
            namespace,
            next_value,
            outcome_reads_payloads,
        )?;
        let established = established_on_true(&decision, namespace).unwrap_or_default();
        Ok(PlannedGuard {
            decision,
            established,
        })
    }

    fn plan_decision<'a>(
        decision: LoweredBooleanDecision,
        declared_cases: &impl Fn(PlaceId) -> Option<&'a [StructuralCaseDeclaration]>,
        qualifications: &terminal_psi::ScalarQualificationCatalog,
        namespace: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        outcome_reads_payloads: bool,
    ) -> Result<LoweredBooleanDecision, LoweringError> {
        macro_rules! descend {
            ($decision:expr) => {
                plan_decision(
                    $decision,
                    declared_cases,
                    qualifications,
                    namespace,
                    next_value,
                    outcome_reads_payloads,
                )
            };
        }
        match decision {
            LoweredBooleanDecision::Value(expression) => {
                Ok(LoweredBooleanDecision::Value(expression))
            }
            LoweredBooleanDecision::Test {
                condition,
                when_true,
                when_false,
            } => {
                let LoweredBooleanReturnExpression::StructuralCaseMembership { source, path, case } =
                    &condition
                else {
                    return Ok(LoweredBooleanDecision::Test {
                        condition,
                        when_true: Box::new(descend!(*when_true)?),
                        when_false: Box::new(descend!(*when_false)?),
                    });
                };
                let declared = if path.is_empty()
                    && (decision_reads_case(&when_true, *source, *case)
                        || (outcome_reads_payloads && reaches_true_outcome(&when_true)))
                {
                    declared_cases(*source)
                } else {
                    None
                };
                let Some(declared) = declared else {
                    return Ok(LoweredBooleanDecision::Test {
                        when_true: Box::new(descend!(*when_true)?),
                        when_false: Box::new(descend!(*when_false)?),
                        condition,
                    });
                };
                let (source, selected) = (*source, *case);
                let selected_case = declared
                    .iter()
                    .find(|declared| declared.id == selected)
                    .ok_or(LoweringError::Unsupported(
                        "case dispatch selects a case outside its root's sum",
                    ))?;
                let mut payloads = Vec::new();
                for field in &selected_case.fields {
                    let scalar_type = match field.field_type {
                        StructuralFieldType::Scalar(scalar) => scalar,
                        StructuralFieldType::BoundedInteger(integer) => {
                            semantic_vocabulary::ScalarType::Integer(integer.integer_type())
                        }
                        StructuralFieldType::RefinedScalar(ref refinement) => refinement.carrier(),
                        _ => continue,
                    };
                    if field.relevance.is_erased() {
                        continue;
                    }
                    let payload = crate::shared::terminal_identities::fresh_field_snapshot(
                        next_value,
                        scalar_type,
                        field.id,
                        &qualifications.fields,
                    )?;
                    payloads.push((field.id, payload));
                }
                let bound = payloads
                    .iter()
                    .map(|(field, value)| {
                        namespace.push(*value);
                        EstablishedCasePayload {
                            source,
                            path: Vec::new(),
                            case: selected,
                            field: *field,
                            position: namespace.len() - 1,
                        }
                    })
                    .collect::<Vec<_>>();
                let when_true = substitute_decision(*when_true, &bound);
                Ok(LoweredBooleanDecision::CaseDispatch {
                    dispatch: CaseDispatch {
                        source,
                        selected,
                        cases: declared.iter().map(|declared| declared.id).collect(),
                        payloads,
                    },
                    when_true: Box::new(descend!(when_true)?),
                    when_false: Box::new(descend!(*when_false)?),
                })
            }
            LoweredBooleanDecision::CaseDispatch { .. } => {
                unsupported("guard case dispatch was planned twice")
            }
        }
    }

    fn established_on_true(
        decision: &LoweredBooleanDecision,
        namespace: &[ValueDeclaration],
    ) -> Option<Vec<EstablishedCasePayload>> {
        let intersect =
            |left: Option<Vec<EstablishedCasePayload>>,
             right: Option<Vec<EstablishedCasePayload>>| match (left, right) {
                (Some(left), Some(right)) => {
                    Some(left.into_iter().filter(|row| right.contains(row)).collect())
                }
                (left, None) => left,
                (None, right) => right,
            };
        match decision {
            LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value }) => {
                value.then(Vec::new)
            }
            LoweredBooleanDecision::Value(_) => Some(Vec::new()),
            LoweredBooleanDecision::Test {
                when_true,
                when_false,
                ..
            } => intersect(
                established_on_true(when_true, namespace),
                established_on_true(when_false, namespace),
            ),
            LoweredBooleanDecision::CaseDispatch {
                dispatch,
                when_true,
                when_false,
            } => {
                let selected = established_on_true(when_true, namespace).map(|mut rows| {
                    rows.extend(dispatch.payloads.iter().map(|(field, value)| {
                        EstablishedCasePayload {
                            source: dispatch.source,
                            path: Vec::new(),
                            case: dispatch.selected,
                            field: *field,
                            position: namespace
                                .iter()
                                .position(|candidate| candidate.id == value.id)
                                .expect("planned payload parameters join the guard namespace"),
                        }
                    }));
                    rows
                });
                intersect(selected, established_on_true(when_false, namespace))
            }
        }
    }

    fn substitute_decision(
        decision: LoweredBooleanDecision,
        bound: &[EstablishedCasePayload],
    ) -> LoweredBooleanDecision {
        if bound.is_empty() {
            return decision;
        }
        match decision {
            LoweredBooleanDecision::Value(expression) => {
                LoweredBooleanDecision::Value(substitute_boolean(expression, bound))
            }
            LoweredBooleanDecision::Test {
                condition,
                when_true,
                when_false,
            } => LoweredBooleanDecision::Test {
                condition: substitute_boolean(condition, bound),
                when_true: Box::new(substitute_decision(*when_true, bound)),
                when_false: Box::new(substitute_decision(*when_false, bound)),
            },
            LoweredBooleanDecision::CaseDispatch {
                dispatch,
                when_true,
                when_false,
            } => LoweredBooleanDecision::CaseDispatch {
                dispatch,
                when_true: Box::new(substitute_decision(*when_true, bound)),
                when_false: Box::new(substitute_decision(*when_false, bound)),
            },
        }
    }

    fn bound_position(
        bound: &[EstablishedCasePayload],
        source: &PrimitiveReadSource,
        path: &[CanonicalStructuralPathSegment],
        field: StructuralFieldId,
    ) -> Option<usize> {
        let PrimitiveReadSource::Place(source) = source else {
            return None;
        };
        let Some((CanonicalStructuralPathSegment::Case(case), prefix)) = path.split_last() else {
            return None;
        };
        if !prefix.is_empty() {
            return None;
        }
        bound
            .iter()
            .find(|row| {
                row.source == *source
                    && row.path.is_empty()
                    && row.case == *case
                    && row.field == field
            })
            .map(|row| row.position)
    }

    fn substitute_boolean(
        expression: LoweredBooleanReturnExpression,
        bound: &[EstablishedCasePayload],
    ) -> LoweredBooleanReturnExpression {
        use LoweredBooleanReturnExpression as Boolean;
        match expression {
            Boolean::ScalarObservation { operand } => Boolean::ScalarObservation {
                operand: Box::new(substitute_direct(*operand, bound)),
            },
            Boolean::StructuralField {
                qualifications,
                source,
                path,
                field,
            } => match bound_position(bound, &source, &path, field) {
                Some(position) => Boolean::Parameter { position },
                None => Boolean::StructuralField {
                    qualifications,
                    source,
                    path,
                    field,
                },
            },
            Boolean::Not { operand } => Boolean::Not {
                operand: Box::new(substitute_boolean(*operand, bound)),
            },
            Boolean::Equal { left, right } => Boolean::Equal {
                left: Box::new(substitute_boolean(*left, bound)),
                right: Box::new(substitute_boolean(*right, bound)),
            },
            Boolean::And { left, right } => Boolean::And {
                left: Box::new(substitute_boolean(*left, bound)),
                right: Box::new(substitute_boolean(*right, bound)),
            },
            Boolean::Or { left, right } => Boolean::Or {
                left: Box::new(substitute_boolean(*left, bound)),
                right: Box::new(substitute_boolean(*right, bound)),
            },
            Boolean::IntegerComparison { kind, left, right } => Boolean::IntegerComparison {
                kind,
                left: Box::new(substitute_direct(*left, bound)),
                right: Box::new(substitute_direct(*right, bound)),
            },
            Boolean::IndexedCaseMembership {
                source,
                observes_element_view_length,
                path,
                index,
                element_path,
                case,
            } => Boolean::IndexedCaseMembership {
                source,
                observes_element_view_length,
                path,
                index: Box::new(substitute_direct(*index, bound)),
                element_path,
                case,
            },
            Boolean::ByteSequencePredicate {
                predicate,
                source,
                path,
                field,
            } => Boolean::ByteSequencePredicate {
                predicate,
                source,
                path: substitute_path(path, bound),
                field,
            },
            Boolean::ByteSequenceEqual { left, right } => Boolean::ByteSequenceEqual {
                left: substitute_byte_sequence_operand(left, bound),
                right: substitute_byte_sequence_operand(right, bound),
            },
            expression @ (Boolean::StructuralCaseMembership { .. }
            | Boolean::PrimitiveRead { .. }
            | Boolean::Constant { .. }
            | Boolean::Parameter { .. }
            | Boolean::Local { .. }
            | Boolean::UnresolvedStructuralParameterField { .. }) => expression,
        }
    }

    fn substitute_byte_sequence_operand(
        operand: crate::emission::operation_emission::boolean::LoweredByteSequenceOperand,
        bound: &[EstablishedCasePayload],
    ) -> crate::emission::operation_emission::boolean::LoweredByteSequenceOperand {
        crate::emission::operation_emission::boolean::LoweredByteSequenceOperand {
            source: operand.source,
            path: substitute_path(operand.path, bound),
            field: operand.field,
        }
    }

    fn substitute_path(
        path: Vec<crate::emission::operation_emission::expressions::LoweredElementPathSegment>,
        bound: &[EstablishedCasePayload],
    ) -> Vec<crate::emission::operation_emission::expressions::LoweredElementPathSegment> {
        use crate::emission::operation_emission::expressions::LoweredElementPathSegment;
        path.into_iter()
            .map(|segment| match segment {
                LoweredElementPathSegment::RuntimeIndex {
                    selector,
                    extent,
                    bound_decided,
                } => LoweredElementPathSegment::RuntimeIndex {
                    selector: substitute_direct(selector, bound),
                    extent: extent.map(|extent| substitute_direct(extent, bound)),
                    bound_decided,
                },
                segment => segment,
            })
            .collect()
    }

    pub(crate) fn substitute_direct(
        expression: LoweredDirectExpression,
        bound: &[EstablishedCasePayload],
    ) -> LoweredDirectExpression {
        use LoweredDirectExpression as Direct;
        match expression {
            atomic @ Direct::AtomicLoad { .. } => atomic,
            Direct::StructuralField {
                qualifications,
                source,
                path,
                field,
                scalar_type,
            } => match bound_position(bound, &source, &path, field) {
                Some(position) => Direct::Parameter {
                    position,
                    scalar_type,
                },
                None => Direct::StructuralField {
                    qualifications,
                    source,
                    path,
                    field,
                    scalar_type,
                },
            },
            Direct::ElementViewRead {
                source,
                index,
                path,
                scalar_type,
                bound_decided,
            } => Direct::ElementViewRead {
                source,
                index: Box::new(substitute_direct(*index, bound)),
                path: substitute_path(path, bound),
                scalar_type,
                bound_decided,
            },
            Direct::IndexedPrimitiveRead {
                qualifications,
                source,
                path,
                index,
                extent,
                element_path,
                scalar_type,
                bound_decided,
            } => Direct::IndexedPrimitiveRead {
                qualifications,
                bound_decided,
                source,
                path,
                index: Box::new(substitute_direct(*index, bound)),
                extent: extent.map(|extent| Box::new(substitute_direct(*extent, bound))),
                element_path: substitute_path(element_path, bound),
                scalar_type,
            },
            Direct::ByteSequenceFieldRead {
                source,
                path,
                field,
                index,
                scalar_type,
                bound_decided,
            } => Direct::ByteSequenceFieldRead {
                source,
                path: substitute_path(path, bound),
                field,
                index: Box::new(substitute_direct(*index, bound)),
                scalar_type,
                bound_decided,
            },
            Direct::ByteSequenceFieldLength {
                source,
                path,
                field,
                scalar_type,
            } => Direct::ByteSequenceFieldLength {
                source,
                path: substitute_path(path, bound),
                field,
                scalar_type,
            },
            Direct::SaturatingShiftLeft {
                scalar_type,
                left,
                right,
            } => Direct::SaturatingShiftLeft {
                scalar_type,
                left: Box::new(substitute_direct(*left, bound)),
                right: Box::new(substitute_direct(*right, bound)),
            },
            Direct::IntegerBinary {
                kind,
                scalar_type,
                left,
                right,
                formation_decided,
            } => Direct::IntegerBinary {
                kind,
                scalar_type,
                left: Box::new(substitute_direct(*left, bound)),
                right: Box::new(substitute_direct(*right, bound)),
                formation_decided,
            },
            Direct::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => Direct::IntegerBitwiseNot {
                scalar_type,
                operand: Box::new(substitute_direct(*operand, bound)),
            },
            Direct::IntegerWiden {
                scalar_type,
                operand,
            } => Direct::IntegerWiden {
                scalar_type,
                operand: Box::new(substitute_direct(*operand, bound)),
            },
            Direct::IntegerExactCast {
                scalar_type,
                operand,
                formation_decided,
            } => Direct::IntegerExactCast {
                scalar_type,
                operand: Box::new(substitute_direct(*operand, bound)),
                formation_decided,
            },
            Direct::RepresentationView {
                scalar_type,
                operand,
            } => Direct::RepresentationView {
                scalar_type,
                operand: Box::new(substitute_direct(*operand, bound)),
            },
            Direct::Boolean { expression } => Direct::Boolean {
                expression: Box::new(substitute_boolean(*expression, bound)),
            },
            Direct::ByteSequenceLength {
                source,
                path,
                access,
                scalar_type,
            } => Direct::ByteSequenceLength {
                source,
                path: substitute_path(path, bound),
                access,
                scalar_type,
            },
            Direct::ElementViewLength {
                source,
                path,
                access,
                scalar_type,
            } => Direct::ElementViewLength {
                source,
                path: substitute_path(path, bound),
                access,
                scalar_type,
            },
            expression @ (Direct::PrimitiveRead { .. }
            | Direct::Parameter { .. }
            | Direct::ErasedParameter { .. }
            | Direct::Local { .. }
            | Direct::IntegerLiteral { .. }
            | Direct::ScalarApplication { .. }
            | Direct::IeeeFloatLiteral { .. }
            | Direct::ImplementationSelection { .. }) => expression,
        }
    }

    fn collect_source_case_reads(
        source: &PrimitiveReadSource,
        path: &[CanonicalStructuralPathSegment],
        reads: &mut Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        let origin = source.field_origin(path);
        if origin
            .1
            .iter()
            .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
        {
            reads.push(origin);
        }
    }

    fn case_reads(
        expression: &LoweredBooleanReturnExpression,
        reads: &mut Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        use LoweredBooleanReturnExpression as Boolean;
        match expression {
            Boolean::ScalarObservation { operand } => collect_direct_case_reads(operand, reads),
            Boolean::PrimitiveRead { source, .. } => collect_source_case_reads(source, &[], reads),
            Boolean::StructuralField { source, path, .. } => {
                collect_source_case_reads(source, path, reads);
            }
            Boolean::Not { operand } => case_reads(operand, reads),
            Boolean::Equal { left, right }
            | Boolean::And { left, right }
            | Boolean::Or { left, right } => {
                case_reads(left, reads);
                case_reads(right, reads);
            }
            Boolean::IntegerComparison { left, right, .. } => {
                collect_direct_case_reads(left, reads);
                collect_direct_case_reads(right, reads);
            }
            Boolean::IndexedCaseMembership { index, .. } => collect_direct_case_reads(index, reads),
            Boolean::ByteSequencePredicate { path, .. } => {
                for segment in path {
                    if let crate::emission::operation_emission::expressions::LoweredElementPathSegment::RuntimeIndex { selector, extent, .. } =
                        segment
                    {
                        collect_direct_case_reads(selector, reads);
                        if let Some(extent) = extent {
                            collect_direct_case_reads(extent, reads);
                        }
                    }
                }
            }
            Boolean::ByteSequenceEqual { left, right } => {
                for operand in [left, right] {
                    for segment in &operand.path {
                        if let crate::emission::operation_emission::expressions::LoweredElementPathSegment::RuntimeIndex { selector, extent, .. } =
                            segment
                        {
                            collect_direct_case_reads(selector, reads);
                            if let Some(extent) = extent {
                                collect_direct_case_reads(extent, reads);
                            }
                        }
                    }
                }
            }
            Boolean::StructuralCaseMembership { .. }
            | Boolean::Constant { .. }
            | Boolean::Parameter { .. }
            | Boolean::Local { .. }
            | Boolean::UnresolvedStructuralParameterField { .. } => {}
        }
    }

    fn collect_path_case_reads(
        path: &[crate::emission::operation_emission::expressions::LoweredElementPathSegment],
        reads: &mut Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        for segment in path {
            if let crate::emission::operation_emission::expressions::LoweredElementPathSegment::RuntimeIndex { selector, extent, .. } = segment {
                collect_direct_case_reads(selector, reads);
                if let Some(extent) = extent { collect_direct_case_reads(extent, reads); }
            }
        }
    }

    fn collect_direct_case_reads(
        expression: &LoweredDirectExpression,
        reads: &mut Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        use LoweredDirectExpression as Direct;
        match expression {
            Direct::AtomicLoad { .. } => {}
            Direct::PrimitiveRead { source, .. } => collect_source_case_reads(source, &[], reads),
            Direct::StructuralField { source, path, .. } => {
                collect_source_case_reads(source, path, reads);
            }
            Direct::ElementViewRead { index, path, .. } => {
                collect_direct_case_reads(index, reads);
                collect_path_case_reads(path, reads);
            }
            Direct::IndexedPrimitiveRead {
                index,
                extent,
                element_path,
                ..
            } => {
                if let Some(extent) = extent {
                    collect_direct_case_reads(extent, reads);
                }
                collect_direct_case_reads(index, reads);
                collect_path_case_reads(element_path, reads);
            }
            Direct::ByteSequenceFieldRead { path, index, .. } => {
                collect_path_case_reads(path, reads);
                collect_direct_case_reads(index, reads);
            }
            Direct::ByteSequenceLength { path, .. }
            | Direct::ByteSequenceFieldLength { path, .. }
            | Direct::ElementViewLength { path, .. } => collect_path_case_reads(path, reads),
            Direct::IntegerBinary { left, right, .. }
            | Direct::SaturatingShiftLeft { left, right, .. } => {
                collect_direct_case_reads(left, reads);
                collect_direct_case_reads(right, reads);
            }
            Direct::IntegerBitwiseNot { operand, .. }
            | Direct::IntegerWiden { operand, .. }
            | Direct::IntegerExactCast { operand, .. }
            | Direct::RepresentationView { operand, .. } => {
                collect_direct_case_reads(operand, reads)
            }
            Direct::Boolean { expression } => case_reads(expression, reads),
            Direct::Parameter { .. }
            | Direct::ErasedParameter { .. }
            | Direct::Local { .. }
            | Direct::IntegerLiteral { .. }
            | Direct::IeeeFloatLiteral { .. }
            | Direct::ImplementationSelection { .. }
            | Direct::ScalarApplication { .. } => {}
        }
    }

    fn reaches_true_outcome(decision: &LoweredBooleanDecision) -> bool {
        match decision {
            LoweredBooleanDecision::Value(expression) => !matches!(
                expression,
                LoweredBooleanReturnExpression::Constant { value: false }
            ),
            LoweredBooleanDecision::Test {
                when_true,
                when_false,
                ..
            }
            | LoweredBooleanDecision::CaseDispatch {
                when_true,
                when_false,
                ..
            } => reaches_true_outcome(when_true) || reaches_true_outcome(when_false),
        }
    }

    fn decision_reads_case(
        decision: &LoweredBooleanDecision,
        source: PlaceId,
        case: StructuralCaseId,
    ) -> bool {
        let reads_case = |expression: &LoweredBooleanReturnExpression| {
            let mut reads = Vec::new();
            case_reads(expression, &mut reads);
            reads.iter().any(|(root, path)| {
                *root == source && path.first() == Some(&CanonicalStructuralPathSegment::Case(case))
            })
        };
        match decision {
            LoweredBooleanDecision::Value(expression) => reads_case(expression),
            LoweredBooleanDecision::Test {
                condition,
                when_true,
                when_false,
            } => {
                reads_case(condition)
                    || decision_reads_case(when_true, source, case)
                    || decision_reads_case(when_false, source, case)
            }
            LoweredBooleanDecision::CaseDispatch {
                when_true,
                when_false,
                ..
            } => {
                decision_reads_case(when_true, source, case)
                    || decision_reads_case(when_false, source, case)
            }
        }
    }
}
pub(crate) mod selected_comparison {

    use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;
    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use checked_trees::CheckedTrees;
    use checked_trees::types::PrimitiveType;
    use lowered_psi::LoweredSelectedComparisonOperandOrder;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct SelectedComparison {
        pub operator_use: checked_trees::CheckedOperatorUseHandle,
        pub application_site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
        pub requirement_operator: symbols::SymbolHandle,
        pub meaning: SelectedComparisonMeaning,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum SelectedComparisonMeaning {
        IeeeFloat {
            comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
            format: semantic_vocabulary::IeeeFloatFormat,
        },
        Integer {
            comparison: LoweredIntegerComparisonKind,
            operand_order: LoweredSelectedComparisonOperandOrder,
            negated: bool,
            integer_type: semantic_vocabulary::IntegerType,
        },
    }

    impl SelectedComparison {
        pub(crate) const fn operand_type(&self) -> semantic_vocabulary::ScalarType {
            match self.meaning {
                SelectedComparisonMeaning::IeeeFloat { format, .. } => {
                    semantic_vocabulary::ScalarType::IeeeFloat(format)
                }
                SelectedComparisonMeaning::Integer { integer_type, .. } => {
                    semantic_vocabulary::ScalarType::Integer(integer_type)
                }
            }
        }
    }

    pub(crate) fn occurrence(
        checked: &CheckedTrees,
        handle: checked_trees::CheckedOperatorUseHandle,
    ) -> Result<SelectedComparison, LoweringError> {
        let meaning = selected_meaning(checked, handle)?;
        let selected = checked.facts.operators.uses.get(handle);
        Ok(SelectedComparison {
            operator_use: handle,
            application_site: selected.application_site(),
            requirement_operator: selected.selected_operator_symbol,
            meaning,
        })
    }

    fn selected_meaning(
        checked: &CheckedTrees,
        handle: checked_trees::CheckedOperatorUseHandle,
    ) -> Result<SelectedComparisonMeaning, LoweringError> {
        let operators = &checked.facts.operators;
        if let Some((comparison, primitive)) =
            operators.selected_float_comparison(&checked.typed, handle)
        {
            let format = match primitive {
                PrimitiveType::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
                _ => return unsupported("selected comparison has a non-float operand"),
            };
            return Ok(SelectedComparisonMeaning::IeeeFloat { comparison, format });
        }
        let Some((operation, primitive)) =
            operators.selected_integer_comparison(&checked.typed, handle)
        else {
            return unsupported("comparison has no exact selected Terminal meaning");
        };
        let Some((comparison, operand_order, negated)) = admitted_integer_emission(operation)
        else {
            return unsupported(
                "selected integer comparison has no admitted Terminal operation and operand mapping to join",
            );
        };
        let Some(integer_type) = terminal_scalar_type(primitive)?.as_integer() else {
            return unsupported("selected integer comparison has a non-integer operand");
        };
        Ok(SelectedComparisonMeaning::Integer {
            comparison,
            operand_order,
            negated,
            integer_type,
        })
    }

    fn admitted_integer_emission(
        operator: checked_trees::expression::BinaryOperator,
    ) -> Option<(
        LoweredIntegerComparisonKind,
        LoweredSelectedComparisonOperandOrder,
        bool,
    )> {
        use LoweredSelectedComparisonOperandOrder::{Authored, Swapped};
        use checked_trees::expression::BinaryOperator;
        match operator {
            BinaryOperator::Equal => Some((LoweredIntegerComparisonKind::Equal, Authored, false)),
            BinaryOperator::NotEqual => Some((LoweredIntegerComparisonKind::Equal, Authored, true)),
            BinaryOperator::Less => Some((LoweredIntegerComparisonKind::LessThan, Authored, false)),
            BinaryOperator::LessOrEqual => {
                Some((LoweredIntegerComparisonKind::LessOrEqual, Authored, false))
            }
            BinaryOperator::Greater => {
                Some((LoweredIntegerComparisonKind::LessThan, Swapped, false))
            }
            BinaryOperator::GreaterOrEqual => {
                Some((LoweredIntegerComparisonKind::LessOrEqual, Swapped, false))
            }
            _ => None,
        }
    }
}

pub(crate) fn next_byte_sequence_literal_ordinal(
    places: &[StructuralPlaceDeclaration],
) -> Result<u32, LoweringError> {
    let literals = places
        .iter()
        .filter(|place| matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. }))
        .count();
    u32::try_from(literals)
        .map_err(|_| LoweringError::Unsupported("literal declaration ordinal exceeds u32"))
}

pub(crate) fn emit_u64_coordinate(
    operand: ValueId,
    source: ScalarType,
    bound_decided: bool,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut operation_emission::buffer::OperationBuffer,
) -> Result<ValueId, LoweringError> {
    let Some(source_type) = source.as_integer() else {
        return unsupported("byte index requires an integer carrier");
    };
    let coordinate_type = terminal_scalar_type(PrimitiveType::U64)?;
    if source == coordinate_type {
        return Ok(operand);
    }
    let Some(target_type) = coordinate_type.as_integer() else {
        return unsupported("byte coordinate requires an integer carrier");
    };
    let kind = if source_type.can_widen_to(target_type) {
        OperationKind::IntegerWiden { operand }
    } else {
        let obligation = obligation_id(allocate_dense(next_obligation)?);
        if bound_decided {
            operations.source_checked_obligations.push(obligation);
        }
        OperationKind::IntegerExactCast {
            operand,
            obligation,
        }
    };
    let coordinate = value_id(allocate_dense(next_value)?);
    operations.push_leaf(scalar_result_declaration(coordinate, coordinate_type), kind);
    Ok(coordinate)
}

pub(crate) mod quotient_values {
    use super::StructuralPlaceDeclaration;
    use crate::emission::operation_emission::buffer::{
        OperationBuffer, SourceCallCoordinate, scalar_result_declaration,
        structural_result_declaration,
    };
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::emission::operation_emission::expressions::emit_scalar_leaf;
    use crate::emission::operation_emission::place_declarations::operation_result_place;
    use crate::shared::direct_calls::{CallRecord, DirectCall, DirectCallResult, emit_direct_call};
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use crate::shared::terminal_identities::value_id;
    use crate::unit::attached_unit::ordinary_calls::prepare_requirement_obligations;
    use crate::unit::attached_unit::parameters::authored_result_qualification_domains;
    use crate::unit::attached_unit::primitive_locals::completed_scalar;
    use crate::unit::attached_unit::signatures::find;
    use checked_trees::{CheckedStructuralValueHandle, CheckedStructuralValueKind, CheckedTrees};
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::{StructuralArgument, StructuralMultiplicity};

    pub(crate) fn emit_call(
        checked: &CheckedTrees,
        value: CheckedStructuralValueHandle,
        actuals: &[(StructuralArgument, StructuralTypeId)],
        place: PlaceId,
        structural_type: StructuralTypeId,
        next_value: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<StructuralPlaceDeclaration, LoweringError> {
        let node = checked.facts.values.structural_values.nodes.get(value);
        let CheckedStructuralValueKind::QuotientCall {
            request,
            representative_state,
            arguments,
        } = &node.kind
        else {
            return unsupported("quotient invocation has no checked call node");
        };
        let request = *request;
        let representative_state = *representative_state;
        let mut approvals = checked.facts.quotient_operations.iter().filter(|row| {
            row.request == request
                && row.public_machine == node.machine
                && row.public_state == node.state
                && row.representative_state == representative_state
        });
        if approvals.next().is_none() || approvals.next().is_some() {
            return unsupported("quotient invocation lost its unique checked correspondence");
        }
        let (representative, state) =
            validation::find_machine_by_entry_state(&checked.typed, representative_state).ok_or(
                LoweringError::Unsupported("quotient representative body is absent"),
            )?;
        let signature = find(calls.signatures, representative.symbol)?.clone();
        let crash_routes = crate::unit::effective_crash_routes(checked, representative.symbol)?;
        if !signature.erased_scalar_parameters.is_empty()
            || !signature.erased_proof_parameters.is_empty()
            || !crash_routes.is_empty()
            || signature.parameters.len() + signature.scalar_parameters.len() != arguments.len()
            || checked.typed.state_parameters(state).len() != actuals.len()
        {
            return unsupported(
                "quotient invocation requires an unimplemented operand or contract lane",
            );
        }
        if checked.typed.type_multiplicity(state.return_type)
            != language_semantics::Multiplicity::Unrestricted
            || !validation::view_result_carrier_qualifications(&checked.typed, state.return_type)
                .map_err(LoweringError::Unsupported)?
                .is_empty()
            || !authored_result_qualification_domains(checked, representative.symbol).is_empty()
            || !validation::structural_result_projected_qualifications(
                &checked.typed,
                state.return_type,
            )
            .map_err(LoweringError::Unsupported)?
            .is_empty()
        {
            return unsupported("quotient invocation result carries custody or qualifications");
        }
        let mut arguments = Vec::new();
        let mut scalar_arguments = Vec::new();
        let mut structural_formals = signature.parameters.iter();
        let mut scalar_formals = signature.scalar_parameters.iter();
        for ((actual, ty), source_formal) in
            actuals.iter().zip(checked.typed.state_parameters(state))
        {
            if !actual.path.is_empty() {
                return unsupported("quotient invocation input is not a whole checked value");
            }
            if let Some(primitive) = checked
                .typed
                .primitive_type_reference(source_formal.type_reference)
            {
                let scalar_type = crate::emission::scalar_types::terminal_scalar_type(primitive)?;
                let formal = scalar_formals.next().ok_or(LoweringError::Unsupported(
                    "quotient scalar formal is absent",
                ))?;
                if formal.scalar_type != scalar_type
                    || !formal.qualifications.is_empty()
                    || !calls.structural_types.iter().any(|declaration| {
                        declaration.id == *ty
                            && declaration.shape
                                == terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type)
                    })
                {
                    return unsupported("quotient scalar input has a different checked carrier");
                }
                scalar_arguments.push(emit_scalar_leaf(
                    terminal_psi::OperationKind::PrimitiveScalarRead {
                        source: actual.place,
                        path: Vec::new(),
                    },
                    scalar_type,
                    next_value,
                    operations,
                ));
            } else {
                let formal = structural_formals.next().ok_or(LoweringError::Unsupported(
                    "quotient structural formal is absent",
                ))?;
                if *ty != formal.structural_type
                    || formal.multiplicity != StructuralMultiplicity::Unrestricted
                    || !formal.qualifications.is_empty()
                    || !formal.projected_qualifications.is_empty()
                {
                    return unsupported(
                        "quotient invocation input is not an exact custody-free carrier",
                    );
                }
                arguments.push(StructuralArgument {
                    access: formal.access,
                    ..actual.clone()
                });
            }
        }
        if structural_formals.next().is_some() || scalar_formals.next().is_some() {
            return unsupported("quotient invocation argument lanes are incomplete");
        }
        let requirement_obligations =
            prepare_requirement_obligations(&signature.call_target(), calls, operations)?;
        let callee = crate::shared::terminal_identities::lookup_machine_id(
            calls.machine_ids,
            representative.symbol,
        )?;
        let (result, mode, scalar) = if let Some(scalar) = signature.scalar_result {
            if !scalar.qualifications.is_empty() {
                return unsupported("quotient scalar result has unimplemented qualifications");
            }
            let id = value_id(crate::shared::terminal_identities::allocate_dense(
                next_value,
            )?);
            (
                scalar_result_declaration(id, scalar.scalar_type),
                DirectCallResult::Scalar,
                Some(id),
            )
        } else {
            (
                structural_result_declaration(
                    place,
                    structural_type,
                    StructuralMultiplicity::Unrestricted,
                ),
                DirectCallResult::Structural {
                    multiplicity: StructuralMultiplicity::Unrestricted,
                },
                None,
            )
        };
        let call = emit_direct_call(
            operations,
            SourceCallCoordinate {
                state: node.state,
                statement_index: node.statement_index as usize,
                call_ordinal: 0,
            },
            None,
            representative_state,
            CallRecord::Quotient(value),
            result,
            DirectCall::new(
                callee,
                scalar_arguments,
                Vec::new(),
                Vec::new(),
                arguments,
                Vec::new(),
                requirement_obligations,
                Vec::new(),
                mode,
            ),
        )?;
        if let Some(scalar) = scalar {
            let scalar = completed_scalar(scalar, &[], operations)?;
            crate::unit::attached_unit::primitive_locals::establish_at(
                place,
                operations,
                scalar,
                structural_type,
                crate::unit::attached_unit::primitive_locals::PrimitiveStorageCatalog {
                    types: calls.structural_types,
                    domains: calls.domain_declarations,
                    scalars: calls.scalar_qualifications,
                },
                None,
            )
        } else {
            Ok(operation_result_place(place, call, structural_type))
        }
    }

    pub(crate) fn emit_adaptation(
        value: CheckedStructuralValueHandle,
        source: PlaceId,
        source_type: StructuralTypeId,
        place: PlaceId,
        structural_type: StructuralTypeId,
        operations: &mut OperationBuffer,
    ) -> StructuralPlaceDeclaration {
        let copy = operations.allocate();
        operations.push(terminal_psi::Operation::unsuspended(
            copy,
            structural_result_declaration(
                place,
                structural_type,
                StructuralMultiplicity::Unrestricted,
            ),
            terminal_psi::OperationKind::StructuralLeafCopy {
                source,
                path: Vec::new(),
            },
        ));
        operations.quotient_adaptations.push(
            crate::emission::operation_emission::buffer::SourceQuotientAdaptation {
                value,
                copy,
                source,
                result: place,
                source_type,
                result_type: structural_type,
            },
        );
        operation_result_place(place, copy, structural_type)
    }
}

pub(crate) fn value_successor(edge: EdgeId, target: BlockId) -> SuccessorEdge {
    SuccessorEdge {
        edge,
        target,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    }
}

pub(crate) mod reference_backings {

    use semantic_vocabulary::PlaceId;
    use terminal_psi::StructuralArgument;

    pub(crate) struct ReferenceBackings {
        first_place: u64,
        sources: Vec<Option<ReferenceBacking>>,
        valid_frame: bool,
    }

    #[derive(Clone)]
    enum ReferenceBacking {
        Source(StructuralArgument),
        Unavailable,
    }

    impl ReferenceBackings {
        pub(crate) fn new(first_place: u64) -> Self {
            Self {
                first_place,
                sources: Vec::new(),
                valid_frame: true,
            }
        }

        fn index(&self, place: PlaceId) -> Option<usize> {
            place
                .get()
                .checked_sub(self.first_place)
                .and_then(|index| usize::try_from(index).ok())
        }

        pub(crate) fn record(&mut self, place: PlaceId, source: StructuralArgument) {
            self.merge(place, ReferenceBacking::Source(source));
        }

        pub(crate) fn source(&self, place: PlaceId) -> Option<&StructuralArgument> {
            if !self.valid_frame {
                return None;
            }
            match self
                .sources
                .get(self.index(place)?)
                .and_then(Option::as_ref)?
            {
                ReferenceBacking::Source(source) => Some(source),
                ReferenceBacking::Unavailable => None,
            }
        }

        pub(crate) fn forward(&mut self, argument: &StructuralArgument, destination: PlaceId) {
            if argument.path.is_empty() && argument.place == destination {
                return;
            }
            let source = if argument.path.is_empty() {
                argument
                    .place
                    .get()
                    .checked_sub(self.first_place)
                    .and_then(|index| usize::try_from(index).ok())
                    .and_then(|index| self.sources.get(index))
                    .and_then(Option::as_ref)
                    .cloned()
                    .unwrap_or(ReferenceBacking::Unavailable)
            } else {
                ReferenceBacking::Unavailable
            };
            self.merge(destination, source);
        }

        fn merge(&mut self, place: PlaceId, source: ReferenceBacking) {
            let Some(index) = self.index(place) else {
                self.valid_frame = false;
                return;
            };
            if self.sources.len() <= index {
                self.sources.resize_with(index + 1, || None);
            }
            let slot = &mut self.sources[index];
            match (slot.as_ref(), &source) {
                (None, _) => *slot = Some(source),
                (Some(ReferenceBacking::Source(left)), ReferenceBacking::Source(right))
                    if left == right => {}
                _ => *slot = Some(ReferenceBacking::Unavailable),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use terminal_psi::StructuralAccess;

        fn argument(place: u64) -> StructuralArgument {
            StructuralArgument {
                place: PlaceId::new(place).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }
        }

        #[test]
        fn owned_descriptor_join_keeps_its_original_backing() {
            let mut backings = ReferenceBackings::new(10);
            let source = argument(2);
            backings.record(argument(10).place, source.clone());
            backings.forward(&argument(10), argument(11).place);
            assert_eq!(backings.source(argument(11).place), Some(&source));
        }

        #[test]
        fn an_out_of_frame_producer_cannot_supply_another_local_value() {
            let mut backings = ReferenceBackings::new(10);
            backings.record(argument(5).place, argument(2));
            backings.record(argument(10).place, argument(3));
            assert!(backings.source(argument(5).place).is_none());
            assert!(backings.source(argument(10).place).is_none());
        }

        #[test]
        fn one_known_arm_cannot_supply_an_unresolved_or_different_arm() {
            for unknown in [true, false] {
                for reverse in [true, false] {
                    let mut backings = ReferenceBackings::new(10);
                    backings.record(argument(10).place, argument(2));
                    if !unknown {
                        backings.record(argument(11).place, argument(3));
                    }
                    let arms = if reverse { [11, 10] } else { [10, 11] };
                    for arm in arms {
                        backings.forward(&argument(arm), argument(12).place);
                    }
                    assert!(backings.source(argument(12).place).is_none());
                }
            }
        }
    }
}
pub(crate) mod reference_formations {
    use crate::shared::lowering_error::LoweringError;
    use checked_trees::CheckedTrees;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone)]
    pub(crate) struct ReferenceFormations {
        rows: Vec<SourceReferenceRequirement>,
        loans: Vec<(
            (usize, u32, u32),
            arena::Handle<checked_trees::BorrowLoanFact>,
        )>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SourceReferenceRequirement {
        statement_index: usize,
        expression: checked_trees::expression::ExpressionHandle,
        requirements: Vec<checked_trees::CheckedSourceQualification>,
    }

    impl ReferenceFormations {
        pub(crate) fn prepare<'a>(
            checked: &CheckedTrees,
            state: SymbolHandle,
            operations: impl Iterator<Item = &'a checked_trees::CheckedUnitEffectOperationPlan>,
            edges: impl Iterator<Item = &'a checked_trees::CheckedStructuralControlSuccessorPlan>,
        ) -> Result<Self, LoweringError> {
            let mut rows = Vec::new();
            let mut record =
                |statement_index: u32,
                 expression,
                 contract: &checked_trees::CheckedReferenceSourceContract| {
                    if let checked_trees::CheckedReferenceSourceContract::Standing {
                        source_requirements,
                        ..
                    } = contract
                    {
                        rows.push(SourceReferenceRequirement {
                            statement_index: statement_index as usize,
                            expression,
                            requirements: source_requirements.clone(),
                        });
                    }
                };
            for operation in operations {
                if let checked_trees::CheckedUnitEffectOperationPlan::EstablishReference {
                    formation_expression,
                    source_contract,
                    result,
                    ..
                } = operation
                {
                    record(
                        result.statement_index,
                        *formation_expression,
                        source_contract,
                    );
                }
            }
            for edge in edges {
                for transfer in &edge.transfers {
                    if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                        formation_expression,
                        source_contract,
                        ..
                    } = &transfer.source
                    {
                        record(
                            edge.statement_ordinal,
                            *formation_expression,
                            source_contract,
                        );
                    }
                }
            }
            let values = &checked.facts.values.structural_values;
            values.with_state_nodes(state, |handles| {
                for handle in handles {
                    let value = values.nodes.get(*handle);
                    if let checked_trees::CheckedStructuralValueKind::Reference {
                        source_contract,
                        ..
                    } = &value.kind
                    {
                        record(value.statement_index, value.expression, source_contract);
                    }
                }
            });
            rows.sort_by_key(|row| {
                (
                    row.statement_index,
                    row.expression.arena_index(),
                    row.expression.generation(),
                )
            });
            for pair in rows.windows(2) {
                if (pair[0].statement_index, pair[0].expression)
                    == (pair[1].statement_index, pair[1].expression)
                    && pair[0].requirements != pair[1].requirements
                {
                    let (machine, _) =
                        crate::shared::declarations::state_declaration(checked, state)?;
                    return Err(LoweringError::at_statement(
                        checked,
                        machine.symbol,
                        state,
                        pair[0].statement_index as u32,
                        "reference occurrence has conflicting source requirements",
                    ));
                }
            }
            rows.dedup();
            let (machine, _) = crate::shared::declarations::state_declaration(checked, state)?;
            let borrow = &checked.facts.borrow;
            let mut loans = borrow
                .unique_state_borrow(machine.symbol, state)
                .into_iter()
                .flat_map(|state| borrow.state_loans(state))
                .filter(|(_, loan)| loan.formation_expression.is_valid())
                .map(|(handle, loan)| {
                    (
                        formation_key(loan.statement_index, loan.formation_expression),
                        handle,
                    )
                })
                .collect::<Vec<_>>();
            loans.sort_by_key(|(key, _)| *key);
            Ok(Self { rows, loans })
        }

        pub(crate) fn find(
            &self,
            statement: usize,
            expression: checked_trees::expression::ExpressionHandle,
        ) -> Option<usize> {
            self.rows
                .binary_search_by_key(
                    &(statement, expression.arena_index(), expression.generation()),
                    |row| {
                        (
                            row.statement_index,
                            row.expression.arena_index(),
                            row.expression.generation(),
                        )
                    },
                )
                .ok()
        }
        pub(crate) fn requirements(
            &self,
            index: usize,
        ) -> &[checked_trees::CheckedSourceQualification] {
            &self.rows[index].requirements
        }

        pub(crate) fn loans_for(
            &self,
            statement: usize,
            expression: checked_trees::expression::ExpressionHandle,
        ) -> impl Iterator<Item = arena::Handle<checked_trees::BorrowLoanFact>> + '_ {
            let key = formation_key(statement, expression);
            let start = self
                .loans
                .partition_point(|(candidate, _)| *candidate < key);
            let end = self
                .loans
                .partition_point(|(candidate, _)| *candidate <= key);
            self.loans[start..end].iter().map(|(_, loan)| *loan)
        }
    }

    fn formation_key(
        statement: usize,
        expression: checked_trees::expression::ExpressionHandle,
    ) -> (usize, u32, u32) {
        (statement, expression.arena_index(), expression.generation())
    }
}
