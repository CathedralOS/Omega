//! Owned component data uses the ordinary complete-content capture and entry
//! initializer. Dense field and case IDs belong to their respective artifacts;
//! rejoin each through its declared identity before the child admits the data.
//! This transfers initialized scalar/tag contents, never loans, backing grants,
//! erased evidence, qualifications or cleanup authority.

use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
use terminal_psi::{StructuralArgument, StructuralTypeDeclaration, TerminalIdentityTable};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::program::{ProgramDeclarations, TerminalProgram};
use crate::structural_storage::contents::{self, StorageLeafKind};
use crate::{
    TerminalBoundaryStorageValue, TerminalStorageContents, TerminalStructuralContents,
    TerminalStructuralValue,
};

#[cfg(test)]
mod tests;

impl TerminalExecution<'_> {
    pub(super) fn capture_component_contents(
        &self,
        argument: &StructuralArgument,
        value: &TerminalStructuralValue,
        argument_index: usize,
        provider: &TerminalProgram<'_>,
        destination_type: StructuralTypeId,
    ) -> Result<TerminalStructuralContents, TerminalInterpretError> {
        let description = contents::describe(self, value, argument.access, None)?;
        if description.requires_erased_establishment
            || description.leaves.iter().any(|leaf| {
                matches!(
                    leaf.kind,
                    StorageLeafKind::Opaque { .. } | StorageLeafKind::ByteField { .. }
                )
            })
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "component contents cannot manufacture opaque backing or erased custody",
            ));
        }
        let (values, _) = contents::capture(
            self,
            argument.place,
            value,
            None,
            None,
            argument.access,
            &description.leaves,
            &[],
        )?;
        let mapping = ComponentContentsMapping {
            source_types: self.structural_types(),
            source_identities: &self.program.catalogs.identities,
            destination_types: provider.program.structural_types(),
            destination_identities: &provider.program.catalogs.identities,
        };
        let mut values = values
            .into_iter()
            .map(|contents| mapping.rehome(value.structural_type, destination_type, contents))
            .collect::<Result<Vec<_>, _>>()?;
        values.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(TerminalStructuralContents {
            argument_index: u32::try_from(argument_index).map_err(|_| malformed())?,
            values,
        })
    }
}

struct ComponentContentsMapping<'source, 'destination> {
    source_types: ProgramDeclarations<'source, StructuralTypeDeclaration>,
    source_identities: &'source TerminalIdentityTable,
    destination_types: ProgramDeclarations<'destination, StructuralTypeDeclaration>,
    destination_identities: &'destination TerminalIdentityTable,
}

impl ComponentContentsMapping<'_, '_> {
    fn rehome(
        &self,
        mut source_type: StructuralTypeId,
        mut destination_type: StructuralTypeId,
        contents: TerminalBoundaryStorageValue,
    ) -> Result<TerminalBoundaryStorageValue, TerminalInterpretError> {
        let mut path = Vec::with_capacity(contents.path.len());
        let mut source_case_fields = None;
        let mut destination_case_fields = None;
        for (position, segment) in contents.path.iter().enumerate() {
            let source = self.source_types.declaration(&source_type)?;
            let destination = self.destination_types.declaration(&destination_type)?;
            if self.map_identity(source.identity)? != destination.identity {
                return Err(malformed());
            }
            match segment {
                Path::Case(case) => {
                    if source_case_fields.is_some() {
                        return Err(malformed());
                    }
                    let source = terminal_psi::find_structural_case(
                        source.shape.sum_cases().ok_or_else(malformed)?,
                        *case,
                    )
                    .ok_or_else(malformed)?;
                    let identity = self.map_identity(source.identity)?;
                    let destination = terminal_psi::structural_case_by_identity(
                        destination.shape.sum_cases().ok_or_else(malformed)?,
                        identity,
                    )
                    .ok_or_else(malformed)?;
                    path.push(Path::Case(destination.id));
                    source_case_fields = Some(source.fields.as_slice());
                    destination_case_fields = Some(destination.fields.as_slice());
                }
                Path::Field(field) => {
                    let source_fields = source_case_fields
                        .take()
                        .or_else(|| source.shape.direct_fields())
                        .ok_or_else(malformed)?;
                    let source = source_fields
                        .iter()
                        .find(|candidate| candidate.id == *field)
                        .ok_or_else(malformed)?;
                    let destination_fields = destination_case_fields
                        .take()
                        .or_else(|| destination.shape.direct_fields())
                        .ok_or_else(malformed)?;
                    let destination = terminal_psi::live_field_by_identity(
                        destination_fields,
                        self.map_identity(source.identity)?,
                    )
                    .ok_or_else(malformed)?;
                    if source.relevance.is_erased() {
                        return Err(malformed());
                    }
                    path.push(Path::Field(destination.id));
                    // Scalar fields terminate a path. Structural children retain
                    // their own nominal declaration for every further segment.
                    match (
                        source.field_type.as_structural(),
                        destination.field_type.as_structural(),
                    ) {
                        (Some(source), Some(destination)) => {
                            source_type = source;
                            destination_type = destination;
                        }
                        (None, None) if position + 1 == contents.path.len() => {}
                        _ => return Err(malformed()),
                    }
                }
                Path::FixedIndex(element_index) => {
                    if source_case_fields.is_some() {
                        return Err(malformed());
                    }
                    let (
                        terminal_psi::StructuralTypeShape::FixedArray {
                            element: source,
                            length: source_length,
                        },
                        terminal_psi::StructuralTypeShape::FixedArray {
                            element: destination,
                            length: destination_length,
                        },
                    ) = (&source.shape, &destination.shape)
                    else {
                        return Err(malformed());
                    };
                    if source_length.literal() != destination_length.literal()
                        || !source_length
                            .literal()
                            .is_some_and(|length| *element_index < length)
                    {
                        return Err(malformed());
                    }
                    source_type = *source;
                    destination_type = *destination;
                    path.push(*segment);
                }
                Path::Referent => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "component contents cannot cross a reference",
                    ));
                }
            }
        }
        if source_case_fields.is_some() {
            return Err(malformed());
        }
        if self.map_identity(self.source_types.declaration(&source_type)?.identity)?
            != self
                .destination_types
                .declaration(&destination_type)?
                .identity
        {
            return Err(malformed());
        }
        let contents = match contents.contents {
            TerminalStorageContents::Scalar { .. } => contents.contents,
            TerminalStorageContents::Case(case) => {
                let source = self.source_types.declaration(&source_type)?;
                let source = terminal_psi::find_structural_case(
                    source.shape.sum_cases().ok_or_else(malformed)?,
                    case,
                )
                .ok_or_else(malformed)?;
                let destination = self.destination_types.declaration(&destination_type)?;
                let destination = terminal_psi::structural_case_by_identity(
                    destination.shape.sum_cases().ok_or_else(malformed)?,
                    self.map_identity(source.identity)?,
                )
                .ok_or_else(malformed)?;
                TerminalStorageContents::Case(destination.id)
            }
        };
        Ok(TerminalBoundaryStorageValue { path, contents })
    }

    fn map_identity(
        &self,
        identity: terminal_psi::TerminalIdentityIndex,
    ) -> Result<terminal_psi::TerminalIdentityIndex, TerminalInterpretError> {
        let identity = self
            .source_identities
            .text(identity)
            .filter(|identity| !identity.is_empty())
            .ok_or_else(malformed)?;
        self.destination_identities
            .index_of(identity)
            .ok_or_else(malformed)
    }
}
