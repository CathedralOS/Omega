//! Resolve checked-local storage against its current SSA value or primitive place.

use super::*;
use crate::scalar_source_custody as source_custody;

pub(super) mod structural_cases;
pub(super) mod structural_fields;
#[cfg(test)]
mod tests;
pub(super) use structural_fields::StructuralScalarFieldBinding;

#[derive(Clone)]
pub(super) struct ScalarBindings {
    immutable: Vec<Option<usize>>,
    storage: Vec<(symbols::SymbolHandle, ScalarType, usize)>,
    primitive_storage: Vec<(symbols::SymbolHandle, PlaceId, ScalarType)>,
    /// Authored parameter positions stay separate from dense Terminal positions.
    structural_parameters: Vec<(u32, StructuralParameterDeclaration)>,
    structural_locals: Vec<(symbols::SymbolHandle, StructuralArgument)>,
    local_cases: Vec<structural_cases::LocalCaseBinding>,
    structural_fields: Vec<StructuralScalarFieldBinding>,
    structural_cases: Vec<structural_cases::StructuralCaseBinding>,
}

impl ScalarBindings {
    /// Observe an established whole local without transferring its ownership.
    pub(crate) fn structural_local_observation(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        case: symbols::SymbolHandle,
    ) -> Result<(PlaceId, semantic_vocabulary::StructuralCaseId), LoweringError> {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            argument.source
        else {
            return unsupported("case observation requires an established structural local");
        };
        let mut matches = self
            .structural_locals
            .iter()
            .filter(|(candidate, _)| *candidate == symbol);
        let (_, source) = matches.next().ok_or(LoweringError::Unsupported(
            "case observation lost its local place",
        ))?;
        if !symbol.is_valid()
            || matches.next().is_some()
            || !argument.path.is_empty()
            || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
            || !source.path.is_empty()
            || source.access != StructuralAccess::Owned
        {
            return unsupported("case observation changed its local custody");
        }
        let mut bindings = self.local_cases.iter().filter(|binding| {
            binding.symbol == symbol
                && binding.source == source.place
                && binding.type_identity == argument.type_identity
        });
        let binding = bindings.next().ok_or(LoweringError::Unsupported(
            "local observation lost its exact declared case namespace",
        ))?;
        if bindings.next().is_some() {
            return unsupported("local observation has ambiguous declared case namespaces");
        }
        let case = binding
            .cases
            .iter()
            .find_map(|(symbol, identity)| (*symbol == case).then_some(*identity))
            .ok_or(LoweringError::Unsupported(
                "local observation selected a foreign case",
            ))?;
        Ok((source.place, case))
    }

    pub(crate) fn with_local_cases(mut self, cases: &[structural_cases::LocalCaseBinding]) -> Self {
        self.local_cases = cases.to_vec();
        self
    }

    pub(crate) fn with_structural_observations(
        mut self,
        types: &[StructuralTypeDeclaration],
    ) -> Self {
        self.structural_fields =
            StructuralScalarFieldBinding::collect(&self.structural_parameters, types);
        self.structural_cases =
            structural_cases::StructuralCaseBinding::collect(&self.structural_parameters, types);
        self
    }

    pub(crate) fn with_resolved_structural_observations(
        mut self,
        fields: &[StructuralScalarFieldBinding],
        cases: &[structural_cases::StructuralCaseBinding],
    ) -> Self {
        self.structural_fields = fields.to_vec();
        self.structural_cases = cases.to_vec();
        self
    }

    pub(super) fn for_computation_operands(offset: usize, count: usize) -> Self {
        Self {
            immutable: (offset..offset + count).map(Some).collect(),
            storage: Vec::new(),
            primitive_storage: Vec::new(),
            structural_parameters: Vec::new(),
            structural_locals: Vec::new(),
            local_cases: Vec::new(),
            structural_fields: Vec::new(),
            structural_cases: Vec::new(),
        }
    }

    pub(super) fn new(parameters: usize) -> Self {
        Self {
            immutable: (0..parameters).map(Some).collect(),
            storage: Vec::new(),
            primitive_storage: Vec::new(),
            structural_parameters: Vec::new(),
            structural_locals: Vec::new(),
            local_cases: Vec::new(),
            structural_fields: Vec::new(),
            structural_cases: Vec::new(),
        }
    }

    pub(super) fn with_structural_parameters(
        mut self,
        parameters: &[(u32, StructuralParameterDeclaration)],
    ) -> Self {
        self.structural_parameters = parameters.to_vec();
        self.structural_fields.clear();
        self.structural_cases.clear();
        self
    }

    /// Register initialized primitive places supplied by the enclosing Unit producer.
    /// Reads use these places even if an earlier SSA storage row still exists.
    pub(super) fn with_primitive_storage(
        mut self,
        storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    ) -> Self {
        self.primitive_storage = storage.to_vec();
        self
    }

