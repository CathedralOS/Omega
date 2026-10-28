//! Normal host writeback conserves captured membership on exact input storage.
//! The declaration selects promises; live input values supply their existing
//! membership. Resolve stored references once at capture, not from host words
//! or a later descriptor generation. No scope grants an issuer or permission.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralPathQualification, StructuralPathSegment};

#[derive(Debug, Clone)]
pub(crate) struct CapturedNormalQualification {
    pub(crate) path: Vec<StructuralPathSegment>,
    /// None is a proved inactive case, never an unknown pointer.
    subject: Option<StructuralRuntimePlace>,
    carrier: StructuralTypeId,
    domain: StructuralDomainId,
}

impl TerminalExecution<'_> {
    pub(crate) fn normal_qualifications_cover(
        &self,
        value: &TerminalStructuralValue,
        whole: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
        captured: &[CapturedNormalQualification],
    ) -> Result<bool, TerminalInterpretError> {
        for domain in whole {
            if !self.normal_qualification_is_covered(value, &[], *domain, captured)? {
                return Ok(false);
            }
        }
        for row in projected {
            if !self.normal_qualification_is_covered(value, &row.path, row.domain, captured)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) fn capture_normal_qualifications(
        &self,
        owner: &TerminalStructuralValue,
        promised: &[StructuralPathQualification],
    ) -> Result<Vec<CapturedNormalQualification>, TerminalInterpretError> {
        let mut captured = Vec::new();
        for row in promised {
            let carrier = terminal_semantics::qualification_path_carrier(
                self.program.structural_types().values(),
                owner.structural_type,
                &row.path,
            )
            .ok_or_else(malformed)?;
            let domain = self
                .program
                .catalogs
                .structural_domains
                .iter()
                .find(|domain| domain.id == row.domain && domain.carrier == carrier)
                .ok_or_else(malformed)?;
            if self.normal_qualification_is_intrinsic(owner, &row.path, row.domain) {
                continue;
            }
            let subject = self.normal_qualification_subject(owner, &row.path)?;
            if subject.as_ref().is_some_and(|subject| {
                subject.structural_type != domain.carrier
                    || !subject.qualifications.contains(&row.domain)
            }) {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "normal host preservation lacks the exact original input membership",
                ));
            }
            captured.push(CapturedNormalQualification {
                path: row.path.clone(),
                subject: subject.as_ref().map(StructuralRuntimePlace::from),
                carrier,
                domain: row.domain,
            });
        }
        Ok(captured)
    }

    pub(crate) fn normal_qualification_is_covered(
        &self,
        value: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        domain: StructuralDomainId,
        captured: &[CapturedNormalQualification],
    ) -> Result<bool, TerminalInterpretError> {
        if self.normal_qualification_is_intrinsic(value, path, domain) {
            return Ok(true);
        }
        let Some(subject) = self.normal_qualification_subject(value, path)? else {
            return Ok(true);
        };
        Ok(captured.iter().any(|row| {
            row.domain == domain
                && row.carrier == subject.structural_type
                && row.subject.as_ref() == Some(&StructuralRuntimePlace::from(&subject))
        }))
    }

    /// New active routed payloads need establishment, not the old inactive
    /// case's signature annotation. Case removal grants nothing; a previously
    /// inactive promise cannot acquire a captured source from host scalars.
    pub(crate) fn validate_normal_case_preservation(
        &self,
        owner: &TerminalStructuralValue,
        captured: &[CapturedNormalQualification],
        changed: &[(
            StructuralRuntimePlace,
            semantic_vocabulary::StructuralCaseId,
        )],
    ) -> Result<(), TerminalInterpretError> {
        for row in captured.iter().filter(|row| row.subject.is_none()) {
            for (position, segment) in row.path.iter().enumerate() {
                let StructuralPathSegment::Case(expected) = segment else {
                    continue;
                };
                let carrier = self
                    .resolve_reference_storage_projection(
                        owner,
                        &row.path[..position],
                        StructuralAccess::SharedBorrow,
                    )?
                    .ok_or_else(malformed)?
                    .value;
                let home = StructuralRuntimePlace::from(&carrier);
                let actual = self.structural_cases.get(&home).ok_or_else(malformed)?;
                if actual.structural_type != carrier.structural_type {
                    return Err(malformed());
                }
                let returned = changed
                    .iter()
                    .find(|(place, _)| *place == home)
                    .map_or(actual.case, |(_, case)| *case);
                if actual.case != *expected {
                    if returned == *expected {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "case replacement cannot establish an uncaptured normal membership",
                        ));
                    }
                    break;
                }
                if returned != *expected {
                    break;
                }
            }
        }
        Ok(())
    }

    fn normal_qualification_is_intrinsic(
        &self,
        value: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        domain: StructuralDomainId,
    ) -> bool {
        terminal_semantics::scalar_projection_entails_membership(
            self.program.structural_types().values(),
            self.program.catalogs.structural_domains.iter(),
            value.structural_type,
            path,
            domain,
        )
    }

    fn normal_qualification_subject(
        &self,
        value: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
    ) -> Result<Option<TerminalStructuralValue>, TerminalInterpretError> {
        for (position, segment) in path.iter().enumerate() {
            match segment {
                StructuralPathSegment::AllElements => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "normal host membership preservation needs an exact captured element",
                    ));
                }
                StructuralPathSegment::Case(expected) => {
                    let carrier = self
                        .resolve_reference_storage_projection(
                            value,
                            &path[..position],
                            StructuralAccess::SharedBorrow,
                        )?
                        .ok_or_else(malformed)?
                        .value;
                    let actual = self
                        .structural_cases
                        .get(&StructuralRuntimePlace::from(&carrier))
                        .ok_or_else(malformed)?;
                    if actual.structural_type != carrier.structural_type {
                        return Err(malformed());
                    }
                    if actual.case != *expected {
                        return Ok(None);
                    }
                }
                _ => {}
            }
        }
        if path.is_empty() {
            return Ok(Some(value.clone()));
        }
        self.resolve_reference_storage_projection(value, path, StructuralAccess::SharedBorrow)?
            .map(|projection| Some(projection.value))
            .ok_or_else(malformed)
    }
}
