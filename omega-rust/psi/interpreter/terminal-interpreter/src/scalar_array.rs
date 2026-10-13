use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;
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

mod loans;
/// Exact row-major primitive payload; the type retains all array dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarArrayValue {
    pub structural_type: StructuralTypeId,
    pub elements: Vec<TerminalScalarValue>,
}

/// An owned unrestricted array returned without claims or qualifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarArrayResult {
    pub value: TerminalScalarArrayValue,
}

impl TerminalExecution<'_> {
    /// Copy a complete unrestricted actual into its exact callee parameter.
    /// Borrowed occurrences retain live storage through `loans`, not this copy.
    /// A whole array value already holds its payload; an array kept in its
    /// owner's storage (`buffer.values`, or a copied field) is read out of it.
    pub(crate) fn prepare_scalar_array_argument(
        &self,
        machine: &TerminalMachine,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
        // Only an owned occurrence copies payload. Borrowed arrays retain their
        // activation home through loans.rs, for reads and writes alike.
        if parameter.access != StructuralAccess::Owned
            || argument.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
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
        self.scalar_array_snapshot(argument, parameter.structural_type)
    }

    /// One canonical owned snapshot for array arguments and structural copies.
    /// Whole payloads clone their exact bits; placed fields and selected rows
    /// read through the ordinary primitive-storage owner. The caller establishes
    /// copy permission; this method never creates source access or a loan.
    pub(crate) fn scalar_array_snapshot(
        &self,
        argument: &StructuralArgument,
        structural_type: StructuralTypeId,
    ) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
        let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
            self.program.structural_types().values(),
            structural_type,
        )
        .ok_or_else(malformed)?;
        let Some(value) = argument
            .path
            .is_empty()
            .then(|| self.scalar_array_values.get(&argument.place))
            .flatten()
        else {
            return self.stored_scalar_array_copy(argument, structural_type, scalar_type, count);
        };
        if value.structural_type != structural_type
            || u64::try_from(value.elements.len()).ok() != Some(count)
            || value
                .elements
                .iter()
                .any(|value| value.scalar_type() != scalar_type)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
            || self.structural_values.contains_key(&argument.place)
        {
            return Err(malformed());
        }
        Ok(value.clone())
    }

    /// Read every leaf of the array `argument` selects, in row-major order,
    /// through the same primitive storage an indexed read of it observes. The
    /// owner keeps its storage; the copy is independent of later writes.
    fn stored_scalar_array_copy(
        &self,
        argument: &StructuralArgument,
        structural_type: StructuralTypeId,
        scalar_type: ScalarType,
        count: u64,
    ) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
        let types = self.program.structural_types();
        let root_type = self.live_structural_type(argument.place)?;
        if crate::custody::resolve_structural_path_type(&types, root_type, &argument.path)?
            != structural_type
        {
            return Err(malformed());
        }
        let mut dimensions = Vec::new();
        let mut current = structural_type;
        while let Some(StructuralTypeShape::FixedArray { element, length }) =
            types.get(&current).map(|declaration| &declaration.shape)
        {
            dimensions.push(*length);
            current = *element;
        }
        // The common shape owner validates the primitive endpoint and computes
        // the zero-aware leaf count. Recomputing a partial dimension product
        // here could overflow before a later zero, rejecting an empty value.
        let mut elements = Vec::with_capacity(usize::try_from(count).map_err(|_| malformed())?);
        let mut path = argument.path.clone();
        let prefix = path.len();
        for leaf in 0..count {
            path.truncate(prefix);
            let mut remainder = leaf;
            let mut stride = count;
            for length in &dimensions {
                stride /= length;
                path.push(StructuralPathSegment::FixedIndex(remainder / stride));
                remainder %= stride;
            }
            elements.push(self.read_primitive_place(argument.place, &path, scalar_type)?);
        }
        Ok(TerminalScalarArrayValue {
            structural_type,
            elements,
        })
    }

    pub(crate) fn execute_scalar_array_establishment(
        &mut self,
        operation: &terminal_psi::Operation,
        elements: &[ValueId],
    ) -> Result<(), TerminalInterpretError> {
        let result = operation.result.structural().ok_or_else(malformed)?;
        let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
            self.program.structural_types().values(),
            result.structural_type,
        )
        .ok_or_else(malformed)?;
        // A cyclic traversal re-establishes the payload on each visit; block
        // entry already retired the earlier visit's elements, so a value
        // still at the place is a second live definition.
        if !result.is_unrestricted_plain()
            || u64::try_from(elements.len()).ok() != Some(count)
            || self.structural_values.contains_key(&result.place)
            || self.scalar_array_values.contains_key(&result.place)
        {
            return Err(malformed());
        }
        let mut contents = Vec::with_capacity(elements.len());
        for element in elements {
            let value = self.scalar_value(*element)?;
            if value.scalar_type() != scalar_type {
                return Err(malformed());
            }
            contents.push(value);
        }
        self.scalar_array_values.insert(
            result.place,
            TerminalScalarArrayValue {
                structural_type: result.structural_type,
                elements: contents,
            },
        );
        Ok(())
    }
}