    /// Retain places established by the ordinary structural operation sequence.
    pub(super) fn with_structural_locals(
        mut self,
        locals: &[(symbols::SymbolHandle, StructuralArgument)],
    ) -> Self {
        self.structural_locals = locals.to_vec();
        self
    }

    /// Resolve an already source-validated whole primitive borrow without
    /// materializing its contents as an immutable scalar argument.
    pub(super) fn primitive_borrow(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        scalar_type: ScalarType,
    ) -> Result<StructuralArgument, LoweringError> {
        let access = match argument.access {
            checked_trees::CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
            checked_trees::CheckedStructuralAccess::MutableBorrow => {
                StructuralAccess::MutableBorrow
            }
            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
                StructuralAccess::WriteOnlyBorrow
            }
            checked_trees::CheckedStructuralAccess::Owned => {
                return unsupported("computed primitive argument cannot transfer ownership");
            }
        };
        if !argument.path.is_empty() {
            return unsupported("computed primitive argument requires a whole referent");
        }
        let place = match argument.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } => {
                primitive_storage_place(&self.primitive_storage, symbol, scalar_type)?
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } => {
                let (_, parameter) = self
                    .structural_parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "computed primitive borrow has no enclosing parameter",
                    ))?;
                if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !matches!(
                        (parameter.access, access),
                        (
                            StructuralAccess::SharedBorrow,
                            StructuralAccess::SharedBorrow
                        ) | (
                            StructuralAccess::MutableBorrow,
                            StructuralAccess::SharedBorrow
                                | StructuralAccess::MutableBorrow
                                | StructuralAccess::WriteOnlyBorrow
                        ) | (
                            StructuralAccess::WriteOnlyBorrow,
                            StructuralAccess::WriteOnlyBorrow
                        )
                    )
                {
                    return unsupported("computed primitive borrow widens parameter custody");
                }
                parameter.place
            }
            _ => return unsupported("computed primitive borrow has no supported source place"),
        };
        Ok(StructuralArgument {
            place,
            path: Vec::new(),
            access,
        })
    }

    /// Resolve a source-validated whole owned parameter without inventing a loan.
    pub(super) fn owned_argument(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            argument.source
        {
            let mut locals = self.structural_locals.iter().filter(|row| row.0 == symbol);
            let (_, source) = locals.next().ok_or(LoweringError::Unsupported(
                "computed owned array operand lost its established local",
            ))?;
            if !symbol.is_valid()
                || locals.next().is_some()
                || !argument.path.is_empty()
                || argument.access != checked_trees::CheckedStructuralAccess::Owned
                || !source.path.is_empty()
                || source.access != StructuralAccess::Owned
            {
                return unsupported("computed owned array operand changes its source custody");
            }
            return Ok(source.clone());
        }
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            argument.source
        else {
            return unsupported("computed owned operand requires an existing parameter");
        };
        let (_, parameter) = self
            .structural_parameters
            .get(parameter_index as usize)
            .ok_or(LoweringError::Unsupported(
                "computed owned operand lost its source parameter",
            ))?;
        if !argument.path.is_empty()
            || argument.access != checked_trees::CheckedStructuralAccess::Owned
            || parameter.access != StructuralAccess::Owned
            || !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            )
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return unsupported("computed owned operand changes its source custody");
        }
        Ok(StructuralArgument {
            place: parameter.place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        })
    }

    pub(super) fn initialize_parameter(
        &mut self,
        symbol: symbols::SymbolHandle,
        scalar_type: ScalarType,
        position: usize,
    ) -> Result<(), LoweringError> {
        if self.immutable_position(position)? != position {
            return unsupported("mutable parameter storage has no exact entry operand");
        }
        self.append(
            checked_trees::CheckedScalarBindingDestination::StorageInitialize { symbol },
            scalar_type,
            position,
        )?;
        // Keep ordinal slots for subsequent immutable locals, but never let
        // a mutable formal be read through the immutable entry-value path.
        self.immutable[position] = None;
        Ok(())
    }

    pub(super) fn append(
        &mut self,
        destination: checked_trees::CheckedScalarBindingDestination,
        scalar_type: ScalarType,
        position: usize,
    ) -> Result<(), LoweringError> {
        use checked_trees::CheckedScalarBindingDestination;
        match destination {
            CheckedScalarBindingDestination::Immutable => self.immutable.push(Some(position)),
            CheckedScalarBindingDestination::StorageInitialize { symbol } => {
                if !symbol.is_valid() || self.storage.iter().any(|row| row.0 == symbol) {
                    return unsupported(
                        "scalar storage initialization identity is missing or duplicated",
                    );
                }
                self.storage.push((symbol, scalar_type, position));
            }
            CheckedScalarBindingDestination::StorageAssign { symbol } => {
                let row = self.storage.iter_mut().find(|row| row.0 == symbol).ok_or(
                    LoweringError::Unsupported(
                        "scalar storage assignment has no initialized destination",
                    ),
                )?;
                if row.1 != scalar_type {
                    return unsupported("scalar storage assignment changes its declared type");
                }
                row.2 = position;
            }
        }
        Ok(())
    }

    fn storage_position(
        &self,
        symbol: symbols::SymbolHandle,
        scalar_type: ScalarType,
    ) -> Result<usize, LoweringError> {
        self.storage
            .iter()
            .find(|row| row.0 == symbol && row.1 == scalar_type)
            .map(|row| row.2)
            .ok_or(LoweringError::Unsupported(
                "scalar storage read has no initialized value of its declared type",
            ))
    }

    fn immutable_position(&self, position: usize) -> Result<usize, LoweringError> {
        self.immutable
            .get(position)
            .copied()
            .flatten()
            .ok_or(LoweringError::Unsupported(
                "scalar immutable operand is outside the established namespace",
            ))
    }

    fn scalar(&self, expression: &mut CheckedScalarExpression) -> Result<(), LoweringError> {
        match expression {
            CheckedScalarExpression::StructuralParameterIndexedRead { index, .. } => {
                self.scalar(index)?
            }
            CheckedScalarExpression::Parameter { position, .. }
            | CheckedScalarExpression::Local { position, .. } => {
                *position = self.immutable_position(*position)?
            }
            CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            } => {
                if self.primitive_storage.iter().any(|row| row.0 == *symbol) {
                    // Retain the read occurrence for emission, not an entry snapshot.
                    return Ok(());
                }
                *expression = CheckedScalarExpression::Local {
                    position: self
                        .storage_position(*symbol, terminal_scalar_type(*primitive_type)?)?,
                    primitive_type: *primitive_type,
                };
            }
            CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                self.scalar(left)?;
                self.scalar(right)?;
            }
            CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
            | CheckedScalarExpression::IntegerWiden { operand, .. }
            | CheckedScalarExpression::IntegerWrappingCast { operand, .. }
            | CheckedScalarExpression::IntegerExactCast { operand, .. } => self.scalar(operand)?,
            CheckedScalarExpression::IntegerTrappingCast { .. } => {
                return Err(LoweringError::Unsupported(
                    "checked trapping conversion requires runtime policy realization",
                ));
            }
            CheckedScalarExpression::Boolean(expression) => self.boolean(expression)?,
            CheckedScalarExpression::IntegerLiteral { .. }
            | CheckedScalarExpression::StructuralParameterByteLength { .. }
            | CheckedScalarExpression::IeeeFloatLiteral { .. }
            | CheckedScalarExpression::StructuralParameterField { .. } => {}
        }
        Ok(())
    }

    fn boolean(&self, expression: &mut CheckedBooleanExpression) -> Result<(), LoweringError> {
        match expression {
            CheckedBooleanExpression::Parameter { position }
            | CheckedBooleanExpression::Local { position } => {
                *position = self.immutable_position(*position)?
            }
            CheckedBooleanExpression::StorageRead { symbol } => {
                if self.primitive_storage.iter().any(|row| row.0 == *symbol) {
                    return Ok(());
                }
                *expression = CheckedBooleanExpression::Local {
                    position: self.storage_position(*symbol, ScalarType::Boolean)?,
                }
            }
            CheckedBooleanExpression::Not(operand) => self.boolean(operand)?,
            CheckedBooleanExpression::Equal { left, right }
            | CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right } => {
                self.boolean(left)?;
                self.boolean(right)?;
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                self.scalar(left)?;
                self.scalar(right)?;
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::PayloadlessSumEqual { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => {}
        }
        Ok(())
    }

    pub(super) fn expression_at(
        &self,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let (binding, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement, role)
            .ok_or(LoweringError::Unsupported(
                "scalar computation needs one checked expression and one source binding",
            ))?;
        let expression = self.expression(expression)?;
        source_custody::validate_pure(checked, binding, expression.scalar_type())?;
        Ok(expression)
    }

    pub(super) fn expression(
        &self,
        expression: &CheckedScalarExpression,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let mut expression = expression.clone();
        self.scalar(&mut expression)?;
        crate::scalar_graph_lowering::lower_checked_scalar_expression_with_parameters(
            &expression,
            &self.structural_parameters,
            &self.structural_fields,
            &self.structural_cases,
            &self.primitive_storage,
        )
    }
}

pub(super) fn primitive_storage_place(
    storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    symbol: symbols::SymbolHandle,
    scalar_type: ScalarType,
) -> Result<PlaceId, LoweringError> {
    let mut matching = storage.iter().filter(|row| row.0 == symbol);
    let Some((_, place, declared_type)) = matching.next() else {
        return unsupported("primitive storage read has no initialized place");
    };
    if !symbol.is_valid() || matching.next().is_some() {
        return unsupported("primitive storage read identity is missing or duplicated");
    }
    if *declared_type != scalar_type {
        return unsupported("primitive storage read changes its declared type");
    }
    Ok(*place)
}
