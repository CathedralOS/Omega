use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::scalar_storage::ScalarCell;
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

mod bindings;
pub(crate) mod empty_storage;
mod loans;
mod storage;
#[cfg(test)]
mod tests;
pub(crate) use bindings::{PackedHomeRetirement, ScalarArrayBindings};
pub(crate) use loans::borrowed_array_memberships_match;
use std::sync::Arc;
use storage::ScalarArrayStorage;
/// Portable owned scalar payload. It contains no invocation-local residents.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalScalarArrayValue {
    pub(crate) storage_identity: Option<u64>,
    pub qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub structural_type: StructuralTypeId,
    elements: Arc<ScalarArrayStorage<TerminalScalarValue>>,
}

impl TerminalScalarArrayValue {
    pub fn elements(&self) -> &[TerminalScalarValue] {
        self.elements.elements()
    }
    pub fn len(&self) -> usize {
        self.elements().len()
    }
    pub fn is_empty(&self) -> bool {
        self.elements().is_empty()
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

    fn snapshot(&self) -> Result<Self, TerminalInterpretError> {
        Ok(Self {
            storage_identity: None,
            qualifications: self.qualifications.clone(),
            structural_type: self.structural_type,
            elements: Arc::new(self.elements.copy()?),
        })
    }

    fn into_runtime(self) -> Result<PackedScalarArrayValue, TerminalInterpretError> {
        let storage = match Arc::try_unwrap(self.elements) {
            Ok(storage) => storage,
            Err(shared) => shared.copy()?,
        };
        Ok(PackedScalarArrayValue {
            storage_identity: self.storage_identity,
            qualifications: self.qualifications,
            structural_type: self.structural_type,
            elements: Arc::new(storage.into_cells()?),
        })
    }
}

/// Exact row-major primitive payload; the type retains all array dimensions.
/// Inspection clones share cells and their reservation. Guest copies observe
/// independent local snapshots. Mutation detaches shared cell containers while
/// preserving this value's logical identity and every resident endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackedScalarArrayValue {
    /// Allocated from the ordinary referent allocator when this payload's
    /// location is first observed. Snapshots clear it: a copy has new storage.
    pub(crate) storage_identity: Option<u64>,
    pub qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub structural_type: StructuralTypeId,
    elements: Arc<ScalarArrayStorage<ScalarCell>>,
}

impl PackedScalarArrayValue {
    pub fn elements(&self) -> Result<Vec<TerminalScalarValue>, TerminalInterpretError> {
        self.cells().iter().map(ScalarCell::read).collect()
    }

    pub fn len(&self) -> usize {
        self.cells().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells().is_empty()
    }

    pub fn element(
        &self,
        index: usize,
    ) -> Result<Option<TerminalScalarValue>, TerminalInterpretError> {
        self.cell(index).map(ScalarCell::read).transpose()
    }

    pub(crate) fn cells(&self) -> &[ScalarCell] {
        self.elements.cells()
    }

    pub(crate) fn cell(&self, index: usize) -> Option<&ScalarCell> {
        self.cells().get(index)
    }

    pub(crate) fn set_element(
        &mut self,
        index: usize,
        value: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        if self.cell(index).ok_or_else(malformed)?.scalar_type() != value.scalar_type()
            || !crate::scalar_operations::terminal_scalar_belongs_to_type(value)
        {
            return Err(malformed());
        }
        self.mutable_storage()?.cells_mut()[index].write(value)
    }

    /// Validate and fund installation before any original roots are published.
    /// Detaching inspection storage preserves cells; no resident is installed.
    pub(crate) fn prepare_resident_install(
        &mut self,
        index: usize,
        resident: &ScalarCell,
    ) -> Result<(), TerminalInterpretError> {
        let previous = self.cell(index).ok_or_else(malformed)?;
        if !matches!(resident, ScalarCell::Resident { .. })
            || previous.scalar_type() != resident.scalar_type()
            || previous.read()? != resident.read()?
            || (matches!(previous, ScalarCell::Resident { .. }) && previous != resident)
        {
            return Err(malformed());
        }
        self.mutable_storage()?;
        Ok(())
    }

