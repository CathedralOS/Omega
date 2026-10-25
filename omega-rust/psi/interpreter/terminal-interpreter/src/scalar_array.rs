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
mod storage;
#[cfg(test)]
mod tests;
pub(crate) use loans::borrowed_array_memberships_match;
use std::sync::Arc;
use storage::ScalarArrayStorage;
/// Exact row-major primitive payload; the type retains all array dimensions.
/// Inspection clones share read-only backing and its reservation. Guest value
/// copies use explicit snapshots; writes preserve this value's logical identity
/// while detaching any shared physical backing before mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarArrayValue {
    /// Allocated from the ordinary referent allocator when this payload's
    /// location is first observed. Snapshots clear it: a copy has new storage.
    pub(crate) storage_identity: Option<u64>,
    pub qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub structural_type: StructuralTypeId,
    elements: Arc<ScalarArrayStorage>,
}

impl TerminalScalarArrayValue {
    pub fn elements(&self) -> &[TerminalScalarValue] {
        self.elements.elements()
    }

    fn collect(
        structural_type: StructuralTypeId,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        count: u64,
        values: impl IntoIterator<Item = Result<TerminalScalarValue, TerminalInterpretError>>,
        account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    ) -> Result<Self, TerminalInterpretError> {
        Ok(Self {
            storage_identity: None,
            qualifications,
            structural_type,
            elements: Arc::new(ScalarArrayStorage::collect(count, values, account)?),
        })
    }

    /// A guest value copy has independent storage and no observed address.
    /// `Clone` is only a read-only inspection alias, not this guest operation.
    fn snapshot(&self) -> Result<Self, TerminalInterpretError> {
        Ok(Self {
            storage_identity: None,
            qualifications: self.qualifications.clone(),
            structural_type: self.structural_type,
            elements: Arc::new(self.elements.copy()?),
        })
    }

    pub(crate) fn elements_mut(
        &mut self,
    ) -> Result<&mut [TerminalScalarValue], TerminalInterpretError> {
        if Arc::get_mut(&mut self.elements).is_none() {
            self.elements = Arc::new(self.elements.copy()?);
        }
        Ok(Arc::get_mut(&mut self.elements)
            .ok_or_else(malformed)?
            .elements_mut())
    }
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

    /// Logical membership was discharged by Psi and authority was admitted by
    /// the verified product. Runtime transport checks the exact scalar shape
    /// and nominal carrier, not a byte-only reconstruction of that proof.
    pub(crate) fn validate_scalar_array_qualifications(
        &self,
        snapshot: &TerminalScalarArrayValue,
        qualifications: &[semantic_vocabulary::StructuralDomainId],
    ) -> Result<(), TerminalInterpretError> {
        self.validate_scalar_array_contents_qualifications(
            snapshot.structural_type,
            snapshot.elements(),
            qualifications,
        )
    }

    /// Packed operands and placed snapshots obey the same transport contract.
    /// Qualification adds no payload bytes and does not change array geometry.
    pub(crate) fn validate_scalar_array_contents_qualifications(
        &self,
        structural_type: StructuralTypeId,
        elements: &[TerminalScalarValue],
        qualifications: &[semantic_vocabulary::StructuralDomainId],
    ) -> Result<(), TerminalInterpretError> {
        let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
            self.program.structural_types().values(),
            structural_type,
        )
        .ok_or_else(malformed)?;
        if u64::try_from(elements.len()).ok() != Some(count)
            || elements
                .iter()
                .any(|value| value.scalar_type() != scalar_type)
            || qualifications.iter().any(|domain| {
                !self
                    .program
                    .catalogs
                    .structural_domains
                    .iter()
                    .any(|declaration| {
                        declaration.id == *domain && declaration.carrier == structural_type
                    })
            })
        {
            return Err(malformed());
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
            || u64::try_from(value.elements().len()).ok() != Some(count)
            || value
                .elements()
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
        value.snapshot()
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
        let mut path = argument.path.clone();
        let prefix = path.len();
        let elements = (0..count).map(|leaf| {
            path.truncate(prefix);
            let mut remainder = leaf;
            let mut stride = count;
            for length in &dimensions {
                stride /= length;
                path.push(StructuralPathSegment::FixedIndex(remainder / stride));
                remainder %= stride;
            }
            self.read_primitive_place(argument.place, &path, scalar_type)
        });
        TerminalScalarArrayValue::collect(
            structural_type,
            Vec::new(),
            count,
            elements,
            self.account.as_ref(),
        )
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
        let contents = elements.iter().map(|element| {
            let value = self.scalar_value(*element)?;
            if value.scalar_type() != scalar_type {
                return Err(malformed());
            }
            Ok(value)
        });
        let snapshot = TerminalScalarArrayValue::collect(
            result.structural_type,
            result.qualifications.clone(),
            count,
            contents,
            self.account.as_ref(),
        )?;
        self.validate_scalar_array_qualifications(&snapshot, &result.qualifications)?;
        self.scalar_array_values.insert(result.place, snapshot)?;
        Ok(())
    }
}
