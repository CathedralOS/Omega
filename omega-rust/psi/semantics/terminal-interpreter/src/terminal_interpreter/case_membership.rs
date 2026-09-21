//! Discriminator observations retain the original structural root and path.

use crate::terminal_interpreter::custody::resolve_structural_path_type;
use crate::terminal_interpreter::values::StructuralRuntimePlace;
use crate::terminal_interpreter::{TerminalExecution, TerminalInterpretError};
use semantic_vocabulary::{PlaceId, StructuralCaseId, StructuralTypeId};
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

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
    structural_type: StructuralTypeId,
    case: StructuralCaseId,
}

impl TerminalExecution {
    pub(super) fn bind_structural_cases(
        &mut self,
        cases: &[TerminalStructuralCaseValue],
    ) -> Result<(), TerminalInterpretError> {
        let machine = self
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
                &self.structural_types,
                root.structural_type,
                &contents.path,
            )?;
            let declaration = self
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

    pub(super) fn observe_structural_case(
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
        let selected_type =
            resolve_structural_path_type(&self.structural_types, root.structural_type, path)?;
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
