//! Discriminator observations retain the original structural root and path.

use crate::custody::{project_structural_value, resolve_structural_path_type};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::runtime_rows::RuntimeRows;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralCaseId, StructuralTypeId,
};
use std::collections::BTreeMap;
use terminal_psi::{
    Operation, StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// Initialized payloadless case contents below one structural entry argument.
/// The path selects the sum itself, not its case payload. Neither an opaque
/// identity nor an integer discriminator is evidence of these contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralCaseValue {
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
    pub case: StructuralCaseId,
}

/// Runtime contents retain nominal type independently of the observing view.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StructuralCaseContents {
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) case: StructuralCaseId,
}

/// Join a logical projection to the discriminator in its original backing.
/// Constructors use staged backing; established values use the live store.
/// Neither caller may substitute a Case segment's spelling for these contents.
pub(crate) fn stored_case_at_projection(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    root: &TerminalStructuralValue,
    cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
    path: &[StructuralPathSegment],
) -> Option<StructuralCaseId> {
    let selected = project_structural_value(types, root, path).ok()?;
    let contents = cases.get(&StructuralRuntimePlace::from(&selected))?;
    let declaration = types.get(&selected.structural_type)?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declaration.shape
    else {
        return None;
    };
    (contents.structural_type == selected.structural_type
        && terminal_psi::find_structural_case(cases, contents.case).is_some())
    .then_some(contents.case)
}

