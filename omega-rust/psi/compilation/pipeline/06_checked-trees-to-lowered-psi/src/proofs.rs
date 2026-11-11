/*
Proposition, contract, crash-route, content-conservation, float-meaning projection, integer
predicate, and semantic evidence lowering into `TerminalModule`.
*/

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedPropositionBinderArgumentKind,
    CheckedPropositionBinderKind, CheckedPropositionEvidence, CheckedScalarExpression,
    CheckedTrees, CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, ContentIdentityReshuffleFact,
};
use language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity, ContentArithmeticOperator,
    ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentProjectionExpression as CheckedContentProjectionExpression,
    ContentScalarExpression as CheckedContentScalarExpression,
    ContentStructuralPlace as CheckedContentStructuralPlace, conservation_report_fingerprint,
};
use language_semantics::{PermissionClaimIdentity, SemanticDomainId};
use lowered_psi::LoweredPsi;
use semantic_vocabulary::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentConservation, ContentDomainId, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    ContentStructuralPlace, ContentTerm, EvidenceTermId, IeeeFloatFormat, IeeeFloatStructuralField,
    IntegerSign, IntegerType, IntegerValue, MachineId, PlaceId, Proposition, PropositionContext,
    PropositionId, ScalarTerm, ScalarType, StructuralCaseSubject, StructuralPlaceKind,
    StructuralTypeId,
};
use terminal_psi::{
    BoundaryContentGuarantee, ClaimContentProjection, ContentConservationGuarantee,
    ContentEntryClaim, ContentIdentityReshuffle, CrashCause as TerminalCrashCause,
    EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidenceProjectionIdentity, EvidenceRequirementIdentity, EvidenceTermDeclaration,
    OperationResult, OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity,
    OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificGuard, ProofOutput,
    ProofOutputCall, ProofOutputRuntimeCall, PropositionApplicationIdentity,
    PropositionBinderArgumentIdentity, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, StaticRequirementDispatch,
    StructuralContentProjection, StructuralFieldType, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
    ValueDeclaration,
};

use crate::emission::scalar_types::{integer_scalar_type, terminal_scalar_type};
use crate::expression_preparation::prepare_expression::lower_checked_scalar_expression;
use crate::shared::lowering_error::{LoweringError, unsupported};
use crate::shared::terminal_identities::{
    dense_identity, lookup_claim_id, obligation_id, proposition_id, value_id,
};