    /// Attach an authenticated endpoint. A prior preparation over the same
    /// exclusive container ensures this commit needs no further allocation.
    pub(crate) fn install_resident(
        &mut self,
        index: usize,
        resident: ScalarCell,
    ) -> Result<(), TerminalInterpretError> {
        self.prepare_resident_install(index, &resident)?;
        Arc::get_mut(&mut self.elements)
            .ok_or_else(malformed)?
            .cells_mut()[index] = resident;
        Ok(())
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
            elements: Arc::new(ScalarArrayStorage::collect(
                count,
                values.into_iter().map(|value| value.map(ScalarCell::Local)),
                account,
            )?),
        })
    }

    /// A guest value copy has independent storage and no observed address.
    /// `Clone` is only a read-only inspection alias, not this guest operation.
    pub(crate) fn snapshot(&self) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
        Ok(TerminalScalarArrayValue {
            storage_identity: None,
            qualifications: self.qualifications.clone(),
            structural_type: self.structural_type,
            elements: Arc::new(self.elements.observe()?),
        })
    }

    fn mutable_storage(
        &mut self,
    ) -> Result<&mut ScalarArrayStorage<ScalarCell>, TerminalInterpretError> {
        if Arc::get_mut(&mut self.elements).is_none() {
            self.elements = Arc::new(self.elements.copy_for_mutation()?);
        }
        Arc::get_mut(&mut self.elements).ok_or_else(malformed)
    }
}

/// An owned unrestricted array with its checked predicate memberships and no claims.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
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
            snapshot.elements().iter().map(|value| value.scalar_type()),
            qualifications,
        )
    }

    /// Packed operands and placed snapshots obey the same transport contract.
    /// Qualification adds no payload bytes and does not change array geometry.
    pub(crate) fn validate_scalar_array_contents_qualifications(
        &self,
        structural_type: StructuralTypeId,
        mut elements: impl ExactSizeIterator<Item = ScalarType>,
        qualifications: &[semantic_vocabulary::StructuralDomainId],
    ) -> Result<(), TerminalInterpretError> {
        let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
            self.program.structural_types().values(),
            structural_type,
        )
        .ok_or_else(malformed)?;
        if u64::try_from(elements.len()).ok() != Some(count)
            || elements.any(|element_type| element_type != scalar_type)
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
            || u64::try_from(value.len()).ok() != Some(count)
            || value
                .cells()
                .iter()
                .any(|cell| cell.scalar_type() != scalar_type)
            || crate::custody::place_is_claimed(&self.live_claims, argument.place)
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
        if count == 0 {
            let initialized = if let Some(array) = self.scalar_array_values.get(&argument.place) {
                array.structural_type == root_type
                    && terminal_semantics::scalar_array_leaf_shape(types.values(), root_type)
                        .is_some_and(|(_, count)| count == 0 && array.is_empty())
            } else {
                let selected = self.resolve_live_storage_projection(
                    argument.place,
                    &argument.path,
                    StructuralAccess::SharedBorrow,
                )?;
                if let Some(loan) = selected.array_loan {
                    loan.initialized_empty_scalar_array(self, &[], structural_type)?
                } else {
                    if selected.value.structural_type != structural_type {
                        return Err(malformed());
                    }
                    self.initialized_empty_scalar_array(
                        &crate::values::StructuralRuntimePlace::from(&selected.value),
                        structural_type,
                    )?
                }
            };
            if !initialized {
                return Err(malformed());
            }
        }
        let mut dimensions = Vec::new();
        let mut current = structural_type;
        while let Some(StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        }) = types.get(&current).map(|declaration| &declaration.shape)
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

    pub(crate) fn initialized_empty_scalar_array(
        &self,
        home: &crate::values::StructuralRuntimePlace,
        structural_type: StructuralTypeId,
    ) -> Result<bool, TerminalInterpretError> {
        self.empty_array_storage.initialized(
            &self.program.structural_types(),
            &self.structural_byte_storage,
            home,
            structural_type,
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
        self.scalar_array_values.publish(result.place, snapshot)?;
        Ok(())
    }
}
