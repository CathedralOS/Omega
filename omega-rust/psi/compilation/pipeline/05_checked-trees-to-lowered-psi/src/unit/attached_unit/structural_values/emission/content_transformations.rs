//! Lower the checked completion transaction, independently of payload shape.
//! Raw children retain their original claims and memberships. This operation
//! alone consumes the live parent accounts and publishes the derived result;
//! a later local binding or return never backdates authority to construction.

mod boundary_correspondence;

use super::{Emission, LoweringError, StructuralArgument, unsupported};
use semantic_vocabulary::{ContentPlaceSegment, ContentPlaceVersion, ContentStructuralPlace};
use terminal_psi::{
    ContentClaimProjection, ContentTransformationDerivation, StructuralPathQualification,
};

fn source_path(
    checked: &checked_trees::CheckedTrees,
    segments: &[facts::PlaceSegment],
) -> Result<Vec<ContentPlaceSegment>, LoweringError> {
    use checked_trees::data::DataMember;
    segments
        .iter()
        .map(|segment| match segment {
            facts::PlaceSegment::FixedIndex { index } => Ok(ContentPlaceSegment::FixedIndex(
                u64::try_from(*index)
                    .map_err(|_| LoweringError::Unsupported("content path index overflows"))?,
            )),
            facts::PlaceSegment::Field { symbol } => {
                for definition in checked.data_definitions() {
                    for member in checked.data_members(definition) {
                        match member {
                            DataMember::Field(field) if field.symbol == *symbol => {
                                return Ok(ContentPlaceSegment::Field(field.path_identity()));
                            }
                            DataMember::Variant(variant) => {
                                if let Some(field) = checked
                                    .data_payload_fields(variant)
                                    .iter()
                                    .find(|field| field.symbol == *symbol)
                                {
                                    return Ok(ContentPlaceSegment::Field(field.path_identity()));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                unsupported("content field has no declared member identity")
            }
            facts::PlaceSegment::Case { variant: symbol } => {
                for definition in checked.data_definitions() {
                    for member in checked.data_members(definition) {
                        if let DataMember::Variant(variant) = member
                            && variant.symbol == *symbol
                        {
                            return Ok(ContentPlaceSegment::Case(variant.path_identity()));
                        }
                    }
                }
                unsupported("content case has no declared member identity")
            }
            _ => unsupported("content transformation requires an exact finite owned path"),
        })
        .collect()
}

impl Emission<'_, '_, '_> {
    pub(super) fn content_transformation(
        &self,
        node: &checked_trees::CheckedStructuralValue,
    ) -> Result<Option<checked_trees::facts::ContentBodyTransformation>, LoweringError> {
        let mut matching = self
            .checked
            .facts
            .qualifications
            .content
            .body_transformations
            .iter()
            .filter_map(|(_, transaction)| {
                let occurrence = self
                    .checked
                    .facts
                    .flow
                    .control
                    .operands
                    .get(transaction.completed_value);
                (transaction.machine_symbol == node.machine
                    && transaction.state_symbol == node.state
                    && occurrence.statement_index == node.statement_index as usize
                    && occurrence.expression == node.expression)
                    .then_some(transaction)
            });
        let selected = matching.next().cloned();
        if matching.next().is_some() {
            return unsupported("content completion has ambiguous evaluation transactions");
        }
        Ok(selected)
    }

    pub(super) fn transform_content(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: StructuralArgument,
        transaction: checked_trees::facts::ContentBodyTransformation,
    ) -> Result<StructuralArgument, LoweringError> {
        use semantic_vocabulary::StructuralPlaceKind;
        use terminal_psi::{
            Operation, OperationKind, OperationResult, StructuralAccess, StructuralOperationResult,
            StructuralPlaceDeclaration,
        };
        let content = &self.checked.facts.qualifications.content;
        let ownership = &self.checked.facts.flow.ownership;
        let place = super::place_id(super::allocate_dense(self.environment.next_place)?);
        let operation = self.operations.allocate();
        if source.access != StructuralAccess::Owned || !source.path.is_empty() {
            return unsupported("content formation requires its complete owned payload");
        }
        let claims =
            self.evaluation
                .current_claims
                .source(&source)?
                .ok_or(LoweringError::Unsupported(
                    "content payload has no completed custody",
                ))?;
        let (whole, projected) =
            self.completed_memberships(source.place)
                .ok_or(LoweringError::Unsupported(
                    "content payload lacks its completed membership roster",
                ))?;
        let mut qualifications = whole.to_vec();
        let mut projected_qualifications = projected.to_vec();
        let mut parents = Vec::new();
        let mut derivations = Vec::new();
        let (_, authored) = crate::declarations::state_declaration(self.checked, self.state)?;
        let source_parameters = self.checked.state_parameters(authored);
        for derived in content
            .body_derivations
            .span_or_empty(transaction.derivations)
        {
            let equation = content.body_conservations.get(derived.conservation);
            let mut inputs = Vec::new();
            let mut outputs = Vec::new();
            let mut algebra = None;
            let mut input_bindings = Vec::new();
            let mut output_bindings = Vec::new();
            for available_handle in equation.available_inputs.handles() {
                let available = content.body_available_inputs.get(available_handle);
                let binding = content.body_claim_bindings.get(available.binding);
                let (mut argument, carrier) =
                    crate::expression_preparation::bindings::owned_structural_home(
                        available.root,
                        source_parameters,
                        &self.evaluation.structural_parameters,
                        &self.evaluation.structural_locals,
                    )?;
                let root_carrier =
                    crate::expression_preparation::bindings::published_structural_type(
                        &self.evaluation.structural_parameters,
                        &self.evaluation.structural_locals,
                        self.environment.private_places,
                        argument.place,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "content alias root carrier is absent",
                    ))?;
                let mut segments = terminal_semantics::content_transformations::content_path(
                    self.structural_types.iter(),
                    &identities.snapshot(),
                    root_carrier,
                    &argument.path,
                )
                .ok_or(LoweringError::Unsupported(
                    "content alias requires an exact typed owned path",
                ))?;
                let relative = source_path(
                    self.checked,
                    ownership.segments.span_or_empty(available.segments),
                )?;
                let relative_path = terminal_semantics::content_transformations::structural_path(
                    self.structural_types.iter(),
                    &identities.snapshot(),
                    carrier,
                    &relative,
                )
                .ok_or(LoweringError::Unsupported(
                    "content parent path has no typed carrier",
                ))?;
                argument.path.extend(relative_path);
                segments.extend(relative);
                if !parents.contains(&argument) {
                    parents.push(argument.clone());
                }
                let projection = self.content_projection(binding.semantic_domain)?;
                algebra = Some(projection.algebra.clone());
                let input = ContentClaimProjection {
                    claim: crate::terminal_identities::lookup_claim_id(
                        self.calls.claims.bindings(),
                        binding.claim_identity,
                    )?,
                    projection: projection.identity,
                    subject: ContentStructuralPlace {
                        version: ContentPlaceVersion::Current,
                        root: argument.place,
                        segments,
                    },
                };
                input_bindings.push((available_handle, input.clone()));
                inputs.push(input);
            }
            for binding_handle in equation.outputs.handles() {
                let binding = content.body_claim_bindings.get(binding_handle);
                let segments = source_path(
                    self.checked,
                    ownership.segments.span_or_empty(binding.segments),
                )?;
                let path = terminal_semantics::content_transformations::structural_path(
                    self.structural_types.iter(),
                    &identities.snapshot(),
                    self.structural_type,
                    &segments,
                )
                .ok_or(LoweringError::Unsupported(
                    "content result path has no typed carrier",
                ))?;
                let projection = self.content_projection(binding.semantic_domain)?;
                let domain = self
                    .domain_declarations
                    .iter()
                    .find(|domain| domain.content_projection.as_ref() == Some(projection))
                    .ok_or(LoweringError::Unsupported(
                        "derived membership has no exact domain",
                    ))?;
                if matches!(
                    derived.custody,
                    checked_trees::facts::ContentBodyCustody::Derived
                ) {
                    if path.is_empty() {
                        qualifications.push(domain.id);
                    } else {
                        projected_qualifications.push(StructuralPathQualification {
                            path,
                            domain: domain.id,
                        });
                    }
                }
                let output = ContentClaimProjection {
                    claim: crate::terminal_identities::lookup_claim_id(
                        self.calls.claims.bindings(),
                        binding.claim_identity,
                    )?,
                    projection: projection.identity,
                    subject: ContentStructuralPlace {
                        version: ContentPlaceVersion::Current,
                        root: place,
                        segments,
                    },
                };
                output_bindings.push((binding_handle, output.clone()));
                outputs.push(output);
            }
            let custody = self.boundary_body_custody(derived, &input_bindings, &output_bindings)?;
            inputs.sort();
            outputs.sort();
            let conservation = semantic_vocabulary::ContentConservation::new(
                algebra.ok_or(LoweringError::Unsupported(
                    "content transformation has no input algebra",
                ))?,
                terminal_semantics::content_transformations::claim_term(&inputs)
                    .map_err(LoweringError::Unsupported)?,
                terminal_semantics::content_transformations::claim_term(&outputs)
                    .map_err(LoweringError::Unsupported)?,
            );
            derivations.push(ContentTransformationDerivation {
                conservation,
                lineage_anchor: crate::terminal_identities::lookup_claim_id(
                    self.calls.claims.bindings(),
                    derived.lineage_anchor,
                )?,
                ancestry: self.content_ancestry(derived.conservation)?,
                custody,
                inputs,
                outputs,
            });
        }
        qualifications.sort();
        qualifications.dedup();
        projected_qualifications.sort();
        projected_qualifications.dedup();
        derivations.sort();
        // The transaction consumes these exact subtrees. Keep unaffected
        // siblings at their homes; only the receiving operation may publish
        // the payload's transported names after this completion.
        for consumed in parents.iter().chain(std::iter::once(&source)) {
            if self.evaluation.current_claims.contains(consumed.place) {
                self.evaluation
                    .current_claims
                    .replace(consumed.place, &consumed.path, &[])?;
            }
        }
        self.evaluation.retire_consumed_structural_arguments(
            parents.iter().chain(std::iter::once(&source)),
            self.operations,
        )?;
        self.operations.push(Operation {
            id: operation,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type: self.structural_type,
                multiplicity: self.multiplicity,
                qualifications,
                projected_qualifications,
                qualification_establishments: Vec::new(),
                claims,
            }),
            kind: OperationKind::TransformContent {
                source,
                parents,
                derivations,
            },
        });
        self.environment
            .private_places
            .push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: self.structural_type,
                },
            });
        self.operations
            .register_structural_materialization(place, self.state, self.statement)?;
        Ok(StructuralArgument::whole(place, StructuralAccess::Owned))
    }

    fn content_ancestry(
        &self,
        conservation: arena::Handle<checked_trees::facts::ContentBodyConservationFact>,
    ) -> Result<Vec<terminal_psi::ContentLineageStep>, LoweringError> {
        use checked_trees::facts::ContentClaimTransformationSource;
        let content = &self.checked.facts.qualifications.content;
        let equation = content.body_conservations.get(conservation);
        let completed = self
            .checked
            .facts
            .flow
            .control
            .operands
            .get(equation.completed_value);
        let mut retained = Vec::new();
        for entry in equation.inputs.handles() {
            let frontier = content
                .descendant_frontier(
                    entry,
                    equation.machine_symbol,
                    equation.state_symbol,
                    completed.statement_index,
                    equation.evaluation,
                    conservation,
                )
                .map_err(LoweringError::Unsupported)?;
            for (handle, _, _) in frontier.replacements {
                if !retained.contains(&handle) {
                    retained.push(handle);
                }
            }
        }
        let mut ancestry = Vec::new();
        for handle in retained {
            let transformation = content.claim_transformations.get(handle);
            let claims =
                |bindings: arena::HandleSpan<checked_trees::facts::ContentBodyClaimBinding>| {
                    let mut claims = content
                        .body_claim_bindings
                        .span_or_empty(bindings)
                        .iter()
                        .map(|binding| {
                            crate::terminal_identities::lookup_claim_id(
                                self.calls.claims.bindings(),
                                binding.claim_identity,
                            )
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    claims.sort();
                    Ok::<_, LoweringError>(claims)
                };
            let inputs = claims(transformation.inputs)?;
            let outputs = claims(transformation.outputs)?;
            let producer_operation = match transformation.source {
                ContentClaimTransformationSource::Call { call_ordinal, .. } => {
                    let mut calls = self.operations.source_calls.iter().filter(|call| {
                        call.source_state == transformation.state_symbol
                            && call.statement_index == transformation.statement_index
                            && call.call_ordinal == call_ordinal
                    });
                    let call = calls.next().ok_or(LoweringError::Unsupported(
                        "content ancestry call was not emitted",
                    ))?;
                    if calls.next().is_some() {
                        return unsupported("content ancestry call has ambiguous emission");
                    }
                    let lowered_psi::LoweredSourceCallSite::Operation(operation) =
                        call.terminal_site
                    else {
                        return unsupported(
                            "content ancestry cannot come from a nonreturning exit",
                        );
                    };
                    operation
                }
                ContentClaimTransformationSource::Body { .. } => {
                    let mut producers = self.operations.iter().filter(|operation| {
                        matches!(&operation.kind, terminal_psi::OperationKind::TransformContent { derivations, .. }
                            if derivations.iter().any(|derived| derived.outputs.iter().map(|output| output.claim).collect::<Vec<_>>() == outputs))
                    });
                    let producer = producers.next().ok_or(LoweringError::Unsupported(
                        "content ancestry transformation was not emitted",
                    ))?;
                    if producers.next().is_some() {
                        return unsupported(
                            "content ancestry transformation has ambiguous emission",
                        );
                    }
                    producer.id
                }
                ContentClaimTransformationSource::Unknown => {
                    return unsupported("content ancestry has no producer");
                }
            };
            ancestry.push(terminal_psi::ContentLineageStep {
                producer_operation,
                inputs,
                outputs,
            });
        }
        Ok(ancestry)
    }

    fn content_projection(
        &self,
        domain: language_semantics::SemanticDomainId,
    ) -> Result<&terminal_psi::StructuralContentProjection, LoweringError> {
        self.domain_declarations
            .iter()
            .filter_map(|domain| domain.content_projection.as_ref())
            .find(|projection| projection.identity.domain.get() == u64::from(domain.0))
            .ok_or(LoweringError::Unsupported(
                "content transformation has no retained owner projection",
            ))
    }
}