pub(crate) mod content_conservation {
    use super::{
        BTreeMap, BTreeSet, BoundaryContentGuarantee, CheckedContentAlgebraIdentity,
        CheckedContentConservationTerm, CheckedContentPlaceRoot, CheckedContentPlaceSegment,
        CheckedContentPlaceVersion, CheckedContentProjectionExpression,
        CheckedContentScalarExpression, CheckedContentStructuralPlace, CheckedTrees,
        CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan,
        CheckedUnitStructuralPathSegment, ClaimContentProjection, ClaimId, ContentAlgebra,
        ContentArithmeticOperator, ContentConservation, ContentConservationGuarantee,
        ContentConservationOwnerKind, ContentConservationPlan, ContentDomainId, ContentEntryClaim,
        ContentIdentityReshuffle, ContentIdentityReshuffleFact, ContentPlaceSegment,
        ContentPlaceVersion, ContentProjectionExpression, ContentProjectionIdentity,
        ContentProjectionScalar, ContentStructuralPlace, ContentTerm, LoweringError,
        PermissionClaimIdentity, PlaceId, Proposition, PropositionContext, SemanticDomainId,
        StructuralContentProjection, StructuralParameterDeclaration, StructuralPlaceDeclaration,
        StructuralPlaceKind, StructuralTypeDeclaration, conservation_report_fingerprint,
        lookup_claim_id, unsupported,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LoweredContentConservation {
        pub source_report_fingerprint: u64,
        pub structural_places: Vec<StructuralPlaceDeclaration>,
        pub proposition: Proposition,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LoweredContentIdentityReshuffles {
        pub structural_places: Vec<StructuralPlaceDeclaration>,
        pub entry_claims: Vec<ContentEntryClaim>,
        pub reshuffles: Vec<ContentIdentityReshuffle>,
        pub source_claims: Vec<(PermissionClaimIdentity, ClaimId)>,
    }

    fn content_algebra(identity: &CheckedContentAlgebraIdentity) -> ContentAlgebra {
        identity.canonical_algebra()
    }

    fn content_field_identity(
        checked: &CheckedTrees,
        symbol: symbols::SymbolHandle,
    ) -> Option<String> {
        checked.data_definitions().iter().find_map(|definition| {
            checked.data_members(definition).iter().find_map(|member| {
                let checked_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.symbol == symbol).then(|| field.path_identity())
            })
        })
    }

    fn lower_content_projection_scalar(
        checked: &CheckedTrees,
        value: &CheckedContentScalarExpression,
        depth: usize,
    ) -> Result<ContentProjectionScalar, LoweringError> {
        if depth > 256 {
            return unsupported("content projection expression is too deeply nested");
        }
        let path = |segments: &[language_semantics::content::ContentFieldSegment]| {
            segments
                .iter()
                .map(|segment| {
                    let identity = content_field_identity(checked, segment.symbol).ok_or(
                        LoweringError::Unsupported(
                            "content projection references an unknown structural field",
                        ),
                    )?;
                    Ok(identity)
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        };
        Ok(match value {
            CheckedContentScalarExpression::SubjectField(segments) => {
                ContentProjectionScalar::SubjectField(path(segments)?)
            }
            CheckedContentScalarExpression::RuntimeScalarEmbedding(segments) => {
                ContentProjectionScalar::RuntimeScalarEmbedding(path(segments)?)
            }
            CheckedContentScalarExpression::Natural(value) => {
                if value.is_empty() {
                    return unsupported("content projection contains an empty natural");
                }
                ContentProjectionScalar::Natural(value.clone())
            }
            CheckedContentScalarExpression::Successor(inner) => ContentProjectionScalar::Successor(
                Box::new(lower_content_projection_scalar(checked, inner, depth + 1)?),
            ),
            CheckedContentScalarExpression::Arithmetic {
                operator,
                left,
                right,
            } => {
                let left = Box::new(lower_content_projection_scalar(checked, left, depth + 1)?);
                let right = Box::new(lower_content_projection_scalar(checked, right, depth + 1)?);
                match operator {
                    ContentArithmeticOperator::Add => ContentProjectionScalar::Add(left, right),
                    ContentArithmeticOperator::Subtract => {
                        ContentProjectionScalar::Subtract(left, right)
                    }
                    ContentArithmeticOperator::Multiply => {
                        ContentProjectionScalar::Multiply(left, right)
                    }
                }
            }
        })
    }

    fn lower_content_projection_expression(
        checked: &CheckedTrees,
        expression: &CheckedContentProjectionExpression,
    ) -> Result<ContentProjectionExpression, LoweringError> {
        Ok(match expression {
            CheckedContentProjectionExpression::IntervalSet { members } => {
                ContentProjectionExpression::IntervalSet(
                    members
                        .iter()
                        .map(|member| {
                            Ok((
                                lower_content_projection_scalar(checked, member.start(), 0)?,
                                lower_content_projection_scalar(checked, member.end(), 0)?,
                            ))
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?,
                )
            }
            CheckedContentProjectionExpression::CountedQuantity { magnitude } => {
                ContentProjectionExpression::CountedQuantity(lower_content_projection_scalar(
                    checked, magnitude, 0,
                )?)
            }
        })
    }

    pub(crate) fn lower_structural_content_projection(
        checked: &CheckedTrees,
        domain: SemanticDomainId,
        expected_carrier_identity: &str,
    ) -> Result<Option<StructuralContentProjection>, LoweringError> {
        let Some(projection) = checked
            .facts
            .qualifications
            .content
            .for_semantic_domain(domain)
        else {
            return Ok(None);
        };
        if projection.carrier_identity != expected_carrier_identity
            || projection.report_fingerprint == 0
        {
            return unsupported(
                "structural domain content projection disagrees with its carrier or identity",
            );
        }
        let algebra = content_algebra(&projection.algebra);
        let expression = lower_content_projection_expression(checked, &projection.expression)?;
        let identity = ContentProjectionIdentity {
            domain: ContentDomainId::new(u64::from(domain.0))
                .ok_or(LoweringError::InvalidContentDomainIdentity)?,
            projection_report_fingerprint: projection.report_fingerprint,
        };
        if language_semantics::content::terminal_projection_report_fingerprint(
            &algebra,
            &expression,
        ) != identity.projection_report_fingerprint
        {
            return unsupported(
                "structural domain content projection report_fingerprint does not replay",
            );
        }
        Ok(Some(StructuralContentProjection {
            identity,
            algebra,
            expression,
        }))
    }

    pub(crate) fn lower_whole_content_entry_claims(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        structural_types: &[StructuralTypeDeclaration],
        checked_parameters: &[CheckedUnitStructuralParameterPlan],
        parameters: &[StructuralParameterDeclaration],
        entry_claims: &[CheckedUnitEntryClaimPlan],
        claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    ) -> Result<Vec<ContentEntryClaim>, LoweringError> {
        if checked_parameters.len() != parameters.len() {
            return unsupported("content entry parameter catalogs disagree");
        }

        let mut output = Vec::new();
        for entry_claim in entry_claims {
            let parameter_index = usize::try_from(entry_claim.parameter_index).map_err(|_| {
                LoweringError::Unsupported("content entry claim parameter index exceeds usize")
            })?;
            let checked_parameter =
                checked_parameters
                    .get(parameter_index)
                    .ok_or(LoweringError::Unsupported(
                        "content entry claim has an invalid checked parameter",
                    ))?;
            let parameter = parameters
                .get(parameter_index)
                .ok_or(LoweringError::Unsupported(
                    "content entry claim has an invalid terminal parameter",
                ))?;

            if checked_parameter
                .qualifications
                .windows(2)
                .any(|pair| pair[0].0 >= pair[1].0)
                || checked_parameter
                    .projected_qualifications
                    .windows(2)
                    .any(|pair| {
                        pair[0].path > pair[1].path
                            || (pair[0].path == pair[1].path
                                && pair[0].domain.0 >= pair[1].domain.0)
                    })
                || checked_parameter
                    .projected_qualifications
                    .iter()
                    .any(|row| row.path.is_empty())
            {
                return unsupported("content entry qualifications are not canonical");
            }
            let content = checked_parameter
                .qualifications
                .iter()
                .copied()
                .filter(|_| entry_claim.path.is_empty())
                .chain(
                    checked_parameter
                        .projected_qualifications
                        .iter()
                        .filter(|row| row.path == entry_claim.path)
                        .map(|row| row.domain),
                )
                .filter_map(|qualification| {
                    checked
                        .facts
                        .qualifications
                        .content
                        .for_semantic_domain(qualification)
                })
                .collect::<Vec<_>>();
            if content.is_empty() {
                continue;
            }
            let mut segments = Vec::with_capacity(entry_claim.path.len());
            for segment in &entry_claim.path {
                segments.push(match segment {
                    CheckedUnitStructuralPathSegment::Field(identity) => {
                        identities.intern(&identity.spelling());
                        ContentPlaceSegment::Field(identity.spelling())
                    }
                    CheckedUnitStructuralPathSegment::Case(identity) => {
                        identities.intern(&identity.spelling());
                        ContentPlaceSegment::Case(identity.spelling())
                    }
                    CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                        ContentPlaceSegment::FixedIndex(*index)
                    }
                    _ => return unsupported("content claim has an unsupported or mistyped path"),
                });
            }
            let path = terminal_semantics::content_transformations::structural_path(
                structural_types.iter(),
                &identities.snapshot(),
                parameter.structural_type,
                &segments,
            )
            .ok_or(LoweringError::Unsupported(
                "content claim has an unsupported or mistyped path",
            ))?;
            let carrier = terminal_semantics::runtime_structural_path_tip(
                structural_types.iter(),
                parameter.structural_type,
                &path,
            )
            .ok_or(LoweringError::Unsupported(
                "content claim leaf carrier is absent",
            ))?;
            let mut projections = content
                .into_iter()
                .map(|projection| {
                    if projection.carrier_identity
                        != identities.text(carrier.identity).unwrap_or_default()
                    {
                        return unsupported(
                            "content projection carrier disagrees with its qualified parameter",
                        );
                    }
                    if projection.report_fingerprint == 0 {
                        return Err(LoweringError::ZeroContentProjectionFingerprint);
                    }
                    let domain = ContentDomainId::new(u64::from(projection.semantic_domain.0))
                        .ok_or(LoweringError::InvalidContentDomainIdentity)?;
                    let algebra = content_algebra(&projection.algebra);
                    Ok(ClaimContentProjection {
                        projection: ContentProjectionIdentity {
                            domain,
                            projection_report_fingerprint: projection.report_fingerprint,
                        },
                        algebra,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            if projections.is_empty() {
                continue;
            }
            projections.sort();
            if projections.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(LoweringError::DuplicateContentIdentityProjection);
            }
            let claim = lookup_claim_id(claim_bindings, entry_claim.claim_identity)?;
            output.push(ContentEntryClaim {
                claim,
                input: ContentStructuralPlace {
                    version: ContentPlaceVersion::Entry,
                    root: parameter.place,
                    segments,
                },
                projections,
            });
        }

        output.sort_by_key(|entry| entry.claim);
        if output.windows(2).any(|pair| pair[0].claim == pair[1].claim) {
            return unsupported("content entry claims must name distinct source claims");
        }
        Ok(output)
    }

    pub fn lower_content_conservation_plan(
        plan: &ContentConservationPlan,
    ) -> Result<LoweredContentConservation, LoweringError> {
        let expected_fingerprint = conservation_report_fingerprint(&plan.algebra, &plan.equation);
        if plan.report_fingerprint != expected_fingerprint {
            return Err(LoweringError::ContentConservationFingerprintMismatch {
                expected: expected_fingerprint,
                actual: plan.report_fingerprint,
            });
        }

        let algebra = content_algebra(&plan.algebra);
        let mut structural_places = BTreeMap::new();
        let left = lower_content_term(plan.equation.left(), &mut structural_places, 0)?;
        let right = lower_content_term(plan.equation.right(), &mut structural_places, 0)?;
        let proposition =
            Proposition::ContentConservation(ContentConservation::new(algebra, left, right));
        let context = PropositionContext::from_value_types_and_places(
            [],
            structural_places.iter().map(|(id, kind)| (*id, *kind)),
        )
        .map_err(LoweringError::InvalidContentProposition)?;
        context
            .validate(&proposition)
            .map_err(LoweringError::InvalidContentProposition)?;

        Ok(LoweredContentConservation {
            source_report_fingerprint: plan.report_fingerprint,
            structural_places: structural_places
                .into_iter()
                .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                .collect(),
            proposition,
        })
    }

    pub fn lower_boundary_content_guarantees(
        plans: &[ContentConservationPlan],
        callable: symbols::SymbolHandle,
    ) -> Result<Vec<BoundaryContentGuarantee>, LoweringError> {
        let mut guarantees = plans
            .iter()
            .filter(|plan| {
                plan.owner_kind == ContentConservationOwnerKind::TraitRequirement
                    && plan.callable == callable
            })
            .map(|plan| {
                let lowered = lower_content_conservation_plan(plan)?;
                let conservation = lowered_conservation(lowered.proposition)?;
                Ok(BoundaryContentGuarantee::Conservation(
                    ContentConservationGuarantee {
                        report_fingerprint: lowered.source_report_fingerprint,
                        structural_places: lowered.structural_places,
                        conservation,
                    },
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        guarantees.sort();
        if guarantees.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(LoweringError::DuplicateContentPartitionComposition);
        }
        Ok(guarantees)
    }

    pub fn lower_content_identity_reshuffles(
        facts: &[ContentIdentityReshuffleFact],
    ) -> Result<LoweredContentIdentityReshuffles, LoweringError> {
        #[derive(Debug)]
        struct Group {
            source_claim: PermissionClaimIdentity,
            input: ContentStructuralPlace,
            output: ContentStructuralPlace,
            projections: Vec<ClaimContentProjection>,
        }

        let Some(first) = facts.first() else {
            return Ok(LoweredContentIdentityReshuffles {
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                reshuffles: Vec::new(),
                source_claims: Vec::new(),
            });
        };
        let callable = (first.machine_symbol, first.state_symbol);
        let mut structural_places = BTreeMap::new();
        let mut projection_algebras = BTreeMap::<ContentProjectionIdentity, ContentAlgebra>::new();
        let mut groups = Vec::<Group>::new();

        for fact in facts {
            if (fact.machine_symbol, fact.state_symbol) != callable
                || fact.plan.owner_kind != ContentConservationOwnerKind::Machine
                || fact.plan.owner != fact.machine_symbol
                || fact.plan.callable != fact.state_symbol
            {
                return Err(LoweringError::ContentIdentityFactOwnerMismatch);
            }
            if fact.claim_identity == PermissionClaimIdentity::Unknown {
                return Err(LoweringError::UnknownContentClaimIdentity);
            }
            validate_identity_input_symbol(fact)?;

            let lowered = lower_content_conservation_plan(&fact.plan)?;
            for declaration in lowered.structural_places {
                if let Some(previous) = structural_places.insert(declaration.id, declaration.kind)
                    && previous != declaration.kind
                {
                    return Err(LoweringError::ConflictingContentPlaceRoot {
                        id: declaration.id,
                        first: previous,
                        second: declaration.kind,
                    });
                }
            }
            let Proposition::ContentConservation(conservation) = lowered.proposition else {
                unreachable!("content plan lowering always yields content conservation")
            };
            let (input, output, projection) = direct_identity_projection(&conservation)?;
            let content = ClaimContentProjection {
                projection,
                algebra: conservation.algebra().clone(),
            };
            if let Some(previous) =
                projection_algebras.insert(content.projection, content.algebra.clone())
                && previous != content.algebra
            {
                return Err(LoweringError::ContentProjectionAlgebraMismatch(
                    content.projection,
                ));
            }

            if let Some(group) = groups
                .iter_mut()
                .find(|group| group.source_claim == fact.claim_identity && group.output == output)
            {
                if group.input != input || group.output != output {
                    return Err(LoweringError::ContentIdentityClaimMapsMultiplePlaces);
                }
                group.projections.push(content);
            } else {
                groups.push(Group {
                    source_claim: fact.claim_identity,
                    input,
                    output,
                    projections: vec![content],
                });
            }
        }

        for group in &mut groups {
            group.projections.sort();
            if group.projections.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(LoweringError::DuplicateContentIdentityProjection);
            }
        }
        groups.sort_by(|left, right| {
            (&left.input, &left.output, &left.projections).cmp(&(
                &right.input,
                &right.output,
                &right.projections,
            ))
        });
        for (position, group) in groups.iter().enumerate() {
            for previous in &groups[..position] {
                if previous.source_claim == group.source_claim {
                    if previous.input != group.input
                        || previous.projections != group.projections
                        || !previous.output.is_case_alternative(&group.output)
                    {
                        return Err(LoweringError::ContentIdentityClaimMapsMultiplePlaces);
                    }
                } else if previous.input == group.input {
                    return Err(LoweringError::DuplicateContentIdentityInput);
                } else if content_places_overlap(&previous.input, &group.input) {
                    return Err(LoweringError::OverlappingContentIdentityInput);
                }
            }
        }
        let mut outputs = BTreeSet::<&ContentStructuralPlace>::new();
        for group in &groups {
            if !outputs.insert(&group.output) {
                return Err(LoweringError::DuplicateContentIdentityOutput);
            }
            if outputs.iter().any(|previous| {
                **previous != group.output && content_places_overlap(previous, &group.output)
            }) {
                return Err(LoweringError::OverlappingContentIdentityOutput);
            }
        }

        let mut source_claims = Vec::new();
        let reshuffles = groups
            .into_iter()
            .map(|group| {
                let claim = source_claims
                    .iter()
                    .find_map(|(source, claim)| (*source == group.source_claim).then_some(*claim))
                    .unwrap_or_else(|| {
                        let claim = ClaimId::new(
                            u64::try_from(source_claims.len())
                                .expect("an in-memory fact count fits u64")
                                + 1,
                        )
                        .expect("dense claim identities begin at one");
                        source_claims.push((group.source_claim, claim));
                        claim
                    });
                ContentIdentityReshuffle {
                    claim,
                    input: group.input,
                    output: group.output,
                    projections: group.projections,
                }
            })
            .collect::<Vec<_>>();
        let mut entry_claims = Vec::new();
        for reshuffle in &reshuffles {
            if !entry_claims
                .iter()
                .any(|entry: &ContentEntryClaim| entry.claim == reshuffle.claim)
            {
                entry_claims.push(ContentEntryClaim {
                    claim: reshuffle.claim,
                    input: reshuffle.input.clone(),
                    projections: reshuffle.projections.clone(),
                });
            }
        }
        Ok(LoweredContentIdentityReshuffles {
            structural_places: structural_places
                .into_iter()
                .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                .collect(),
            entry_claims,
            reshuffles,
            source_claims,
        })
    }

    fn lowered_conservation(
        proposition: Proposition,
    ) -> Result<ContentConservation, LoweringError> {
        match proposition {
            Proposition::ContentConservation(conservation) => Ok(conservation),
            _ => Err(LoweringError::ContentPartitionNotConservation),
        }
    }

    fn validate_identity_input_symbol(
        fact: &ContentIdentityReshuffleFact,
    ) -> Result<(), LoweringError> {
        let roots = [fact.plan.equation.left(), fact.plan.equation.right()];
        let has_input = roots.iter().any(|term| {
            matches!(
                term,
                CheckedContentConservationTerm::Projection {
                    subject: CheckedContentStructuralPlace {
                        version: CheckedContentPlaceVersion::Entry,
                        root: CheckedContentPlaceRoot::Parameter { symbol, .. },
                        ..
                    },
                    ..
                } if *symbol == fact.input_parameter_symbol
            )
        });
        if has_input {
            Ok(())
        } else {
            Err(LoweringError::ContentIdentityInputParameterMismatch)
        }
    }

    fn direct_identity_projection(
        conservation: &ContentConservation,
    ) -> Result<
        (
            ContentStructuralPlace,
            ContentStructuralPlace,
            ContentProjectionIdentity,
        ),
        LoweringError,
    > {
        let projection = |term: &ContentTerm| match term {
            ContentTerm::Projection {
                projection,
                subject,
            } => Some((*projection, subject.clone())),
            ContentTerm::Separate(_) => None,
        };
        let (left_projection, left) = projection(conservation.left())
            .ok_or(LoweringError::ContentIdentityNotDirectEquality)?;
        let (right_projection, right) = projection(conservation.right())
            .ok_or(LoweringError::ContentIdentityNotDirectEquality)?;
        if left_projection != right_projection {
            return Err(LoweringError::ContentIdentityProjectionMismatch);
        }
        let (input, output) = match (left.version, right.version) {
            (ContentPlaceVersion::Entry, ContentPlaceVersion::Current) => (left, right),
            (ContentPlaceVersion::Current, ContentPlaceVersion::Entry) => (right, left),
            _ => return Err(LoweringError::ContentIdentityDirectionMismatch),
        };
        if input.root.get() >= RESULT_STRUCTURAL_PLACE_ID
            || output.root.get() != RESULT_STRUCTURAL_PLACE_ID
        {
            return Err(LoweringError::ContentIdentityRootMismatch);
        }
        Ok((input, output, left_projection))
    }

    fn content_places_overlap(
        left: &ContentStructuralPlace,
        right: &ContentStructuralPlace,
    ) -> bool {
        if left.version != right.version || left.root != right.root {
            return false;
        }
        let shared = left.segments.len().min(right.segments.len());
        left.segments[..shared] == right.segments[..shared]
    }

    const MAX_CONTENT_TERM_DEPTH: usize = 256;
    pub(crate) const RESULT_STRUCTURAL_PLACE_ID: u64 = 4_294_967_297;

    fn lower_content_term(
        term: &CheckedContentConservationTerm,
        structural_places: &mut BTreeMap<PlaceId, StructuralPlaceKind>,
        depth: usize,
    ) -> Result<ContentTerm, LoweringError> {
        if depth > MAX_CONTENT_TERM_DEPTH {
            return Err(LoweringError::ContentTermNestingTooDeep);
        }
        match term {
            CheckedContentConservationTerm::Projection {
                semantic_domain,
                projection_report_fingerprint,
                subject,
                ..
            } => {
                let domain = ContentDomainId::new(u64::from(semantic_domain.0))
                    .ok_or(LoweringError::InvalidContentDomainIdentity)?;
                if *projection_report_fingerprint == 0 {
                    return Err(LoweringError::ZeroContentProjectionFingerprint);
                }
                Ok(ContentTerm::Projection {
                    projection: ContentProjectionIdentity {
                        domain,
                        projection_report_fingerprint: *projection_report_fingerprint,
                    },
                    subject: lower_content_place(subject, structural_places)?,
                })
            }
            CheckedContentConservationTerm::Separate(terms) => ContentTerm::separate(
                terms
                    .iter()
                    .map(|term| lower_content_term(term, structural_places, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .map_err(LoweringError::InvalidContentProposition),
        }
    }

    fn lower_content_place(
        place: &CheckedContentStructuralPlace,
        structural_places: &mut BTreeMap<PlaceId, StructuralPlaceKind>,
    ) -> Result<ContentStructuralPlace, LoweringError> {
        let version = match place.version {
            CheckedContentPlaceVersion::Entry => ContentPlaceVersion::Entry,
            CheckedContentPlaceVersion::Current => ContentPlaceVersion::Current,
        };
        let (root, kind) = match &place.root {
            CheckedContentPlaceRoot::Parameter { position, .. } => (
                PlaceId::new(u64::from(*position) + 1)
                    .expect("a parameter position plus one is nonzero"),
                StructuralPlaceKind::Parameter {
                    position: *position,
                },
            ),
            CheckedContentPlaceRoot::Result => (
                PlaceId::new(RESULT_STRUCTURAL_PLACE_ID)
                    .expect("the reserved result place is nonzero"),
                StructuralPlaceKind::Result,
            ),
        };
        if let Some(previous) = structural_places.insert(root, kind)
            && previous != kind
        {
            return Err(LoweringError::ConflictingContentPlaceRoot {
                id: root,
                first: previous,
                second: kind,
            });
        }
        let segments = place
            .segments
            .iter()
            .map(|segment| match segment {
                CheckedContentPlaceSegment::Case(case) => {
                    ContentPlaceSegment::Case(case.name.clone())
                }
                CheckedContentPlaceSegment::Field(field) => {
                    ContentPlaceSegment::Field(field.name.clone())
                }
                CheckedContentPlaceSegment::FixedIndex(index) => {
                    ContentPlaceSegment::FixedIndex(*index)
                }
            })
            .collect();
        Ok(ContentStructuralPlace {
            version,
            root,
            segments,
        })
    }
}
pub(crate) mod contract_predicates {

    use super::{
        CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
        LoweringError, Proposition, ScalarTerm, unsupported,
    };
    use semantic_vocabulary::{IntegerMathTerm, ScalarType};

    pub(crate) mod integer_math {

        use super::{IntegerMathTerm, LoweringError, PredicateTerms, unsupported};
        use checked_trees::CheckedScalarExpression;

        pub(crate) fn lower(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &checked_trees::CheckedIntegerMathExpression,
            terms: &impl PredicateTerms,
            remaining: &mut usize,
        ) -> Result<IntegerMathTerm, LoweringError> {
            fn lower(
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expression: &checked_trees::CheckedIntegerMathExpression,
                handle: arena::Handle<checked_trees::CheckedIntegerMathTerm>,
                terms: &impl PredicateTerms,
                remaining: &mut usize,
                depth: usize,
            ) -> Result<IntegerMathTerm, LoweringError> {
                use checked_trees::CheckedIntegerMathTerm;
                *remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
                    "mathematical contract exceeds its lowering budget",
                ))?;
                if depth >= 64 || !expression.terms.is_valid(handle) {
                    return unsupported("mathematical contract has an invalid term graph");
                }
                match expression.terms.get(handle) {
                    CheckedIntegerMathTerm::Embed { operand, .. } => {
                        let scalar = terms.scalar(identities, operand)?;
                        IntegerMathTerm::embed(scalar).ok_or(LoweringError::Unsupported(
                            "mathematical embedding requires an integer scalar denotation",
                        ))
                    }
                    CheckedIntegerMathTerm::Literal(literal) => {
                        let term = IntegerMathTerm::IntegerLiteral(*literal);
                        term.validate()
                            .map_err(LoweringError::InvalidCrashPredicate)?;
                        Ok(term)
                    }
                    CheckedIntegerMathTerm::BuiltinTypeMember(member) => {
                        let primitive = match member {
                            symbols::BuiltinTypeMember::AddressBound => {
                                checked_trees::types::PrimitiveType::Addr
                            }
                        };
                        let carrier =
                            crate::emission::scalar_types::integer_scalar_type(primitive)?
                                .as_integer()
                                .ok_or(LoweringError::InvalidPsiIntegerType)?;
                        Ok(IntegerMathTerm::Add(
                            Box::new(IntegerMathTerm::literal(carrier.maximum_value())),
                            Box::new(IntegerMathTerm::literal(
                                semantic_vocabulary::IntegerValue::Unsigned(1),
                            )),
                        ))
                    }
                    CheckedIntegerMathTerm::Add { left, right }
                    | CheckedIntegerMathTerm::Subtract { left, right }
                    | CheckedIntegerMathTerm::Multiply { left, right } => {
                        let left = Box::new(lower(
                            identities,
                            expression,
                            *left,
                            terms,
                            remaining,
                            depth + 1,
                        )?);
                        let right = Box::new(lower(
                            identities,
                            expression,
                            *right,
                            terms,
                            remaining,
                            depth + 1,
                        )?);
                        Ok(match expression.terms.get(handle) {
                            CheckedIntegerMathTerm::Add { .. } => IntegerMathTerm::Add(left, right),
                            CheckedIntegerMathTerm::Subtract { .. } => {
                                IntegerMathTerm::Subtract(left, right)
                            }
                            _ => IntegerMathTerm::Multiply(left, right),
                        })
                    }
                }
            }
            lower(identities, expression, expression.root, terms, remaining, 0)
        }

        pub(crate) fn validate_namespace(
            expression: &checked_trees::CheckedIntegerMathExpression,
            scalar: &impl Fn(&CheckedScalarExpression) -> Result<(), LoweringError>,
        ) -> Result<(), LoweringError> {
            use checked_trees::CheckedIntegerMathTerm;
            let mut pending = vec![(expression.root, 0)];
            let mut remaining = 4096usize;
            while let Some((handle, depth)) = pending.pop() {
                remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
                    "mathematical contract namespace exceeds its traversal budget",
                ))?;
                if depth >= 64 || !expression.terms.is_valid(handle) {
                    return Err(LoweringError::Unsupported(
                        "mathematical contract has an invalid term graph",
                    ));
                }
                match expression.terms.get(handle) {
                    CheckedIntegerMathTerm::Embed { operand, .. } => scalar(operand)?,
                    CheckedIntegerMathTerm::Literal(_)
                    | CheckedIntegerMathTerm::BuiltinTypeMember(_) => {}
                    CheckedIntegerMathTerm::Add { left, right }
                    | CheckedIntegerMathTerm::Subtract { left, right }
                    | CheckedIntegerMathTerm::Multiply { left, right } => {
                        pending.push((*right, depth + 1));
                        pending.push((*left, depth + 1));
                    }
                }
            }
            Ok(())
        }
    }
    pub(crate) use integer_math::validate_namespace as validate_math_namespace;
    pub(crate) trait PredicateTerms {
        fn scalar(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedScalarExpression,
        ) -> Result<ScalarTerm, LoweringError>;
        fn boolean(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
        ) -> Result<ScalarTerm, LoweringError>;
        fn case_membership(
            &self,
            _identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            _expression: &CheckedBooleanExpression,
        ) -> Result<Proposition, LoweringError> {
            unsupported("contract namespace has no structural case subject")
        }
        fn structural_identity(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            left: &checked_trees::CheckedStructuralParameterField,
            right: &checked_trees::CheckedStructuralParameterField,
        ) -> Result<Proposition, LoweringError>;
        fn strict_bound(&self, left: ScalarTerm, right: ScalarTerm) -> Proposition {
            Proposition::LessThan(left, right)
        }
    }

    pub(crate) fn proposition(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        predicate: &CheckedBooleanExpression,
        terms: &impl PredicateTerms,
    ) -> Result<Proposition, LoweringError> {
        proposition_with_polarity(identities, predicate, terms, true, &mut 4096)
    }

    fn has_scalar_boolean_denotation(
        predicate: &CheckedBooleanExpression,
        remaining: &mut usize,
    ) -> Result<bool, LoweringError> {
        let mut pending = vec![(predicate, 0)];
        while let Some((predicate, depth)) = pending.pop() {
            *remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
                "scalar contract Boolean expansion exceeds its lowering budget",
            ))?;
            if depth >= 64 {
                return unsupported("scalar contract Boolean denotation exceeds its depth limit");
            }
            match predicate {
                CheckedBooleanExpression::Constant(_)
                | CheckedBooleanExpression::Parameter { .. }
                | CheckedBooleanExpression::StructuralParameterField { .. }
                | CheckedBooleanExpression::ByteSequencePredicate { .. } => {}
                CheckedBooleanExpression::Not(operand) => pending.push((operand, depth + 1)),
                CheckedBooleanExpression::Equal { left, right } => {
                    pending.extend([(left.as_ref(), depth + 1), (right.as_ref(), depth + 1)]);
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    pub(crate) fn proposition_with_polarity(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        predicate: &CheckedBooleanExpression,
        terms: &impl PredicateTerms,
        positive: bool,
        remaining: &mut usize,
    ) -> Result<Proposition, LoweringError> {
        *remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
            "scalar contract Boolean expansion exceeds its lowering budget",
        ))?;
        match predicate {
            CheckedBooleanExpression::StructuralIdentity { left, right } => {
                let identity = terms.structural_identity(identities, left, right)?;
                Ok(if positive {
                    identity
                } else {
                    Proposition::Implication {
                        premise: Box::new(identity),
                        conclusion: Box::new(Proposition::Falsehood),
                    }
                })
            }
            CheckedBooleanExpression::StructuralCaseMembership { .. } => {
                let membership = terms.case_membership(identities, predicate)?;
                Ok(if positive {
                    membership
                } else {
                    Proposition::Implication {
                        premise: Box::new(membership),
                        conclusion: Box::new(Proposition::Falsehood),
                    }
                })
            }
            CheckedBooleanExpression::IntegerMathComparison { kind, left, right } => {
                let left = integer_math::lower(identities, left, terms, remaining)?;
                let right = integer_math::lower(identities, right, terms, remaining)?;
                Ok(match (kind, positive) {
                    (CheckedIntegerComparisonKind::Equal, true) => {
                        let (left, right) = if left > right {
                            (right, left)
                        } else {
                            (left, right)
                        };
                        Proposition::IntegerMathEqual(left, right)
                    }
                    (CheckedIntegerComparisonKind::Equal, false) => connective(
                        Proposition::IntegerMathLessThan(left.clone(), right.clone()),
                        Proposition::IntegerMathLessThan(right, left),
                        false,
                    )?,
                    (CheckedIntegerComparisonKind::LessThan, true) => {
                        Proposition::IntegerMathLessThan(left, right)
                    }
                    (CheckedIntegerComparisonKind::LessOrEqual, false) => {
                        Proposition::IntegerMathLessThan(right, left)
                    }
                    (CheckedIntegerComparisonKind::LessOrEqual, true) => {
                        Proposition::IntegerMathLessOrEqual(left, right)
                    }
                    (CheckedIntegerComparisonKind::LessThan, false) => {
                        Proposition::IntegerMathLessOrEqual(right, left)
                    }
                })
            }
            CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                let left = terms.scalar(identities, left)?;
                let right = terms.scalar(identities, right)?;
                if left.scalar_type() != right.scalar_type() {
                    let Some(left_type) = left.scalar_type().as_integer() else {
                        return unsupported("integer contract comparison has a noninteger operand");
                    };
                    let comparison = match kind {
                        CheckedIntegerComparisonKind::Equal => {
                            ScalarTerm::integer_equal(left_type, left, right)
                        }
                        CheckedIntegerComparisonKind::LessThan => {
                            ScalarTerm::integer_less_than(left_type, left, right)
                        }
                        CheckedIntegerComparisonKind::LessOrEqual => {
                            ScalarTerm::integer_less_or_equal(left_type, left, right)
                        }
                    }
                    .map_err(LoweringError::InvalidCrashPredicate)?;
                    return canonical_equality(comparison, ScalarTerm::boolean(positive));
                }
                Ok(match (kind, positive) {
                    (CheckedIntegerComparisonKind::Equal, true) => canonical_equality(left, right)?,
                    (CheckedIntegerComparisonKind::Equal, false) => connective(
                        terms.strict_bound(left.clone(), right.clone()),
                        terms.strict_bound(right, left),
                        false,
                    )?,
                    (CheckedIntegerComparisonKind::LessThan, true) => {
                        terms.strict_bound(left, right)
                    }
                    (CheckedIntegerComparisonKind::LessOrEqual, false) => {
                        terms.strict_bound(right, left)
                    }
                    (CheckedIntegerComparisonKind::LessOrEqual, true) => {
                        Proposition::LessOrEqual(left, right)
                    }
                    (CheckedIntegerComparisonKind::LessThan, false) => {
                        Proposition::LessOrEqual(right, left)
                    }
                })
            }
            CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } => {
                ieee_float_proposition(
                    if positive { *kind } else { kind.negated() },
                    terms.scalar(identities, left)?,
                    terms.scalar(identities, right)?,
                )
            }
            CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right } => {
                let left = proposition_with_polarity(identities, left, terms, positive, remaining)?;
                let right =
                    proposition_with_polarity(identities, right, terms, positive, remaining)?;
                connective(
                    left,
                    right,
                    matches!(predicate, CheckedBooleanExpression::And { .. }) == positive,
                )
            }
            CheckedBooleanExpression::Not(operand) => {
                proposition_with_polarity(identities, operand, terms, !positive, remaining)
            }
            CheckedBooleanExpression::Constant(value) if *value == positive => {
                Ok(Proposition::Truth)
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::ByteSequencePredicate { .. } => canonical_equality(
                terms.boolean(identities, predicate)?,
                ScalarTerm::boolean(positive),
            ),
            CheckedBooleanExpression::Equal { left, right } => {
                if let CheckedBooleanExpression::Constant(value) = left.as_ref() {
                    return proposition_with_polarity(
                        identities,
                        right,
                        terms,
                        *value == positive,
                        remaining,
                    );
                }
                if let CheckedBooleanExpression::Constant(value) = right.as_ref() {
                    return proposition_with_polarity(
                        identities,
                        left,
                        terms,
                        *value == positive,
                        remaining,
                    );
                }
                if positive
                    && has_scalar_boolean_denotation(left, remaining)?
                    && has_scalar_boolean_denotation(right, remaining)?
                {
                    return canonical_equality(
                        terms.boolean(identities, left)?,
                        terms.boolean(identities, right)?,
                    );
                }
                equality_from_polarities(
                    left,
                    right,
                    positive,
                    remaining,
                    |operand, polarity, budget| {
                        proposition_with_polarity(identities, operand, terms, polarity, budget)
                    },
                )
            }
            _ => unsupported("result contract has an unsupported scalar predicate"),
        }
    }

    pub(crate) fn ieee_float_proposition(
        kind: semantic_vocabulary::IeeeFloatComparisonKind,
        mut left: ScalarTerm,
        mut right: ScalarTerm,
    ) -> Result<Proposition, LoweringError> {
        let Some(format) = left.scalar_type().as_ieee_float() else {
            return unsupported("IEEE contract comparison operand is not a float");
        };
        if right.scalar_type() != ScalarType::IeeeFloat(format) {
            return unsupported("IEEE contract comparison operands have different float formats");
        }
        if kind.is_symmetric() && left > right {
            std::mem::swap(&mut left, &mut right);
        }
        let proposition = Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        };
        proposition
            .validate()
            .map_err(LoweringError::InvalidCrashPredicate)?;
        Ok(proposition)
    }

    pub(crate) fn equality_from_polarities(
        left: &CheckedBooleanExpression,
        right: &CheckedBooleanExpression,
        positive: bool,
        remaining: &mut usize,
        mut lower: impl FnMut(
            &CheckedBooleanExpression,
            bool,
            &mut usize,
        ) -> Result<Proposition, LoweringError>,
    ) -> Result<Proposition, LoweringError> {
        connective(
            connective(
                lower(left, true, remaining)?,
                lower(right, positive, remaining)?,
                true,
            )?,
            connective(
                lower(left, false, remaining)?,
                lower(right, !positive, remaining)?,
                true,
            )?,
            false,
        )
    }

    pub(crate) fn canonical_equality(
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Proposition, LoweringError> {
        let left_key = terminal_codec::canonical_scalar_term_order_key(&left)
            .map_err(LoweringError::DebugSemanticCodec)?;
        let right_key = terminal_codec::canonical_scalar_term_order_key(&right)
            .map_err(LoweringError::DebugSemanticCodec)?;
        Ok(if left_key <= right_key {
            Proposition::Equal(left, right)
        } else {
            Proposition::Equal(right, left)
        })
    }

    pub(crate) fn canonical_propositions(
        propositions: impl IntoIterator<Item = Proposition>,
    ) -> Result<Vec<Proposition>, LoweringError> {
        let mut keyed = propositions
            .into_iter()
            .map(|proposition| {
                terminal_codec::canonical_proposition_order_key(&proposition)
                    .map(|key| (key, proposition))
                    .map_err(LoweringError::DebugSemanticCodec)
            })
            .collect::<Result<Vec<_>, _>>()?;
        keyed.sort_by(|left, right| left.0.cmp(&right.0));
        keyed.dedup_by(|left, right| left.0 == right.0);
        Ok(keyed
            .into_iter()
            .map(|(_, proposition)| proposition)
            .collect())
    }

    pub(crate) fn canonical_connectives(
        parts: impl IntoIterator<Item = Proposition>,
        conjunction: bool,
    ) -> Result<Proposition, LoweringError> {
        let mut flattened = Vec::new();
        for proposition in parts {
            match proposition {
                Proposition::Conjunction(nested) if conjunction => flattened.extend(nested),
                Proposition::Disjunction(nested) if !conjunction => flattened.extend(nested),
                proposition => flattened.push(proposition),
            }
        }
        let mut parts = canonical_propositions(flattened)?;
        if parts.len() == 1 {
            return Ok(parts.pop().expect("one distinct predicate"));
        }
        Ok(if conjunction {
            Proposition::Conjunction(parts)
        } else {
            Proposition::Disjunction(parts)
        })
    }

    pub(crate) fn connective(
        left: Proposition,
        right: Proposition,
        conjunction: bool,
    ) -> Result<Proposition, LoweringError> {
        match (&left, &right, conjunction) {
            (Proposition::Truth, _, true) => return Ok(right),
            (_, Proposition::Truth, true) => return Ok(left),
            (Proposition::Truth, _, false) | (_, Proposition::Truth, false) => {
                return Ok(Proposition::Truth);
            }
            _ => {}
        }
        canonical_connectives([left, right], conjunction)
    }
}
pub(crate) mod crash_routes {

    pub(crate) mod argument_prefix {

        use crate::shared::lowering_error::LoweringError;
        use crate::shared::lowering_error::unsupported;
        use semantic_vocabulary::CanonicalStructuralPathSegment;
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralFieldType,
            StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
            find_structural_type,
        };

        pub(crate) fn captured_crash_argument(
            checked: &checked_trees::CheckedTrees,
            coordinate: crate::emission::operation_emission::buffer::SourceCallCoordinate,
            formal_position: u32,
            argument: &StructuralArgument,
            captures: &[lowered_psi::LoweredSourceLoanCapture],
            operations: &[terminal_psi::Operation],
            structural_types: &[StructuralTypeDeclaration],
            root_type: &mut impl FnMut(
                &StructuralArgument,
            )
                -> Result<semantic_vocabulary::StructuralTypeId, LoweringError>,
        ) -> Result<StructuralArgument, LoweringError> {
            let Some((StructuralPathSegment::Referent, suffix)) = argument.path.split_first()
            else {
                return Ok(argument.clone());
            };
            let flow = &checked.facts.flow;
            let state = flow.control.state_by_symbol(coordinate.state).ok_or(
                LoweringError::Unsupported("crash observation has no checked caller state"),
            )?;
            let call_handle = state
                .calls
                .handles()
                .find(|handle| {
                    let call = flow.control.calls.get(*handle);
                    call.statement_index == coordinate.statement_index
                        && call.call_ordinal == coordinate.call_ordinal
                })
                .ok_or(LoweringError::Unsupported(
                    "crash observation has no exact checked invocation",
                ))?;
            let call = flow.control.calls.get(call_handle);
            let receiver_offset = usize::from(call.authored_receiver.is_valid());
            let owner_operand = call
                .operands
                .handles()
                .nth(formal_position as usize + receiver_offset)
                .ok_or(LoweringError::Unsupported(
                    "crash observation has no captured actual",
                ))?;
            let borrow = &checked.facts.borrow;
            let mut resources = borrow
                .call_loan_resources
                .iter()
                .filter_map(|(_, resource)| {
                    (resource.call == call_handle
                        && resource.owner_operand == owner_operand
                        && resource.capture_path.is_empty())
                    .then_some(resource)
                });
            let resource = resources.next().ok_or(LoweringError::Unsupported(
                "reference crash observation has no captured loan resource",
            ))?;
            if resources.next().is_some() {
                return unsupported("reference crash observation has ambiguous loan resources");
            }
            let loan = match resource.source {
                checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => loan,
                checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                    let Some(loan) = borrow
                        .call_loan_parents
                        .span(parents)
                        .and_then(|parents| parents.first())
                    else {
                        return observed_referent(
                            carried_formation_source(argument, operations)?,
                            suffix,
                            argument.access,
                            structural_types,
                            root_type,
                        );
                    };
                    *loan
                }
            };
            let mut pending = vec![loan];
            let mut visited = Vec::new();
            let mut captured = None;
            while let Some(loan) = pending.pop() {
                if visited.contains(&loan) {
                    continue;
                }
                visited.push(loan);
                let mut selected = captures.iter().filter(|capture| capture.loan == loan);
                if let Some(capture) = selected.next() {
                    if selected.any(|other| other != capture)
                        || captured.is_some_and(|previous| previous != capture)
                    {
                        return unsupported(
                            "reference crash observation has conflicting formation captures",
                        );
                    }
                    captured = Some(capture);
                    continue;
                }
                let occurrence = borrow.loans.get(loan);
                let suppliers = borrow
                    .loan_suppliers
                    .span_or_empty(occurrence.supplying_loans);
                if suppliers.is_empty() {
                    return unsupported(
                        "reference crash observation has no emitted formation capture",
                    );
                }
                for supplier in suppliers {
                    let supplying = borrow.loans.get(*supplier);
                    if occurrence.incoming_reference != supplying.incoming_reference
                        || borrow.access_segments.span(occurrence.incoming_owner_path)
                            != borrow.access_segments.span(supplying.incoming_owner_path)
                        || occurrence.root != supplying.root
                        || borrow.access_segments.span_or_empty(occurrence.segments)
                            != borrow.access_segments.span_or_empty(supplying.segments)
                    {
                        return unsupported(
                            "reference crash transport changes its captured endpoint",
                        );
                    }
                    pending.push(*supplier);
                }
            }
            let observed = observed_referent(
                &captured
                    .ok_or(LoweringError::Unsupported(
                        "reference crash observation has no resolved formation capture",
                    ))?
                    .source,
                suffix,
                argument.access,
                structural_types,
                root_type,
            )?;
            Ok(observed)
        }

        fn carried_formation_source<'operations>(
            argument: &StructuralArgument,
            operations: &'operations [terminal_psi::Operation],
        ) -> Result<&'operations StructuralArgument, LoweringError> {
            let mut formations = operations.iter().filter_map(|operation| {
                operation
                    .result
                    .structural()
                    .filter(|result| result.place == argument.place)
                    .and(match &operation.kind {
                        terminal_psi::OperationKind::EstablishAccessReference {
                            source, ..
                        }
                        | terminal_psi::OperationKind::EstablishReference { source, .. }
                        | terminal_psi::OperationKind::EstablishElementView { source, .. }
                        | terminal_psi::OperationKind::EstablishMutableElementView {
                            source, ..
                        } => Some(source),
                        _ => None,
                    })
            });
            let source = formations.next().ok_or(LoweringError::Unsupported(
                "reference crash observation has no carried formation source",
            ))?;
            if formations.next().is_some() {
                return unsupported("reference crash observation has ambiguous formation sources");
            }
            Ok(source)
        }

        fn observed_referent(
            source: &StructuralArgument,
            suffix: &[StructuralPathSegment],
            access: StructuralAccess,
            structural_types: &[StructuralTypeDeclaration],
            root_type: &mut impl FnMut(
                &StructuralArgument,
            )
                -> Result<semantic_vocabulary::StructuralTypeId, LoweringError>,
        ) -> Result<StructuralArgument, LoweringError> {
            let mut observed = source.clone();
            observed.path = terminal_semantics::reference_source_storage_path(
                structural_types.iter(),
                root_type(&observed)?,
                &observed.path,
            )
            .to_vec();
            observed.path.extend_from_slice(suffix);
            observed.access = access;
            Ok(observed)
        }

        pub(crate) fn structural_crash_route_prefix_from_type(
            argument: &StructuralArgument,
            mut structural_type: semantic_vocabulary::StructuralTypeId,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<Vec<CanonicalStructuralPathSegment>, LoweringError> {
            let mut prefix = Vec::with_capacity(argument.path.len());
            let mut selected_case: Option<&terminal_psi::StructuralCaseDeclaration> = None;
            for (position, segment) in argument.path.iter().enumerate() {
                let declaration = find_structural_type(structural_types, structural_type).ok_or(
                    LoweringError::Unsupported(
                        "structural crash route argument path type is absent",
                    ),
                )?;
                if selected_case.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
                    return unsupported(
                        "structural crash route case must select its payload field",
                    );
                }
                match segment {
                    StructuralPathSegment::AllElements => {
                        return unsupported("universal qualification is not a crash operand path");
                    }
                    StructuralPathSegment::RepresentationView { .. } => {
                        return unsupported(
                            "returned view origins cannot create canonical crash predicate places",
                        );
                    }
                    StructuralPathSegment::Case(selected) => {
                        let cases = match &declaration.shape {
                            StructuralTypeShape::Sum { cases }
                            | StructuralTypeShape::Mixed { cases, .. } => cases,
                            _ => {
                                return unsupported(
                                    "structural crash route case has no source sum",
                                );
                            }
                        };
                        let mut matches = cases.iter().filter(|case| case.id == *selected);
                        let case = matches.next().ok_or(LoweringError::Unsupported(
                            "structural crash route case belongs to another declaration",
                        ))?;
                        if matches.next().is_some() {
                            return unsupported("structural crash route case is ambiguous");
                        }
                        selected_case = Some(case);
                        prefix.push(CanonicalStructuralPathSegment::Case(case.id));
                    }
                    StructuralPathSegment::FixedRange { .. } => {
                        return unsupported(
                            "byte window crash predicates require exact view extent custody",
                        );
                    }
                    StructuralPathSegment::Referent => {
                        if selected_case.is_some() {
                            return unsupported("reference predicate path has an unfinished case");
                        }
                        let StructuralTypeShape::Reference { referent, access } = declaration.shape
                        else {
                            return unsupported(
                                "reference predicate prefix requires a reference carrier",
                            );
                        };
                        if access == StructuralAccess::WriteOnlyBorrow {
                            return unsupported(
                                "reference predicate prefix requires readable access",
                            );
                        }
                        prefix.push(CanonicalStructuralPathSegment::Referent);
                        structural_type = referent;
                    }
                    StructuralPathSegment::RuntimeIndex { .. } => {
                        return unsupported(
                            "runtime-indexed crash predicates have no canonical element prefix",
                        );
                    }
                    StructuralPathSegment::Field(identity) => {
                        let fields = if let Some(case) = selected_case.take() {
                            &case.fields
                        } else {
                            match &declaration.shape {
                                terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                                _ => {
                                    return unsupported(
                                        "structural crash route argument path receiver is not a record",
                                    );
                                }
                            }
                        };
                        let field = terminal_psi::field_by_identity(fields, *identity)
                            .filter(|field| !field.relevance.is_erased())
                            .ok_or(LoweringError::Unsupported(
                                "structural crash route argument field is absent or erased",
                            ))?;
                        prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                        match field.field_type {
                            StructuralFieldType::Structural(next) => structural_type = next,
                            StructuralFieldType::ByteSequence(
                                ByteSequenceCarrier::BoundedOwned { .. },
                            ) if position + 1 == argument.path.len()
                                && argument.access == StructuralAccess::MutableBorrow =>
                            {
                                return Ok(prefix);
                            }
                            _ => {
                                return unsupported(
                                    "structural crash route argument field is not structural",
                                );
                            }
                        }
                    }
                    StructuralPathSegment::FixedIndex(index) => {
                        let StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        } = declaration.shape
                        else {
                            return unsupported(
                                "structural crash route argument fixed index receiver is not an array",
                            );
                        };
                        if *index >= length {
                            return unsupported(
                                "structural crash route argument fixed index is out of bounds",
                            );
                        }
                        prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                        structural_type = element;
                    }
                }
            }
            if selected_case.is_some() {
                return unsupported("structural crash route case has no payload field");
            }
            Ok(prefix)
        }
    }
    pub(crate) mod crash_predicates {

        use crate::proofs::contract_predicates::equality_from_polarities;
        use crate::proofs::crash_routes::scalar_terms::{
            checked_boolean_scalar_term, checked_scalar_term,
        };
        use crate::proofs::{
            CheckedBooleanExpression, CheckedScalarExpression, LoweringError, Proposition,
            ScalarTerm, ValueDeclaration, unsupported,
        };
        use checked_trees::CheckedIeeeFloatComparisonKind;

        pub(crate) fn flatten_checked_boolean_connective<'expression>(
            expression: &'expression CheckedBooleanExpression,
            conjunction: bool,
            output: &mut Vec<&'expression CheckedBooleanExpression>,
        ) {
            match expression {
                CheckedBooleanExpression::And { left, right } if conjunction => {
                    flatten_checked_boolean_connective(left, conjunction, output);
                    flatten_checked_boolean_connective(right, conjunction, output);
                }
                CheckedBooleanExpression::Or { left, right } if !conjunction => {
                    flatten_checked_boolean_connective(left, conjunction, output);
                    flatten_checked_boolean_connective(right, conjunction, output);
                }
                expression => output.push(expression),
            }
        }

        pub(crate) fn checked_boolean_proposition(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
            values: &[ValueDeclaration],
        ) -> Result<Proposition, LoweringError> {
            let mut remaining = boolean_input_budget(expression)?;
            checked_boolean_proposition_with_budget(
                identities,
                expression,
                values,
                &mut remaining,
                0,
            )
        }

        pub(crate) fn boolean_input_budget(
            expression: &CheckedBooleanExpression,
        ) -> Result<usize, LoweringError> {
            let mut remaining = 4096;
            let mut pending = vec![(expression, 0)];
            while let Some((expression, depth)) = pending.pop() {
                charge_boolean_expansion(&mut remaining, depth)?;
                match expression {
                    CheckedBooleanExpression::Not(operand) => pending.push((operand, depth + 1)),
                    facts::checked_boolean_triple_pattern!(left, right, ..) => {
                        pending.extend([(left.as_ref(), depth + 1), (right.as_ref(), depth + 1)]);
                    }
                    _ => {}
                }
            }
            Ok(remaining)
        }

        pub(crate) fn charge_boolean_expansion(
            remaining: &mut usize,
            depth: usize,
        ) -> Result<(), LoweringError> {
            if depth >= 64 {
                return unsupported("crash Boolean expansion exceeds its depth limit");
            }
            *remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
                "crash Boolean expansion exceeds its lowering budget",
            ))?;
            Ok(())
        }

        pub(crate) fn checked_boolean_proposition_with_budget(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
            values: &[ValueDeclaration],
            remaining: &mut usize,
            depth: usize,
        ) -> Result<Proposition, LoweringError> {
            charge_boolean_expansion(remaining, depth)?;
            match expression {
                CheckedBooleanExpression::Not(operand) if contains_boolean_connective(operand) => {
                    checked_boolean_connective_polarity(
                        identities,
                        operand,
                        values,
                        false,
                        remaining,
                        depth + 1,
                    )
                }
                CheckedBooleanExpression::Not(operand)
                    if let CheckedBooleanExpression::ScalarIeeeFloatComparison {
                        kind,
                        left,
                        right,
                    } = operand.as_ref() =>
                {
                    scalar_ieee_float_comparison_proposition(
                        identities,
                        negated_float_comparison_kind(*kind),
                        left,
                        right,
                        values,
                    )
                }
                CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } => {
                    scalar_ieee_float_comparison_proposition(identities, *kind, left, right, values)
                }
                CheckedBooleanExpression::Equal { .. }
                    if contains_boolean_connective(expression) =>
                {
                    checked_boolean_connective_polarity(
                        identities,
                        expression,
                        values,
                        true,
                        remaining,
                        depth + 1,
                    )
                }
                CheckedBooleanExpression::Constant(value) => Ok(if *value {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                }),
                CheckedBooleanExpression::And { left, right }
                | CheckedBooleanExpression::Or { left, right } => {
                    let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
                    let mut leaves = Vec::new();
                    flatten_checked_boolean_connective(left, conjunction, &mut leaves);
                    flatten_checked_boolean_connective(right, conjunction, &mut leaves);
                    let propositions = leaves
                        .into_iter()
                        .map(|leaf| {
                            checked_boolean_proposition_with_budget(
                                identities,
                                leaf,
                                values,
                                remaining,
                                depth + 1,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    crate::proofs::contract_predicates::canonical_connectives(
                        propositions,
                        conjunction,
                    )
                }
                expression => {
                    let mut left =
                        checked_boolean_scalar_term(identities, expression, values, &[])?;
                    let mut right = ScalarTerm::boolean(true);
                    if left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    Ok(Proposition::Equal(left, right))
                }
            }
        }

        pub(crate) fn contains_boolean_connective(expression: &CheckedBooleanExpression) -> bool {
            match expression {
                CheckedBooleanExpression::And { .. } | CheckedBooleanExpression::Or { .. } => true,
                CheckedBooleanExpression::Not(operand) => contains_boolean_connective(operand),
                CheckedBooleanExpression::Equal { left, right } => {
                    contains_boolean_connective(left) || contains_boolean_connective(right)
                }
                _ => false,
            }
        }

        fn checked_boolean_connective_polarity(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
            values: &[ValueDeclaration],
            positive: bool,
            remaining: &mut usize,
            depth: usize,
        ) -> Result<Proposition, LoweringError> {
            charge_boolean_expansion(remaining, depth)?;
            match expression {
                CheckedBooleanExpression::Constant(value) => Ok(if *value == positive {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                }),
                CheckedBooleanExpression::Not(operand) => checked_boolean_connective_polarity(
                    identities,
                    operand,
                    values,
                    !positive,
                    remaining,
                    depth + 1,
                ),
                CheckedBooleanExpression::Equal { left, right } => {
                    match (left.as_ref(), right.as_ref()) {
                        (CheckedBooleanExpression::Constant(value), operand)
                        | (operand, CheckedBooleanExpression::Constant(value)) => {
                            checked_boolean_connective_polarity(
                                identities,
                                operand,
                                values,
                                *value == positive,
                                remaining,
                                depth + 1,
                            )
                        }
                        _ if contains_boolean_connective(expression) => equality_from_polarities(
                            left,
                            right,
                            positive,
                            remaining,
                            |operand, polarity, budget| {
                                checked_boolean_connective_polarity(
                                    identities,
                                    operand,
                                    values,
                                    polarity,
                                    budget,
                                    depth + 1,
                                )
                            },
                        ),
                        _ => checked_boolean_atom_polarity(
                            identities,
                            expression,
                            values,
                            positive,
                            remaining,
                            depth + 1,
                        ),
                    }
                }
                CheckedBooleanExpression::And { left, right }
                | CheckedBooleanExpression::Or { left, right } => {
                    let conjunction =
                        matches!(expression, CheckedBooleanExpression::And { .. }) == positive;
                    let propositions = [left.as_ref(), right.as_ref()]
                        .into_iter()
                        .map(|operand| {
                            checked_boolean_connective_polarity(
                                identities,
                                operand,
                                values,
                                positive,
                                remaining,
                                depth + 1,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    crate::proofs::contract_predicates::canonical_connectives(
                        propositions,
                        conjunction,
                    )
                }
                _ => checked_boolean_atom_polarity(
                    identities,
                    expression,
                    values,
                    positive,
                    remaining,
                    depth + 1,
                ),
            }
        }

        pub(crate) fn checked_boolean_atom_polarity(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
            values: &[ValueDeclaration],
            positive: bool,
            remaining: &mut usize,
            depth: usize,
        ) -> Result<Proposition, LoweringError> {
            charge_boolean_expansion(remaining, depth)?;
            if let CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } =
                expression
            {
                return scalar_ieee_float_comparison_proposition(
                    identities,
                    if positive {
                        *kind
                    } else {
                        negated_float_comparison_kind(*kind)
                    },
                    left,
                    right,
                    values,
                );
            }
            if positive {
                return checked_boolean_proposition_with_budget(
                    identities,
                    expression,
                    values,
                    remaining,
                    depth + 1,
                );
            }
            let mut left = ScalarTerm::boolean_not(checked_boolean_scalar_term(
                identities,
                expression,
                values,
                &[],
            )?)
            .map_err(LoweringError::InvalidCrashPredicate)?;
            let mut right = ScalarTerm::boolean(true);
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Ok(Proposition::Equal(left, right))
        }

        fn negated_float_comparison_kind(
            kind: CheckedIeeeFloatComparisonKind,
        ) -> CheckedIeeeFloatComparisonKind {
            kind.negated()
        }

        fn scalar_ieee_float_comparison_proposition(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            kind: CheckedIeeeFloatComparisonKind,
            left: &CheckedScalarExpression,
            right: &CheckedScalarExpression,
            values: &[ValueDeclaration],
        ) -> Result<Proposition, LoweringError> {
            crate::proofs::contract_predicates::ieee_float_proposition(
                kind,
                checked_scalar_term(identities, left, values, &[])?,
                checked_scalar_term(identities, right, values, &[])?,
            )
        }
    }
    pub(crate) mod scalar_terms {

        use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
        use crate::emission::operation_emission::expressions::LoweredDirectExpression;
        use crate::emission::operation_emission::integer::{
            LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
        };
        use crate::expression_preparation::bindings::ScalarBindings;
        use crate::expression_preparation::saturating_shift::expansion;
        use crate::proofs::{
            CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
            LoweringError, ScalarTerm, ScalarType, ValueDeclaration,
            lower_checked_scalar_expression, unsupported,
        };

        pub(crate) fn checked_boolean_scalar_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
            values: &[ValueDeclaration],
            erased: &[ValueDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            Ok(match expression {
                CheckedBooleanExpression::ErasedParameter { position } => {
                    let value = erased.get(*position).ok_or(LoweringError::Unsupported(
                        "erased formal position is outside the contract's erased roster",
                    ))?;
                    if value.scalar_type != ScalarType::Boolean {
                        return unsupported("erased formal predicate value has a non-Boolean type");
                    }
                    ScalarTerm::value(value.id, value.scalar_type)
                }
                CheckedBooleanExpression::Constant(value) => ScalarTerm::boolean(*value),
                CheckedBooleanExpression::StorageRead { .. } => {
                    return unsupported(
                        "crash predicate requires an established scalar value, not mutable storage",
                    );
                }
                CheckedBooleanExpression::Parameter { position }
                | CheckedBooleanExpression::Local { position } => {
                    let value = values.get(*position).ok_or(LoweringError::Unsupported(
                        "crash predicate value position is outside the selected scalar namespace",
                    ))?;
                    if value.scalar_type != ScalarType::Boolean {
                        return unsupported("crash predicate Boolean value has a non-Boolean type");
                    }
                    ScalarTerm::value(value.id, value.scalar_type)
                }
                CheckedBooleanExpression::StructuralParameterField { .. } => {
                    return unsupported(
                        "structural crash predicates require structural signature lowering",
                    );
                }
                CheckedBooleanExpression::Not(operand) => ScalarTerm::boolean_not(
                    checked_boolean_scalar_term(identities, operand, values, erased)?,
                )
                .map_err(LoweringError::InvalidCrashPredicate)?,
                CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                    checked_boolean_scalar_term(identities, left, values, erased)?,
                    checked_boolean_scalar_term(identities, right, values, erased)?,
                )
                .map_err(LoweringError::InvalidCrashPredicate)?,
                CheckedBooleanExpression::IntegerMathComparison { .. } => {
                    return unsupported(
                        "mathematical comparison is a proof proposition, not a scalar term",
                    );
                }
                CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                    let left = checked_scalar_term(identities, left, values, erased)?;
                    let right = checked_scalar_term(identities, right, values, erased)?;
                    let Some(integer_type) = left.scalar_type().as_integer() else {
                        return unsupported("crash comparison operand is not an integer");
                    };
                    checked_integer_comparison(*kind, integer_type, left, right)
                        .map_err(LoweringError::InvalidCrashPredicate)?
                }
                CheckedBooleanExpression::ScalarIeeeFloatComparison { .. } => {
                    return unsupported(
                        "scalar IEEE equality is an atomic proposition, not a scalar term",
                    );
                }
                CheckedBooleanExpression::IeeeFloatComparison { .. }
                | CheckedBooleanExpression::StructuralIdentity { .. }
                | CheckedBooleanExpression::ByteSequenceEqual { .. }
                | CheckedBooleanExpression::ByteSequencePredicate { .. }
                | CheckedBooleanExpression::StructuralCaseMembership { .. } => {
                    return unsupported(
                        "structural equality requires structural signature lowering",
                    );
                }
                CheckedBooleanExpression::And { .. } | CheckedBooleanExpression::Or { .. } => {
                    return unsupported(
                        "short-circuit Boolean crash predicate is not one scalar terminal term",
                    );
                }
            })
        }

        pub(crate) fn checked_integer_comparison(
            kind: CheckedIntegerComparisonKind,
            integer_type: semantic_vocabulary::IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Result<ScalarTerm, semantic_vocabulary::PropositionError> {
            match kind {
                CheckedIntegerComparisonKind::Equal => {
                    ScalarTerm::integer_equal(integer_type, left, right)
                }
                CheckedIntegerComparisonKind::LessThan => {
                    ScalarTerm::integer_less_than(integer_type, left, right)
                }
                CheckedIntegerComparisonKind::LessOrEqual => {
                    ScalarTerm::integer_less_or_equal(integer_type, left, right)
                }
            }
        }

        pub(crate) fn checked_scalar_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedScalarExpression,
            values: &[ValueDeclaration],
            erased: &[ValueDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            let expression = lower_checked_scalar_expression(identities, expression, &[])?;
            lowered_direct_scalar_term(&expression, values, erased)
        }

        pub(crate) fn checked_scalar_term_with_views(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedScalarExpression,
            values: &[ValueDeclaration],
            erased: &[ValueDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            let scalar_bindings = ScalarBindings::new(values.iter().map(|value| value.scalar_type));
            let expression =
                crate::expression_preparation::prepare_expression::PreparedExpressionContext {
                    structural_fields: &Default::default(),
                    structural_cases: &[],
                    primitive_storage: &[],
                    view_locals: &[],
                    structural_locals: &[],
                    scalar_bindings: Some(&scalar_bindings),
                    proven_indexes: &[],
                }
                .lower_scalar(identities, expression)?;
            lowered_direct_scalar_term(&expression, values, erased)
        }

        pub(crate) fn lowered_erased_scalar_term(
            expression: &LoweredDirectExpression,
            values: &[ValueDeclaration],
            erased: &[ValueDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            if let LoweredDirectExpression::StructuralField {
                source,
                path,
                field,
                scalar_type,
                ..
            } = expression
            {
                let (root, mut path) = source.field_origin(path);
                path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    *field,
                ));
                return Ok(match scalar_type {
                    ScalarType::Boolean => ScalarTerm::boolean_field_path(root, path),
                    ScalarType::Integer(integer) => {
                        ScalarTerm::integer_field_path(root, path, *integer)
                    }
                    ScalarType::IeeeFloat(format) => {
                        ScalarTerm::ieee_float_field_path(root, path, *format)
                    }
                });
            }
            lowered_direct_scalar_term(expression, values, erased)
        }

        pub(crate) fn lowered_erased_scalar_terms(
            expressions: &[LoweredDirectExpression],
            values: &[ValueDeclaration],
            erased: &[ValueDeclaration],
        ) -> Result<Vec<ScalarTerm>, LoweringError> {
            expressions
                .iter()
                .map(|expression| lowered_erased_scalar_term(expression, values, erased))
                .collect()
        }

        pub(crate) fn lowered_direct_scalar_term(
            expression: &LoweredDirectExpression,
            values: &[ValueDeclaration],
            erased: &[ValueDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            Ok(match expression {
                LoweredDirectExpression::ScalarApplication {
                    application,
                    scalar_type,
                } => ScalarTerm::Application {
                    application: *application,
                    scalar_type: *scalar_type,
                },
                LoweredDirectExpression::ErasedParameter {
                    position,
                    scalar_type,
                } => {
                    let value = erased.get(*position).ok_or(LoweringError::Unsupported(
                        "erased formal position is outside the contract's erased roster",
                    ))?;
                    if value.scalar_type != *scalar_type {
                        return unsupported(
                            "erased formal term type does not match its checked plan",
                        );
                    }
                    ScalarTerm::value(value.id, *scalar_type)
                }
                LoweredDirectExpression::PrimitiveRead { .. }
                | LoweredDirectExpression::IndexedPrimitiveRead { .. } => {
                    return unsupported(
                        "primitive storage read requires an occurrence-bound crash predicate",
                    );
                }
                LoweredDirectExpression::StructuralField { .. }
                | LoweredDirectExpression::AtomicLoad { .. } => {
                    return unsupported(
                        "runtime field read requires an occurrence-bound crash predicate",
                    );
                }
                LoweredDirectExpression::ImplementationSelection { .. } => {
                    return unsupported("an implementation selector is not a crash predicate term");
                }
                LoweredDirectExpression::ByteSequenceLength {
                    source,
                    path,
                    scalar_type,
                    ..
                }
                | LoweredDirectExpression::ElementViewLength {
                    source,
                    path,
                    scalar_type,
                    ..
                } => {
                    if !path.is_empty() {
                        return unsupported(
                            "projected crash extent requires its canonical descriptor path",
                        );
                    }
                    let Some(extent_type) = scalar_type.as_integer() else {
                        return unsupported("view extent observation has a non-integer carrier");
                    };
                    ScalarTerm::ViewExtent {
                        root: *source,
                        path: Vec::new(),
                        scalar_type: extent_type,
                    }
                }
                LoweredDirectExpression::ByteSequenceFieldLength { .. }
                | LoweredDirectExpression::ByteSequenceFieldRead { .. }
                | LoweredDirectExpression::ElementViewRead { .. } => {
                    return unsupported("view observation has no retained crash predicate term");
                }
                LoweredDirectExpression::Parameter {
                    position,
                    scalar_type,
                }
                | LoweredDirectExpression::Local {
                    position,
                    scalar_type,
                } => {
                    let value = values.get(*position).ok_or(LoweringError::Unsupported(
                        "crash predicate value position is outside the selected scalar namespace",
                    ))?;
                    if value.scalar_type != *scalar_type {
                        return unsupported(
                            "crash predicate value type does not match its checked plan",
                        );
                    }
                    ScalarTerm::value(value.id, *scalar_type)
                }
                LoweredDirectExpression::IntegerLiteral { value, scalar_type } => {
                    let Some(integer_type) = scalar_type.as_integer() else {
                        return unsupported(
                            "crash predicate integer literal has a non-integer type",
                        );
                    };
                    ScalarTerm::integer(integer_type, *value)
                        .map_err(LoweringError::InvalidCrashPredicate)?
                }
                LoweredDirectExpression::SaturatingShiftLeft {
                    scalar_type,
                    left,
                    right,
                } => {
                    let expanded = expansion(*scalar_type, *left.clone(), *right.clone());
                    lowered_direct_scalar_term(&expanded, values, erased)?
                }
                LoweredDirectExpression::IntegerBinary {
                    kind,
                    scalar_type,
                    left,
                    right,
                    ..
                } => {
                    let Some(integer_type) = scalar_type.as_integer() else {
                        return unsupported("crash predicate arithmetic has a non-integer type");
                    };
                    let left = lowered_direct_scalar_term(left, values, erased)?;
                    let right = lowered_direct_scalar_term(right, values, erased)?;
                    match kind {
                        LoweredIntegerBinaryKind::WrappingShiftLeft => {
                            let Some(count_type) = right.scalar_type().as_integer() else {
                                return unsupported("crash shift count is not an integer");
                            };
                            ScalarTerm::wrapping_integer_shift_left(
                                integer_type,
                                count_type,
                                left,
                                right,
                            )
                            .map_err(LoweringError::InvalidCrashPredicate)?
                        }
                        LoweredIntegerBinaryKind::WrappingShiftRight => {
                            let Some(count_type) = right.scalar_type().as_integer() else {
                                return unsupported("crash shift count is not an integer");
                            };
                            ScalarTerm::wrapping_integer_shift_right(
                                integer_type,
                                count_type,
                                left,
                                right,
                            )
                            .map_err(LoweringError::InvalidCrashPredicate)?
                        }
                        LoweredIntegerBinaryKind::ExactShiftLeft => {
                            let Some(count_type) = right.scalar_type().as_integer() else {
                                return unsupported("crash shift count is not an integer");
                            };
                            ScalarTerm::exact_integer_shift_left(
                                integer_type,
                                count_type,
                                left,
                                right,
                            )
                            .map_err(LoweringError::InvalidCrashPredicate)?
                        }
                        LoweredIntegerBinaryKind::ExactShiftRight => {
                            let Some(count_type) = right.scalar_type().as_integer() else {
                                return unsupported("crash shift count is not an integer");
                            };
                            ScalarTerm::exact_integer_shift_right(
                                integer_type,
                                count_type,
                                left,
                                right,
                            )
                            .map_err(LoweringError::InvalidCrashPredicate)?
                        }
                        LoweredIntegerBinaryKind::TrappingAdd
                        | LoweredIntegerBinaryKind::TrappingSubtract
                        | LoweredIntegerBinaryKind::TrappingMultiply
                        | LoweredIntegerBinaryKind::TrappingDivide
                        | LoweredIntegerBinaryKind::TrappingRemainder
                        | LoweredIntegerBinaryKind::TrappingShiftLeft
                        | LoweredIntegerBinaryKind::TrappingShiftRight => {
                            return unsupported(
                                "Trapping arithmetic forms no crash predicate term",
                            );
                        }
                        kind => {
                            let constructor: fn(
                                semantic_vocabulary::IntegerType,
                                ScalarTerm,
                                ScalarTerm,
                            ) -> Result<
                                ScalarTerm,
                                semantic_vocabulary::PropositionError,
                            > = match kind {
                                LoweredIntegerBinaryKind::BitwiseAnd => {
                                    ScalarTerm::integer_bitwise_and
                                }
                                LoweredIntegerBinaryKind::BitwiseOr => {
                                    ScalarTerm::integer_bitwise_or
                                }
                                LoweredIntegerBinaryKind::BitwiseXor => {
                                    ScalarTerm::integer_bitwise_xor
                                }
                                LoweredIntegerBinaryKind::ExactAdd => ScalarTerm::exact_integer_add,
                                LoweredIntegerBinaryKind::ExactSubtract => {
                                    ScalarTerm::exact_integer_subtract
                                }
                                LoweredIntegerBinaryKind::ExactMultiply => {
                                    ScalarTerm::exact_integer_multiply
                                }
                                LoweredIntegerBinaryKind::ExactDivide => {
                                    ScalarTerm::exact_integer_divide
                                }
                                LoweredIntegerBinaryKind::ExactRemainder => {
                                    ScalarTerm::exact_integer_remainder
                                }
                                LoweredIntegerBinaryKind::WrappingDivide => {
                                    ScalarTerm::wrapping_integer_divide
                                }
                                LoweredIntegerBinaryKind::WrappingRemainder => {
                                    ScalarTerm::wrapping_integer_remainder
                                }
                                LoweredIntegerBinaryKind::SaturatingDivide => {
                                    ScalarTerm::saturating_integer_divide
                                }
                                LoweredIntegerBinaryKind::SaturatingRemainder => {
                                    ScalarTerm::saturating_integer_remainder
                                }
                                LoweredIntegerBinaryKind::WrappingAdd => {
                                    ScalarTerm::wrapping_integer_add
                                }
                                LoweredIntegerBinaryKind::SaturatingAdd => {
                                    ScalarTerm::saturating_integer_add
                                }
                                LoweredIntegerBinaryKind::WrappingSubtract => {
                                    ScalarTerm::wrapping_integer_subtract
                                }
                                LoweredIntegerBinaryKind::SaturatingSubtract => {
                                    ScalarTerm::saturating_integer_subtract
                                }
                                LoweredIntegerBinaryKind::WrappingMultiply => {
                                    ScalarTerm::wrapping_integer_multiply
                                }
                                LoweredIntegerBinaryKind::SaturatingMultiply => {
                                    ScalarTerm::saturating_integer_multiply
                                }
                                _ => unreachable!("shift and Trapping kinds stage their own terms"),
                            };
                            constructor(integer_type, left, right)
                                .map_err(LoweringError::InvalidCrashPredicate)?
                        }
                    }
                }
                LoweredDirectExpression::IntegerBitwiseNot {
                    scalar_type,
                    operand,
                } => {
                    let Some(integer_type) = scalar_type.as_integer() else {
                        return unsupported("crash predicate bitwise-not has a non-integer type");
                    };
                    ScalarTerm::IntegerBitwiseNot {
                        scalar_type: integer_type,
                        operand: Box::new(lowered_direct_scalar_term(operand, values, erased)?),
                    }
                }
                LoweredDirectExpression::IntegerWiden {
                    scalar_type,
                    operand,
                } => {
                    let Some(target_type) = scalar_type.as_integer() else {
                        return unsupported("crash predicate widen has a non-integer target");
                    };
                    let operand = lowered_direct_scalar_term(operand, values, erased)?;
                    let Some(source_type) = operand.scalar_type().as_integer() else {
                        return unsupported("crash predicate widen has a non-integer operand");
                    };
                    ScalarTerm::IntegerWiden {
                        source_type,
                        target_type,
                        operand: Box::new(operand),
                    }
                }
                LoweredDirectExpression::RepresentationView { .. } => {
                    return unsupported("a representation re-view forms no crash predicate term");
                }
                LoweredDirectExpression::IntegerExactCast {
                    scalar_type,
                    operand,
                    ..
                } => {
                    let Some(target_type) = scalar_type.as_integer() else {
                        return unsupported("crash predicate cast has a non-integer target");
                    };
                    let operand = lowered_direct_scalar_term(operand, values, erased)?;
                    let Some(source_type) = operand.scalar_type().as_integer() else {
                        return unsupported("crash predicate cast has a non-integer operand");
                    };
                    ScalarTerm::IntegerExactCast {
                        source_type,
                        target_type,
                        operand: Box::new(operand),
                    }
                }
                LoweredDirectExpression::IeeeFloatLiteral { value } => {
                    ScalarTerm::IeeeFloatLiteral(*value)
                }
                LoweredDirectExpression::Boolean { expression } => {
                    return checked_boolean_scalar_term_from_lowered(expression, values, erased);
                }
            })
        }

        fn checked_boolean_scalar_term_from_lowered(
            expression: &LoweredBooleanReturnExpression,
            values: &[ValueDeclaration],
            erased: &[ValueDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            match expression {
                LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
                | LoweredBooleanReturnExpression::IndexedCaseMembership { .. }
                | LoweredBooleanReturnExpression::ByteSequencePredicate { .. }
                | LoweredBooleanReturnExpression::ByteSequenceEqual { .. }
                | LoweredBooleanReturnExpression::PrimitiveRead { .. } => {
                    unsupported("storage observation requires an occurrence-bound crash predicate")
                }
                LoweredBooleanReturnExpression::Constant { value } => {
                    Ok(ScalarTerm::boolean(*value))
                }
                LoweredBooleanReturnExpression::Parameter { position }
                | LoweredBooleanReturnExpression::Local { position } => {
                    let value = values.get(*position).ok_or(LoweringError::Unsupported(
                        "crash predicate value position is outside the selected scalar namespace",
                    ))?;
                    (value.scalar_type == ScalarType::Boolean)
                        .then(|| ScalarTerm::value(value.id, value.scalar_type))
                        .ok_or(LoweringError::Unsupported(
                            "crash predicate Boolean value has a non-Boolean type",
                        ))
                }
                LoweredBooleanReturnExpression::StructuralField {
                    source,
                    path,
                    field,
                    ..
                } => {
                    let (source, mut path) = source.field_origin(path);
                    path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                        *field,
                    ));
                    Ok(ScalarTerm::boolean_field_path(source, path))
                }
                LoweredBooleanReturnExpression::ScalarObservation { operand } => {
                    lowered_direct_scalar_term(operand, values, erased)
                }
                LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
                    unsupported("unresolved structural field crossed crash-predicate lowering")
                }
                LoweredBooleanReturnExpression::Not { operand } => ScalarTerm::boolean_not(
                    checked_boolean_scalar_term_from_lowered(operand, values, erased)?,
                )
                .map_err(LoweringError::InvalidCrashPredicate),
                LoweredBooleanReturnExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                    checked_boolean_scalar_term_from_lowered(left, values, erased)?,
                    checked_boolean_scalar_term_from_lowered(right, values, erased)?,
                )
                .map_err(LoweringError::InvalidCrashPredicate),
                LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
                    let left = lowered_direct_scalar_term(left, values, erased)?;
                    let right = lowered_direct_scalar_term(right, values, erased)?;
                    let Some(integer_type) = left.scalar_type().as_integer() else {
                        return unsupported("crash comparison operand is not an integer");
                    };
                    match kind {
                        LoweredIntegerComparisonKind::Equal => {
                            ScalarTerm::integer_equal(integer_type, left, right)
                        }
                        LoweredIntegerComparisonKind::LessThan => {
                            ScalarTerm::integer_less_than(integer_type, left, right)
                        }
                        LoweredIntegerComparisonKind::LessOrEqual => {
                            ScalarTerm::integer_less_or_equal(integer_type, left, right)
                        }
                    }
                    .map_err(LoweringError::InvalidCrashPredicate)
                }
                LoweredBooleanReturnExpression::And { .. }
                | LoweredBooleanReturnExpression::Or { .. } => unsupported(
                    "short-circuit Boolean crash predicate is not one scalar terminal term",
                ),
            }
        }
    }
    pub(crate) mod structural_arithmetic {

        use crate::proofs::{
            IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
        };

        pub(crate) fn safe_exact_structural_divisor(
            integer_type: IntegerType,
            dividend: &ScalarTerm,
            divisor: &ScalarTerm,
            requirements: &[Proposition],
        ) -> bool {
            match divisor {
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(value),
                } => return *scalar_type == integer_type && *value != 0,
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Signed(value),
                } => return *scalar_type == integer_type && *value != 0 && *value != -1,
                _ => {}
            }

            let one = integer_type.one();
            if let Ok(one) = ScalarTerm::integer(integer_type, one)
                && requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
            {
                return true;
            }
            if integer_type.sign() != IntegerSign::Signed {
                return false;
            }
            if let Ok(negative_two) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-2))
                && requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_two))
            {
                return true;
            }
            let Ok(negative_one) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-1))
            else {
                return false;
            };
            if !requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_one)) {
                return false;
            }
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed fixed integer has a signed minimum")
            };
            ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
            )
            .is_ok_and(|minimum_plus_one| {
                requirements.contains(&Proposition::LessOrEqual(
                    minimum_plus_one,
                    dividend.clone(),
                ))
            })
        }

        pub(crate) fn safe_policy_structural_divisor(
            integer_type: IntegerType,
            divisor: &ScalarTerm,
            requirements: &[Proposition],
        ) -> bool {
            match divisor {
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(value),
                } => return *scalar_type == integer_type && *value != 0,
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Signed(value),
                } => return *scalar_type == integer_type && *value != 0,
                _ => {}
            }

            let one = integer_type.one();
            if ScalarTerm::integer(integer_type, one).is_ok_and(|one| {
                requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
            }) {
                return true;
            }
            if integer_type.sign() != IntegerSign::Signed {
                return false;
            }
            [IntegerValue::Signed(-1), IntegerValue::Signed(-2)]
                .into_iter()
                .filter_map(|bound| ScalarTerm::integer(integer_type, bound).ok())
                .any(|bound| {
                    requirements.contains(&Proposition::LessOrEqual(divisor.clone(), bound))
                })
        }

        fn nonnegative_shift_count(value: IntegerValue) -> Option<u32> {
            match value {
                IntegerValue::Unsigned(value) => u32::try_from(value).ok(),
                IntegerValue::Signed(value) => u32::try_from(value).ok(),
            }
        }

        fn exact_structural_shift_maximum_count(
            value_type: IntegerType,
            count_type: IntegerType,
            count: &ScalarTerm,
            requirements: &[Proposition],
        ) -> Option<u32> {
            if count.scalar_type() != ScalarType::Integer(count_type) {
                return None;
            }
            if let Some((literal_type, literal)) = count.integer_value() {
                let literal = nonnegative_shift_count(literal)?;
                return (literal_type == count_type && literal < u32::from(value_type.bits()))
                    .then_some(literal);
            }

            if count_type.sign() == IntegerSign::Signed {
                let zero = ScalarTerm::integer(count_type, IntegerValue::Signed(0)).ok()?;
                if !requirements.contains(&Proposition::LessOrEqual(zero, count.clone())) {
                    return None;
                }
            }

            let width = u32::from(value_type.bits());
            let intrinsic_maximum = nonnegative_shift_count(count_type.maximum_value())?;
            if intrinsic_maximum < width {
                return Some(intrinsic_maximum);
            }
            requirements
                .iter()
                .filter_map(|requirement| match requirement {
                    Proposition::LessOrEqual(left, right) if left == count => {
                        let (right_type, right) = right.integer_value()?;
                        let right = nonnegative_shift_count(right)?;
                        (right_type == count_type && right < width).then_some(right)
                    }
                    Proposition::LessThan(left, right) if left == count => {
                        let (right_type, right) = right.integer_value()?;
                        let right = nonnegative_shift_count(right)?;
                        (right_type == count_type && right > 0 && right <= width)
                            .then_some(right - 1)
                    }
                    _ => None,
                })
                .min()
        }

        pub(crate) fn safe_exact_structural_shift(
            left_shift: bool,
            value_type: IntegerType,
            count_type: IntegerType,
            value: &ScalarTerm,
            count: &ScalarTerm,
            requirements: &[Proposition],
        ) -> bool {
            if value.scalar_type() != ScalarType::Integer(value_type) {
                return false;
            }
            let Some(maximum_count) =
                exact_structural_shift_maximum_count(value_type, count_type, count, requirements)
            else {
                return false;
            };
            if !left_shift || maximum_count == 0 {
                return true;
            }
            if let Some((literal_type, literal)) = value.integer_value() {
                let maximum_count_value = match count_type.sign() {
                    IntegerSign::Signed => IntegerValue::Signed(i128::from(maximum_count)),
                    IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(maximum_count)),
                };
                return literal_type == value_type
                    && value_type
                        .exact_shift_left(literal, count_type, maximum_count_value)
                        .is_some();
            }
            match value_type.sign() {
                IntegerSign::Unsigned => {
                    let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                        unreachable!("unsigned fixed integer has an unsigned maximum")
                    };
                    ScalarTerm::integer(
                        value_type,
                        IntegerValue::Unsigned(maximum >> maximum_count),
                    )
                    .is_ok_and(|maximum| {
                        requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
                    })
                }
                IntegerSign::Signed => {
                    let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                        (value_type.minimum_value(), value_type.maximum_value())
                    else {
                        unreachable!("signed fixed integer has signed bounds")
                    };
                    let minimum = ScalarTerm::integer(
                        value_type,
                        IntegerValue::Signed(minimum >> maximum_count),
                    );
                    let maximum = ScalarTerm::integer(
                        value_type,
                        IntegerValue::Signed(maximum >> maximum_count),
                    );
                    minimum.is_ok_and(|minimum| {
                        requirements.contains(&Proposition::LessOrEqual(minimum, value.clone()))
                    }) && maximum.is_ok_and(|maximum| {
                        requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
                    })
                }
            }
        }
    }
    pub(crate) mod structural_members {

        use crate::emission::scalar_types::terminal_scalar_type;
        use crate::emission::structural_observations::declared_predicate_path;
        use crate::emission::structural_observations::explicit_path;
        use crate::proofs::{
            ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatFormat,
            IeeeFloatStructuralField, LoweringError, PlaceId, ScalarTerm, ScalarType,
            StructuralCaseSubject, StructuralFieldType, StructuralParameterDeclaration,
            StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, unsupported,
        };

        pub(crate) fn lower_structural_member_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            parameter_position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<
            (
                PlaceId,
                Vec<CanonicalStructuralPathSegment>,
                StructuralFieldType,
            ),
            LoweringError,
        > {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == parameter_position)
                .ok_or(LoweringError::Unsupported(
                    "structural scalar contract names a non-structural parameter",
                ))?;
            lower_structural_place_path(
                identities,
                parameter.place,
                parameter.structural_type,
                parameter.access,
                path,
                structural_types,
            )
        }

        pub(crate) fn lower_structural_identity(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            left: &checked_trees::CheckedStructuralParameterField,
            right: &checked_trees::CheckedStructuralParameterField,
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<semantic_vocabulary::Proposition, LoweringError> {
            let mut subject = |field: &checked_trees::CheckedStructuralParameterField| {
                let (root, path, endpoint) = lower_structural_member_path(
                    identities,
                    field.parameter_position,
                    &field.path,
                    parameters,
                    structural_types,
                )?;
                let StructuralFieldType::Structural(structural_type) = endpoint else {
                    return unsupported("structural identity requires a complete data endpoint");
                };
                Ok(semantic_vocabulary::StructuralIdentitySubject::new(
                    root,
                    path,
                    structural_type,
                ))
            };
            let mut left = subject(left)?;
            let mut right = subject(right)?;
            if left.structural_type() != right.structural_type() {
                return unsupported("structural identity operands have different exact types");
            }
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Ok(semantic_vocabulary::Proposition::StructuralIdentity { left, right })
        }

        fn lower_structural_place_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            root: PlaceId,
            root_type: StructuralTypeId,
            access: terminal_psi::StructuralAccess,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<
            (
                PlaceId,
                Vec<CanonicalStructuralPathSegment>,
                StructuralFieldType,
            ),
            LoweringError,
        > {
            if access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return unsupported("structural scalar contract requires readable access");
            }
            let selected = declared_predicate_path(identities, root_type, path, structural_types)?;
            Ok((root, selected.path, selected.field_type))
        }

        fn structural_observation_path<'declaration>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            root_type: StructuralTypeId,
            access: terminal_psi::StructuralAccess,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            structural_types: &'declaration [StructuralTypeDeclaration],
        ) -> Result<
            crate::emission::structural_observations::ObservationPath<'declaration>,
            LoweringError,
        > {
            if access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return unsupported("structural scalar contract requires readable access");
            }
            explicit_path(identities, root_type, path, structural_types)
        }

        pub(crate) fn lower_sequence_extent_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            parameter_position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == parameter_position)
                .ok_or(LoweringError::Unsupported(
                    "closed sequence extent names a non-structural parameter",
                ))?;
            let selected = structural_observation_path(
                identities,
                parameter.structural_type,
                parameter.access,
                path,
                structural_types,
            )?;
            require_readable_sequence_extent(selected.field_type()?, structural_types)?;
            let integer = terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?
                .as_integer()
                .ok_or(LoweringError::Unsupported(
                    "sequence extent is not an integer",
                ))?;
            if let Some(path) = selected.canonical {
                return Ok(ScalarTerm::view_extent(parameter.place, path, integer));
            }
            let mut path = selected.observation.ok_or(LoweringError::Unsupported(
                "sequence extent has no exact descriptor observation path",
            ))?;
            path.push(semantic_vocabulary::ScalarReadProjection::Extent);
            ScalarTerm::structural_read(parameter.place, path, ScalarType::Integer(integer))
                .map_err(LoweringError::InvalidCrashPredicate)
        }

        fn require_readable_sequence_extent(
            carrier: StructuralFieldType,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<(), LoweringError> {
            let carrier = match carrier {
                terminal_psi::StructuralFieldType::Structural(identity) => {
                    let declaration =
                        terminal_psi::find_structural_type(structural_types, identity).ok_or(
                            LoweringError::Unsupported("closed sequence extent type is absent"),
                        )?;
                    match &declaration.shape {
                        terminal_psi::StructuralTypeShape::ElementView { .. } => None,
                        terminal_psi::StructuralTypeShape::ByteSequence(carrier) => Some(*carrier),
                        _ => return unsupported("closed sequence extent leaf is not a descriptor"),
                    }
                }
                leaf => Some(
                    leaf.byte_sequence_carrier()
                        .ok_or(LoweringError::Unsupported(
                            "closed sequence extent leaf is not a descriptor",
                        ))?,
                ),
            };
            if matches!(
                carrier,
                Some(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                    } | terminal_psi::ByteSequenceCarrier::BoundedView {
                        access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                        ..
                    }
                )
            ) {
                return unsupported("closed sequence extent requires readable access");
            }
            Ok(())
        }

        pub(crate) fn lower_sequence_extent_projection_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            parameter_position: u32,
            path: &[checked_trees::CheckedScalarReadProjection],
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
            lower_selector: &mut impl FnMut(
                &mut terminal_psi::TerminalIdentityTableBuilder,
                &checked_trees::CheckedScalarExpression,
            ) -> Result<ScalarTerm, LoweringError>,
        ) -> Result<ScalarTerm, LoweringError> {
            if let Some(path) = checked_trees::CheckedScalarReadProjection::static_path(path) {
                return lower_sequence_extent_term(
                    identities,
                    parameter_position,
                    &path,
                    parameters,
                    structural_types,
                );
            }
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == parameter_position)
                .ok_or(LoweringError::Unsupported(
                    "closed sequence extent names a non-structural parameter",
                ))?;
            let (mut path, carrier) = lower_checked_read_projections(
                identities,
                parameter.structural_type,
                parameter.access,
                path,
                structural_types,
                lower_selector,
            )?;
            require_readable_sequence_extent(carrier, structural_types)?;
            path.push(semantic_vocabulary::ScalarReadProjection::Extent);
            let scalar_type = terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
            ScalarTerm::structural_read(parameter.place, path, scalar_type)
                .map_err(LoweringError::InvalidCrashPredicate)
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn lower_structural_indexed_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &checked_trees::CheckedScalarExpression,
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
            lower_selector: &mut impl FnMut(
                &mut terminal_psi::TerminalIdentityTableBuilder,
                &checked_trees::CheckedScalarExpression,
            ) -> Result<ScalarTerm, LoweringError>,
        ) -> Result<ScalarTerm, LoweringError> {
            let checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
                root, ..
            } = expression
            else {
                return unsupported("indexed contract observation has no selected read");
            };
            let position = root.parameter().ok_or(LoweringError::Unsupported(
                "indexed contract observation requires a structural formal",
            ))?;
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == position)
                .ok_or(LoweringError::Unsupported(
                    "indexed contract formal is absent",
                ))?;
            lower_structural_indexed_place(
                identities,
                expression,
                parameter.place,
                parameter.structural_type,
                parameter.access,
                structural_types,
                lower_selector,
            )
        }

        pub(crate) fn lower_structural_result_indexed_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &checked_trees::CheckedScalarExpression,
            result: &terminal_psi::StructuralResultDeclaration,
            structural_types: &[StructuralTypeDeclaration],
            lower_selector: &mut impl FnMut(
                &mut terminal_psi::TerminalIdentityTableBuilder,
                &checked_trees::CheckedScalarExpression,
            ) -> Result<ScalarTerm, LoweringError>,
        ) -> Result<ScalarTerm, LoweringError> {
            lower_structural_indexed_place(
                identities,
                expression,
                result.place,
                result.structural_type,
                terminal_psi::StructuralAccess::Owned,
                structural_types,
                lower_selector,
            )
        }

        fn lower_structural_indexed_place(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &checked_trees::CheckedScalarExpression,
            root: PlaceId,
            root_type: StructuralTypeId,
            access: terminal_psi::StructuralAccess,
            structural_types: &[StructuralTypeDeclaration],
            lower_selector: &mut impl FnMut(
                &mut terminal_psi::TerminalIdentityTableBuilder,
                &checked_trees::CheckedScalarExpression,
            ) -> Result<ScalarTerm, LoweringError>,
        ) -> Result<ScalarTerm, LoweringError> {
            use checked_trees::CheckedScalarReadProjection;
            use semantic_vocabulary::ScalarReadProjection;
            let checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
                path,
                index,
                element_path,
                primitive_type,
                ..
            } = expression
            else {
                return unsupported("indexed contract observation has no selected read");
            };
            let mut projections = path
                .iter()
                .cloned()
                .map(CheckedScalarReadProjection::Static)
                .collect::<Vec<_>>();
            projections.push(CheckedScalarReadProjection::RuntimeIndex((**index).clone()));
            projections.extend_from_slice(element_path);
            let (terminal_path, _) = lower_checked_read_projections(
                identities,
                root_type,
                access,
                &projections,
                structural_types,
                lower_selector,
            )?;
            let actual = terminal_semantics::structural_read_scalar_type(
                structural_types.iter(),
                root_type,
                &terminal_path,
            );
            let expected = terminal_scalar_type(*primitive_type)?;
            if actual != Some(expected) {
                return unsupported(
                    "indexed contract path does not select its retained scalar type",
                );
            }
            let fixed = terminal_path
                .iter()
                .map(|step| match step {
                    ScalarReadProjection::Static(step) => Some(*step),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>();
            let term = match (fixed, expected) {
                (Some(path), ScalarType::Integer(integer)) => {
                    ScalarTerm::integer_field_path(root, path, integer)
                }
                (Some(path), ScalarType::Boolean) => ScalarTerm::boolean_field_path(root, path),
                (Some(path), ScalarType::IeeeFloat(format)) => {
                    ScalarTerm::ieee_float_field_path(root, path, format)
                }
                (None, _) => ScalarTerm::StructuralRead {
                    root,
                    path: terminal_path,
                    scalar_type: expected,
                },
            };
            term.validate()
                .map_err(LoweringError::InvalidCrashPredicate)?;
            Ok(term)
        }

        fn lower_checked_read_projections(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            root_type: StructuralTypeId,
            access: terminal_psi::StructuralAccess,
            projections: &[checked_trees::CheckedScalarReadProjection],
            structural_types: &[StructuralTypeDeclaration],
            lower_selector: &mut impl FnMut(
                &mut terminal_psi::TerminalIdentityTableBuilder,
                &checked_trees::CheckedScalarExpression,
            ) -> Result<ScalarTerm, LoweringError>,
        ) -> Result<
            (
                Vec<semantic_vocabulary::ScalarReadProjection>,
                StructuralFieldType,
            ),
            LoweringError,
        > {
            use checked_trees::CheckedScalarReadProjection;
            use semantic_vocabulary::ScalarReadProjection;
            if access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return unsupported("indexed contract observation requires readable access");
            }
            let mut terminal_path = Vec::new();
            let mut selected = StructuralFieldType::Structural(root_type);
            let mut cursor = 0;
            while cursor < projections.len() {
                if matches!(&projections[cursor], CheckedScalarReadProjection::Static(_)) {
                    let mut static_path = Vec::new();
                    while let Some(CheckedScalarReadProjection::Static(segment)) =
                        projections.get(cursor)
                    {
                        static_path.push(segment.clone());
                        cursor += 1;
                    }
                    let StructuralFieldType::Structural(structural_type) = selected else {
                        return unsupported("indexed contract member follows a scalar leaf");
                    };
                    let observation = structural_observation_path(
                        identities,
                        structural_type,
                        access,
                        &static_path,
                        structural_types,
                    )?;
                    selected = observation.field_type()?;
                    terminal_path.extend(observation.observation.ok_or(
                        LoweringError::Unsupported(
                            "indexed contract member has no retained observation path",
                        ),
                    )?);
                    continue;
                }
                let CheckedScalarReadProjection::RuntimeIndex(selector) = &projections[cursor]
                else {
                    unreachable!();
                };
                let selector = lower_selector(identities, &selector.expression)?;
                if !matches!(selector.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
                {
                    return unsupported("indexed contract selector is not a fixed integer");
                }
                let byte_carrier = match &selected {
                    StructuralFieldType::Structural(identity) => {
                        let declaration =
                            terminal_psi::find_structural_type(structural_types, *identity).ok_or(
                                LoweringError::Unsupported(
                                    "indexed contract collection type is absent",
                                ),
                            )?;
                        match &declaration.shape {
                            StructuralTypeShape::FixedArray { element, .. }
                            | StructuralTypeShape::ElementView { element, .. } => {
                                selected = StructuralFieldType::Structural(*element);
                                None
                            }
                            StructuralTypeShape::ByteSequence(carrier) => Some(*carrier),
                            _ => {
                                return unsupported(
                                    "indexed contract receiver is not a collection",
                                );
                            }
                        }
                    }
                    field => Some(field.byte_sequence_carrier().ok_or(
                        LoweringError::Unsupported("indexed contract receiver is not a collection"),
                    )?),
                };
                if let Some(carrier) = byte_carrier {
                    if matches!(
                        carrier,
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                        } | terminal_psi::ByteSequenceCarrier::BoundedView {
                            access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                            ..
                        }
                    ) {
                        return unsupported(
                            "indexed contract byte observation requires readable access",
                        );
                    }
                    selected = StructuralFieldType::Scalar(terminal_scalar_type(
                        checked_trees::types::PrimitiveType::U8,
                    )?);
                }
                let fixed = match &selector {
                    ScalarTerm::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(value),
                        ..
                    } => u64::try_from(*value).ok(),
                    ScalarTerm::Integer {
                        value: semantic_vocabulary::IntegerValue::Signed(value),
                        ..
                    } => u64::try_from(*value).ok(),
                    _ => None,
                };
                terminal_path.push(match fixed {
                    Some(index) => ScalarReadProjection::Static(
                        CanonicalStructuralPathSegment::FixedIndex(index),
                    ),
                    None => ScalarReadProjection::Index(Box::new(selector)),
                });
                cursor += 1;
            }
            Ok((terminal_path, selected))
        }

        pub(crate) fn lower_structural_member_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            parameter_position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            expected: ScalarType,
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == parameter_position)
                .ok_or(LoweringError::Unsupported(
                    "structural contract has no formal",
                ))?;
            lower_observation_term(
                identities,
                parameter.place,
                parameter.structural_type,
                parameter.access,
                path,
                expected,
                structural_types,
            )
        }

        /*
            A scalar field of a caller's structural local, as the proof-side
            term an erased call actual substitutes for the callee's erased
            scalar lane. It names the local's place; it emits no operation.
        */
        pub(crate) fn lower_structural_local_member_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            home: Option<(PlaceId, StructuralTypeId, terminal_psi::StructuralAccess)>,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            expected: ScalarType,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            let Some((place, structural_type, access)) = home else {
                // A local with no place has no formal to observe either; the
                // formal lookup reports that one missing root.
                return lower_structural_member_term(
                    identities,
                    u32::MAX,
                    path,
                    expected,
                    &[],
                    structural_types,
                );
            };
            lower_observation_term(
                identities,
                place,
                structural_type,
                access,
                path,
                expected,
                structural_types,
            )
        }

        pub(crate) fn lower_structural_result_member_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            result: &terminal_psi::StructuralResultDeclaration,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            expected: ScalarType,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            lower_observation_term(
                identities,
                result.place,
                result.structural_type,
                terminal_psi::StructuralAccess::Owned,
                path,
                expected,
                structural_types,
            )
        }

        fn lower_observation_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            root: PlaceId,
            root_type: StructuralTypeId,
            access: terminal_psi::StructuralAccess,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            expected: ScalarType,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            let selected =
                structural_observation_path(identities, root_type, access, path, structural_types)?;
            let actual = selected.field_type()?;
            if let Some(canonical) = selected.canonical {
                return structural_member_term(root, canonical, actual, expected);
            }
            if actual.scalar_type() != Some(expected) {
                return unsupported(
                    "structural observation contract has a different scalar carrier",
                );
            }
            let path = selected.observation.ok_or(LoweringError::Unsupported(
                "structural observation contract requires evaluated selector terms",
            ))?;
            ScalarTerm::structural_read(root, path, expected)
                .map_err(LoweringError::InvalidCrashPredicate)
        }

        fn structural_member_term(
            root: PlaceId,
            terminal_path: Vec<CanonicalStructuralPathSegment>,
            actual: StructuralFieldType,
            expected: ScalarType,
        ) -> Result<ScalarTerm, LoweringError> {
            if actual.scalar_type() != Some(expected) {
                return unsupported(
                    "structural scalar contract path does not end at the retained scalar type",
                );
            }
            Ok(match expected {
                ScalarType::Boolean => ScalarTerm::boolean_field_path(root, terminal_path),
                ScalarType::Integer(integer_type) => {
                    ScalarTerm::integer_field_path(root, terminal_path, integer_type)
                }
                ScalarType::IeeeFloat(format) => {
                    ScalarTerm::ieee_float_field_path(root, terminal_path, format)
                }
            })
        }

        pub(crate) fn lower_ieee_float_field(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            field: &checked_trees::CheckedStructuralParameterField,
            format: IeeeFloatFormat,
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<IeeeFloatStructuralField, LoweringError> {
            let (root, path, actual) = lower_structural_member_path(
                identities,
                field.parameter_position,
                &field.path,
                parameters,
                structural_types,
            )?;
            if actual != StructuralFieldType::IeeeFloat(format) {
                return unsupported("structural IEEE predicate leaf has the wrong retained format");
            }
            IeeeFloatStructuralField::new(root, path).map_err(LoweringError::InvalidCrashPredicate)
        }

        pub(crate) fn lower_byte_predicate_term(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            predicate: semantic_vocabulary::ByteSequencePredicate,
            root: checked_trees::CheckedStorageRoot,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<ScalarTerm, LoweringError> {
            let position = root.parameter().ok_or(LoweringError::Unsupported(
                "closed byte predicate requires a structural formal",
            ))?;
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == position)
                .ok_or(LoweringError::Unsupported(
                    "byte predicate names a non-structural formal",
                ))?;
            let fixed_bytes = |identity| {
                terminal_psi::find_structural_type(structural_types, identity).is_some_and(|row| {
                    matches!(
                        row.shape,
                        terminal_psi::StructuralTypeShape::FixedArray { .. }
                    ) && terminal_semantics::fixed_byte_extent(structural_types.iter(), identity)
                        .is_some()
                })
            };
            let (root, path, carrier, inline_fixed) = if path.is_empty() {
                let carrier =
                    terminal_psi::find_structural_type(structural_types, parameter.structural_type)
                        .and_then(|declaration| match declaration.shape {
                            terminal_psi::StructuralTypeShape::ByteSequence(carrier) => {
                                Some(carrier)
                            }
                            _ => None,
                        });
                (
                    parameter.place,
                    Vec::new(),
                    carrier,
                    fixed_bytes(parameter.structural_type),
                )
            } else {
                let (root, path, field) =
                    crate::proofs::crash_routes::lower_structural_member_path(
                        identities,
                        position,
                        path,
                        parameters,
                        structural_types,
                    )?;
                (
                    root,
                    path,
                    field.byte_sequence_carrier(),
                    field.as_structural().is_some_and(fixed_bytes),
                )
            };
            let readable = inline_fixed
                || match carrier {
                    Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }) => true,
                    Some(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
                    ) => {
                        matches!(
                            access.unwrap_or(parameter.access),
                            terminal_psi::StructuralAccess::SharedBorrow
                                | terminal_psi::StructuralAccess::MutableBorrow
                        )
                    }
                    _ => false,
                };
            if !readable || parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return unsupported("byte predicate formal is not a readable byte carrier");
            }
            Ok(ScalarTerm::ByteSequencePredicate {
                predicate,
                root,
                path,
            })
        }

        pub(crate) fn lower_byte_sequence_field(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            field: &checked_trees::CheckedStructuralParameterField,
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<ByteSequenceStructuralField, LoweringError> {
            let (root, path, actual) = lower_structural_member_path(
                identities,
                field.parameter_position,
                &field.path,
                parameters,
                structural_types,
            )?;
            if actual.byte_sequence_carrier().is_none() {
                return unsupported(
                    "structural byte-sequence predicate leaf has the wrong retained type",
                );
            }
            ByteSequenceStructuralField::new(root, path)
                .map_err(LoweringError::InvalidCrashPredicate)
        }

        fn lower_sum_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            root: PlaceId,
            structural_type: StructuralTypeId,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<(StructuralCaseSubject, StructuralTypeId), LoweringError> {
            let selected =
                declared_predicate_path(identities, structural_type, path, structural_types)?;
            let StructuralFieldType::Structural(structural_type) = selected.field_type else {
                return unsupported(
                    "structural sum predicate path does not reach a structural value",
                );
            };
            if !matches!(
                terminal_psi::find_structural_type(structural_types, structural_type)
                    .map(|declaration| &declaration.shape),
                Some(terminal_psi::sum_or_mixed_shape_pattern!())
            ) {
                return unsupported("structural sum predicate subject is not a sum");
            }
            Ok((
                StructuralCaseSubject::new(root, selected.path),
                structural_type,
            ))
        }

        pub(crate) fn lower_structural_case_membership(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            subject: &checked_trees::CheckedStructuralCaseSubject,
            case_identity: &checked_trees::CheckedMemberIdentity,
            parameters: &[StructuralParameterDeclaration],
            result: Option<&terminal_psi::StructuralResultDeclaration>,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<semantic_vocabulary::Proposition, LoweringError> {
            let (root, structural_type) = match subject.root {
                checked_trees::CheckedStructuralCaseRoot::Parameter { position } => {
                    let parameter = parameters
                        .iter()
                        .find(|parameter| parameter.position == position)
                        .ok_or(LoweringError::Unsupported(
                            "case membership parameter is absent",
                        ))?;
                    (parameter.place, parameter.structural_type)
                }
                checked_trees::CheckedStructuralCaseRoot::Result => {
                    let result = result.ok_or(LoweringError::Unsupported(
                        "case membership result is outside the contract namespace",
                    ))?;
                    (result.place, result.structural_type)
                }
            };
            let (subject, structural_type) = lower_sum_path(
                identities,
                root,
                structural_type,
                &subject.path,
                structural_types,
            )?;
            let declaration = terminal_psi::find_structural_type(structural_types, structural_type)
                .ok_or(LoweringError::Unsupported(
                    "case membership declaration is absent",
                ))?;
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return unsupported("case membership declaration is not a sum");
            };
            let mut matching = cases
                .iter()
                .filter(|case| case.identity == identities.intern(&case_identity.spelling()));
            let case = matching
                .next()
                .ok_or(LoweringError::Unsupported("case membership was redirected"))?;
            if matching.next().is_some() {
                return unsupported("case membership identity is ambiguous");
            }
            Ok(semantic_vocabulary::Proposition::StructuralCaseMembership {
                subject,
                case: case.id,
            })
        }
    }
    pub(crate) mod structural_route_buckets {

        use crate::proofs::contract_predicates::canonical_connectives;
        use crate::proofs::contract_predicates::canonical_equality;
        use crate::proofs::contract_predicates::connective;
        use crate::proofs::contract_predicates::equality_from_polarities;
        use crate::proofs::contract_predicates::proposition_with_polarity;
        use crate::proofs::crash_routes::crash_predicates::{
            boolean_input_budget, charge_boolean_expansion, contains_boolean_connective,
            flatten_checked_boolean_connective,
        };
        use crate::proofs::crash_routes::scalar_terms::{
            checked_boolean_scalar_term, checked_scalar_term,
        };
        use crate::proofs::crash_routes::structural_members::{
            lower_byte_sequence_field, lower_ieee_float_field, lower_structural_identity,
            lower_structural_member_term,
        };
        use crate::proofs::{
            CanonicalStructuralPathSegment, CheckedBooleanExpression, CheckedScalarExpression,
            IeeeFloatFormat, LoweringError, PlaceId, PrimitiveType, Proposition, ScalarTerm,
            ScalarType, StructuralParameterDeclaration, StructuralTypeDeclaration,
            ValueDeclaration, unsupported,
        };

        pub(crate) fn lower_structural_crash_route_buckets(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            buckets: &[checked_trees::CrashRouteBucket],
            scalar_parameters: &[ValueDeclaration],
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
            runtime_requirements: &[Proposition],
        ) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
            let terms = RouteTerms {
                scalar_parameters,
                parameters,
                structural_types,
                runtime_requirements,
            };
            super::lower_crash_route_buckets(buckets, |predicate| {
                if let Some(expression) = predicate.scalar_expression() {
                    let mut remaining = boolean_input_budget(expression)?;
                    terms.lower_proposition(identities, expression, &mut remaining, 0)
                } else {
                    let mut path = Vec::new();
                    let parameter_position = predicate
                        .expression()
                        .and_then(|expression| checked_member_path(expression, &mut path))
                        .ok_or(LoweringError::Unsupported(
                            "structural crash route is outside checked Boolean member lowering",
                        ))?;
                    Ok(Proposition::Equal(
                        ScalarTerm::boolean(true),
                        lower_structural_member_term(
                            identities,
                            parameter_position,
                            &path
                                .into_iter()
                                .map(checked_trees::CheckedStructuralPredicatePathSegment::Field)
                                .collect::<Vec<_>>(),
                            ScalarType::Boolean,
                            parameters,
                            structural_types,
                        )?,
                    ))
                }
            })
        }

        pub(crate) fn lower_structural_crash_predicates(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            predicates: &[CheckedBooleanExpression],
            scalar_parameters: &[ValueDeclaration],
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
            runtime_requirements: &[Proposition],
        ) -> Result<Vec<terminal_psi::CrashPredicateTerm>, LoweringError> {
            let terms = RouteTerms {
                scalar_parameters,
                parameters,
                structural_types,
                runtime_requirements,
            };
            let mut lowered = predicates
                .iter()
                .map(|predicate| {
                    let mut remaining = boolean_input_budget(predicate)?;
                    terms
                        .lower_proposition(identities, predicate, &mut remaining, 0)
                        .map(terminal_psi::CrashPredicateTerm::new)
                })
                .collect::<Result<Vec<_>, _>>()?;
            lowered.sort();
            lowered.dedup();
            Ok(lowered)
        }

        struct RouteTerms<'a> {
            scalar_parameters: &'a [ValueDeclaration],
            parameters: &'a [StructuralParameterDeclaration],
            structural_types: &'a [StructuralTypeDeclaration],
            runtime_requirements: &'a [Proposition],
        }

        impl crate::proofs::contract_predicates::PredicateTerms for RouteTerms<'_> {
            fn structural_identity(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                left: &checked_trees::CheckedStructuralParameterField,
                right: &checked_trees::CheckedStructuralParameterField,
            ) -> Result<Proposition, LoweringError> {
                lower_structural_identity(
                    identities,
                    left,
                    right,
                    self.parameters,
                    self.structural_types,
                )
            }

            fn scalar(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expression: &CheckedScalarExpression,
            ) -> Result<ScalarTerm, LoweringError> {
                self.lower_scalar_term(identities, expression)
            }

            fn boolean(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expression: &CheckedBooleanExpression,
            ) -> Result<ScalarTerm, LoweringError> {
                self.lower_term(identities, expression)
            }
        }

        impl RouteTerms<'_> {
            fn lower_term(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expression: &CheckedBooleanExpression,
            ) -> Result<ScalarTerm, LoweringError> {
                match expression {
                    CheckedBooleanExpression::Constant(value) => Ok(ScalarTerm::boolean(*value)),
                    CheckedBooleanExpression::Parameter { .. } => checked_boolean_scalar_term(
                        identities,
                        expression,
                        self.scalar_parameters,
                        &[],
                    ),
                    CheckedBooleanExpression::StorageRead { .. } => {
                        unsupported("crash predicate cannot reconstruct mutable storage")
                    }
                    CheckedBooleanExpression::StructuralParameterField {
                        parameter_position,
                        path,
                        ..
                    } => lower_structural_member_term(
                        identities,
                        *parameter_position,
                        path,
                        ScalarType::Boolean,
                        self.parameters,
                        self.structural_types,
                    ),
                    CheckedBooleanExpression::Not(operand) => {
                        ScalarTerm::boolean_not(self.lower_term(identities, operand)?)
                            .map_err(LoweringError::InvalidCrashPredicate)
                    }
                    CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                        self.lower_term(identities, left)?,
                        self.lower_term(identities, right)?,
                    )
                    .map_err(LoweringError::InvalidCrashPredicate),
                    CheckedBooleanExpression::IntegerMathComparison { .. } => unsupported(
                        "mathematical comparison is a proof proposition, not a scalar term",
                    ),
                    CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                        let left = self.lower_scalar_term(identities, left)?;
                        let right = self.lower_scalar_term(identities, right)?;
                        let Some(integer_type) = left.scalar_type().as_integer() else {
                            return unsupported(
                                "structural crash comparison operand is not an integer",
                            );
                        };
                        super::scalar_terms::checked_integer_comparison(
                            *kind,
                            integer_type,
                            left,
                            right,
                        )
                        .map_err(LoweringError::InvalidCrashPredicate)
                    }
                    CheckedBooleanExpression::IeeeFloatComparison { .. } => {
                        unsupported("IEEE equality lowers as an atomic proposition")
                    }
                    CheckedBooleanExpression::StructuralIdentity { .. } => {
                        unsupported("structural identity is a proof proposition, not a scalar term")
                    }
                    CheckedBooleanExpression::ScalarIeeeFloatComparison { .. } => {
                        unsupported("scalar IEEE equality lowers as an atomic proposition")
                    }
                    CheckedBooleanExpression::ByteSequenceEqual { .. } => {
                        unsupported("byte-sequence equality lowers as an atomic proposition")
                    }
                    CheckedBooleanExpression::ByteSequencePredicate {
                        predicate,
                        root,
                        path,
                    } => super::structural_members::lower_byte_predicate_term(
                        identities,
                        *predicate,
                        *root,
                        path,
                        self.parameters,
                        self.structural_types,
                    ),
                    CheckedBooleanExpression::StructuralCaseMembership { .. } => {
                        unsupported("sum membership lowers as an atomic proposition")
                    }
                    CheckedBooleanExpression::Local { .. }
                    | CheckedBooleanExpression::ErasedParameter { .. }
                    | CheckedBooleanExpression::And { .. }
                    | CheckedBooleanExpression::Or { .. } => {
                        unsupported("structural crash route contains an unsupported Boolean term")
                    }
                }
            }

            fn lower_scalar_term(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expression: &CheckedScalarExpression,
            ) -> Result<ScalarTerm, LoweringError> {
                if !uses_structural_storage(expression) {
                    return checked_scalar_term(
                        identities,
                        expression,
                        self.scalar_parameters,
                        &[],
                    );
                }
                crate::proofs::integer_predicates::lower(
                    expression,
                    self.runtime_requirements,
                    &mut |expression| match expression {
                        expression if !uses_structural_storage(expression) => {
                            checked_scalar_term(identities, expression, self.scalar_parameters, &[])
                        }
                        CheckedScalarExpression::StructuralParameterField {
                            parameter_position,
                            path,
                            primitive_type,
                            ..
                        } => lower_structural_member_term(
                            identities,
                            *parameter_position,
                            path,
                            crate::emission::scalar_types::terminal_scalar_type(*primitive_type)?,
                            self.parameters,
                            self.structural_types,
                        ),
                        CheckedScalarExpression::StructuralParameterIndexedRead { .. } => {
                            super::structural_members::lower_structural_indexed_term(
                                identities,
                                expression,
                                self.parameters,
                                self.structural_types,
                                &mut |identities, selector| {
                                    self.lower_scalar_term(identities, selector)
                                },
                            )
                        }
                        _ => unsupported(
                            "structural crash integer predicate contains an unsupported operand",
                        ),
                    },
                )
            }

            fn lower_polarity(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expression: &CheckedBooleanExpression,
                positive: bool,
                remaining: &mut usize,
                depth: usize,
            ) -> Result<Proposition, LoweringError> {
                charge_boolean_expansion(remaining, depth)?;
                let mut lower =
                    |operand: &CheckedBooleanExpression, polarity, budget: &mut usize| {
                        self.lower_polarity(identities, operand, polarity, budget, depth + 1)
                    };
                match expression {
                    CheckedBooleanExpression::Not(operand) => lower(operand, !positive, remaining),
                    CheckedBooleanExpression::Equal { left, right } => {
                        match (left.as_ref(), right.as_ref()) {
                            (CheckedBooleanExpression::Constant(value), operand)
                            | (operand, CheckedBooleanExpression::Constant(value)) => {
                                lower(operand, *value == positive, remaining)
                            }
                            _ if contains_boolean_connective(expression)
                                || contains_structural_atomic_proposition(expression) =>
                            {
                                equality_from_polarities(left, right, positive, remaining, lower)
                            }
                            _ => self.lower_atom_polarity(
                                identities,
                                expression,
                                positive,
                                remaining,
                                depth + 1,
                            ),
                        }
                    }
                    CheckedBooleanExpression::And { left, right }
                    | CheckedBooleanExpression::Or { left, right } => connective(
                        lower(left, positive, remaining)?,
                        lower(right, positive, remaining)?,
                        matches!(expression, CheckedBooleanExpression::And { .. }) == positive,
                    ),
                    _ => self.lower_atom_polarity(
                        identities,
                        expression,
                        positive,
                        remaining,
                        depth + 1,
                    ),
                }
            }

            fn lower_atom_polarity(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                expression: &CheckedBooleanExpression,
                positive: bool,
                remaining: &mut usize,
                depth: usize,
            ) -> Result<Proposition, LoweringError> {
                charge_boolean_expansion(remaining, depth)?;
                if matches!(
                    expression,
                    CheckedBooleanExpression::IntegerMathComparison { .. }
                ) {
                    return proposition_with_polarity(
                        identities, expression, self, positive, remaining,
                    );
                }
                if let CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } =
                    expression
                {
                    return crate::proofs::contract_predicates::ieee_float_proposition(
                        if positive { *kind } else { kind.negated() },
                        self.lower_scalar_term(identities, left)?,
                        self.lower_scalar_term(identities, right)?,
                    );
                }
                if positive || contains_structural_atomic_proposition(expression) {
                    let proposition =
                        self.lower_proposition(identities, expression, remaining, depth + 1)?;
                    return Ok(if positive {
                        proposition
                    } else {
                        Proposition::Implication {
                            premise: Box::new(proposition),
                            conclusion: Box::new(Proposition::Falsehood),
                        }
                    });
                }
                let term = self.lower_term(identities, expression)?;
                canonical_equality(
                    ScalarTerm::boolean(true),
                    ScalarTerm::boolean_not(term).map_err(LoweringError::InvalidCrashPredicate)?,
                )
            }

            fn lower_proposition(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,

                expression: &CheckedBooleanExpression,
                remaining: &mut usize,
                depth: usize,
            ) -> Result<Proposition, LoweringError> {
                if matches!(
                    expression,
                    CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
                        | CheckedBooleanExpression::IntegerMathComparison { .. }
                ) {
                    return self
                        .lower_atom_polarity(identities, expression, true, remaining, depth);
                }
                if let CheckedBooleanExpression::Not(operand) = expression
                    && matches!(
                        operand.as_ref(),
                        CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
                            | CheckedBooleanExpression::IntegerMathComparison { .. }
                    )
                {
                    return self.lower_atom_polarity(identities, operand, false, remaining, depth);
                }
                charge_boolean_expansion(remaining, depth)?;
                if matches!(expression, CheckedBooleanExpression::Equal { .. })
                    && (contains_boolean_connective(expression)
                        || contains_structural_atomic_proposition(expression))
                    || matches!(expression, CheckedBooleanExpression::Not(operand)
                        if contains_boolean_connective(operand)
                            && !contains_structural_atomic_proposition(operand))
                {
                    return self.lower_polarity(identities, expression, true, remaining, depth + 1);
                }
                if let CheckedBooleanExpression::Not(operand) = expression
                    && contains_structural_atomic_proposition(operand)
                {
                    return Ok(Proposition::Implication {
                        premise: Box::new(self.lower_proposition(
                            identities,
                            operand,
                            remaining,
                            depth + 1,
                        )?),
                        conclusion: Box::new(Proposition::Falsehood),
                    });
                }
                if let CheckedBooleanExpression::StructuralIdentity { left, right } = expression {
                    return lower_structural_identity(
                        identities,
                        left,
                        right,
                        self.parameters,
                        self.structural_types,
                    );
                }
                if let CheckedBooleanExpression::IeeeFloatComparison {
                    kind,
                    primitive_type,
                    left,
                    right,
                } = expression
                {
                    let format = match primitive_type {
                        PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                        PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                        _ => return unsupported("structural IEEE equality has a non-float format"),
                    };
                    let mut left = lower_ieee_float_field(
                        identities,
                        left,
                        format,
                        self.parameters,
                        self.structural_types,
                    )?;
                    let mut right = lower_ieee_float_field(
                        identities,
                        right,
                        format,
                        self.parameters,
                        self.structural_types,
                    )?;
                    if kind.is_symmetric() && left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    return Ok(Proposition::IeeeFloatComparison {
                        kind: *kind,
                        format,
                        left,
                        right,
                    });
                }
                if let CheckedBooleanExpression::ByteSequenceEqual { left, right } = expression {
                    let left = left.parameter_field().ok_or(LoweringError::Unsupported(
                        "byte equality entry fact requires a parameter source",
                    ))?;
                    let right = right.parameter_field().ok_or(LoweringError::Unsupported(
                        "byte equality entry fact requires a parameter source",
                    ))?;
                    let mut left = lower_byte_sequence_field(
                        identities,
                        &left,
                        self.parameters,
                        self.structural_types,
                    )?;
                    let mut right = lower_byte_sequence_field(
                        identities,
                        &right,
                        self.parameters,
                        self.structural_types,
                    )?;
                    if left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    return Ok(Proposition::ByteSequenceEqual { left, right });
                }
                if let CheckedBooleanExpression::StructuralCaseMembership { subject, case } =
                    expression
                {
                    return super::structural_members::lower_structural_case_membership(
                        identities,
                        subject,
                        case,
                        self.parameters,
                        None,
                        self.structural_types,
                    );
                }
                if let CheckedBooleanExpression::And { left, right }
                | CheckedBooleanExpression::Or { left, right } = expression
                {
                    let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
                    let mut leaves = Vec::new();
                    flatten_checked_boolean_connective(left, conjunction, &mut leaves);
                    flatten_checked_boolean_connective(right, conjunction, &mut leaves);
                    let propositions = leaves
                        .into_iter()
                        .map(|leaf| self.lower_proposition(identities, leaf, remaining, depth + 1))
                        .collect::<Result<Vec<_>, _>>()?;
                    return canonical_connectives(propositions, conjunction);
                }
                let mut left = ScalarTerm::boolean(true);
                let mut right = self.lower_term(identities, expression)?;
                let order_key = |term: &ScalarTerm| {
                    terminal_codec::canonical_scalar_term_order_key(term).map_err(|_| {
                        LoweringError::Unsupported(
                            "mixed crash Boolean term is not canonically encodable",
                        )
                    })
                };
                if order_key(&left)? > order_key(&right)? {
                    std::mem::swap(&mut left, &mut right);
                }
                Ok(Proposition::Equal(left, right))
            }
        }

        fn uses_structural_storage(expression: &CheckedScalarExpression) -> bool {
            let mut pending = vec![expression];
            while let Some(expression) = pending.pop() {
                match expression {
                    CheckedScalarExpression::StructuralParameterField { .. }
                    | CheckedScalarExpression::StructuralLocalField { .. }
                    | CheckedScalarExpression::StructuralParameterIndexedRead { .. }
                    | CheckedScalarExpression::StructuralSequenceLength { .. }
                    | CheckedScalarExpression::Boolean(_) => return true,
                    CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                        pending.extend([left.as_ref(), right.as_ref()]);
                    }
                    CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
                    | CheckedScalarExpression::IntegerWiden { operand, .. }
                    | CheckedScalarExpression::IntegerExactCast { operand, .. }
                    | CheckedScalarExpression::RepresentationView { operand, .. } => {
                        pending.push(operand)
                    }
                    CheckedScalarExpression::ConstantApplication { .. }
                    | CheckedScalarExpression::StorageRead { .. }
                    | CheckedScalarExpression::Parameter { .. }
                    | CheckedScalarExpression::ErasedParameter { .. }
                    | CheckedScalarExpression::Local { .. }
                    | CheckedScalarExpression::IntegerLiteral { .. }
                    | CheckedScalarExpression::IeeeFloatLiteral { .. }
                    | CheckedScalarExpression::ImplementationSelection { .. } => {}
                }
            }
            false
        }

        fn checked_member_path(
            expression: &checked_trees::CrashPredicateExpression,
            path: &mut Vec<checked_trees::CheckedMemberIdentity>,
        ) -> Option<u32> {
            match expression {
                checked_trees::CrashPredicateExpression::Parameter(position) => Some(*position),
                checked_trees::CrashPredicateExpression::Member { receiver, member } => {
                    let parameter = checked_member_path(receiver, path)?;
                    path.push(checked_trees::CheckedMemberIdentity::of_spelling(member));
                    Some(parameter)
                }
                _ => None,
            }
        }

        fn contains_structural_atomic_proposition(expression: &CheckedBooleanExpression) -> bool {
            match expression {
                CheckedBooleanExpression::IeeeFloatComparison { .. }
                | CheckedBooleanExpression::IntegerMathComparison { .. }
                | CheckedBooleanExpression::StructuralIdentity { .. }
                | CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
                | CheckedBooleanExpression::ByteSequenceEqual { .. }
                | CheckedBooleanExpression::ByteSequencePredicate { .. }
                | CheckedBooleanExpression::StructuralCaseMembership { .. } => true,
                CheckedBooleanExpression::Not(operand) => {
                    contains_structural_atomic_proposition(operand)
                }
                facts::checked_boolean_triple_pattern!(left, right, ..) => {
                    contains_structural_atomic_proposition(left)
                        || contains_structural_atomic_proposition(right)
                }
                CheckedBooleanExpression::Constant(_)
                | CheckedBooleanExpression::StorageRead { .. }
                | CheckedBooleanExpression::Parameter { .. }
                | CheckedBooleanExpression::Local { .. }
                | CheckedBooleanExpression::StructuralParameterField { .. }
                | CheckedBooleanExpression::ErasedParameter { .. }
                | CheckedBooleanExpression::IntegerComparison { .. } => false,
            }
        }

        pub(crate) fn substitute_structural_crash_route_roots(
            buckets: &mut [terminal_psi::CrashRouteBucket],
            substitutions: &mut dyn FnMut(
                PlaceId,
            ) -> Result<
                Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
                LoweringError,
            >,
        ) -> Result<(), LoweringError> {
            fn substitute_term(
                term: &mut ScalarTerm,
                substitutions: &mut dyn FnMut(
                    PlaceId,
                ) -> Result<
                    Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
                    LoweringError,
                >,
            ) -> Result<(), LoweringError> {
                match term {
                    ScalarTerm::StructuralRead { root, path, .. } => {
                        for projection in path.iter_mut() {
                            if let semantic_vocabulary::ScalarReadProjection::Index(selector) =
                                projection
                            {
                                substitute_term(selector, substitutions)?;
                            }
                        }
                        if let Some((replacement, prefix)) = substitutions(*root)? {
                            *root = replacement;
                            let mut rebased = prefix
                                .into_iter()
                                .map(semantic_vocabulary::ScalarReadProjection::from)
                                .collect::<Vec<_>>();
                            rebased.append(path);
                            *path = rebased;
                        }
                    }

                    semantic_vocabulary::scalar_term_predicate_quint_pattern!(root, path, ..) => {
                        let Some((replacement, prefix)) = substitutions(*root)? else {
                            return Ok(());
                        };
                        *root = replacement;
                        if !prefix.is_empty() {
                            let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                            rebased.extend(prefix);
                            rebased.append(path);
                            *path = rebased;
                        }
                    }
                    semantic_vocabulary::scalar_term_unary_operand!(operand) => {
                        substitute_term(operand, substitutions)?
                    }
                    semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
                        substitute_term(left, substitutions)?;
                        substitute_term(right, substitutions)?;
                    }
                    _ => {}
                }
                Ok(())
            }

            fn substitute_proposition(
                proposition: &mut Proposition,
                substitutions: &mut dyn FnMut(
                    PlaceId,
                ) -> Result<
                    Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
                    LoweringError,
                >,
            ) -> Result<(), LoweringError> {
                let conjunction = matches!(proposition, Proposition::Conjunction(_));
                match proposition {
                    semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                        let mut pending = vec![left, right];
                        while let Some(term) = pending.pop() {
                            if let semantic_vocabulary::IntegerMathTerm::Embed(scalar) = term {
                                substitute_term(scalar, substitutions)?;
                            } else {
                                pending.extend(term.operands_mut());
                            }
                        }
                    }
                    Proposition::Equal(left, right) => {
                        substitute_term(left, substitutions)?;
                        substitute_term(right, substitutions)?;
                        *proposition = canonical_equality(left.clone(), right.clone())?;
                    }
                    Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
                        substitute_term(left, substitutions)?;
                        substitute_term(right, substitutions)?;
                    }
                    Proposition::ScalarIeeeFloatComparison {
                        kind, left, right, ..
                    } => {
                        substitute_term(left, substitutions)?;
                        substitute_term(right, substitutions)?;
                        if matches!(
                            kind,
                            semantic_vocabulary::IeeeFloatComparisonKind::Equal
                                | semantic_vocabulary::IeeeFloatComparisonKind::NotEqual
                        ) && right < left
                        {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::Conjunction(propositions)
                    | Proposition::Disjunction(propositions) => {
                        for proposition in propositions.iter_mut() {
                            substitute_proposition(proposition, substitutions)?;
                        }
                        *proposition =
                            canonical_connectives(std::mem::take(propositions), conjunction)?;
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        substitute_proposition(premise, substitutions)?;
                        substitute_proposition(conclusion, substitutions)?;
                    }
                    Proposition::IeeeFloatComparison {
                        kind, left, right, ..
                    } => {
                        for field in [&mut *left, &mut *right] {
                            if let Some((root, prefix)) = substitutions(field.root())? {
                                *field = field.rebase(root, &prefix);
                            }
                        }
                        if kind.is_symmetric() && *left > *right {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::ByteSequenceEqual { left, right } => {
                        for field in [&mut *left, &mut *right] {
                            if let Some((root, prefix)) = substitutions(field.root())? {
                                *field = field.rebase(root, &prefix);
                            }
                        }
                        if *left > *right {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::StructuralCaseMembership { subject, .. } => {
                        if let Some((root, prefix)) = substitutions(subject.root())? {
                            *subject = subject.rebase(root, &prefix);
                        }
                    }
                    Proposition::StructuralIdentity { left, right } => {
                        for subject in [&mut *left, &mut *right] {
                            if let Some((root, prefix)) = substitutions(subject.root())? {
                                *subject = subject.rebase(root, &prefix);
                            }
                        }
                        if *left > *right {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::ContentConservation(_) => {
                        return unsupported(
                            "content conservation requires its own call correspondence",
                        );
                    }
                    Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
                }
                if let Proposition::IntegerMathEqual(left, right) = proposition
                    && *right < *left
                {
                    std::mem::swap(left, right);
                }
                Ok(())
            }

            for bucket in buckets {
                for alternative in &mut bucket.alternatives {
                    let terminal_psi::CrashRouteGuard::Predicate(predicate) = alternative else {
                        continue;
                    };
                    let mut proposition = predicate.proposition().clone();
                    substitute_proposition(&mut proposition, substitutions)?;
                    *predicate = terminal_psi::CrashPredicateTerm::new(proposition);
                }
                bucket.alternatives.sort();
                bucket.alternatives.dedup();
            }
            Ok(())
        }
    }

    pub(crate) use argument_prefix::{
        captured_crash_argument, structural_crash_route_prefix_from_type,
    };
    pub(crate) use crash_predicates::checked_boolean_proposition;
    pub(crate) use scalar_terms::{
        checked_boolean_scalar_term, checked_scalar_term, checked_scalar_term_with_views,
        lowered_direct_scalar_term, lowered_erased_scalar_term, lowered_erased_scalar_terms,
    };
    pub(crate) use structural_members::{
        lower_byte_predicate_term, lower_sequence_extent_projection_term,
        lower_sequence_extent_term, lower_structural_case_membership, lower_structural_identity,
        lower_structural_indexed_term, lower_structural_local_member_term,
        lower_structural_member_path, lower_structural_member_term,
        lower_structural_result_indexed_term, lower_structural_result_member_term,
    };
    pub(crate) use structural_route_buckets::{
        lower_structural_crash_predicates, lower_structural_crash_route_buckets,
        substitute_structural_crash_route_roots,
    };

    use super::{
        BTreeSet, CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedTrees, ClaimId,
        LoweringError, PermissionClaimIdentity, Proposition, ScalarType, TerminalCrashCause,
        ValueDeclaration, dense_identity, unsupported, value_id,
    };
    use crate::scalar_lowering::branch_states::prepared::LoweredCrashExit;

    pub(crate) fn lower_boundary_crash_routes(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        boundary: &CheckedBoundaryMachinePlan,
        scalar_types: &[ScalarType],
        structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
        runtime_requirements: &[Proposition],
    ) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
        let buckets = checked
            .facts
            .contract_plans
            .for_machine(boundary.contract_owner)
            .map(|contract| contract.crash.published())
            .or_else(|| {
                checked
                    .facts
                    .contract_plans
                    .crash_capsule(boundary.contract_owner, boundary.state)
                    .map(|capsule| capsule.published_buckets())
            })
            .ok_or(LoweringError::Unsupported(
                "boundary crash contract owner is absent",
            ))?;
        lower_structural_crash_route_buckets(
            identities,
            &normalize_formal_crash_routes(buckets),
            &formal_crash_parameters(scalar_types)?,
            structural_parameters,
            structural_types,
            runtime_requirements,
        )
    }

    pub(crate) fn lower_formal_crash_routes(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        buckets: &[checked_trees::CrashRouteBucket],
        scalar_types: &[ScalarType],
    ) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
        lower_checked_crash_route_buckets(
            identities,
            &normalize_formal_crash_routes(buckets),
            &formal_crash_parameters(scalar_types)?,
        )
    }

    fn formal_crash_parameters(
        scalar_types: &[ScalarType],
    ) -> Result<Vec<ValueDeclaration>, LoweringError> {
        scalar_types
            .iter()
            .enumerate()
            .map(|(position, scalar_type)| {
                Ok(ValueDeclaration {
                    id: value_id(dense_identity(position)?),
                    scalar_type: *scalar_type,
                    qualifications: Default::default(),
                })
            })
            .collect()
    }

    fn normalize_formal_crash_routes(
        buckets: &[checked_trees::CrashRouteBucket],
    ) -> Vec<checked_trees::CrashRouteBucket> {
        buckets
            .iter()
            .filter_map(|bucket| {
                let guards = bucket
                    .alternative_guards()
                    .iter()
                    .filter_map(|guard| match guard {
                        checked_trees::CrashRouteGuard::Predicate(predicate) => {
                            match predicate.scalar_expression() {
                                Some(CheckedBooleanExpression::Constant(false)) => None,
                                Some(CheckedBooleanExpression::Constant(true)) => {
                                    Some(checked_trees::CrashRouteGuard::Truth)
                                }
                                _ => Some(guard.clone()),
                            }
                        }
                        _ => Some(guard.clone()),
                    })
                    .collect();
                checked_trees::CrashRouteBucket::new(bucket.cause(), guards)
            })
            .collect()
    }

    pub(crate) fn lower_checked_crash_frontier(
        frontier: &[PermissionClaimIdentity],
        source_claims: &[(PermissionClaimIdentity, ClaimId)],
    ) -> Result<Vec<ClaimId>, LoweringError> {
        let mut lowered = frontier
            .iter()
            .map(|identity| {
                source_claims
                    .iter()
                    .find_map(|(source, claim)| (source == identity).then_some(*claim))
                    .ok_or(LoweringError::CrashFrontierClaimNotLowered(*identity))
            })
            .collect::<Result<Vec<_>, _>>()?;
        lowered.sort();
        lowered.dedup();
        Ok(lowered)
    }

    pub(crate) fn lower_checked_crash_exit(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_ordinal: u32,
        source_claims: &[(PermissionClaimIdentity, ClaimId)],
    ) -> Result<LoweredCrashExit, LoweringError> {
        let Some(crash_plan) = checked
            .facts
            .contract_plans
            .for_machine(machine)
            .map(|contract| &contract.crash)
        else {
            return unsupported("explicit crash has no checked machine-contract plan");
        };
        let Some(checked_site) = crash_plan.checked_site_at(state, statement_ordinal) else {
            return unsupported("explicit crash has no body-derived checked crash-site row");
        };
        let matching_contracts = match crash_plan.interface() {
            checked_trees::CrashInterface::InternalInferred => {
                crate::unit::crash_interface::effective_crash_routes(checked, machine)?
                    .into_iter()
                    .filter(|bucket| bucket.cause() == checked_site.cause())
                    .collect::<Vec<_>>()
            }
            checked_trees::CrashInterface::PublishedCeiling => crash_plan
                .covering_buckets_for_site(checked_site)
                .map(|(_, bucket)| bucket.clone())
                .collect(),
        };
        let [covering_bucket] = matching_contracts.as_slice() else {
            return unsupported(
                "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering route bucket",
            );
        };
        let site_identities = checked_site
            .path_guard_conjuncts()
            .iter()
            .chain(checked_site.path_guard_consequences())
            .collect::<BTreeSet<_>>();
        let site_guard = covering_bucket
            .alternative_guards()
            .iter()
            .filter_map(|guard| match guard {
                checked_trees::CrashRouteGuard::Truth => None,
                checked_trees::CrashRouteGuard::Predicate(predicate)
                    if site_identities.contains(predicate) =>
                {
                    Some(
                        predicate
                            .scalar_expression()
                            .cloned()
                            .ok_or(LoweringError::Unsupported(
                                "guarded crash site is outside structured scalar predicate lowering",
                            )),
                    )
                }
                checked_trees::CrashRouteGuard::Predicate(_) => None,
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !covering_bucket
            .alternative_guards()
            .contains(&checked_trees::CrashRouteGuard::Truth)
            && site_guard.is_empty()
        {
            return unsupported("guarded crash site has no structured covering predicate");
        }
        Ok(LoweredCrashExit {
            cause: match checked_site.cause() {
                checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
                checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
            },
            site_guard,
            frontier_lower_bound: lower_checked_crash_frontier(
                checked_site.frontier_lower_bound(),
                source_claims,
            )?,
        })
    }

    pub(crate) fn lower_crash_route_buckets(
        buckets: &[checked_trees::CrashRouteBucket],
        mut lower_predicate: impl FnMut(
            &checked_trees::CrashPredicateIdentity,
        ) -> Result<Proposition, LoweringError>,
    ) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
        buckets
            .iter()
            .map(|bucket| {
                let mut alternatives = bucket
                    .alternative_guards()
                    .iter()
                    .map(|guard| match guard {
                        checked_trees::CrashRouteGuard::Truth => {
                            Ok(terminal_psi::CrashRouteGuard::Truth)
                        }
                        checked_trees::CrashRouteGuard::Predicate(predicate) => {
                            Ok(terminal_psi::CrashRouteGuard::Predicate(
                                terminal_psi::CrashPredicateTerm::new(lower_predicate(predicate)?),
                            ))
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                alternatives.sort();
                alternatives.dedup();
                Ok(terminal_psi::CrashRouteBucket {
                    cause: lower_crash_cause(bucket.cause()),
                    alternatives,
                })
            })
            .collect()
    }

    fn lower_crash_cause(cause: checked_trees::CrashCause) -> TerminalCrashCause {
        match cause {
            checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
            checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
        }
    }

    pub(crate) fn lower_checked_crash_route_buckets(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        buckets: &[checked_trees::CrashRouteBucket],
        parameters: &[ValueDeclaration],
    ) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
        lower_crash_route_buckets(buckets, |predicate| {
            let expression = predicate
                .scalar_expression()
                .ok_or(LoweringError::Unsupported(
                    "guarded crash route is outside structured scalar predicate lowering",
                ))?;
            checked_boolean_proposition(identities, expression, parameters)
        })
    }
}
pub(crate) mod evidence_lowering {

    use crate::machine_lowering::declarations::semantic_identity;
    pub(crate) mod evidence_terms {

        use crate::proofs::evidence_lowering::proof_output_calls::proof_output_forwarded_source;
        use crate::proofs::evidence_lowering::{
            checked_evidence_requirement_identity, evidence_projection_identity,
        };
        use crate::proofs::{
            BTreeMap, BTreeSet, CheckedPropositionBinderKind, CheckedPropositionEvidence,
            CheckedTrees, EvidenceContractLane, EvidenceContractLaneKind,
            EvidenceInterfaceIdentity, EvidenceRequirementIdentity, EvidenceTermDeclaration,
            EvidenceTermId, LoweringError, MachineId, PropositionApplicationIdentity,
            PropositionBinderArgumentIdentity, PropositionBinderDeclaration, PropositionBinderKind,
            PropositionDeclaration, PropositionEvidence, PropositionId, proposition_id,
        };

        pub(crate) fn evidence_term_index<T>(handle: arena::Handle<T>) -> usize {
            usize::try_from(handle.arena_index() - 1)
                .expect("arena indices fit the host address space")
        }

        fn evidence_term_root(parents: &mut [usize], mut index: usize) -> usize {
            while parents[index] != index {
                let parent = parents[index];
                parents[index] = parents[parent];
                index = parents[index];
            }
            index
        }

        pub(crate) fn lower_proposition_vocabulary(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            term_ids: &[Option<EvidenceTermId>],
        ) -> Result<
            (
                Vec<PropositionDeclaration>,
                Vec<PropositionApplicationIdentity>,
                Vec<(symbols::SymbolHandle, PropositionId)>,
            ),
            LoweringError,
        > {
            let placeholder = proposition_id(1);
            let mut declarations = checked
                .facts
                .proof
                .proposition_vocabulary
                .declarations
                .iter()
                .map(|declaration| {
                    let evidence = match &declaration.evidence {
                        CheckedPropositionEvidence::FactOnly => PropositionEvidence::FactOnly,
                        CheckedPropositionEvidence::Witness { evidence_type } => {
                            PropositionEvidence::Witness {
                                evidence_type: evidence_type.clone(),
                            }
                        }
                    };
                    let binders = declaration
                        .binders
                        .iter()
                        .map(|binder| PropositionBinderDeclaration {
                            name: binder.name.clone(),
                            kind: match &binder.kind {
                                CheckedPropositionBinderKind::Type => PropositionBinderKind::Type,
                                CheckedPropositionBinderKind::Const { type_identity } => {
                                    PropositionBinderKind::Const {
                                        type_identity: identities.intern(
                                            &checked
                                                .type_identity_text(*type_identity)
                                                .unwrap_or_default(),
                                        ),
                                    }
                                }
                                CheckedPropositionBinderKind::Machine => {
                                    PropositionBinderKind::Machine
                                }
                            },
                        })
                        .collect();
                    (
                        declaration.symbol,
                        PropositionDeclaration {
                            id: placeholder,
                            name: declaration.name.clone(),
                            binders,
                            parameter_types: declaration.parameter_types.clone(),
                            evidence,
                        },
                    )
                })
                .collect::<Vec<_>>();
            declarations.sort_by(|left, right| left.1.cmp(&right.1));
            for (index, (_, declaration)) in declarations.iter_mut().enumerate() {
                declaration.id = proposition_id(
                    u64::try_from(index)
                        .expect("proposition declaration count fits u64")
                        .checked_add(1)
                        .expect("one-based proposition identity fits u64"),
                );
            }
            let declaration_ids = declarations
                .iter()
                .map(|(symbol, declaration)| (*symbol, declaration.id))
                .collect::<Vec<_>>();

            let retained_term_applications =
                checked
                    .facts
                    .proof
                    .evidence_terms
                    .iter()
                    .filter_map(|(handle, term)| {
                        let index = evidence_term_index(handle);
                        term_ids
                            .get(index)
                            .copied()
                            .flatten()
                            .map(|_| &term.proposition)
                    });
            let mut applications = Vec::new();
            for application in checked
                .facts
                .proof
                .proposition_vocabulary
                .applications
                .iter()
                .chain(retained_term_applications)
            {
                let Some(declaration) = declaration_ids
                    .iter()
                    .find_map(|(symbol, id)| (*symbol == application.declaration).then_some(*id))
                else {
                    continue;
                };
                let mut binder_arguments = Vec::new();
                let mut belongs_to_emitted_bodies = true;
                for argument in &application.binder_arguments {
                    let evidence_projection =
                        if let Some(projection) = &argument.evidence_projection {
                            let index = evidence_term_index(projection.term);
                            let Some(term) = term_ids.get(index).copied().flatten() else {
                                belongs_to_emitted_bodies = false;
                                break;
                            };
                            Some(evidence_projection_identity(
                                identities, checked, projection, term,
                            )?)
                        } else {
                            None
                        };
                    binder_arguments.push(PropositionBinderArgumentIdentity {
                        kind: super::binder_argument_kind(argument.kind),
                        identity: identities.intern(&argument.diagnostic_label.clone()),
                        evidence_projection,
                    });
                }
                if !belongs_to_emitted_bodies {
                    continue;
                }
                applications.push(PropositionApplicationIdentity {
                    id: placeholder,
                    declaration,
                    binder_arguments,
                    arguments: application.arguments.clone(),
                    evidence_interface: application
                        .evidence_interface
                        .as_ref()
                        .map(|interface| lower_evidence_interface(identities, checked, interface))
                        .transpose()?,
                });
            }
            applications.sort();
            applications.dedup();
            for (index, application) in applications.iter_mut().enumerate() {
                application.id = proposition_id(
                    u64::try_from(index)
                        .expect("proposition application count fits u64")
                        .checked_add(1)
                        .expect("one-based proposition identity fits u64"),
                );
            }
            Ok((
                declarations
                    .into_iter()
                    .map(|(_, declaration)| declaration)
                    .collect(),
                applications,
                declaration_ids,
            ))
        }

        pub(crate) struct LoweredEvidenceTerms {
            pub(crate) declarations: Vec<EvidenceTermDeclaration>,
            pub(crate) term_ids: Vec<Option<EvidenceTermId>>,
        }

        pub(crate) struct LoweredEvidenceTermIds {
            pub(crate) term_ids: Vec<Option<EvidenceTermId>>,
        }

        pub(crate) fn lower_evidence_term_ids(
            checked: &CheckedTrees,
            source_machine_ids: &[(symbols::SymbolHandle, MachineId)],
        ) -> Result<LoweredEvidenceTermIds, LoweringError> {
            let mut parents = (0..checked.facts.proof.evidence_terms.len()).collect::<Vec<_>>();
            let guarded_terms = source_machine_ids
                .iter()
                .flat_map(|(machine, _)| selected_guarded_evidence_terms(checked, *machine))
                .collect::<BTreeSet<_>>();
            let body_position = |machine| {
                source_machine_ids
                    .iter()
                    .position(|(source, _)| *source == machine)
            };
            let invocations = checked
                .facts
                .proof
                .proof_output_calls
                .iter()
                .filter_map(|(_, invocation)| {
                    body_position(invocation.caller_machine_symbol).map(|_| invocation)
                })
                .collect::<Vec<_>>();
            for (_, forwarding) in checked.facts.proof.evidence_forwardings.iter() {
                if body_position(forwarding.machine_symbol).is_none() {
                    continue;
                }
                if let checked_trees::EvidenceAssignmentSource::Forwarded { term: source } =
                    &forwarding.source
                {
                    let output = evidence_term_index(forwarding.output);
                    let source = evidence_term_index(*source);
                    let output_root = evidence_term_root(&mut parents, output);
                    let source_root = evidence_term_root(&mut parents, source);
                    parents[output_root] = source_root;
                }
            }
            for invocation in &invocations {
                for output in &invocation.outputs {
                    let Some(source) = proof_output_forwarded_source(checked, invocation, output)
                    else {
                        continue;
                    };
                    if let Some(output) = output.output {
                        let output = evidence_term_index(output);
                        let source = evidence_term_index(source);
                        let output_root = evidence_term_root(&mut parents, output);
                        let source_root = evidence_term_root(&mut parents, source);
                        parents[output_root] = source_root;
                    }
                }
            }

            let mut roots = BTreeMap::<usize, (usize, u8, usize)>::new();
            for (handle, term) in checked.facts.proof.evidence_terms.iter() {
                let checked_trees::ContractProofFactOwner::Machine { machine_symbol } = term.owner
                else {
                    continue;
                };
                let Some(machine_position) = body_position(machine_symbol) else {
                    continue;
                };
                let index = evidence_term_index(handle);
                let root = evidence_term_root(&mut parents, index);
                let lane_key = match term.kind {
                    checked_trees::ContractProofFactKind::Requires => {
                        (machine_position, 0_u8, term.lane_position)
                    }
                    checked_trees::ContractProofFactKind::Ensures
                        if guarded_terms.contains(&evidence_term_index(handle)) =>
                    {
                        (machine_position, 2_u8, term.lane_position)
                    }
                    checked_trees::ContractProofFactKind::Ensures => {
                        (machine_position, 1_u8, term.lane_position)
                    }
                };
                roots
                    .entry(root)
                    .and_modify(|previous| *previous = (*previous).min(lane_key))
                    .or_insert(lane_key);
            }
            let mut package_identity_position = 0_usize;
            for invocation in invocations {
                let machine_position = body_position(invocation.caller_machine_symbol)
                    .expect("retained proof-output caller belongs to an emitted body");
                for argument in &invocation.evidence_arguments {
                    let index = evidence_term_index(argument.source);
                    let root = evidence_term_root(&mut parents, index);
                    roots.entry(root).or_insert((
                        machine_position,
                        3_u8,
                        package_identity_position,
                    ));
                    package_identity_position = package_identity_position
                        .checked_add(1)
                        .expect("proof-output identity order fits usize");
                }
                for output in &invocation.outputs {
                    let forwarded =
                        proof_output_forwarded_source(checked, invocation, output).is_some();
                    let retains_callee_term =
                        !forwarded && invocation.static_requirement_dispatch.is_none();
                    for handle in output
                        .output
                        .into_iter()
                        .chain(retains_callee_term.then_some(output.callee_output))
                    {
                        let index = evidence_term_index(handle);
                        let root = evidence_term_root(&mut parents, index);
                        roots.entry(root).or_insert((
                            machine_position,
                            3_u8,
                            package_identity_position,
                        ));
                        package_identity_position = package_identity_position
                            .checked_add(1)
                            .expect("proof-output identity order fits usize");
                    }
                }
            }
            let mut selected_output_position = 0_usize;
            for (_, arm) in checked.facts.proof.outcome_specific_arms.iter() {
                let Some(machine_position) = body_position(arm.caller_machine_symbol) else {
                    continue;
                };
                for term in arm.rows.iter().filter_map(|row| row.selected_term) {
                    let index = evidence_term_index(term);
                    let root = evidence_term_root(&mut parents, index);
                    roots
                        .entry(root)
                        .or_insert((machine_position, 4_u8, selected_output_position));
                    selected_output_position = selected_output_position
                        .checked_add(1)
                        .expect("selected-output identity order fits usize");
                }
            }
            let mut roots = roots
                .into_iter()
                .map(|(root, lane_key)| (lane_key, root))
                .collect::<Vec<_>>();
            roots.sort_unstable();
            let root_ids = roots
                .into_iter()
                .enumerate()
                .map(|(index, (_, root))| {
                    let id = EvidenceTermId::new(
                        u64::try_from(index)
                            .expect("evidence term count fits u64")
                            .checked_add(1)
                            .expect("one-based evidence term identity fits u64"),
                    )
                    .expect("one-based evidence term identity is nonzero");
                    (root, id)
                })
                .collect::<BTreeMap<_, _>>();
            let mut term_ids = vec![None; parents.len()];
            for (handle, _) in checked.facts.proof.evidence_terms.iter() {
                let index = evidence_term_index(handle);
                let root = evidence_term_root(&mut parents, index);
                term_ids[index] = root_ids.get(&root).copied();
            }
            Ok(LoweredEvidenceTermIds { term_ids })
        }

        pub(crate) fn lower_evidence_terms(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            declaration_ids: &[(symbols::SymbolHandle, PropositionId)],
            applications: &[PropositionApplicationIdentity],
            term_ids: Vec<Option<EvidenceTermId>>,
        ) -> Result<LoweredEvidenceTerms, LoweringError> {
            let mut identities_by_id =
                BTreeMap::<EvidenceTermId, (PropositionId, EvidenceInterfaceIdentity)>::new();
            for (handle, term) in checked.facts.proof.evidence_terms.iter() {
                let index = evidence_term_index(handle);
                if term_ids.get(index).copied().flatten().is_none() {
                    continue;
                }
                let id = term_ids[index].ok_or(LoweringError::Unsupported(
                    "selected terminal evidence term has no canonical identity",
                ))?;
                let declaration = declaration_ids
                    .iter()
                    .find_map(|(symbol, id)| {
                        (*symbol == term.proposition.declaration).then_some(*id)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "checked evidence term has no terminal proposition declaration",
                    ))?;
                let binder_arguments = term
                    .proposition
                    .binder_arguments
                    .iter()
                    .map(|argument| {
                        let evidence_projection = argument
                            .evidence_projection
                            .as_ref()
                            .map(|projection| {
                                let projection_index = evidence_term_index(projection.term);
                                evidence_projection_identity(
                                    identities,
                                    checked,
                                    projection,
                                    term_ids.get(projection_index).copied().flatten().ok_or(
                                        LoweringError::Unsupported(
                                            "evidence-term proposition projects an unrelated term",
                                        ),
                                    )?,
                                )
                            })
                            .transpose()?;
                        Ok(PropositionBinderArgumentIdentity {
                            kind: super::binder_argument_kind(argument.kind),
                            identity: identities.intern(&argument.diagnostic_label),
                            evidence_projection,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let proposition = applications
                    .iter()
                    .find(|application| {
                        application.declaration == declaration
                            && application.binder_arguments == binder_arguments
                            && application.arguments == term.proposition.arguments
                    })
                    .map(|application| application.id)
                    .ok_or(LoweringError::Unsupported(
                        "checked evidence term has no terminal proposition application",
                    ))?;
                let checked_interface =
                    term.evidence_interface
                        .as_ref()
                        .ok_or(LoweringError::Unsupported(
                            "terminal evidence term has an unresolved carrierless interface",
                        ))?;
                let interface = lower_evidence_interface(identities, checked, checked_interface)?;
                if let Some((previous_proposition, previous_interface)) = identities_by_id.get(&id)
                {
                    if *previous_proposition != proposition || *previous_interface != interface {
                        return Err(LoweringError::Unsupported(
                            "forwarded evidence terms disagree on exact terminal identity",
                        ));
                    }
                } else {
                    identities_by_id.insert(id, (proposition, interface));
                }
            }
            let declarations = identities_by_id
                .into_iter()
                .map(|(id, (proposition, interface))| EvidenceTermDeclaration {
                    id,
                    proposition,
                    interface,
                })
                .collect();
            Ok(LoweredEvidenceTerms {
                declarations,
                term_ids,
            })
        }

        pub(crate) fn lower_evidence_interface(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            interface: &checked_trees::CheckedEvidenceInterfaceIdentity,
        ) -> Result<EvidenceInterfaceIdentity, LoweringError> {
            let mut requirements = interface
                .requirements
                .iter()
                .map(|requirement| {
                    Ok(EvidenceRequirementIdentity {
                        declaring_trait_identity: identities.intern(
                            &checked
                                .symbols
                                .display_path(requirement.declaring_trait, "::"),
                        ),
                        declaring_trait_arguments: requirement.declaring_trait_arguments.clone(),
                        requirement_identity: identities.intern(
                            &checked_evidence_requirement_identity(
                                checked,
                                requirement.declaring_trait,
                                requirement.requirement,
                            )?,
                        ),
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            requirements.sort();
            requirements.dedup();
            Ok(EvidenceInterfaceIdentity {
                trait_identity: identities
                    .intern(&checked.symbols.display_path(interface.trait_symbol, "::")),
                arguments: interface
                    .arguments
                    .iter()
                    .map(|argument| checked.type_identity_text(*argument).unwrap_or_default())
                    .collect(),
                requirements,
            })
        }

        pub(crate) fn lower_evidence_contract_lanes(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            module: &terminal_psi::TerminalModule,
            selected_machine: symbols::SymbolHandle,
            terminal_machine: MachineId,
            term_ids: &[Option<EvidenceTermId>],
        ) -> Result<Vec<EvidenceContractLane>, LoweringError> {
            let guarded_terms = selected_guarded_evidence_terms(checked, selected_machine);
            let mut lanes = checked
                .facts
                .proof
                .evidence_terms
                .iter()
                .filter_map(|(handle, term)| {
                    (term.owner
                        == checked_trees::ContractProofFactOwner::Machine {
                            machine_symbol: selected_machine,
                        }
                        && !guarded_terms.contains(&evidence_term_index(handle)))
                    .then_some((handle, term))
                })
                .map(|(handle, term)| {
                    let index = evidence_term_index(handle);
                    let term_id = term_ids.get(index).copied().flatten().ok_or(
                        LoweringError::Unsupported(
                            "selected terminal contract lane has no evidence-term identity",
                        ),
                    )?;
                    let kind = match term.kind {
                        checked_trees::ContractProofFactKind::Requires => {
                            EvidenceContractLaneKind::Requires
                        }
                        checked_trees::ContractProofFactKind::Ensures => {
                            EvidenceContractLaneKind::Ensures
                        }
                    };
                    let argument_subjects = evidence_argument_subjects(
                        identities,
                        checked,
                        module,
                        selected_machine,
                        terminal_machine,
                        handle,
                    )?;
                    Ok(EvidenceContractLane {
                        machine: terminal_machine,
                        kind,
                        position: u32::try_from(term.lane_position).map_err(|_| {
                            LoweringError::Unsupported(
                                "terminal evidence contract lane position exceeds u32",
                            )
                        })?,
                        term: term_id,
                        output_field: (kind == EvidenceContractLaneKind::Ensures)
                            .then(|| term.name.clone()),
                        argument_subjects,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            lanes.sort_unstable();
            Ok(lanes)
        }

        fn selected_guarded_evidence_terms(
            checked: &CheckedTrees,
            selected_machine: symbols::SymbolHandle,
        ) -> BTreeSet<usize> {
            checked
                .facts
                .proof
                .outcome_specific_guarantees
                .iter()
                .filter_map(|(_, guarantee)| {
                    (guarantee.machine_symbol == selected_machine)
                        .then_some(guarantee.evidence_term)
                        .flatten()
                        .map(evidence_term_index)
                })
                .collect()
        }

        fn evidence_argument_subjects(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            module: &terminal_psi::TerminalModule,
            source_machine: symbols::SymbolHandle,
            terminal_machine: MachineId,
            term: arena::Handle<checked_trees::CheckedEvidenceTerm>,
        ) -> Result<Vec<terminal_psi::EvidenceArgumentSubject>, LoweringError> {
            let mut contracts = checked
                .facts
                .proof
                .contract_facts
                .iter()
                .map(|(_, row)| row)
                .filter(|row| {
                    row.evidence_term == Some(term)
                        && row.owner
                            == (checked_trees::ContractProofFactOwner::Machine {
                                machine_symbol: source_machine,
                            })
                });
            let contract = contracts.next().ok_or(LoweringError::Unsupported(
                "evidence lane has no exact declaration fact",
            ))?;
            if contracts.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "evidence lane declaration fact is ambiguous",
                ));
            }
            let roots = contract
                .nominal_argument_roots
                .and_then(|roots| checked.facts.proof.nominal_argument_roots.span(roots))
                .ok_or(LoweringError::Unsupported(
                    "evidence lane fact has no checked nominal argument subjects",
                ))?;
            let callee = module
                .machine(terminal_machine)
                .ok_or(LoweringError::Unsupported(
                    "evidence lane emitted machine is absent",
                ))?;
            roots
                .iter()
                .enumerate()
                .map(|(position, root)| {
                    super::outcome_specific_calls::lower_argument_subject(
                        identities,
                        checked,
                        source_machine,
                        callee,
                        module,
                        position,
                        root,
                    )
                })
                .collect()
        }
    }
    pub(crate) mod outcome_specific_calls {

        use crate::proofs::evidence_lowering::evidence_terms::{
            lower_evidence_contract_lanes, lower_evidence_interface,
        };
        use crate::proofs::evidence_lowering::outcome_specific_ensures::outcome_specific_ensure_rows;
        use crate::proofs::evidence_lowering::proof_output_calls::{
            terminal_evidence_term_id, terminal_proposition_application_id,
        };
        use crate::proofs::{
            CheckedTrees, EvidenceTermDeclaration, EvidenceTermId, LoweredPsi, LoweringError,
            OperationResult, OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity,
            Proposition, PropositionApplicationIdentity, PropositionDeclaration,
        };
        use crate::scalar_lowering::computations::fields::field_path_identity;
        use crate::shared::checked_facts::flow_facts;

        fn dependency_places(
            occurrences: &[checked_trees::OutcomeReferencedOccurrence],
            result_place: semantic_vocabulary::PlaceId,
            unsupported_root: &'static str,
        ) -> Result<Vec<semantic_vocabulary::PlaceId>, LoweringError> {
            let mut places = Vec::with_capacity(occurrences.len());
            for occurrence in occurrences {
                match &occurrence.root {
                    checked_trees::OutcomeOccurrenceRoot::CalleeResult => places.push(result_place),
                    checked_trees::OutcomeOccurrenceRoot::CalleeParameter { .. }
                    | checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection { .. }
                    | checked_trees::OutcomeOccurrenceRoot::ClosedValue => {}
                    checked_trees::OutcomeOccurrenceRoot::Other => {
                        return Err(LoweringError::Unsupported(unsupported_root));
                    }
                }
            }
            places.sort();
            places.dedup();
            Ok(places)
        }

        pub(crate) fn lower_outcome_specific_call_evidence(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            lowered: &mut LoweredPsi,
            term_ids: &[Option<EvidenceTermId>],
            declarations: &[PropositionDeclaration],
            applications: &[PropositionApplicationIdentity],
            evidence_terms: &[EvidenceTermDeclaration],
        ) -> Result<(), LoweringError> {
            for (_, arm) in checked
                .facts
                .proof
                .outcome_specific_arms
                .iter()
                .filter(|(_, arm)| {
                    arm.caller_machine_symbol == machine
                        && arm.rows.iter().any(|row| row.selected_term.is_some())
                })
            {
                let Some(first_row) = arm.rows.iter().find(|row| row.selected_term.is_some())
                else {
                    continue;
                };
                let callee_symbol = checked
                    .facts
                    .proof
                    .outcome_specific_guarantees
                    .get(first_row.guarantee)
                    .machine_symbol;
                let mut occurrences = lowered.source_call_occurrences.iter().filter(|occurrence| {
                    occurrence.origin.statement().is_some_and(|site| {
                        site.state == arm.caller_state_symbol
                            && site.statement_index == arm.result_call_statement_index
                    }) && occurrence.source_site
                        == Some(checked_trees::NominalMachineUseSite::Expression(
                            arm.result_call_expression,
                        ))
                });
                let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
                    "outcome-specific arm has no exact lowered call occurrence",
                ))?;
                if occurrences.next().is_some() {
                    return Err(LoweringError::Unsupported(
                        "outcome-specific arm call occurrence is not unique",
                    ));
                }
                let operation_id =
                    occurrence
                        .terminal_site
                        .operation()
                        .ok_or(LoweringError::Unsupported(
                            "outcome-specific arm names a completion edge, not a call operation",
                        ))?;
                let (result_place, callee_id) = {
                    let operation = lowered
                        .semantic_module
                        .machines
                        .iter()
                        .flat_map(|machine| machine.blocks.iter())
                        .flat_map(|block| block.operations.iter())
                        .find(|operation| operation.id == operation_id)
                        .ok_or(LoweringError::Unsupported(
                            "outcome-specific arm operation is absent",
                        ))?;
                    let terminal_psi::call_structural_pair_pattern!(callee, ..) = &operation.kind
                    else {
                        return Err(LoweringError::Unsupported(
                            "outcome-specific arm operation is not a structural call",
                        ));
                    };
                    let OperationResult::Structural(result) = &operation.result else {
                        return Err(LoweringError::Unsupported(
                            "outcome-specific arm call has no structural result",
                        ));
                    };
                    (result.place, *callee)
                };
                let callee = lowered.semantic_module.machine(callee_id).ok_or(
                    LoweringError::Unsupported("outcome-specific arm callee is absent"),
                )?;
                if callee.structural_parameters.iter().any(|parameter| {
                    parameter.access != terminal_psi::StructuralAccess::Owned
                        || parameter.multiplicity
                            != terminal_psi::StructuralMultiplicity::Unrestricted
                        || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                        || terminal_psi::structural_type_has_borrowed_storage_by(
                            |id| lowered.semantic_module.structural_type(id),
                            parameter.structural_type,
                        )
                }) || !callee.entry_claims.is_empty()
                    || !callee.content_entry_claims.is_empty()
                    || !callee.contract.requires.is_empty()
                    || !callee.contract.ensures.is_empty()
                    || !callee.contract.crash_routes.is_empty()
                    || !lower_evidence_contract_lanes(
                        identities,
                        checked,
                        &lowered.semantic_module,
                        callee_symbol,
                        callee_id,
                        term_ids,
                    )?
                    .is_empty()
                {
                    return Err(LoweringError::Unsupported(
                        "outcome-specific selection from a callee with structural inputs or other contracts is unimplemented",
                    ));
                }
                let callee_result_place = callee
                    .result
                    .structural()
                    .ok_or(LoweringError::Unsupported(
                        "outcome-specific arm callee has no structural result",
                    ))?
                    .place;
                let ensure_rows = outcome_specific_ensure_rows(
                    identities,
                    checked,
                    callee_symbol,
                    callee_id,
                    &lowered.semantic_module,
                    term_ids,
                    evidence_terms,
                )?;

                let mut evidence_rows = Vec::new();
                for row in arm.rows.iter().filter(|row| row.selected_term.is_some()) {
                    let guarantee = checked
                        .facts
                        .proof
                        .outcome_specific_guarantees
                        .get(row.guarantee);
                    let output_field = guarantee
                        .public_selector
                        .clone()
                        .ok_or(LoweringError::Unsupported("selected guarantee is unnamed"))?;
                    let callee_term = terminal_evidence_term_id(
                        term_ids,
                        guarantee.evidence_term.ok_or(LoweringError::Unsupported(
                            "selected guarantee carries no evidence term",
                        ))?,
                        "selected guarantee term has no terminal identity",
                    )?;
                    let ensure = ensure_rows
                        .iter()
                        .find(|ensure| {
                            ensure.evidence.as_ref().is_some_and(|evidence| {
                                evidence.term == callee_term
                                    && evidence.output_field == output_field
                            })
                        })
                        .ok_or(LoweringError::Unsupported(
                            "selected guarantee is absent from the callee's guarded rows",
                        ))?;
                    let Proposition::Atom(callee_proposition) = ensure.proposition else {
                        return Err(LoweringError::Unsupported(
                            "selected guarantee is not a nominal proposition",
                        ));
                    };
                    let checked_term = checked.facts.proof.evidence_terms.get(
                        guarantee
                            .evidence_term
                            .expect("selected evidence term checked"),
                    );
                    let instantiated =
                        row.instantiated_proposition
                            .as_ref()
                            .ok_or(LoweringError::Unsupported(
                                "selected arm row carries no instantiated proposition",
                            ))?;
                    let instantiated_proposition = terminal_proposition_application_id(
                        identities,
                        checked,
                        term_ids,
                        declarations,
                        applications,
                        instantiated,
                    )?;
                    let declared = &checked_term.proposition;
                    if declared.arguments.len() != instantiated.arguments.len()
                        || declared.declaration != instantiated.declaration
                        || declared.binder_arguments != instantiated.binder_arguments
                        || declared.evidence_interface != instantiated.evidence_interface
                    {
                        return Err(LoweringError::Unsupported(
                            "outcome-specific substitution outside the proposition's arguments is unimplemented",
                        ));
                    }
                    let argument_substitutions = ensure
                        .evidence
                        .as_ref()
                        .expect("selected guarantee has evidence")
                        .argument_subjects
                        .clone();
                    let mut result_substitution = None;
                    for (position, (declared, instantiated)) in declared
                        .arguments
                        .iter()
                        .zip(&instantiated.arguments)
                        .enumerate()
                    {
                        match row.argument_roots.get(position) {
                            Some(checked_trees::OutcomeOccurrenceRoot::CalleeResult)
                                if result_substitution.is_none() =>
                            {
                                result_substitution =
                                    Some(terminal_psi::OutcomeSpecificCallResultSubstitution {
                                        argument_position: u32::try_from(position).map_err(|_| {
                                            LoweringError::Unsupported(
                                                "outcome-specific argument position exceeds u32",
                                            )
                                        })?,
                                        callee_result: callee_result_place,
                                        caller_result: result_place,
                                    });
                            }
                            Some(
                                checked_trees::OutcomeOccurrenceRoot::CalleeParameter { .. }
                                | checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection {
                                    ..
                                },
                            ) => {}
                            Some(checked_trees::OutcomeOccurrenceRoot::ClosedValue)
                                if declared == instantiated => {}
                            _ => {
                                return Err(LoweringError::Unsupported(
                                    "outcome-specific substitution of a computed proposition argument is unimplemented",
                                ));
                            }
                        }
                    }
                    if row
                        .validity
                        .evidence_interface_scope
                        .as_ref()
                        .is_some_and(|scope| !scope.reference_regions.is_empty())
                    {
                        return Err(LoweringError::Unsupported(
                            "outcome-specific evidence interface reference scopes are unimplemented",
                        ));
                    }
                    let proposition_dependencies = dependency_places(
                        &row.validity.referenced_occurrences,
                        result_place,
                        "outcome-specific proposition dependencies beyond the call result are unimplemented",
                    )?;
                    let interface_dependencies = row
                        .validity
                        .evidence_interface_scope
                        .as_ref()
                        .map(|scope| {
                            dependency_places(
                                &scope.retained_occurrences,
                                result_place,
                                "outcome-specific interface dependencies beyond the call result are unimplemented",
                            )
                        })
                        .transpose()?
                        .unwrap_or_default();
                    let evidence_interface = checked_term
                        .evidence_interface
                        .as_ref()
                        .or(row
                            .validity
                            .evidence_interface_scope
                            .as_ref()
                            .map(|scope| &scope.interface))
                        .ok_or(LoweringError::Unsupported(
                            "outcome-specific witness has no interface identity",
                        ))
                        .and_then(|interface| {
                            lower_evidence_interface(identities, checked, interface)
                        })?;
                    evidence_rows.push(OutcomeSpecificCallEvidence {
                        guard: ensure.guard,
                        position: ensure.position,
                        callee_obligation: ensure.obligation,
                        callee_term,
                        output_field,
                        callee_proposition,
                        instantiated_proposition,
                        output: terminal_evidence_term_id(
                            term_ids,
                            row.selected_term.expect("selected row carries a term"),
                            "selected arm term has no terminal identity",
                        )?,
                        argument_substitutions,
                        result_substitution,
                        validity: OutcomeSpecificCallEvidenceValidity {
                            result: result_place,
                            proposition_dependencies,
                            evidence_interface,
                            interface_dependencies,
                        },
                        expected_use_count: u32::try_from(
                            checked
                                .facts
                                .proof
                                .proof_output_calls
                                .iter()
                                .filter(|(_, invocation)| {
                                    invocation.caller_machine_symbol == arm.caller_machine_symbol
                                        && invocation.runtime_call.is_some()
                                })
                                .map(|(_, invocation)| {
                                    invocation
                                        .evidence_arguments
                                        .iter()
                                        .filter(|argument| {
                                            Some(argument.source) == row.selected_term
                                        })
                                        .count()
                                })
                                .sum::<usize>(),
                        )
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "selected evidence use cardinality exceeds u32",
                            )
                        })?,
                        uses: Vec::new(),
                    });
                }
                let operation = lowered
                    .semantic_module
                    .machines
                    .iter_mut()
                    .flat_map(|machine| machine.blocks.iter_mut())
                    .flat_map(|block| block.operations.iter_mut())
                    .find(|operation| operation.id == operation_id)
                    .ok_or(LoweringError::Unsupported(
                        "outcome-specific arm operation is absent",
                    ))?;
                let terminal_psi::call_structural_pair_pattern!(selected_evidence, ..) =
                    &mut operation.kind
                else {
                    return Err(LoweringError::Unsupported(
                        "outcome-specific arm operation is not a structural call",
                    ));
                };
                *selected_evidence = evidence_rows;
            }
            Ok(())
        }

        pub(crate) fn lower_argument_subjects(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            callee: &terminal_psi::TerminalMachine,
            module: &terminal_psi::TerminalModule,
            roots: &[checked_trees::OutcomeOccurrenceRoot],
        ) -> Result<Vec<terminal_psi::OutcomeSpecificCallArgumentSubstitution>, LoweringError>
        {
            let mut subjects = Vec::new();
            for (position, root) in roots.iter().enumerate() {
                match lower_argument_subject(
                    identities, checked, machine, callee, module, position, root,
                )? {
                    terminal_psi::EvidenceArgumentSubject::Parameter(subject) => {
                        subjects.push(subject)
                    }
                    terminal_psi::EvidenceArgumentSubject::Closed { .. }
                    | terminal_psi::EvidenceArgumentSubject::Result { .. } => {}
                    terminal_psi::EvidenceArgumentSubject::Unknown { .. } => {
                        return Err(LoweringError::Unsupported(
                            "outcome-specific substitution of a computed proposition argument is unimplemented",
                        ));
                    }
                }
            }
            Ok(subjects)
        }

        pub(crate) fn lower_argument_subject(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            machine: symbols::SymbolHandle,
            callee: &terminal_psi::TerminalMachine,
            module: &terminal_psi::TerminalModule,
            position: usize,
            root: &checked_trees::OutcomeOccurrenceRoot,
        ) -> Result<terminal_psi::EvidenceArgumentSubject, LoweringError> {
            use terminal_psi::EvidenceArgumentSubject as Subject;
            let argument_position = u32::try_from(position).map_err(|_| {
                LoweringError::Unsupported("evidence argument position exceeds u32")
            })?;
            match root {
                checked_trees::OutcomeOccurrenceRoot::CalleeResult => {
                    return Ok(Subject::Result { argument_position });
                }
                checked_trees::OutcomeOccurrenceRoot::ClosedValue => {
                    return Ok(Subject::Closed { argument_position });
                }
                checked_trees::OutcomeOccurrenceRoot::Other => {
                    return Ok(Subject::Unknown { argument_position });
                }
                _ => {}
            }
            let Some(entry) = flow_facts(checked)
                .terminal_unit_effects
                .callable_entry_for_machine(machine)
            else {
                return Err(LoweringError::Unsupported(
                    "outcome-specific subject has no published callable entry",
                ));
            };
            let (parameter_position, structural_path) = match root {
                checked_trees::OutcomeOccurrenceRoot::CalleeParameter { source_position } => {
                    let Some(position) = entry
                        .scalar_parameters
                        .iter()
                        .position(|parameter| parameter.source_position == *source_position)
                    else {
                        return Ok(Subject::Unknown { argument_position });
                    };
                    (position, None)
                }
                checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection {
                    source_position,
                    fields,
                } => {
                    let position = entry
                            .structural_parameters
                            .iter()
                            .position(|parameter| parameter.position == *source_position)
                            .ok_or(LoweringError::Unsupported(
                                "outcome-specific projection names no structural parameter of its callee",
                            ))?;
                    let parameter = callee.structural_parameters.get(position).ok_or(
                        LoweringError::Unsupported(
                            "outcome-specific structural parameter position is absent",
                        ),
                    )?;
                    let path = fields
                        .iter()
                        .map(|field| {
                            field_path_identity(checked, *field).map(|identity| {
                                terminal_psi::StructuralPathSegment::Field(
                                    identities.intern(&identity.spelling()),
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if terminal_semantics::primitive_read_projection_type(
                        module.structural_types.iter(),
                        parameter.structural_type,
                        &path,
                    )
                    .is_none()
                    {
                        return Ok(Subject::Unknown { argument_position });
                    }
                    (position, Some(path))
                }
                checked_trees::OutcomeOccurrenceRoot::CalleeResult
                | checked_trees::OutcomeOccurrenceRoot::ClosedValue
                | checked_trees::OutcomeOccurrenceRoot::Other => {
                    unreachable!("non-parameter subjects returned above")
                }
            };
            Ok(Subject::Parameter(
                terminal_psi::OutcomeSpecificCallArgumentSubstitution {
                    argument_position,
                    parameter_position: u32::try_from(parameter_position).map_err(|_| {
                        LoweringError::Unsupported(
                            "outcome-specific parameter position exceeds u32",
                        )
                    })?,
                    structural_path,
                },
            ))
        }
    }
    pub(crate) mod outcome_specific_ensures {

        use crate::proofs::evidence_lowering::proof_output_calls::terminal_evidence_term_id;
        use crate::proofs::{
            BTreeMap, CheckedPropositionBinderArgumentKind, CheckedTrees, EvidenceTermDeclaration,
            EvidenceTermId, LoweringError, MachineId, OutcomeSpecificEnsure,
            OutcomeSpecificEvidence, OutcomeSpecificGuard, Proposition, StructuralTypeShape,
            TerminalModule, dense_identity, obligation_id, unsupported,
        };

        pub(crate) fn outcome_specific_ensure_rows(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            selected_machine: symbols::SymbolHandle,
            terminal_machine_id: MachineId,
            module: &TerminalModule,
            term_ids: &[Option<EvidenceTermId>],
            evidence_terms: &[EvidenceTermDeclaration],
        ) -> Result<Vec<OutcomeSpecificEnsure>, LoweringError> {
            let guarantees = checked
                .facts
                .proof
                .outcome_specific_guarantees
                .iter()
                .filter_map(|(_, guarantee)| {
                    (guarantee.machine_symbol == selected_machine).then_some(guarantee)
                })
                .collect::<Vec<_>>();
            if guarantees.is_empty() {
                return Ok(Vec::new());
            }
            let state = checked
                .typed
                .machine(selected_machine)
                .and_then(|machine| checked.typed.machine_states(machine).first())
                .ok_or(LoweringError::Unsupported(
                    "guarded payloadless producer state is absent",
                ))?;
            let checked_trees::types::TypeReferenceNode::Named {
                symbol: result_data,
                ..
            } = checked
                .typed
                .type_reference_table
                .type_reference(state.return_type)
            else {
                return unsupported("guarded payloadless producer result is not nominal");
            };
            let data = checked
                .typed
                .data_definition_by_symbol(*result_data)
                .ok_or(LoweringError::Unsupported(
                    "guarded payloadless producer result data is absent",
                ))?;
            let terminal_machine =
                module
                    .machine(terminal_machine_id)
                    .ok_or(LoweringError::Unsupported(
                        "guarded payloadless terminal machine is absent",
                    ))?;
            let terminal_result =
                terminal_machine
                    .result
                    .structural()
                    .ok_or(LoweringError::Unsupported(
                        "guarded payloadless terminal result is not structural",
                    ))?;
            let declaration = module
                .structural_type(terminal_result.structural_type)
                .ok_or(LoweringError::Unsupported(
                    "guarded payloadless terminal result type is absent",
                ))?;
            let StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return unsupported("guarded payloadless terminal result is not a sum");
            };

            let mut next_positions = BTreeMap::<OutcomeSpecificGuard, u32>::new();
            let mut rows = Vec::with_capacity(guarantees.len());
            for guarantee in guarantees {
                if guarantee.result_data != *result_data {
                    return unsupported("guarded guarantee references a foreign result sum");
                }
                let case_identity = checked
                    .typed
                    .data_members(data)
                    .iter()
                    .find_map(|member| {
                        let checked_trees::data::DataMember::Variant(variant) = member else {
                            return None;
                        };
                        (variant.symbol == guarantee.result_case).then(|| variant.member_identity())
                    })
                    .ok_or(LoweringError::Unsupported(
                        "guarded guarantee references an unknown result case",
                    ))?;
                let result_case = cases
                    .iter()
                    .find_map(|case| {
                        (case.identity == identities.intern(&case_identity.spelling()))
                            .then_some(case.id)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "guarded guarantee case is absent from the terminal result sum",
                    ))?;
                let guard = OutcomeSpecificGuard {
                    result_type: terminal_result.structural_type,
                    result_case,
                };
                let position = *next_positions.entry(guard).or_default();
                *next_positions
                    .get_mut(&guard)
                    .expect("guarded position was inserted") = position.checked_add(1).ok_or(
                    LoweringError::Unsupported("guarded guarantee position exceeds u32"),
                )?;
                let (proposition, evidence) = match (
                    guarantee.public_selector.as_ref(),
                    guarantee.evidence_term,
                ) {
                    (Some(selector), Some(term_handle)) => {
                        let checked_term = checked.facts.proof.evidence_terms.get(term_handle);
                        let checked_trees::domain::ProofFact::Proposition(application) =
                            checked.typed.proof_facts.get(guarantee.fact)
                        else {
                            return unsupported("named guarded guarantee is not nominal");
                        };
                        let normalized = checked
                            .typed
                            .normalize_nominal_proposition_application(application, None)
                            .ok_or(LoweringError::Unsupported(
                                "named guarded guarantee has no normalized proposition endpoint",
                            ))?;
                        if normalized.declaration != checked_term.proposition.declaration
                                || normalized.arguments != checked_term.proposition.arguments
                                || normalized.binder_arguments.len()
                                    != checked_term.proposition.binder_arguments.len()
                                || normalized
                                    .binder_arguments
                                    .iter()
                                    .zip(&checked_term.proposition.binder_arguments)
                                    .any(|(left, right)| {
                                        let kind = match left.kind {
                                    checked_trees::proposition::PropositionBinderArgumentKind::Type => {
                                        CheckedPropositionBinderArgumentKind::Type
                                    }
                                    checked_trees::proposition::PropositionBinderArgumentKind::Const => {
                                        CheckedPropositionBinderArgumentKind::Const
                                    }
                                    checked_trees::proposition::PropositionBinderArgumentKind::Machine => {
                                        CheckedPropositionBinderArgumentKind::Machine
                                    }
                                };
                                        kind != right.kind
                                            || left.identity != right.diagnostic_label
                                            || right.evidence_projection.is_some()
                                    })
                            {
                                return unsupported(
                                    "named guarded guarantee disagrees with its evidence term",
                                );
                            }
                        let term = terminal_evidence_term_id(
                            term_ids,
                            term_handle,
                            "guarded guarantee term has no terminal identity",
                        )?;
                        let declaration = evidence_terms
                            .iter()
                            .find(|declaration| declaration.id == term)
                            .ok_or(LoweringError::Unsupported(
                                "guarded guarantee term declaration is absent",
                            ))?;
                        (
                            Proposition::Atom(declaration.proposition),
                            Some(OutcomeSpecificEvidence {
                                term,
                                output_field: selector.clone(),
                                argument_subjects:
                                    super::outcome_specific_calls::lower_argument_subjects(
                                        identities,
                                        checked,
                                        selected_machine,
                                        terminal_machine,
                                        module,
                                        &guarantee.argument_roots,
                                    )?,
                            }),
                        )
                    }
                    (None, None) => {
                        if !guarantee.fact_is_truth {
                            return unsupported(
                                "unnamed guarded guarantee is outside the bounded truth proposition",
                            );
                        }
                        (Proposition::Truth, None)
                    }
                    _ => {
                        return unsupported(
                            "guarded guarantee has an incomplete evidence endpoint",
                        );
                    }
                };
                rows.push(OutcomeSpecificEnsure {
                    guard,
                    position,
                    obligation: obligation_id(1),
                    proposition,
                    evidence,
                });
            }
            rows.sort_by_key(|row| (row.guard, row.position));
            for (index, row) in rows.iter_mut().enumerate() {
                row.obligation = obligation_id(dense_identity(index)?);
            }
            Ok(rows)
        }
    }
    pub(crate) mod proof_output_calls {

        use crate::machine_lowering::declarations::semantic_identity;
        use crate::proofs::evidence_lowering::evidence_terms::{
            evidence_term_index, lower_evidence_interface,
        };
        use crate::proofs::evidence_lowering::{
            checked_evidence_machine_identity, checked_evidence_requirement_identity,
            checked_requirement_schema, checked_static_conformance_rows,
            evidence_projection_identity,
        };
        use crate::proofs::{
            CheckedTrees, EvidenceTermId, LoweringError, MachineId, PrimitiveType, ProofOutput,
            ProofOutputCall, ProofOutputRuntimeCall, PropositionApplicationIdentity,
            PropositionBinderArgumentIdentity, PropositionDeclaration, PropositionId,
            StaticRequirementDispatch, TerminalModule, terminal_scalar_type, unsupported,
        };
        use crate::retention::checked_method_arguments;
        use terminal_psi::ProofOutputRuntimeResult;

        pub(crate) fn lower_proof_output_calls(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            selected_machine: symbols::SymbolHandle,
            terminal_machine: MachineId,
            semantic_module: &TerminalModule,
            occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
            term_ids: &[Option<EvidenceTermId>],
            declarations: &[PropositionDeclaration],
            applications: &[PropositionApplicationIdentity],
        ) -> Result<Vec<ProofOutputCall>, LoweringError> {
            let mut invocations = checked
                .facts
                .proof
                .proof_output_calls
                .iter()
                .filter_map(|(_, invocation)| {
                    (invocation.caller_machine_symbol == selected_machine).then_some(invocation)
                })
                .enumerate()
                .map(|(ordinal, invocation)| {
                    let (runtime_result, runtime_call) = lower_proof_output_runtime_call(
                        checked,
                        terminal_machine,
                        semantic_module,
                        occurrences,
                        invocation,
                    )?;
                    let static_requirement_dispatch = lower_static_requirement_dispatch(
                        identities,
                        checked,
                        terminal_machine,
                        semantic_module,
                        invocation,
                        runtime_result,
                        runtime_call,
                    )?;
                    let evidence_arguments = invocation
                        .evidence_arguments
                        .iter()
                        .map(|argument| {
                            Ok(terminal_psi::ProofOutputEvidenceArgument {
                                input_position: u32::try_from(argument.input_position).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "terminal proof-output input position exceeds u32",
                                    )
                                })?,
                                callee_proposition: terminal_proposition_application_id(
                                    identities,
                                    checked,
                                    term_ids,
                                    declarations,
                                    applications,
                                    &checked
                                        .facts
                                        .proof
                                        .evidence_terms
                                        .get(argument.callee_input)
                                        .proposition,
                                )?,
                                source: terminal_evidence_term_id(
                                    term_ids,
                                    argument.source,
                                    "terminal proof-output source input has no canonical identity",
                                )?,
                                instantiated_proposition: terminal_proposition_application_id(
                                    identities,
                                    checked,
                                    term_ids,
                                    declarations,
                                    applications,
                                    &argument.instantiated_proposition,
                                )?,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    let outputs = invocation
                        .outputs
                        .iter()
                        .map(|output| {
                            let callee_output =
                                checked.facts.proof.evidence_terms.get(output.callee_output);
                            let forwarded_input_position =
                                proof_output_forwarded_input_position(checked, invocation, output)?;
                            Ok(ProofOutput {
                                output_position: u32::try_from(output.output_position).map_err(|_| {
                                    LoweringError::Unsupported("terminal proof-output position exceeds u32")
                                })?,
                                output_field: callee_output.name.clone(),
                                callee_proposition: terminal_proposition_application_id(
                                    identities,
                                    checked,
                                    term_ids,
                                    declarations,
                                    applications,
                                    &callee_output.proposition,
                                )?,
                                callee_output: (forwarded_input_position.is_none()
                                    && invocation.static_requirement_dispatch.is_none())
                                .then(|| {
                                    terminal_evidence_term_id(
                                        term_ids,
                                        output.callee_output,
                                        "terminal proof-output callee term has no canonical identity",
                                    )
                                })
                                .transpose()?,
                                instantiated_proposition: terminal_proposition_application_id(
                                    identities,
                                    checked,
                                    term_ids,
                                    declarations,
                                    applications,
                                    &output.instantiated_proposition,
                                )?,
                                forwarded_input_position,
                                output: output
                                    .output
                                    .map(|output| {
                                        term_ids
                                            .get(evidence_term_index(output))
                                            .copied()
                                            .flatten()
                                            .ok_or(LoweringError::Unsupported(
                                                "terminal proof-output term has no canonical identity",
                                            ))
                                    })
                                    .transpose()?,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    Ok(ProofOutputCall {
                        caller: terminal_machine,
                        ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("terminal proof-output invocation count exceeds u32")
                        })?,
                        target_machine_identity: static_requirement_dispatch
                            .as_ref()
                            .map(|dispatch| Ok(dispatch.public_requirement_identity))
                            .unwrap_or_else(|| {
                                checked_evidence_machine_identity(checked, invocation.target_machine_symbol)
                                    .map(|identity| identities.intern(&identity))
                            })?,
                        static_requirement_dispatch,
                        runtime_result,
                        runtime_call,
                        evidence_arguments,
                        outputs,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            invocations.sort_unstable();
            Ok(invocations)
        }

        fn lower_static_requirement_dispatch(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            terminal_machine: MachineId,
            semantic_module: &TerminalModule,
            invocation: &checked_trees::ProofOutputCallFact,
            runtime_result: Option<ProofOutputRuntimeResult>,
            runtime_call: Option<ProofOutputRuntimeCall>,
        ) -> Result<Option<StaticRequirementDispatch>, LoweringError> {
            let Some(dispatch) = invocation.static_requirement_dispatch else {
                return Ok(None);
            };
            if dispatch.application_report_fingerprint == 0
                || dispatch.application_commitment.is_zero()
            {
                return unsupported(
                    "static requirement proof output lost its exact checked realization",
                );
            }
            let bounded_result = matches!(runtime_result, Some(ProofOutputRuntimeResult::Unit))
                || matches!(
                    runtime_result,
                    Some(ProofOutputRuntimeResult::Scalar(scalar))
                        if scalar == terminal_scalar_type(PrimitiveType::I32)?
                            || scalar == terminal_scalar_type(PrimitiveType::Bool)?
                );
            let Some(runtime_call) = runtime_call else {
                return unsupported(
                    "static requirement proof output has no bounded ordinary runtime call",
                );
            };
            if !bounded_result {
                return unsupported(
                    "static requirement proof output is outside the bounded runtime Unit, exact i32, or exact bool call",
                );
            }
            let declaring_trait_identity =
                checked.symbols.display_path(dispatch.declaring_trait, "::");
            let public_requirement_identity = checked_evidence_requirement_identity(
                checked,
                dispatch.declaring_trait,
                dispatch.requirement,
            )?;
            let requirement_identity = checked.symbols.display_path(dispatch.requirement, "::");
            let realization_identity = checked
                .symbols
                .display_path(dispatch.realization_state, "::");
            if declaring_trait_identity.is_empty()
                || public_requirement_identity.is_empty()
                || requirement_identity.is_empty()
                || realization_identity.is_empty()
            {
                return unsupported(
                    "static requirement proof output has an empty dispatch identity",
                );
            }
            let checked_applications = checked.typed.selected_conformance_applications(
                invocation.caller_machine_symbol,
                dispatch.selection_application,
                dispatch.evidence_parameter,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [checked_application] = checked_applications.as_slice() else {
                return unsupported(
                    "static requirement proof output has no unique checked conformance application",
                );
            };
            let expected_telescope =
                crate::retention::checked_application_telescope(checked, checked_application)?;
            let mut expected_schemas = checked_application
                .rows
                .iter()
                .map(|row| {
                    checked_requirement_schema(
                        identities,
                        checked,
                        row.declaring_trait,
                        row.requirement,
                        row.realization_state,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            expected_schemas.sort_by(|left, right| {
                (
                    identities.text(left.declaring_trait_identity),
                    identities.text(left.public_requirement_identity),
                )
                    .cmp(&(
                        identities.text(right.declaring_trait_identity),
                        identities.text(right.public_requirement_identity),
                    ))
            });
            let declaration_identity = checked
                .symbols
                .display_path(checked_application.declaration, "::");
            let trait_identity = checked
                .symbols
                .display_path(checked_application.trait_definition, "::");
            let mut expected_rows = Vec::new();
            for row in &checked_application.rows {
                let declaring_trait_identity =
                    checked.symbols.display_path(row.declaring_trait, "::");
                let public_requirement_identity = checked_evidence_requirement_identity(
                    checked,
                    row.declaring_trait,
                    row.requirement,
                )?;
                let requirement_identity = checked.symbols.display_path(row.requirement, "::");
                for family_row in checked_static_conformance_rows(
                    checked,
                    row.declaring_trait,
                    row.requirement,
                    row.realization_machine,
                    row.realization_state,
                )? {
                    expected_rows.push(terminal_psi::ClosedConformanceRow {
                        requirement_declaration: terminal_psi::NominalDeclarationIdentity::new(
                            semantic_identity(identities, checked, row.requirement).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "conformance requirement lost its declaration owner",
                                    )
                                },
                            )?,
                        ),
                        declaring_trait_identity: identities
                            .intern(&declaring_trait_identity.clone()),
                        public_requirement_identity: identities
                            .intern(&public_requirement_identity.clone()),
                        family_tuple: family_row
                            .family_tuple
                            .iter()
                            .map(|identity| identities.intern(identity))
                            .collect(),
                        requirement_identity: identities.intern(&requirement_identity.clone()),
                        realization_identity: identities.intern(
                            &checked
                                .symbols
                                .display_path(family_row.realization_state, "::"),
                        ),
                        realization_callable_identity: None,
                    });
                }
            }
            let terminal_applications = semantic_module
                .closed_conformance_applications
                .iter()
                .filter(|application| {
                    application.owner == terminal_machine
                        && application.declaration_identity
                            == identities.intern(&declaration_identity)
                        && application.telescope == expected_telescope
                        && application.schemas == expected_schemas
                        && application.trait_lifetime_arguments
                            == checked_application.trait_lifetime_arguments
                        && application.trait_identity == identities.intern(&trait_identity)
                        && application.trait_arguments == checked_application.trait_arguments
                        && application.rows.len() == expected_rows.len()
                        && application
                            .rows
                            .iter()
                            .zip(&expected_rows)
                            .all(|(actual, expected)| {
                                actual.declaring_trait_identity == expected.declaring_trait_identity
                                    && actual.public_requirement_identity
                                        == expected.public_requirement_identity
                                    && actual.family_tuple == expected.family_tuple
                                    && actual.requirement_identity == expected.requirement_identity
                                    && actual.realization_identity == expected.realization_identity
                            })
                })
                .collect::<Vec<_>>();
            let [application] = terminal_applications.as_slice() else {
                return unsupported(
                    "static requirement proof output has no unique lowered conformance application",
                );
            };
            if application.report_fingerprint == 0 || application.commitment.is_zero() {
                return Err(LoweringError::Unsupported(
                    "static requirement proof output lost its closed conformance application",
                ));
            }
            if application.trait_identity != identities.intern(&declaring_trait_identity) {
                return unsupported(
                    "static requirement proof output selected a different conformance trait",
                );
            }
            let realization_callable_identity =
                checked_evidence_machine_identity(checked, invocation.target_machine_symbol)?;
            let expected_commitment = checked
                .facts
                .specialization_identities
                .for_instance(invocation.target_machine_symbol)
                .map(|identity| identity.commitment.as_bytes());
            let expected_arguments =
                checked_method_arguments(identities, checked, invocation.target_machine_symbol)?;
            let mut applications = application.static_applications.iter().filter(|applied| {
                applied.public_requirement_identity
                    == identities.intern(&public_requirement_identity)
                    && applied.realization_identity == identities.intern(&realization_identity)
                    && applied.source_callable_identity
                        == identities.intern(&realization_callable_identity)
                    && applied.callable
                        == terminal_psi::TargetMachineCallable::Machine(runtime_call.callee)
                    && applied.application_commitment == expected_commitment
                    && applied.method_arguments == expected_arguments
            });
            if applications.next().is_none() || applications.next().is_some() {
                return unsupported(
                    "static requirement proof output lost its exact checked method application",
                );
            }
            let mut callables = application.realization_callables.iter().filter(|callable| {
                callable.source_callable_identity
                    == identities.intern(&realization_callable_identity)
                    && callable.callable
                        == terminal_psi::TargetMachineCallable::Machine(runtime_call.callee)
            });
            if callables.next().is_none() || callables.next().is_some() {
                return unsupported(
                    "static requirement proof output lost its independent callable registry entry",
                );
            }
            Ok(Some(StaticRequirementDispatch {
                conformance_application_report_fingerprint: application.report_fingerprint,
                conformance_application_commitment: application.commitment,
                public_requirement_identity: identities.intern(&public_requirement_identity),
                declaring_trait_identity: identities.intern(&declaring_trait_identity),
                requirement_identity: identities.intern(&requirement_identity),
                realization_identity: identities.intern(&realization_identity),
                realization_callable_identity: identities.intern(&realization_callable_identity),
                realization: runtime_call.callee,
            }))
        }

        pub(crate) fn proof_output_forwarded_source(
            checked: &CheckedTrees,
            invocation: &checked_trees::ProofOutputCallFact,
            output: &checked_trees::ProofOutputFact,
        ) -> Option<arena::Handle<checked_trees::CheckedEvidenceTerm>> {
            proof_output_forwarded_argument(checked, invocation, output)
                .map(|argument| argument.source)
        }

        fn proof_output_forwarded_argument<'a>(
            checked: &CheckedTrees,
            invocation: &'a checked_trees::ProofOutputCallFact,
            output: &checked_trees::ProofOutputFact,
        ) -> Option<&'a checked_trees::ProofOutputEvidenceArgumentFact> {
            if invocation.static_requirement_dispatch.is_some() {
                return None;
            }
            let source =
                checked
                    .facts
                    .proof
                    .evidence_forwardings
                    .iter()
                    .find_map(|(_, forwarding)| {
                        (forwarding.machine_symbol == invocation.target_machine_symbol
                            && forwarding.output == output.callee_output)
                            .then_some(&forwarding.source)
                    });
            let Some(checked_trees::EvidenceAssignmentSource::Forwarded { term }) = source else {
                return None;
            };
            invocation
                .evidence_arguments
                .iter()
                .find(|argument| argument.callee_input == *term)
        }

        fn proof_output_forwarded_input_position(
            checked: &CheckedTrees,
            invocation: &checked_trees::ProofOutputCallFact,
            output: &checked_trees::ProofOutputFact,
        ) -> Result<Option<u32>, LoweringError> {
            let Some(argument) = proof_output_forwarded_argument(checked, invocation, output)
            else {
                return Ok(None);
            };
            u32::try_from(argument.input_position)
                .map(Some)
                .map_err(|_| {
                    LoweringError::Unsupported("proof-output forwarded input position exceeds u32")
                })
        }

        pub(crate) fn terminal_evidence_term_id(
            term_ids: &[Option<EvidenceTermId>],
            handle: arena::Handle<checked_trees::CheckedEvidenceTerm>,
            error: &'static str,
        ) -> Result<EvidenceTermId, LoweringError> {
            term_ids
                .get(evidence_term_index(handle))
                .copied()
                .flatten()
                .ok_or(LoweringError::Unsupported(error))
        }

        pub(crate) fn terminal_proposition_application_id(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            term_ids: &[Option<EvidenceTermId>],
            declarations: &[PropositionDeclaration],
            applications: &[PropositionApplicationIdentity],
            application: &checked_trees::CheckedPropositionApplication,
        ) -> Result<PropositionId, LoweringError> {
            let declaration_name = checked
                .facts
                .proof
                .proposition_vocabulary
                .declarations
                .iter()
                .find_map(|declaration| {
                    (declaration.symbol == application.declaration)
                        .then_some(declaration.name.as_str())
                })
                .ok_or(LoweringError::Unsupported(
                    "proof-output proposition has no checked declaration",
                ))?;
            let declaration = declarations
                .iter()
                .find_map(|declaration| {
                    (declaration.name == declaration_name).then_some(declaration.id)
                })
                .ok_or(LoweringError::Unsupported(
                    "proof-output proposition has no terminal declaration",
                ))?;
            let binder_arguments = application
                .binder_arguments
                .iter()
                .map(|argument| {
                    let evidence_projection = argument
                        .evidence_projection
                        .as_ref()
                        .map(|projection| {
                            evidence_projection_identity(
                                identities,
                                checked,
                                projection,
                                terminal_evidence_term_id(
                                    term_ids,
                                    projection.term,
                                    "proof-output proposition projects an unrelated evidence term",
                                )?,
                            )
                        })
                        .transpose()?;
                    Ok(PropositionBinderArgumentIdentity {
                        kind: super::binder_argument_kind(argument.kind),
                        identity: identities.intern(&argument.diagnostic_label.clone()),
                        evidence_projection,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let evidence_interface = application
                .evidence_interface
                .as_ref()
                .map(|interface| lower_evidence_interface(identities, checked, interface))
                .transpose()?;
            applications
                .iter()
                .find_map(|candidate| {
                    (candidate.declaration == declaration
                        && candidate.binder_arguments == binder_arguments
                        && candidate.arguments == application.arguments
                        && candidate.evidence_interface == evidence_interface)
                        .then_some(candidate.id)
                })
                .ok_or(LoweringError::Unsupported(
                    "proof-output proposition has no terminal application",
                ))
        }

        fn lower_proof_output_runtime_call(
            checked: &CheckedTrees,
            terminal_machine: MachineId,
            semantic_module: &TerminalModule,
            occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
            invocation: &checked_trees::ProofOutputCallFact,
        ) -> Result<
            (
                Option<ProofOutputRuntimeResult>,
                Option<ProofOutputRuntimeCall>,
            ),
            LoweringError,
        > {
            let Some(runtime_call) = invocation.runtime_call else {
                return Ok((None, None));
            };
            let mut matching = occurrences.iter().filter(|occurrence| {
                occurrence.matches_statement(
                    invocation.caller_state_symbol,
                    runtime_call.statement_index,
                    runtime_call.call_ordinal,
                )
            });
            let occurrence = matching.next().ok_or(LoweringError::Unsupported(
                "runtime proof-output call has no emitted terminal operation",
            ))?;
            if matching.next().is_some() {
                return unsupported("runtime proof-output call coordinate is not unique");
            }
            let target = crate::shared::call_sites::selected_module_target(checked, occurrence)?;
            let caller =
                semantic_module
                    .machine(terminal_machine)
                    .ok_or(LoweringError::Unsupported(
                        "runtime proof-output caller is absent from terminal Psi",
                    ))?;
            let emitted = caller
                .module_invocations()
                .find(|emitted| emitted.site == occurrence.terminal_site)
                .ok_or(LoweringError::Unsupported(
                    "runtime proof-output call has no exact emitted invocation",
                ))?;
            if matches!(
                occurrence.terminal_site,
                semantic_vocabulary::InvocationSite::InvokeNoNormal(_)
            ) {
                return Ok((
                    Some(ProofOutputRuntimeResult::NoNormal),
                    Some(ProofOutputRuntimeCall {
                        site: occurrence.terminal_site,
                        callee: emitted.inputs.callee,
                    }),
                ));
            }
            let expected = if target.state.return_type.is_valid() {
                Some(
                    checked
                        .typed
                        .primitive_type_reference(target.state.return_type)
                        .map(terminal_scalar_type)
                        .transpose()?
                        .ok_or(LoweringError::Unsupported(
                            "runtime proof-output target is not scalar-result",
                        ))?,
                )
            } else {
                None
            };
            let semantic_vocabulary::InvocationSite::Operation(operation_id) =
                occurrence.terminal_site
            else {
                return unsupported("runtime proof-output call completes the machine");
            };
            let operation = semantic_module
                .machine(terminal_machine)
                .ok_or(LoweringError::Unsupported(
                    "runtime proof-output caller is absent from terminal Psi",
                ))?
                .operation(operation_id)
                .ok_or(LoweringError::Unsupported(
                    "runtime proof-output call has no emitted terminal operation",
                ))?;
            let parts = crate::shared::direct_calls::direct_call_parts(&operation.kind).ok_or(
                LoweringError::Unsupported("runtime proof-output operation is not a direct call"),
            )?;
            let result = match (parts.role, expected, operation.result.scalar()) {
                (crate::shared::direct_calls::DirectCallRole::Unit, None, None) => {
                    ProofOutputRuntimeResult::Unit
                }
                (
                    crate::shared::direct_calls::DirectCallRole::Scalar,
                    Some(expected),
                    Some(result),
                ) if result.scalar_type == expected => ProofOutputRuntimeResult::Scalar(expected),
                _ => {
                    return unsupported(
                        "runtime proof-output operation disagrees with its target result",
                    );
                }
            };
            Ok((
                Some(result),
                Some(ProofOutputRuntimeCall {
                    site: semantic_vocabulary::InvocationSite::Operation(operation.id),
                    callee: parts.callee,
                }),
            ))
        }
    }

    use super::{CheckedTrees, EvidenceTermId, LoweredPsi, LoweringError, unsupported};
    use crate::proofs::evidence_lowering::evidence_terms::{
        lower_evidence_contract_lanes, lower_evidence_term_ids, lower_evidence_terms,
        lower_proposition_vocabulary,
    };
    use crate::proofs::evidence_lowering::outcome_specific_calls::lower_outcome_specific_call_evidence;
    use crate::proofs::evidence_lowering::outcome_specific_ensures::outcome_specific_ensure_rows;
    use crate::proofs::evidence_lowering::proof_output_calls::lower_proof_output_calls;
    use std::collections::BTreeMap;

    pub(crate) fn lower_and_install_evidence_artifacts(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        source_machine_ids: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
        lowered: &mut LoweredPsi,
    ) -> Result<(), LoweringError> {
        for (position, (source, terminal)) in source_machine_ids.iter().enumerate() {
            if source_machine_ids[..position]
                .iter()
                .any(|(previous_source, previous_terminal)| {
                    previous_source == source || previous_terminal == terminal
                })
            {
                return unsupported("evidence body mapping repeats a source or terminal machine");
            }
            if lowered.semantic_module.machine(*terminal).is_none() {
                return unsupported("evidence body mapping names an absent terminal machine");
            }
        }
        let mut term_ids = lower_evidence_term_ids(checked, source_machine_ids)?.term_ids;
        let mut rounds = 0_u8;
        let (declarations, applications, evidence_terms) = loop {
            let (declarations, applications, declaration_ids) =
                lower_proposition_vocabulary(identities, checked, &term_ids)?;
            let evidence_terms = lower_evidence_terms(
                identities,
                checked,
                &declaration_ids,
                &applications,
                term_ids.clone(),
            )?;
            let mut canonical = evidence_terms
                .declarations
                .iter()
                .map(|term| (term.proposition, term.interface.clone(), term.id))
                .collect::<Vec<_>>();
            canonical.sort();
            let renumbered = canonical
                .iter()
                .enumerate()
                .map(|(position, (_, _, previous))| {
                    let id = EvidenceTermId::new(
                        u64::try_from(position)
                            .expect("evidence term count fits u64")
                            .checked_add(1)
                            .expect("one-based evidence term identity fits u64"),
                    )
                    .expect("one-based evidence term identity is nonzero");
                    (*previous, id)
                })
                .collect::<BTreeMap<_, _>>();
            if renumbered.iter().all(|(previous, id)| previous == id) {
                break (declarations, applications, evidence_terms);
            }
            rounds += 1;
            if rounds > 4 {
                return unsupported("evidence term identities reach no canonical order");
            }
            term_ids = term_ids
                .iter()
                .map(|id| id.map(|previous| renumbered[&previous]))
                .collect();
        };
        let mut evidence_contract_lanes = Vec::new();
        let mut proof_output_calls = Vec::new();
        for &(machine, terminal_machine) in source_machine_ids {
            let outcome_specific_ensures = outcome_specific_ensure_rows(
                identities,
                checked,
                machine,
                terminal_machine,
                &lowered.semantic_module,
                &evidence_terms.term_ids,
                &evidence_terms.declarations,
            )?;
            evidence_contract_lanes.extend(lower_evidence_contract_lanes(
                identities,
                checked,
                &lowered.semantic_module,
                machine,
                terminal_machine,
                &evidence_terms.term_ids,
            )?);
            proof_output_calls.extend(lower_proof_output_calls(
                identities,
                checked,
                machine,
                terminal_machine,
                &lowered.semantic_module,
                &lowered.source_call_occurrences,
                &evidence_terms.term_ids,
                &declarations,
                &applications,
            )?);
            lower_outcome_specific_call_evidence(
                identities,
                checked,
                machine,
                lowered,
                &evidence_terms.term_ids,
                &declarations,
                &applications,
                &evidence_terms.declarations,
            )?;
            let body = lowered
                .semantic_module
                .machines
                .iter_mut()
                .find(|candidate| candidate.id == terminal_machine)
                .ok_or(LoweringError::Unsupported(
                    "terminal body is absent while installing guarded guarantees",
                ))?;
            body.contract.outcome_specific_ensures = outcome_specific_ensures;
        }
        evidence_contract_lanes.sort_unstable();
        proof_output_calls.sort_unstable();
        lowered.semantic_module.proposition_declarations = declarations;
        lowered.semantic_module.proposition_applications = applications;
        lowered.semantic_module.evidence_terms = evidence_terms.declarations;
        lowered.semantic_module.evidence_contract_lanes = evidence_contract_lanes;
        lowered.semantic_module.proof_output_calls = proof_output_calls;
        Ok(())
    }

    pub(crate) fn binder_argument_kind(
        kind: crate::proofs::CheckedPropositionBinderArgumentKind,
    ) -> terminal_psi::PropositionBinderArgumentKind {
        match kind {
            crate::proofs::CheckedPropositionBinderArgumentKind::Type => {
                terminal_psi::PropositionBinderArgumentKind::Type
            }
            crate::proofs::CheckedPropositionBinderArgumentKind::Const => {
                terminal_psi::PropositionBinderArgumentKind::Const
            }
            crate::proofs::CheckedPropositionBinderArgumentKind::Machine => {
                terminal_psi::PropositionBinderArgumentKind::Machine
            }
        }
    }

    pub(crate) fn checked_evidence_requirement_identity(
        checked: &CheckedTrees,
        declaring_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
    ) -> Result<String, LoweringError> {
        Ok(checked.typed.callable_identity_text(
            exact_trait_requirement(checked, declaring_trait, requirement)?.overload_identity,
        ))
    }

    pub(crate) fn evidence_projection_identity(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        projection: &checked_trees::CheckedEvidenceProjection,
        term: crate::proofs::EvidenceTermId,
    ) -> Result<crate::proofs::EvidenceProjectionIdentity, LoweringError> {
        Ok(crate::proofs::EvidenceProjectionIdentity {
            term,
            declaring_trait_identity: identities.intern(
                &checked
                    .symbols
                    .display_path(projection.declaring_trait, "::"),
            ),
            declaring_trait_arguments: projection.declaring_trait_arguments.clone(),
            requirement_identity: identities.intern(&checked_evidence_requirement_identity(
                checked,
                projection.declaring_trait,
                projection.requirement,
            )?),
        })
    }

    pub(crate) struct CheckedRequirementFamilyRow {
        pub family_tuple: Vec<String>,
        pub realization_machine: symbols::SymbolHandle,
        pub realization_state: symbols::SymbolHandle,
    }

    pub(crate) fn checked_requirement_schema(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        declaring_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
        realization_state: symbols::SymbolHandle,
    ) -> Result<terminal_psi::ClosedConformanceSchema, LoweringError> {
        let exact = exact_trait_requirement(checked, declaring_trait, requirement)?;
        let kind = if !exact.declares_local_generic_binders {
            terminal_psi::ClosedConformanceSchemaKind::Nongeneric
        } else {
            match checked.typed.finite_signature_family(exact.signature) {
                checked_trees::finite_family::FamilyProbe::Finite { .. } => {
                    terminal_psi::ClosedConformanceSchemaKind::Finite
                }
                checked_trees::finite_family::FamilyProbe::NotFinite(_) => {
                    terminal_psi::ClosedConformanceSchemaKind::Open
                }
            }
        };
        let definition = validation::contract_identities::canonical_requirement_schema_bytes(
            checked,
            exact.signature,
        )
        .map_err(|_| {
            LoweringError::Unsupported("original conformance schema contract cannot be encoded")
        })?;
        let parameter_kinds = checked
            .typed
            .state_signature_type_parameters(exact.signature)
            .iter()
            .map(|parameter| match parameter.kind {
                checked_trees::data::TypeParameterKind::Type => 1,
                checked_trees::data::TypeParameterKind::Const { .. } => 2,
                checked_trees::data::TypeParameterKind::Value { .. } => 3,
                checked_trees::data::TypeParameterKind::Machine { .. } => 4,
                checked_trees::data::TypeParameterKind::Implementation => 5,
                checked_trees::data::TypeParameterKind::Proposition { .. } => 6,
            })
            .collect();
        let finite_tuples = if kind == terminal_psi::ClosedConformanceSchemaKind::Finite {
            match checked.typed.finite_signature_family(exact.signature) {
                checked_trees::finite_family::FamilyProbe::Finite { mut tuples, .. } => {
                    tuples.sort_by(|left, right| left.identities.cmp(&right.identities));
                    Some(
                        tuples
                            .iter()
                            .map(|tuple| {
                                tuple
                                    .identities
                                    .iter()
                                    .map(|identity| identities.intern(identity))
                                    .collect()
                            })
                            .collect(),
                    )
                }
                _ => {
                    return unsupported(
                        "finite conformance schema lost its original declared roster",
                    );
                }
            }
        } else {
            None
        };
        Ok(terminal_psi::ClosedConformanceSchema {
            requirement_declaration: terminal_psi::NominalDeclarationIdentity::new(
                semantic_identity(identities, checked, requirement).map_err(|_| {
                    LoweringError::Unsupported("conformance requirement lost its declaration owner")
                })?,
            ),
            declaring_trait_identity: identities
                .intern(&checked.symbols.display_path(declaring_trait, "::")),
            public_requirement_identity: identities.intern(&checked_evidence_requirement_identity(
                checked,
                declaring_trait,
                requirement,
            )?),
            requirement_identity: identities
                .intern(&checked.symbols.display_path(requirement, "::")),
            realization_identity: identities
                .intern(&checked.symbols.display_path(realization_state, "::")),
            definition,
            kind,
            lifetime_parameter_count: exact
                .signature
                .lifetime_parameters
                .len()
                .try_into()
                .map_err(|_| {
                    LoweringError::Unsupported(
                        "conformance method lifetime telescope exceeds portable positions",
                    )
                })?,
            parameter_kinds,
            finite_tuples,
        })
    }

    pub(crate) fn checked_static_conformance_rows(
        checked: &CheckedTrees,
        declaring_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
        realization_machine: symbols::SymbolHandle,
        realization_state: symbols::SymbolHandle,
    ) -> Result<Vec<CheckedRequirementFamilyRow>, LoweringError> {
        let exact = exact_trait_requirement(checked, declaring_trait, requirement)?;
        if exact.declares_local_generic_binders
            && matches!(
                checked.typed.finite_signature_family(exact.signature),
                checked_trees::finite_family::FamilyProbe::NotFinite(_)
            )
        {
            return Ok(Vec::new());
        }
        checked_requirement_family_rows(
            checked,
            declaring_trait,
            requirement,
            realization_machine,
            realization_state,
        )
    }

    pub(crate) fn checked_requirement_family_rows(
        checked: &CheckedTrees,
        declaring_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
        realization_machine: symbols::SymbolHandle,
        realization_state: symbols::SymbolHandle,
    ) -> Result<Vec<CheckedRequirementFamilyRow>, LoweringError> {
        let exact = exact_trait_requirement(checked, declaring_trait, requirement)?;
        if !exact.declares_local_generic_binders {
            return Ok(vec![CheckedRequirementFamilyRow {
                family_tuple: Vec::new(),
                realization_machine,
                realization_state,
            }]);
        }
        let tuples = match checked.typed.finite_signature_family(exact.signature) {
            checked_trees::finite_family::FamilyProbe::Finite { tuples, .. } => tuples,
            checked_trees::finite_family::FamilyProbe::NotFinite(_) => {
                return unsupported(
                    "conformance requirement declares requirement-local generic binders without a \
                     finite family roster",
                );
            }
        };
        let template =
            checked
                .typed
                .machine(realization_machine)
                .ok_or(LoweringError::Unsupported(
                    "conformance family row has no exact realization template machine",
                ))?;
        let state_position = typed_trees::state::state_position(
            checked.typed.machine_states(template),
            realization_state,
        )
        .ok_or(LoweringError::Unsupported(
            "conformance family row has no exact realization template state",
        ))?;
        tuples
            .iter()
            .map(|tuple| {
                let mut specializations =
                    checked
                        .typed
                        .machine_specializations
                        .iter()
                        .filter(|specialization| {
                            specialization.template == realization_machine
                                && specialization.const_argument_identities.as_slice()
                                    == tuple.identities.as_ref()
                                && specialization.is_const_only()
                        });
                let Some(specialization) = specializations.next() else {
                    return unsupported(
                        "conformance family tuple has no bare value-tuple specialization of the \
                         row's provider",
                    );
                };
                if specializations.next().is_some() {
                    return unsupported(
                        "conformance family tuple resolves to ambiguous provider specializations",
                    );
                }
                let instance = checked.typed.machine(specialization.instance).ok_or(
                    LoweringError::Unsupported(
                        "conformance family specialization has no retained instance machine",
                    ),
                )?;
                let instance_state = checked
                    .typed
                    .machine_states(instance)
                    .get(state_position)
                    .ok_or(LoweringError::Unsupported(
                        "conformance family specialization lost its tuple state",
                    ))?;
                Ok(CheckedRequirementFamilyRow {
                    family_tuple: tuple.identities.to_vec(),
                    realization_machine: instance.symbol,
                    realization_state: instance_state.symbol,
                })
            })
            .collect()
    }

    struct ExactTraitRequirement<'a> {
        signature: &'a checked_trees::signature::StateSignature,
        overload_identity: typed_trees::type_identity::CallableIdentityId,
        declares_local_generic_binders: bool,
    }

    fn exact_trait_requirement<'a>(
        checked: &'a CheckedTrees,
        declaring_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
    ) -> Result<ExactTraitRequirement<'a>, LoweringError> {
        let (definition, signature) = checked
            .typed
            .trait_machine_signature(requirement)
            .filter(|(definition, _)| definition.symbol == declaring_trait)
            .ok_or(LoweringError::Unsupported(
                "evidence producer row has no exact trait requirement",
            ))?;
        let overload_identity = checked
            .typed
            .trait_requirement_callable_identity(definition, signature);
        if !overload_identity.is_valid() {
            return unsupported("evidence producer row has an empty requirement identity");
        }
        Ok(ExactTraitRequirement {
            signature,
            overload_identity,
            declares_local_generic_binders: !signature.lifetime_parameters.is_empty()
                || !checked
                    .typed
                    .state_signature_type_parameters(signature)
                    .is_empty(),
        })
    }

    pub(crate) fn checked_evidence_machine_identity(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
    ) -> Result<String, LoweringError> {
        let mut matches = checked
            .typed
            .machines()
            .iter()
            .filter(|candidate| candidate.symbol == machine);
        let machine = matches.next().ok_or(LoweringError::Unsupported(
            "evidence producer row has no exact realization machine",
        ))?;
        if matches.next().is_some() {
            return unsupported("evidence producer row has an ambiguous realization machine");
        }
        let mut identity = checked
            .typed
            .callable_identity_text(checked.typed.machine_callable_identity(machine));
        if checked
            .typed
            .executable_application_template(machine.symbol)
            .map_err(LoweringError::Unsupported)?
            .is_some()
        {
            let checked_identity = checked
                .facts
                .specialization_identities
                .for_instance(machine.symbol)
                .ok_or(LoweringError::Unsupported(
                    "evidence machine lost its checked executable application identity",
                ))?;
            identity = terminal_psi::CheckedMachineDeclaration::callable_evidence_identity(
                &identity,
                Some(&checked_identity.commitment.as_bytes()),
            );
        }
        Ok(identity)
    }
}
pub(crate) mod float_meaning_projection {

    use crate::shared::checked_facts::flow_facts;
    use checked_trees::{
        CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
        CheckedFloatMeaningProjectionError, CheckedFloatProjectionSource,
        CheckedFloatSemanticApplication, CheckedFloatSemanticApplicationError,
        CheckedFloatSemanticApplicationOperand, CheckedProofOnlyValueType, CheckedTrees,
        types::PrimitiveType,
    };
    use semantic_vocabulary::{IeeeFloatFormat, MachineId, ScalarType};
    use terminal_psi::{
        DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatParameter,
        DirectMachineFloatResult, DirectOperationFloatResult, DirectStructuralFloatLeaf,
        FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
        FloatMeaningSource, FloatProjectionContractIdentity, FloatProjectionInput,
        FloatProjectionInputId, FloatSemanticApplication, FloatSemanticApplicationOperand,
        FloatSemanticContractIdentity, ProofOnlyValueType, ProofPropositionId,
        ProofValueDeclaration, ProofValueId, TerminalMachine, TerminalMachineResult,
    };

    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::shared::lowering_error::LoweringError;

    pub fn lower_float_meaning_equality(
        checked: CheckedFloatMeaningEqualityProposition,
    ) -> FloatMeaningEqualityProposition {
        FloatMeaningEqualityProposition {
            id: ProofPropositionId(checked.id.0),
            left: ProofValueId(checked.left.0),
            right: ProofValueId(checked.right.0),
        }
    }

    pub fn lower_float_meaning_projection(
        checked: CheckedFloatMeaningProjection,
        direct_source: Option<FloatMeaningSource>,
    ) -> Result<FloatMeaningProjection, FloatMeaningProjectionLoweringError> {
        checked
            .validate()
            .map_err(FloatMeaningProjectionLoweringError::InvalidCheckedProjection)?;
        let source = match checked.source {
            CheckedFloatProjectionSource::TransitionalInput(input) => {
                let format = projection_format(input.primitive)?;
                transitional_input(input.id.0, format)
            }
            CheckedFloatProjectionSource::DirectMachineParameter(parameter) => {
                let format = projection_format(parameter.fallback.primitive)?;
                match direct_source {
                    Some(FloatMeaningSource::DirectMachineParameter(parameter))
                        if parameter.format == format =>
                    {
                        FloatMeaningSource::DirectMachineParameter(parameter)
                    }
                    Some(_) => {
                        return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier);
                    }
                    None => transitional_input(parameter.fallback.id.0, format),
                }
            }
            CheckedFloatProjectionSource::DirectMachineResult(result) => {
                let format = projection_format(result.fallback.primitive)?;
                match direct_source {
                    Some(FloatMeaningSource::DirectMachineResult(result))
                        if result.format == format =>
                    {
                        FloatMeaningSource::DirectMachineResult(result)
                    }
                    Some(_) => {
                        return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier);
                    }
                    None => transitional_input(result.fallback.id.0, format),
                }
            }
            CheckedFloatProjectionSource::DirectBlockParameter(parameter) => {
                let format = projection_format(parameter.fallback.primitive)?;
                match direct_source {
                    Some(FloatMeaningSource::DirectBlockParameter(parameter))
                        if parameter.format == format =>
                    {
                        FloatMeaningSource::DirectBlockParameter(parameter)
                    }
                    Some(_) => {
                        return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier);
                    }
                    None => transitional_input(parameter.fallback.id.0, format),
                }
            }
            CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) => {
                let format = projection_format(leaf.fallback.primitive)?;
                match direct_source {
                    Some(FloatMeaningSource::DirectStructuralLeaf(direct))
                        if direct.format == format =>
                    {
                        FloatMeaningSource::DirectStructuralLeaf(direct)
                    }
                    Some(_) => {
                        return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier);
                    }
                    None => transitional_input(leaf.fallback.id.0, format),
                }
            }
            CheckedFloatProjectionSource::DirectCallResult(result) => {
                let format = projection_format(result.fallback.primitive)?;
                match direct_source {
                    Some(FloatMeaningSource::DirectCallResult(direct))
                        if direct.format == format =>
                    {
                        FloatMeaningSource::DirectCallResult(direct)
                    }
                    Some(_) => {
                        return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier);
                    }
                    None => transitional_input(result.fallback.id.0, format),
                }
            }
            CheckedFloatProjectionSource::DirectOperationResult(result) => {
                let format = projection_format(result.fallback.primitive)?;
                match direct_source {
                    Some(FloatMeaningSource::DirectOperationResult(direct))
                        if direct.format == format =>
                    {
                        FloatMeaningSource::DirectOperationResult(direct)
                    }
                    Some(_) => {
                        return Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier);
                    }
                    None => transitional_input(result.fallback.id.0, format),
                }
            }
            CheckedFloatProjectionSource::ExactBinary32Literal(bits) => {
                FloatMeaningSource::ExactBinary32Literal(bits)
            }
            CheckedFloatProjectionSource::ExactBinary64Literal(bits) => {
                FloatMeaningSource::ExactBinary64Literal(bits)
            }
        };
        let value_type = match checked.result.value_type {
            CheckedProofOnlyValueType::FloatMeaning => ProofOnlyValueType::FloatMeaning,
        };
        let operation = match checked.operation {
            numerics::float_projection::FloatProjectionOperation::Meaning32 => {
                FloatMeaningProjectionOperation::Meaning32
            }
            numerics::float_projection::FloatProjectionOperation::Meaning64 => {
                FloatMeaningProjectionOperation::Meaning64
            }
        };
        Ok(FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(checked.result.id.0),
                value_type,
            },
            source,
            operation,
            contract: FloatProjectionContractIdentity {
                format: checked.contract.format,
                operation: checked.contract.operation,
                declaration: checked.contract.declaration,
                catalog_version: checked.contract.catalog_version,
                commitment: checked.contract.commitment,
            },
        })
    }

    pub(crate) fn resolve_direct_float_source_binding(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine_bindings: &[(symbols::SymbolHandle, MachineId)],
        terminal_machines: &[TerminalMachine],
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
        source_call_occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
        state_blocks: &[lowered_psi::LoweredStateBlock],
        projection: CheckedFloatMeaningProjection,
    ) -> Result<Option<FloatMeaningSource>, LoweringError> {
        let owner_machine = match &projection.source {
            CheckedFloatProjectionSource::DirectMachineParameter(parameter) => {
                parameter.owner_machine
            }
            CheckedFloatProjectionSource::DirectMachineResult(result) => result.owner_machine,
            CheckedFloatProjectionSource::DirectBlockParameter(parameter) => {
                parameter.owner_machine
            }
            CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) => leaf.owner_machine,
            CheckedFloatProjectionSource::DirectCallResult(result) => result.use_site.owner_machine,
            CheckedFloatProjectionSource::DirectOperationResult(result) => {
                result.use_site.owner_machine
            }
            _ => return Ok(None),
        };
        let Some((_, terminal_owner)) = machine_bindings
            .iter()
            .find(|(source_owner, _)| *source_owner == owner_machine)
        else {
            return Ok(None);
        };
        let invalid_source = || {
            LoweringError::InvalidFloatMeaningProjection(
                FloatMeaningProjectionLoweringError::InvalidSourceCarrier,
            )
        };
        let mut terminal_owners = terminal_machines
            .iter()
            .filter(|machine| machine.id == *terminal_owner);
        let terminal_machine = terminal_owners.next().ok_or_else(invalid_source)?;
        if terminal_owners.next().is_some() {
            return Err(invalid_source());
        }
        let source_machine = checked
            .typed
            .machine(owner_machine)
            .ok_or_else(invalid_source)?;
        let source_entry = checked
            .typed
            .machine_states(source_machine)
            .first()
            .ok_or_else(invalid_source)?;
        match projection.source {
            CheckedFloatProjectionSource::DirectMachineParameter(parameter) => {
                resolve_direct_float_parameter(
                    checked,
                    source_entry,
                    terminal_machine,
                    *terminal_owner,
                    parameter,
                )
            }
            CheckedFloatProjectionSource::DirectMachineResult(result) => {
                let format =
                    projection_format(result.fallback.primitive).map_err(|_| invalid_source())?;
                if checked
                    .typed
                    .primitive_type_reference(source_entry.return_type)
                    != Some(result.fallback.primitive)
                {
                    return Err(invalid_source());
                }
                let TerminalMachineResult::Scalar(terminal_result) = terminal_machine.result else {
                    return Err(invalid_source());
                };
                if terminal_result.scalar_type != ScalarType::IeeeFloat(format) {
                    return Err(invalid_source());
                }
                Ok(Some(FloatMeaningSource::DirectMachineResult(
                    DirectMachineFloatResult {
                        owner: *terminal_owner,
                        result: terminal_result.id,
                        format,
                    },
                )))
            }
            CheckedFloatProjectionSource::DirectBlockParameter(parameter) => {
                resolve_direct_block_float_parameter(
                    checked,
                    source_machine,
                    terminal_machine,
                    *terminal_owner,
                    state_blocks,
                    parameter,
                )
            }
            CheckedFloatProjectionSource::DirectCallResult(result) => {
                let format =
                    projection_format(result.fallback.primitive).map_err(|_| invalid_source())?;
                let mut occurrences = source_call_occurrences.iter().filter(|occurrence| {
                    occurrence.matches_statement(
                        result.use_site.owner_state,
                        result.use_site.statement_index,
                        result.use_site.call_ordinal,
                    )
                });
                let Some(occurrence) = occurrences.next() else {
                    return Ok(None);
                };
                if occurrences.next().is_some() {
                    return Err(invalid_source());
                }
                let Some(operation) = occurrence.terminal_site.operation() else {
                    return Err(invalid_source());
                };
                resolve_float_operation_result_source(
                    terminal_machine,
                    *terminal_owner,
                    operation,
                    format,
                    true,
                )
            }
            CheckedFloatProjectionSource::DirectOperationResult(_) => Ok(None),
            CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) => {
                let format =
                    projection_format(leaf.fallback.primitive).map_err(|_| invalid_source())?;
                let parameter = terminal_machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.position == leaf.field.parameter_position)
                    .ok_or_else(invalid_source)?;
                if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                    return Err(invalid_source());
                }
                let (root, path, actual) =
                    crate::proofs::crash_routes::lower_structural_member_path(
                        identities,
                        leaf.field.parameter_position,
                        &leaf.field.path,
                        &terminal_machine.structural_parameters,
                        structural_types,
                    )?;
                if actual != terminal_psi::StructuralFieldType::IeeeFloat(format) {
                    return Err(invalid_source());
                }
                Ok(Some(FloatMeaningSource::DirectStructuralLeaf(
                    DirectStructuralFloatLeaf {
                        owner: *terminal_owner,
                        field: semantic_vocabulary::IeeeFloatStructuralField::new(root, path)
                            .map_err(LoweringError::InvalidCrashPredicate)?,
                        format,
                    },
                )))
            }
            _ => Ok(None),
        }
    }

    fn resolve_direct_float_parameter(
        checked: &CheckedTrees,
        source_entry: &checked_trees::state::State,
        terminal_machine: &TerminalMachine,
        terminal_owner: MachineId,
        parameter: checked_trees::CheckedDirectMachineFloatParameter,
    ) -> Result<Option<FloatMeaningSource>, LoweringError> {
        let invalid_source = || {
            LoweringError::InvalidFloatMeaningProjection(
                FloatMeaningProjectionLoweringError::InvalidSourceCarrier,
            )
        };
        let source_parameters = checked
            .typed
            .public_state_parameters(source_entry)
            .filter_map(|parameter| {
                checked
                    .typed
                    .primitive_type_reference(parameter.type_reference)
                    .map(|primitive| (parameter.symbol, primitive))
            })
            .collect::<Vec<_>>();
        let source_types = source_parameters
            .iter()
            .map(|(_, primitive)| terminal_scalar_type(*primitive))
            .collect::<Result<Vec<_>, _>>()?;
        if source_types
            != terminal_machine
                .parameters
                .iter()
                .map(|parameter| parameter.scalar_type)
                .collect::<Vec<_>>()
        {
            return Ok(None);
        }
        let position = source_parameters
            .iter()
            .position(|(symbol, _)| *symbol == parameter.parameter)
            .ok_or_else(invalid_source)?;
        let terminal_parameter = terminal_machine
            .parameters
            .get(position)
            .ok_or_else(invalid_source)?;
        let format =
            projection_format(parameter.fallback.primitive).map_err(|_| invalid_source())?;
        if terminal_parameter.scalar_type != ScalarType::IeeeFloat(format) {
            return Err(invalid_source());
        }
        Ok(Some(FloatMeaningSource::DirectMachineParameter(
            DirectMachineFloatParameter {
                owner: terminal_owner,
                parameter: terminal_parameter.id,
                format,
            },
        )))
    }

    fn resolve_direct_block_float_parameter(
        checked: &CheckedTrees,
        source_machine: &checked_trees::machine::Machine,
        terminal_machine: &TerminalMachine,
        terminal_owner: MachineId,
        state_blocks: &[lowered_psi::LoweredStateBlock],
        parameter: checked_trees::CheckedDirectBlockFloatParameter,
    ) -> Result<Option<FloatMeaningSource>, LoweringError> {
        let invalid_source = || {
            LoweringError::InvalidFloatMeaningProjection(
                FloatMeaningProjectionLoweringError::InvalidSourceCarrier,
            )
        };
        let format =
            projection_format(parameter.fallback.primitive).map_err(|_| invalid_source())?;
        let plan = flow_facts(checked)
            .terminal_unit_effects
            .composed_for_machine(source_machine.symbol)
            .ok_or_else(invalid_source)?;
        let position = plan
            .state_position(parameter.owner_state)
            .ok_or_else(invalid_source)?;
        if position == 0
            || plan
                .states
                .iter()
                .skip(position + 1)
                .any(|state| state.state == parameter.owner_state)
        {
            return Err(invalid_source());
        }
        let mut headers = state_blocks
            .iter()
            .filter(|row| row.machine == terminal_owner && row.state == parameter.owner_state);
        let header = headers.next().ok_or_else(invalid_source)?;
        if headers.next().is_some() {
            return Err(invalid_source());
        }
        let mut blocks = terminal_machine
            .blocks
            .iter()
            .filter(|block| block.id == header.block);
        let block = blocks.next().ok_or_else(invalid_source)?;
        if blocks.next().is_some() {
            return Err(invalid_source());
        }
        let graph_state = &plan.states[position];
        if block.parameters.len() != graph_state.scalar_parameters.len() {
            return Err(invalid_source());
        }
        let source_state = checked
            .typed
            .state_in_machine(source_machine, parameter.owner_state)
            .ok_or_else(invalid_source)?;
        let authored_position = checked
            .typed
            .state_parameters(source_state)
            .iter()
            .position(|source| source.symbol == parameter.parameter)
            .ok_or_else(invalid_source)?;
        let scalar_index = graph_state
            .scalar_parameters
            .iter()
            .position(|scalar| {
                usize::try_from(scalar.source_position).ok() == Some(authored_position)
            })
            .ok_or_else(invalid_source)?;
        if graph_state.scalar_parameters[scalar_index].primitive_type
            != parameter.fallback.primitive
        {
            return Err(invalid_source());
        }
        let declaration = &block.parameters[scalar_index];
        if declaration.scalar_type != ScalarType::IeeeFloat(format) {
            return Err(invalid_source());
        }
        Ok(Some(FloatMeaningSource::DirectBlockParameter(
            DirectBlockFloatParameter {
                owner: terminal_owner,
                block: block.id,
                parameter: declaration.id,
                format,
            },
        )))
    }

    fn resolve_float_operation_result_source(
        terminal_machine: &TerminalMachine,
        terminal_owner: MachineId,
        terminal_operation: semantic_vocabulary::OperationId,
        format: IeeeFloatFormat,
        call: bool,
    ) -> Result<Option<FloatMeaningSource>, LoweringError> {
        let invalid_source = || {
            LoweringError::InvalidFloatMeaningProjection(
                FloatMeaningProjectionLoweringError::InvalidSourceCarrier,
            )
        };
        let mut producers = terminal_machine
            .operations()
            .filter(|operation| operation.id == terminal_operation);
        let producer = producers.next().ok_or_else(invalid_source)?;
        if producers.next().is_some() {
            return Err(invalid_source());
        }
        let call_kind = matches!(
            producer.kind,
            terminal_psi::OperationKind::Call { .. }
                | terminal_psi::OperationKind::CallStructuralScalar { .. }
                | terminal_psi::OperationKind::BoundaryCall { .. }
        );
        if call_kind != call {
            return Err(invalid_source());
        }
        let terminal_psi::OperationResult::Scalar(declaration) = producer.result else {
            return Err(invalid_source());
        };
        if declaration.scalar_type != ScalarType::IeeeFloat(format) {
            return Err(invalid_source());
        }
        if call {
            return Ok(Some(FloatMeaningSource::DirectCallResult(
                DirectCallFloatResult {
                    owner: terminal_owner,
                    producer: terminal_operation,
                    result: declaration.id,
                    format,
                },
            )));
        }
        if !matches!(
            producer.kind,
            terminal_psi::OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        ) {
            return Err(invalid_source());
        }
        Ok(Some(FloatMeaningSource::DirectOperationResult(
            DirectOperationFloatResult {
                owner: terminal_owner,
                producer: terminal_operation,
                result: declaration.id,
                format,
            },
        )))
    }

    pub(crate) fn rejoin_float_semantic_applications(
        applications: &[CheckedFloatSemanticApplication],
        projections: &mut [FloatMeaningProjection],
    ) -> Result<(), FloatMeaningProjectionLoweringError> {
        for application in applications {
            let result = application.result.0;
            application.validate().map_err(|error| {
                FloatMeaningProjectionLoweringError::InvalidSemanticApplication { result, error }
            })?;
            let row = usize::try_from(result)
                .ok()
                .and_then(|index| projections.get_mut(index))
                .ok_or(
                    FloatMeaningProjectionLoweringError::InvalidSemanticApplicationRow { result },
                )?;
            if row.source.format() != application.format {
                return Err(
                    FloatMeaningProjectionLoweringError::SemanticApplicationFormatMismatch {
                        result,
                    },
                );
            }
            let expected = FloatSemanticApplication {
                contract: FloatSemanticContractIdentity {
                    row: application.contract.row,
                    catalog_version: application.contract.catalog_version,
                    commitment: application.contract.commitment,
                },
                format: application.format,
                operands: application
                    .operands
                    .iter()
                    .map(|operand| match operand {
                        CheckedFloatSemanticApplicationOperand::Format(format) => {
                            FloatSemanticApplicationOperand::Format(*format)
                        }
                        CheckedFloatSemanticApplicationOperand::Meaning(value) => {
                            FloatSemanticApplicationOperand::Meaning(ProofValueId(value.0))
                        }
                    })
                    .collect(),
            };
            match &row.source {
                FloatMeaningSource::TransitionalInput(_) => {
                    row.source = FloatMeaningSource::SemanticApplication(expected);
                }
                FloatMeaningSource::SemanticApplication(existing) if *existing == expected => {}
                _ => {
                    return Err(
                        FloatMeaningProjectionLoweringError::InvalidSemanticApplicationRow {
                            result,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn transitional_input(id: u32, format: IeeeFloatFormat) -> FloatMeaningSource {
        FloatMeaningSource::TransitionalInput(FloatProjectionInput {
            id: FloatProjectionInputId(id),
            format,
        })
    }

    fn projection_format(
        primitive: PrimitiveType,
    ) -> Result<IeeeFloatFormat, FloatMeaningProjectionLoweringError> {
        match primitive {
            PrimitiveType::F32 => Ok(IeeeFloatFormat::Binary32),
            PrimitiveType::F64 => Ok(IeeeFloatFormat::Binary64),
            _ => Err(FloatMeaningProjectionLoweringError::InvalidSourceCarrier),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FloatMeaningProjectionLoweringError {
        InvalidCheckedProjection(CheckedFloatMeaningProjectionError),
        InvalidSemanticApplication {
            result: u32,
            error: CheckedFloatSemanticApplicationError,
        },
        InvalidSemanticApplicationRow {
            result: u32,
        },
        SemanticApplicationFormatMismatch {
            result: u32,
        },
        InvalidSourceCarrier,
    }
}
pub(crate) mod integer_predicates {

    use super::crash_routes::structural_arithmetic::{
        safe_exact_structural_divisor, safe_exact_structural_shift, safe_policy_structural_divisor,
    };
    use super::{
        CheckedIntegerBinaryKind, CheckedScalarExpression, LoweringError, Proposition, ScalarTerm,
        ScalarType, integer_scalar_type, unsupported,
    };

    pub(crate) fn lower(
        expression: &CheckedScalarExpression,
        runtime_requirements: &[Proposition],
        leaf: &mut impl FnMut(&CheckedScalarExpression) -> Result<ScalarTerm, LoweringError>,
    ) -> Result<ScalarTerm, LoweringError> {
        let mut lower_operand = |operand| lower(operand, runtime_requirements, leaf);
        match expression {
            CheckedScalarExpression::IntegerWiden {
                primitive_type,
                operand,
            } => {
                let target_type = integer_scalar_type(*primitive_type)?.as_integer().ok_or(
                    LoweringError::Unsupported("integer predicate widening has no integer target"),
                )?;
                let operand = lower_operand(operand)?;
                let source_type =
                    operand
                        .scalar_type()
                        .as_integer()
                        .ok_or(LoweringError::Unsupported(
                            "integer predicate widening has no integer operand",
                        ))?;
                ScalarTerm::integer_widen(source_type, target_type, operand)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedScalarExpression::IntegerExactCast {
                primitive_type,
                operand,
                formation_decided,
                ..
            } => {
                let target_type = integer_scalar_type(*primitive_type)?.as_integer().ok_or(
                    LoweringError::Unsupported(
                        "integer predicate exact cast has no integer target",
                    ),
                )?;
                let operand = lower_operand(operand)?;
                let source_type =
                    operand
                        .scalar_type()
                        .as_integer()
                        .ok_or(LoweringError::Unsupported(
                            "integer predicate exact cast has no integer operand",
                        ))?;
                if !*formation_decided && !source_type.can_widen_to(target_type) {
                    return unsupported(
                        "integer predicate exact cast lacks independent formation evidence",
                    );
                }
                ScalarTerm::integer_exact_cast(source_type, target_type, operand)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedScalarExpression::IntegerBitwiseNot {
                primitive_type,
                operand,
            } => {
                let Some(integer_type) = integer_scalar_type(*primitive_type)?.as_integer() else {
                    return unsupported("structural crash bitwise-not has a non-integer type");
                };
                let operand = lower_operand(operand)?;
                ScalarTerm::integer_bitwise_not(integer_type, operand)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedScalarExpression::IntegerBinary {
                kind,
                primitive_type,
                left,
                right,
                ..
            } if matches!(
                kind,
                CheckedIntegerBinaryKind::BitwiseAnd
                    | CheckedIntegerBinaryKind::BitwiseOr
                    | CheckedIntegerBinaryKind::BitwiseXor
            ) =>
            {
                let Some(integer_type) = integer_scalar_type(*primitive_type)?.as_integer() else {
                    return unsupported(
                        "structural crash bitwise expression has a non-integer type",
                    );
                };
                let left = lower_operand(left)?;
                let right = lower_operand(right)?;
                match kind {
                    CheckedIntegerBinaryKind::BitwiseAnd => {
                        ScalarTerm::integer_bitwise_and(integer_type, left, right)
                    }
                    CheckedIntegerBinaryKind::BitwiseOr => {
                        ScalarTerm::integer_bitwise_or(integer_type, left, right)
                    }
                    CheckedIntegerBinaryKind::BitwiseXor => {
                        ScalarTerm::integer_bitwise_xor(integer_type, left, right)
                    }
                    _ => unreachable!("guarded bitwise kind"),
                }
                .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedScalarExpression::IntegerBinary {
                kind,
                primitive_type,
                left,
                right,
                formation_decided,
                ..
            } if matches!(
                kind,
                CheckedIntegerBinaryKind::WrappingShiftLeft
                    | CheckedIntegerBinaryKind::WrappingShiftRight
                    | CheckedIntegerBinaryKind::ExactShiftLeft
                    | CheckedIntegerBinaryKind::ExactShiftRight
            ) =>
            {
                let Some(value_type) = integer_scalar_type(*primitive_type)?.as_integer() else {
                    return unsupported("structural crash shift has a non-integer value type");
                };
                let value = lower_operand(left)?;
                let count = lower_operand(right)?;
                if value.scalar_type() != ScalarType::Integer(value_type) {
                    return unsupported(
                        "structural crash shift value does not match its integer type",
                    );
                }
                let Some(count_type) = count.scalar_type().as_integer() else {
                    return unsupported("structural crash shift count is not an integer");
                };
                if matches!(
                    kind,
                    CheckedIntegerBinaryKind::ExactShiftLeft
                        | CheckedIntegerBinaryKind::ExactShiftRight
                ) && !*formation_decided
                    && !safe_exact_structural_shift(
                        matches!(kind, CheckedIntegerBinaryKind::ExactShiftLeft),
                        value_type,
                        count_type,
                        &value,
                        &count,
                        runtime_requirements,
                    )
                {
                    return unsupported(
                        "structural crash Exact shift requires explicit terminal count and overflow safety evidence",
                    );
                }
                match kind {
                    CheckedIntegerBinaryKind::WrappingShiftLeft => {
                        ScalarTerm::wrapping_integer_shift_left(
                            value_type, count_type, value, count,
                        )
                    }
                    CheckedIntegerBinaryKind::WrappingShiftRight => {
                        ScalarTerm::wrapping_integer_shift_right(
                            value_type, count_type, value, count,
                        )
                    }
                    CheckedIntegerBinaryKind::ExactShiftLeft => {
                        ScalarTerm::exact_integer_shift_left(value_type, count_type, value, count)
                    }
                    CheckedIntegerBinaryKind::ExactShiftRight => {
                        ScalarTerm::exact_integer_shift_right(value_type, count_type, value, count)
                    }
                    _ => unreachable!("guarded structural shift kind"),
                }
                .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedScalarExpression::IntegerBinary {
                kind,
                primitive_type,
                left,
                right,
                formation_decided,
                ..
            } if matches!(
                kind,
                CheckedIntegerBinaryKind::ExactAdd
                    | CheckedIntegerBinaryKind::ExactSubtract
                    | CheckedIntegerBinaryKind::ExactMultiply
                    | CheckedIntegerBinaryKind::ExactDivide
                    | CheckedIntegerBinaryKind::ExactRemainder
                    | CheckedIntegerBinaryKind::WrappingAdd
                    | CheckedIntegerBinaryKind::SaturatingAdd
                    | CheckedIntegerBinaryKind::WrappingSubtract
                    | CheckedIntegerBinaryKind::SaturatingSubtract
                    | CheckedIntegerBinaryKind::WrappingMultiply
                    | CheckedIntegerBinaryKind::SaturatingMultiply
                    | CheckedIntegerBinaryKind::WrappingDivide
                    | CheckedIntegerBinaryKind::WrappingRemainder
                    | CheckedIntegerBinaryKind::SaturatingDivide
                    | CheckedIntegerBinaryKind::SaturatingRemainder
            ) =>
            {
                let Some(integer_type) = integer_scalar_type(*primitive_type)?.as_integer() else {
                    return unsupported("structural crash arithmetic has a non-integer type");
                };
                let left = lower_operand(left)?;
                let right = lower_operand(right)?;
                if left.scalar_type() != ScalarType::Integer(integer_type)
                    || right.scalar_type() != ScalarType::Integer(integer_type)
                {
                    return unsupported(
                        "structural crash arithmetic operands do not match its integer type",
                    );
                }
                if matches!(
                    kind,
                    CheckedIntegerBinaryKind::ExactDivide
                        | CheckedIntegerBinaryKind::ExactRemainder
                ) && !*formation_decided
                    && !safe_exact_structural_divisor(
                        integer_type,
                        &left,
                        &right,
                        runtime_requirements,
                    )
                {
                    return unsupported(
                        "structural crash exact division requires explicit terminal divisor safety evidence",
                    );
                }
                if matches!(
                    kind,
                    CheckedIntegerBinaryKind::WrappingDivide
                        | CheckedIntegerBinaryKind::WrappingRemainder
                        | CheckedIntegerBinaryKind::SaturatingDivide
                        | CheckedIntegerBinaryKind::SaturatingRemainder
                ) && !*formation_decided
                    && !safe_policy_structural_divisor(integer_type, &right, runtime_requirements)
                {
                    return unsupported(
                        "structural crash policy division requires explicit terminal nonzero-divisor evidence",
                    );
                }
                let constructor: fn(
                    semantic_vocabulary::IntegerType,
                    ScalarTerm,
                    ScalarTerm,
                )
                    -> Result<ScalarTerm, semantic_vocabulary::PropositionError> = match kind {
                    CheckedIntegerBinaryKind::ExactAdd => ScalarTerm::exact_integer_add,
                    CheckedIntegerBinaryKind::ExactSubtract => ScalarTerm::exact_integer_subtract,
                    CheckedIntegerBinaryKind::ExactMultiply => ScalarTerm::exact_integer_multiply,
                    CheckedIntegerBinaryKind::ExactDivide => ScalarTerm::exact_integer_divide,
                    CheckedIntegerBinaryKind::ExactRemainder => ScalarTerm::exact_integer_remainder,
                    CheckedIntegerBinaryKind::WrappingAdd => ScalarTerm::wrapping_integer_add,
                    CheckedIntegerBinaryKind::SaturatingAdd => ScalarTerm::saturating_integer_add,
                    CheckedIntegerBinaryKind::WrappingSubtract => {
                        ScalarTerm::wrapping_integer_subtract
                    }
                    CheckedIntegerBinaryKind::SaturatingSubtract => {
                        ScalarTerm::saturating_integer_subtract
                    }
                    CheckedIntegerBinaryKind::WrappingMultiply => {
                        ScalarTerm::wrapping_integer_multiply
                    }
                    CheckedIntegerBinaryKind::SaturatingMultiply => {
                        ScalarTerm::saturating_integer_multiply
                    }
                    CheckedIntegerBinaryKind::WrappingDivide => ScalarTerm::wrapping_integer_divide,
                    CheckedIntegerBinaryKind::WrappingRemainder => {
                        ScalarTerm::wrapping_integer_remainder
                    }
                    CheckedIntegerBinaryKind::SaturatingDivide => {
                        ScalarTerm::saturating_integer_divide
                    }
                    CheckedIntegerBinaryKind::SaturatingRemainder => {
                        ScalarTerm::saturating_integer_remainder
                    }
                    _ => unreachable!("guarded structural arithmetic kind"),
                };
                Ok(constructor(integer_type, left, right)
                    .map_err(LoweringError::InvalidCrashPredicate)?)
            }
            _ => leaf(expression),
        }
    }
}
