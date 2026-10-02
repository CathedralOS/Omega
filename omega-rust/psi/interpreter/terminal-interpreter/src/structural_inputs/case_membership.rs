//! Discriminator observations retain the original structural root and path.

use crate::custody::resolve_structural_path_type;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralCaseId, StructuralTypeId,
};
use std::collections::BTreeMap;
use terminal_psi::{
    Operation, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

#[cfg(test)]
mod tests;

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

impl TerminalExecution {
    /// Check every selected overlay against its current stored discriminator.
    /// Verification already proved the source permission and membership; this
    /// joins the admitted operation to the interpreter's actual backing before
    /// a call or write can change it. Runtime indices have already been selected.
    pub(crate) fn validate_case_projections(
        &self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        for projection in operation.kind.structural_projections() {
            validate_case_projection(
                &self.program.structural_types,
                &self.structural_values,
                &self.structural_cases,
                projection.root,
                projection.path,
            )?;
        }
        Ok(())
    }

    /// Runtime sum storage contains only the active case's payload under its
    /// declared field names. A canonical Case segment therefore checks the
    /// stored discriminator rather than adding a storage-path component. Do
    /// not erase it without that check: two cases may reuse a field name.
    pub(crate) fn case_payload_leaf(
        &self,
        source: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let mut leaf = self.structural_values.get(&source).cloned().ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )?;
        let mut case_fields = None;
        let mut saw_case = false;
        for segment in path {
            let declaration = self
                .program
                .structural_types
                .get(&leaf.structural_type)
                .ok_or_else(invalid)?;
            match segment {
                CanonicalStructuralPathSegment::Case(case) => {
                    if case_fields.is_some() {
                        return Err(invalid());
                    }
                    let (StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
                    else {
                        return Err(invalid());
                    };
                    let contents = self
                        .structural_cases
                        .get(&StructuralRuntimePlace::from(&leaf))
                        .ok_or_else(invalid)?;
                    if contents.structural_type != leaf.structural_type || contents.case != *case {
                        return Err(invalid());
                    }
                    case_fields = Some(
                        cases
                            .iter()
                            .find(|candidate| candidate.id == *case)
                            .ok_or_else(invalid)?
                            .fields
                            .as_slice(),
                    );
                    saw_case = true;
                }
                CanonicalStructuralPathSegment::Field(field) => {
                    let fields = match case_fields.take() {
                        Some(fields) => fields,
                        None => match &declaration.shape {
                            StructuralTypeShape::Record { fields }
                            | StructuralTypeShape::Mixed { fields, .. } => fields,
                            _ => return Err(invalid()),
                        },
                    };
                    let field = fields
                        .iter()
                        .find(|candidate| {
                            candidate.id == *field && !candidate.relevance.is_erased()
                        })
                        .ok_or_else(invalid)?;
                    let StructuralFieldType::Structural(child) = field.field_type else {
                        return Err(invalid());
                    };
                    leaf.path
                        .push(StructuralPathSegment::Field(field.identity.clone()));
                    leaf.structural_type = child;
                }
                CanonicalStructuralPathSegment::FixedIndex(index) => {
                    let StructuralTypeShape::FixedArray { element, length } = declaration.shape
                    else {
                        return Err(invalid());
                    };
                    if case_fields.is_some() || *index >= length {
                        return Err(invalid());
                    }
                    leaf.path.push(StructuralPathSegment::FixedIndex(*index));
                    leaf.structural_type = element;
                }
            }
        }
        if !saw_case || case_fields.is_some() {
            return Err(invalid());
        }
        Ok(leaf)
    }

    pub(crate) fn bind_structural_cases(
        &mut self,
        cases: &[TerminalStructuralCaseValue],
    ) -> Result<(), TerminalInterpretError> {
        let machine = self
            .program
            .machines
            .get(&self.current_machine)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        for contents in cases {
            let parameter = machine
                .structural_parameters
                .get(contents.argument_index as usize)
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
            let root = self.structural_values.get(&parameter.place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
            )?;
            let selected_type = resolve_structural_path_type(
                &self.program.structural_types,
                root.structural_type,
                &contents.path,
            )?;
            let declaration = self
                .program
                .structural_types
                .get(&selected_type)
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
            // This input form establishes a complete empty-payload case. It
            // cannot claim that a missing payload satisfies its declaration.
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            };
            if !cases
                .iter()
                .any(|case| case.id == contents.case && case.fields.is_empty())
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
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
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
        }
        Ok(())
    }

    pub(crate) fn observe_structural_case(
        &self,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<StructuralCaseId, TerminalInterpretError> {
        if path.is_empty()
            && let Some(value) = self.scalar_case_values.get(&source)
        {
            return Ok(value.result_case);
        }
        let root = self.structural_values.get(&source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )?;
        let selected_type = resolve_structural_path_type(
            &self.program.structural_types,
            root.structural_type,
            path,
        )?;
        let mut selected = StructuralRuntimePlace::from(root);
        selected.path.extend_from_slice(path);
        let contents = self.structural_cases.get(&selected).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )?;
        if contents.structural_type != selected_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        Ok(contents.case)
    }
}

/// Runtime payload fields overlay their sum's backing. Check each Case before
/// omitting it from the physical path; two cases may use the same field names.
fn validate_case_projection(
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
    source: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<(), TerminalInterpretError> {
    let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
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
        return Ok(());
    }
    let root =
        values
            .get(&source)
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                source,
            ))?;
    resolve_structural_path_type(types, root.structural_type, path)?;
    for (position, case) in selected {
        let prefix = &path[..position];
        let selected_type = resolve_structural_path_type(types, root.structural_type, prefix)?;
        let mut storage = StructuralRuntimePlace::from(root);
        storage.path.extend(
            prefix
                .iter()
                .filter(|segment| !matches!(segment, StructuralPathSegment::Case(_)))
                .cloned(),
        );
        let contents = cases.get(&storage).ok_or_else(invalid)?;
        if contents.structural_type != selected_type || contents.case != case {
            return Err(invalid());
        }
    }
    Ok(())
}
