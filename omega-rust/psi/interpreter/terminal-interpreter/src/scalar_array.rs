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
pub(crate) use loans::borrowed_array_memberships_match;
/// Exact row-major primitive payload; the type retains all array dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarArrayValue {
    pub qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub structural_type: StructuralTypeId,
    pub elements: Vec<TerminalScalarValue>,
}

/// An owned unrestricted array with its checked predicate memberships and no claims.
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
        let mut snapshot = self.scalar_array_snapshot(argument, parameter.structural_type)?;
        self.validate_scalar_array_qualifications(&snapshot, &parameter.qualifications)?;
        snapshot.qualifications = parameter.qualifications.clone();
        Ok(snapshot)
    }

    /// Predicate membership is checked against the exact current payload;
    /// neither a receiving type nor a prior initializer can invent authority.
    pub(crate) fn validate_scalar_array_qualifications(
        &self,
        snapshot: &TerminalScalarArrayValue,
        qualifications: &[semantic_vocabulary::StructuralDomainId],
    ) -> Result<(), TerminalInterpretError> {
        self.validate_scalar_array_contents_qualifications(
            snapshot.structural_type,
            &snapshot.elements,
            qualifications,
        )
    }

    /// The same current-byte judgment applies to packed state operands before
    /// their payload is staged into ordinary placed storage.
    pub(crate) fn validate_scalar_array_contents_qualifications(
        &self,
        structural_type: StructuralTypeId,
        elements: &[TerminalScalarValue],
        qualifications: &[semantic_vocabulary::StructuralDomainId],
    ) -> Result<(), TerminalInterpretError> {
        if !qualifications.is_empty() {
            // Predicate qualification observes the current exact bytes. It
            // never changes the array's storage or lends route/claim authority.
            let types = self.program.structural_types();
            let StructuralTypeShape::FixedArray { element, .. } =
                types.get(&structural_type).ok_or_else(malformed)?.shape
            else {
                return Err(malformed());
            };
            let byte_type = semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                8,
            )
            .map_err(|_| malformed())?;
            if types.get(&element).map(|row| &row.shape)
                != Some(&StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    byte_type,
                )))
            {
                return Err(malformed());
            }
            let bytes = elements
                .iter()
                .map(crate::structural_inputs::byte_arrays::storage::scalar_byte)
                .collect::<Result<Vec<_>, _>>()?;
            if !terminal_semantics::byte_literal_qualifications_hold_in(
                self.program.catalogs.structural_domains.iter(),
                structural_type,
                qualifications,
                &bytes,
            ) {
                return Err(malformed());
            }
        }
        Ok(())
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
            qualifications: Vec::new(),
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
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !result.qualification_establishments.is_empty()
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
        let snapshot = TerminalScalarArrayValue {
            qualifications: result.qualifications.clone(),
            structural_type: result.structural_type,
            elements: contents,
        };
        self.validate_scalar_array_qualifications(&snapshot, &result.qualifications)?;
        self.scalar_array_values.insert(result.place, snapshot);
        Ok(())
    }
}
