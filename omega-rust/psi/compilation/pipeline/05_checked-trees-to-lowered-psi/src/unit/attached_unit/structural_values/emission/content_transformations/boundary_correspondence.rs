//! Preserve conditional body custody without publishing output membership.

use super::{Emission, LoweringError, source_path, unsupported};
use arena::Handle;
use checked_trees::facts::{
    ContentBodyAvailableInput, ContentBodyClaimBinding, ContentBodyCustody, ContentBodyDerivation,
};
use language_semantics::content::{
    ContentConservationTerm as SourceTerm, ContentPlaceRoot as SourceRoot,
};
use semantic_vocabulary::{ContentTerm, StructuralPlaceKind};
use terminal_psi::{ContentClaimProjection, ContentTransformationCustody};

impl Emission<'_, '_, '_> {
    pub(super) fn boundary_body_custody(
        &self,
        derived: &ContentBodyDerivation,
        inputs: &[(Handle<ContentBodyAvailableInput>, ContentClaimProjection)],
        outputs: &[(Handle<ContentBodyClaimBinding>, ContentClaimProjection)],
    ) -> Result<ContentTransformationCustody, LoweringError> {
        let ContentBodyCustody::Boundary {
            requirements,
            addresses,
        } = &derived.custody
        else {
            return Ok(ContentTransformationCustody::Derived);
        };
        let content = &self.checked.facts.qualifications.content;
        let equation = content.body_conservations.get(derived.conservation);
        let body_law = &content.conservation_plans[equation.plan_index];
        let mut retained_requirements = Vec::new();
        for requirement in content
            .boundary_body_requirements
            .span_or_empty(*requirements)
        {
            let boundary = self
                .calls
                .boundaries
                .iter()
                .find(|boundary| boundary.source == requirement.requirement)
                .ok_or(LoweringError::Unsupported(
                    "boundary body custody lost its exact public declaration",
                ))?;
            let source_law = content
                .boundary_requirement_plans
                .get(requirement.conservation_plan_index)
                .ok_or(LoweringError::Unsupported(
                    "boundary body custody lost its retained public law",
                ))?;
            let lowered =
                crate::proofs::content_conservation::lower_content_conservation_plan(source_law)?;
            let semantic_vocabulary::Proposition::ContentConservation(conservation) =
                lowered.proposition
            else {
                return unsupported("boundary body law is not a content equation");
            };
            let guarantee = terminal_psi::ContentConservationGuarantee {
                report_fingerprint: lowered.source_report_fingerprint,
                structural_places: lowered.structural_places,
                conservation,
            };
            let mut substitutions = Vec::new();
            let mut source_terms = Vec::new();
            projections(body_law.equation.left(), &mut source_terms);
            projections(body_law.equation.right(), &mut source_terms);
            let mut public_terms = Vec::new();
            portable_projections(guarantee.conservation.left(), &mut public_terms);
            portable_projections(guarantee.conservation.right(), &mut public_terms);
            for (projection, subject) in public_terms {
                let projection = *projection;
                let kind = guarantee
                    .structural_places
                    .iter()
                    .find(|place| place.id == subject.root)
                    .ok_or(LoweringError::Unsupported(
                        "boundary body law has no declared subject root",
                    ))?
                    .kind;
                let mut targets = Vec::new();
                for source in &source_terms {
                    let SourceTerm::Projection {
                        semantic_domain,
                        subject: source_subject,
                        ..
                    } = source
                    else {
                        continue;
                    };
                    let source_segments = source_subject
                        .segments
                        .iter()
                        .map(|segment| match segment {
                            language_semantics::content::ContentPlaceSegment::Field(field) => {
                                semantic_vocabulary::ContentPlaceSegment::Field(field.name.clone())
                            }
                            language_semantics::content::ContentPlaceSegment::Case(case) => {
                                semantic_vocabulary::ContentPlaceSegment::Case(case.name.clone())
                            }
                            language_semantics::content::ContentPlaceSegment::FixedIndex(index) => {
                                semantic_vocabulary::ContentPlaceSegment::FixedIndex(*index)
                            }
                        })
                        .collect::<Vec<_>>();
                    if source_segments != subject.segments
                        || u64::from(semantic_domain.0) != projection.domain.get()
                    {
                        continue;
                    }
                    match (&kind, &source_subject.root) {
                        (
                            StructuralPlaceKind::Parameter { position },
                            SourceRoot::Parameter {
                                position: source_position,
                                symbol,
                                ..
                            },
                        ) if position == source_position => {
                            let parameter = self.body_parameter_at_completion(equation, *symbol)?;
                            for (available, target) in inputs {
                                let available = content.body_available_inputs.get(*available);
                                let entry =
                                    content.body_claim_bindings.get(available.entry_binding);
                                if entry.parameter_symbol == parameter
                                    && target.projection == projection
                                    && source_path(
                                        self.checked,
                                        self.checked
                                            .facts
                                            .flow
                                            .ownership
                                            .segments
                                            .span_or_empty(entry.segments),
                                    )? == subject.segments
                                {
                                    targets.push(target.clone());
                                }
                            }
                        }
                        (StructuralPlaceKind::Result, SourceRoot::Result) => {
                            targets.extend(
                                outputs
                                    .iter()
                                    .filter(|(_, target)| {
                                        target.projection == projection
                                            && target.subject.segments == subject.segments
                                    })
                                    .map(|(_, target)| target.clone()),
                            );
                        }
                        _ => {}
                    }
                }
                targets.sort();
                targets.dedup();
                let target = terminal_semantics::content_transformations::claim_term(&targets)
                    .map_err(LoweringError::Unsupported)?;
                substitutions.push(terminal_psi::ContentBodyProjectionSubstitution {
                    source: ContentTerm::Projection {
                        projection,
                        subject: subject.clone(),
                    },
                    target,
                });
            }
            substitutions.sort();
            substitutions.dedup();
            retained_requirements.push(terminal_psi::ContentBoundaryBodyRequirement {
                boundary: boundary.id,
                guarantee,
                substitutions,
            });
        }
        let mut retained_addresses = Vec::new();
        for address in content.backing_addresses.span_or_empty(*addresses) {
            let input = inputs
                .iter()
                .find(|(handle, _)| *handle == address.input)
                .map(|(_, binding)| binding)
                .ok_or(LoweringError::Unsupported(
                    "backing input is absent from the exact custody transaction",
                ))?;
            let output = outputs
                .iter()
                .find(|(handle, _)| *handle == address.output)
                .map(|(_, binding)| binding)
                .ok_or(LoweringError::Unsupported(
                    "backing output is absent from the exact custody transaction",
                ))?;
            let mut input_subject = input.subject.clone();
            input_subject.segments.extend(source_path(
                self.checked,
                self.checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(address.input_segments),
            )?);
            let mut output_subject = output.subject.clone();
            output_subject.segments.extend(source_path(
                self.checked,
                self.checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(address.output_segments),
            )?);
            retained_addresses.push(terminal_psi::ContentBackingAddressTransport {
                input: terminal_psi::ContentClaimAddress {
                    claim: input.claim,
                    subject: input_subject,
                },
                output: terminal_psi::ContentClaimAddress {
                    claim: output.claim,
                    subject: output_subject,
                },
            });
        }
        retained_requirements.sort();
        retained_requirements.dedup();
        retained_addresses.sort();
        retained_addresses.dedup();
        Ok(ContentTransformationCustody::Boundary {
            requirements: retained_requirements,
            addresses: retained_addresses,
        })
    }

    fn body_parameter_at_completion(
        &self,
        equation: &checked_trees::facts::ContentBodyConservationFact,
        parameter: symbols::SymbolHandle,
    ) -> Result<symbols::SymbolHandle, LoweringError> {
        let law =
            &self.checked.facts.qualifications.content.conservation_plans[equation.plan_index];
        if law.callable == equation.state_symbol {
            return Ok(parameter);
        }
        let flow = self
            .checked
            .facts
            .flow
            .control
            .state(equation.machine_symbol, equation.state_symbol)
            .ok_or(LoweringError::Unsupported(
                "boundary body has no completion state flow",
            ))?;
        let mut origins = self
            .checked
            .facts
            .flow
            .control
            .exits
            .span_or_empty(flow.exits)
            .iter()
            .flat_map(|exit| {
                self.checked
                    .facts
                    .flow
                    .control
                    .exit_parameter_origins
                    .span_or_empty(exit.parameter_origins)
            })
            .filter(|origin| origin.entry_parameter == parameter);
        let selected = origins
            .next()
            .ok_or(LoweringError::Unsupported(
                "boundary body formal has no proved completion origin",
            ))?
            .state_parameter;
        if !selected.is_valid() || origins.any(|origin| origin.state_parameter != selected) {
            return unsupported("boundary body formal has ambiguous completion origins");
        }
        Ok(selected)
    }
}

fn projections<'a>(term: &'a SourceTerm, output: &mut Vec<&'a SourceTerm>) {
    match term {
        SourceTerm::Projection { .. } => output.push(term),
        SourceTerm::Separate(children) => {
            for child in children {
                projections(child, output);
            }
        }
    }
}

fn portable_projections<'a>(
    term: &'a ContentTerm,
    output: &mut Vec<(
        &'a semantic_vocabulary::ContentProjectionIdentity,
        &'a semantic_vocabulary::ContentStructuralPlace,
    )>,
) {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => output.push((projection, subject)),
        ContentTerm::Separate(children) => {
            for child in children {
                portable_projections(child, output);
            }
        }
    }
}