impl TerminalExecution<'_> {
    /// Select a parent field namespace from its actual discriminator. A Case
    /// suffix selects declarations, never a new physical payload record.
    pub(crate) fn active_field_declarations(
        &self,
        parent_type: StructuralTypeId,
        parent: &StructuralRuntimePlace,
        case: Option<StructuralCaseId>,
    ) -> Result<
        (
            &[terminal_psi::StructuralFieldDeclaration],
            crate::values::StructuralFieldNamespace,
        ),
        TerminalInterpretError,
    > {
        let declaration = self
            .program
            .structural_types()
            .get(&parent_type)
            .ok_or_else(malformed)?;
        if let Some(case) = case {
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return Err(malformed());
            };
            let contents = self.structural_cases.get(parent).ok_or_else(malformed)?;
            if contents.structural_type != parent_type || contents.case != case {
                return Err(malformed());
            }
            let selected = terminal_psi::find_structural_case(cases, case).ok_or_else(malformed)?;
            Ok((
                &selected.fields,
                crate::values::StructuralFieldNamespace::CasePayload,
            ))
        } else {
            Ok((
                declaration.shape.direct_fields().ok_or_else(malformed)?,
                crate::values::StructuralFieldNamespace::Member,
            ))
        }
    }

    /// A typed claim catalogue lists possible payloads, not simultaneously
    /// resident ones. Authenticate each guard in backing storage before joining
    /// either a return roster or the caller's receiving roster to live custody.
    pub(crate) fn active_structural_claims(
        &self,
        source: PlaceId,
        catalogue: &[terminal_psi::StructuralResultClaimBinding],
    ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, TerminalInterpretError> {
        if catalogue.is_empty() {
            return Ok(Vec::new());
        }
        let root = crate::custody::verified_structural_place(&self.structural_values, source)?;
        self.active_claims_in_value(root, catalogue)
    }

    /// Staged values use the same stored-tag join before their place is published.
    pub(crate) fn active_claims_in_value(
        &self,
        root: &TerminalStructuralValue,
        catalogue: &[terminal_psi::StructuralResultClaimBinding],
    ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, TerminalInterpretError> {
        catalogue
            .iter()
            .filter_map(|binding| {
                match case_projection_in_value_is_active(
                    &self.program.structural_types(),
                    root,
                    &self.structural_cases,
                    &binding.path,
                ) {
                    Ok(true) => Some(Ok(binding.clone())),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
    }

    /// A logical owner may retain a physical prefix after projected transfer.
    /// Recover the actual declaration from every containing live occurrence,
    /// not only an empty-path allocation root; all such owners must agree.
    pub(crate) fn actual_storage_type_from_owner(
        &self,
        target: &StructuralRuntimePlace,
    ) -> Result<StructuralTypeId, TerminalInterpretError> {
        let mut actual = None;
        if let Some(owner) = self.activation_storage.owners.get(target) {
            return Ok(owner.structural_type);
        }
        for owner in self
            .structural_values
            .values()
            .chain(
                self.call_stack
                    .iter()
                    .flat_map(|frame| frame.structural_values.values()),
            )
            .chain(
                self.borrowed_entry_storage
                    .iter()
                    .map(|(_, _, value)| value),
            )
        {
            if owner.opaque_identity != target.opaque_identity
                || !target.path.starts_with(&owner.path)
            {
                continue;
            }
            let candidate = self.actual_storage_type(owner, target)?;
            if actual.is_some_and(|previous| previous != candidate) {
                return Err(malformed());
            }
            actual = Some(candidate);
        }
        actual.ok_or_else(malformed)
    }

    /// Runtime paths omit Case after authenticating its overlay. Recover a
    /// containing declaration using the current stored cases, never by choosing
    /// one case whose spelled field happens to fit. This is storage inspection,
    /// not authority to observe or replace that storage.
    pub(crate) fn actual_storage_type(
        &self,
        root: &TerminalStructuralValue,
        target: &StructuralRuntimePlace,
    ) -> Result<StructuralTypeId, TerminalInterpretError> {
        if target.opaque_identity != root.opaque_identity || !target.path.starts_with(&root.path) {
            return Err(malformed());
        }
        let mut structural_type = root.structural_type;
        let mut parent = StructuralRuntimePlace::from(root);
        for segment in &target.path[root.path.len()..] {
            let declaration = self
                .program
                .structural_types()
                .get(&structural_type)
                .ok_or_else(malformed)?;
            match segment {
                StructuralPathSegment::Field(identity) => {
                    let common = match &declaration.shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                            terminal_psi::field_by_identity(fields, *identity)
                        }
                        _ => None,
                    };
                    let field = if let Some(common) = common {
                        common
                    } else {
                        let (StructuralTypeShape::Sum { cases }
                        | StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
                        else {
                            return Err(malformed());
                        };
                        let actual = self.structural_cases.get(&parent).ok_or_else(malformed)?;
                        if actual.structural_type != structural_type {
                            return Err(malformed());
                        }
                        let case = terminal_psi::find_structural_case(cases, actual.case)
                            .ok_or_else(malformed)?;
                        terminal_psi::field_by_identity(&case.fields, *identity)
                            .ok_or_else(malformed)?
                    };
                    let Some(child) = field.field_type.as_structural() else {
                        return Err(malformed());
                    };
                    structural_type = child;
                }
                StructuralPathSegment::FixedIndex(index) => {
                    let StructuralTypeShape::FixedArray { element, length } = declaration.shape
                    else {
                        return Err(malformed());
                    };
                    if *index >= length {
                        return Err(malformed());
                    }
                    structural_type = element;
                }
                _ => return Err(malformed()),
            }
            parent.path.push(segment.clone());
        }
        Ok(structural_type)
    }

    /// Check every selected overlay against its current stored discriminator.
    /// Verification already proved the source permission and membership; this
    /// joins the admitted operation to the interpreter's actual backing before
    /// a call or write can change it. Runtime indices have already been selected.
    pub(crate) fn validate_case_projections(
        &self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        for projection in operation.kind.structural_projections() {
            for (position, segment) in projection.path.iter().enumerate() {
                let StructuralPathSegment::Case(case) = segment else {
                    continue;
                };
                let carrier =
                    self.resolve_case_guard_carrier(projection.root, &projection.path[..position])?;
                self.active_field_declarations(
                    carrier.structural_type,
                    &StructuralRuntimePlace::from(&carrier),
                    Some(*case),
                )?;
            }
        }
        Ok(())
    }

    /// Runtime sum storage contains only the active case's payload under its
    /// declared field names. A canonical Case segment therefore checks the
    /// stored discriminator rather than adding a storage-path component. Do
    /// not erase it without that check: two cases may reuse a field name.
    /// Retain both physical storage and its checked observation path. Case tags
    /// select field namespaces but are absent from physical storage keys; readers
    /// still need the exact live tag and declaration-selected field identities.
    pub(crate) fn case_payload_projection(
        &self,
        source: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<(TerminalStructuralValue, Vec<StructuralPathSegment>), TerminalInterpretError> {
        let mut projection = Vec::with_capacity(path.len());
        let mut leaf =
            crate::custody::verified_structural_place(&self.structural_values, source).cloned()?;
        let mut case_fields = None;
        let mut saw_case = false;
        for segment in path {
            let declaration = self
                .program
                .structural_types()
                .get(&leaf.structural_type)
                .ok_or_else(malformed)?;
            match segment {
                CanonicalStructuralPathSegment::Case(case) => {
                    if case_fields.is_some() {
                        return Err(malformed());
                    }
                    let (StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
                    else {
                        return Err(malformed());
                    };
                    let contents = self
                        .structural_cases
                        .get(&StructuralRuntimePlace::from(&leaf))
                        .ok_or_else(malformed)?;
                    if contents.structural_type != leaf.structural_type || contents.case != *case {
                        return Err(malformed());
                    }
                    case_fields = Some(
                        cases
                            .iter()
                            .find(|candidate| candidate.id == *case)
                            .ok_or_else(malformed)?
                            .fields
                            .as_slice(),
                    );
                    projection.push(StructuralPathSegment::Case(*case));
                    saw_case = true;
                }
                CanonicalStructuralPathSegment::Field(field) => {
                    let fields = match case_fields.take() {
                        Some(fields) => fields,
                        None => match &declaration.shape {
                            terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                            _ => return Err(malformed()),
                        },
                    };
                    let field = fields
                        .iter()
                        .find(|candidate| {
                            candidate.id == *field && !candidate.relevance.is_erased()
                        })
                        .ok_or_else(malformed)?;
                    let Some(child) = field.field_type.as_structural() else {
                        return Err(malformed());
                    };
                    leaf.path.push(StructuralPathSegment::Field(field.identity));
                    projection.push(StructuralPathSegment::Field(field.identity));
                    leaf.structural_type = child;
                }
                CanonicalStructuralPathSegment::FixedIndex(index) => {
                    let StructuralTypeShape::FixedArray { element, length } = declaration.shape
                    else {
                        return Err(malformed());
                    };
                    if case_fields.is_some() || *index >= length {
                        return Err(malformed());
                    }
                    leaf.path.push(StructuralPathSegment::FixedIndex(*index));
                    projection.push(StructuralPathSegment::FixedIndex(*index));
                    leaf.structural_type = element;
                }
            }
        }
        if !saw_case || case_fields.is_some() {
            return Err(malformed());
        }
        Ok((leaf, projection))
    }

    pub(crate) fn bind_structural_cases(
        &mut self,
        cases: &[TerminalStructuralCaseValue],
    ) -> Result<(), TerminalInterpretError> {
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or(malformed())?;
        for contents in cases {
            let parameter = machine
                .structural_parameters
                .get(contents.argument_index as usize)
                .ok_or(malformed())?;
            let root = crate::custody::verified_structural_place(
                &self.structural_values,
                parameter.place,
            )?;
            let selected_type = resolve_structural_path_type(
                &self.program.structural_types(),
                root.structural_type,
                &contents.path,
            )?;
            let declaration = self
                .program
                .structural_types()
                .get(&selected_type)
                .ok_or(malformed())?;
            // This input form establishes a complete empty-payload case. It
            // cannot claim that a missing payload satisfies its declaration.
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return Err(malformed());
            };
            if !cases
                .iter()
                .any(|case| case.id == contents.case && case.fields.is_empty())
            {
                return Err(malformed());
            }
            let mut selected = StructuralRuntimePlace::from(root);
            selected.path.extend_from_slice(&contents.path);
            if self
                .structural_cases
                .insert(
                    selected,
                    StructuralCaseContents {
                        structural_type: selected_type,
                        case: contents.case,
                    },
                )
                .is_some()
            {
                return Err(malformed());
            }
        }
        Ok(())
    }

    pub(crate) fn observe_structural_case(
        &self,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<StructuralCaseId, TerminalInterpretError> {
        if !self.structural_values.contains_key(&source) {
            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                source,
            ));
        }
        let selected =
            self.resolve_live_projection(source, path, StructuralAccess::SharedBorrow)?;
        let contents = self
            .structural_cases
            .get(&StructuralRuntimePlace::from(&selected))
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                source,
            ))?;
        if contents.structural_type != selected.structural_type {
            return Err(malformed());
        }
        Ok(contents.case)
    }
}

/// Runtime payload fields overlay their sum's backing. Check each Case before
/// omitting it from the physical path; two cases may use the same field names.
pub(crate) fn validate_case_projection(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
    source: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<(), TerminalInterpretError> {
    if case_projection_is_active(types, values, cases, source, path)? {
        Ok(())
    } else {
        Err(malformed())
    }
}

/// A typed Case path is also its payload's custody guard. A different valid
/// stored tag makes the path inactive, not a missing permission to fabricate.
/// Unknown contents, incompatible storage types and undeclared tags still fail.
/// Stop at the first contrary outer guard: inner inactive payloads have no
/// runtime contents to inspect.
pub(crate) fn case_projection_is_active(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
    source: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<bool, TerminalInterpretError> {
    if !path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
    {
        return Ok(true);
    }
    let root = crate::custody::verified_structural_place(values, source)?;
    case_projection_in_value_is_active(types, root, cases, path)
}

pub(crate) fn case_projection_in_value_is_active(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    root: &TerminalStructuralValue,
    cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
    path: &[StructuralPathSegment],
) -> Result<bool, TerminalInterpretError> {
    case_projection_with_membership_is_active(types, root.structural_type, path, &|prefix| {
        stored_case_at_projection(types, root, cases, prefix)
    })
}

/// A declaration roster includes absent payloads. Compare value membership
/// only after authenticating its actual stored discriminator at each guard.
pub(crate) fn active_projected_qualifications(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    root: &TerminalStructuralValue,
    cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
    rows: &[terminal_psi::StructuralPathQualification],
) -> Result<Vec<terminal_psi::StructuralPathQualification>, TerminalInterpretError> {
    let mut active = Vec::new();
    for row in rows {
        if case_projection_in_value_is_active(types, root, cases, &row.path)? {
            active.push(row.clone());
        }
    }
    Ok(active)
}

/// Established and uncommitted constructor storage authenticate the same
/// typed guards. The caller supplies actual stored membership at each prefix;
/// absence of a discriminator never counts as an inactive payload.
pub(crate) fn case_projection_with_membership_is_active(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    membership: &impl Fn(&[StructuralPathSegment]) -> Option<StructuralCaseId>,
) -> Result<bool, TerminalInterpretError> {
    let mut selected = path
        .iter()
        .enumerate()
        .filter_map(|(position, segment)| {
            if let StructuralPathSegment::Case(case) = segment {
                Some((position, *case))
            } else {
                None
            }
        })
        .peekable();
    if selected.peek().is_none() {
        return Ok(true);
    }
    // This judgment authenticates the enclosing overlays. The consuming
    // operation validates its endpoint, which may be an inline scalar with no
    // standalone structural declaration or a final payload namespace.
    for (position, case) in selected {
        let prefix = &path[..position];
        let selected_type = resolve_structural_path_type(types, structural_type, prefix)?;
        let stored = membership(prefix).ok_or_else(malformed)?;
        let declaration = types.get(&selected_type).ok_or_else(malformed)?;
        let (StructuralTypeShape::Sum {
            cases: declared_cases,
        }
        | StructuralTypeShape::Mixed {
            cases: declared_cases,
            ..
        }) = &declaration.shape
        else {
            return Err(malformed());
        };
        if terminal_psi::find_structural_case(declared_cases, stored).is_none() {
            return Err(malformed());
        }
        if stored != case {
            return Ok(false);
        }
    }
    Ok(true)
}
