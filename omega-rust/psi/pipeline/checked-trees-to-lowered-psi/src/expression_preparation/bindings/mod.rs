//! Resolve checked-local storage against its current SSA value or primitive place.
use super::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    LoweringError, PlaceId, ScalarType, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::source_custody;

pub(crate) mod structural_cases;
pub(crate) mod structural_fields;
pub(crate) mod structural_paths;
#[cfg(test)]
mod tests;
pub(crate) use structural_fields::StructuralScalarFieldBinding;

#[derive(Clone)]
pub(crate) struct ScalarBindings {
    immutable: Vec<Option<usize>>,
    storage: Vec<(symbols::SymbolHandle, ScalarType, usize)>,
    primitive_storage: Vec<(symbols::SymbolHandle, PlaceId, ScalarType)>,
    /// Authored parameter positions stay separate from dense Terminal positions.
    structural_parameters: Vec<(u32, StructuralParameterDeclaration)>,
    structural_locals: Vec<(symbols::SymbolHandle, StructuralArgument)>,
    local_cases: Vec<structural_cases::LocalCaseBinding>,
    structural_fields: Vec<StructuralScalarFieldBinding>,
    structural_cases: Vec<structural_cases::StructuralCaseBinding>,
    /// Whole element-view parameters resolved to their scalar element type.
    /// Only primitive-element views join: `&[u8]` stays on the byte path.
    element_views: std::collections::BTreeMap<StructuralTypeId, ScalarType>,
}

impl ScalarBindings {
    /// Retire the source name only after its whole value has moved to its consumer.
    pub(crate) fn retire_structural_local(
        &mut self,
        symbol: symbols::SymbolHandle,
        place: PlaceId,
    ) -> Result<(), LoweringError> {
        let position = self
            .structural_locals
            .iter()
            .position(|(candidate, argument)| {
                *candidate == symbol
                    && argument.place == place
                    && argument.access == StructuralAccess::Owned
                    && argument.path.is_empty()
            })
            .ok_or(LoweringError::Unsupported(
                "moved local lost its exact source home",
            ))?;
        self.structural_locals.remove(position);
        Ok(())
    }

    /// Make the completed whole result visible to subsequent computations.
    pub(crate) fn establish_structural_local(
        &mut self,
        symbol: symbols::SymbolHandle,
        place: PlaceId,
    ) -> Result<(), LoweringError> {
        if !symbol.is_valid()
            || self
                .structural_locals
                .iter()
                .any(|(candidate, _)| *candidate == symbol)
        {
            return unsupported("structural local has invalid or repeated identity");
        }
        self.structural_locals.push((
            symbol,
            StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        ));
        Ok(())
    }

    /// Borrow the established referent, not a copy of its scalar field values.
    /// Source replay checks the declaration and loan occurrence; this join binds
    /// that source to the current activation's exact local or parameter place.
    /// `parameters` is the authored signature list: `StructuralLocal` names a
    /// live local or an authored parameter directly by symbol; a receiver place
    /// (`self.member`) roots at the machine's own declared symbol and resolves
    /// to the `is_self` parameter.
    pub(crate) fn shared_structural_argument(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        machine: &checked_trees::machine::Machine,
        parameters: &[checked_trees::signature::StateParameter],
    ) -> Result<StructuralArgument, LoweringError> {
        if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
            return unsupported("computed shared argument changes its access");
        }
        let place = match argument.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                let mut locals = self.structural_locals.iter().filter(|row| row.0 == symbol);
                match locals.next() {
                    Some(source) => {
                        // An owned local lends its whole place; a `&T` local
                        // is itself a shared-borrow join result, so borrowing
                        // through it reuses that same shared custody rather
                        // than fabricating ownership.
                        if !symbol.is_valid()
                            || locals.next().is_some()
                            || !matches!(
                                source.1.access,
                                StructuralAccess::Owned | StructuralAccess::SharedBorrow
                            )
                            || !source.1.path.is_empty()
                        {
                            return unsupported(
                                "computed shared argument changes its local custody",
                            );
                        }
                        source.1.place
                    }
                    None => {
                        let position = parameters
                            .iter()
                            .position(|parameter| parameter.symbol == symbol)
                            .or_else(|| {
                                (symbol == machine.symbol).then(|| {
                                    parameters.iter().position(|parameter| parameter.is_self)
                                })?
                            })
                            .ok_or(LoweringError::Unsupported(
                                "computed shared argument lost its established local",
                            ))?;
                        let (_, source) = self
                            .structural_parameters
                            .iter()
                            .find(|(place_position, _)| *place_position as usize == position)
                            .ok_or(LoweringError::Unsupported(
                                "computed shared argument lost its parameter",
                            ))?;
                        if !matches!(
                            source.access,
                            StructuralAccess::Owned
                                | StructuralAccess::SharedBorrow
                                | StructuralAccess::MutableBorrow
                        ) || !source.qualifications.is_empty()
                            || !source.projected_qualifications.is_empty()
                        {
                            return unsupported(
                                "computed shared argument widens its parameter custody",
                            );
                        }
                        source.place
                    }
                }
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } => {
                let (_, source) = self
                    .structural_parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "computed shared argument lost its parameter",
                    ))?;
                if !matches!(
                    source.access,
                    StructuralAccess::Owned
                        | StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                ) || !source.qualifications.is_empty()
                    || !source.projected_qualifications.is_empty()
                {
                    return unsupported("computed shared argument widens its parameter custody");
                }
                source.place
            }
            _ => return unsupported("computed shared argument has no established source"),
        };
        Ok(StructuralArgument {
            place,
            path: crate::expression_preparation::bindings::structural_paths::lower_structural_path(
                &argument.path,
            ),
            access: StructuralAccess::SharedBorrow,
        })
    }

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

    /// A parameter-field case observation resolves through the dense
    /// structural parameter index, then delegates to the shared
    /// case-binding resolver for the retained path and selected case.
    pub(crate) fn parameter_case_observation(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        case: &str,
    ) -> Result<
        (
            PlaceId,
            Vec<terminal_psi::StructuralPathSegment>,
            semantic_vocabulary::StructuralCaseId,
        ),
        LoweringError,
    > {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            argument.source
        else {
            return unsupported("case observation requires an exact parameter source");
        };
        if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
            return unsupported("case observation changed its parameter custody");
        }
        let (parameter_position, _) = self
            .structural_parameters
            .get(usize::try_from(parameter_index).map_err(|_| {
                LoweringError::Unsupported("case observation parameter index exceeds the host type")
            })?)
            .ok_or(LoweringError::Unsupported(
                "case observation lost its parameter source",
            ))?;
        let path = argument
            .path
            .iter()
            .map(|segment| match segment {
                checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => Some(
                    checked_trees::CheckedStructuralPredicatePathSegment::Field(identity.clone()),
                ),
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    Some(checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(*index))
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(LoweringError::Unsupported(
                "case observation has a non-field parameter path",
            ))?;
        structural_cases::resolve(
            &self.structural_cases,
            &checked_trees::CheckedStructuralParameterField {
                parameter_position: *parameter_position,
                path,
            },
            case,
        )
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
        self.element_views = self
            .structural_parameters
            .iter()
            .filter_map(|(_, parameter)| {
                let shape = types
                    .iter()
                    .find(|declaration| declaration.id == parameter.structural_type)?;
                let terminal_psi::StructuralTypeShape::ElementView { element } = shape.shape else {
                    return None;
                };
                let element = types.iter().find(|declaration| declaration.id == element)?;
                let terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) = element.shape
                else {
                    return None;
                };
                Some((parameter.structural_type, scalar_type))
            })
            .collect();
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

    pub(crate) fn for_computation_operands(offset: usize, count: usize) -> Self {
        Self {
            immutable: (offset..offset + count).map(Some).collect(),
            storage: Vec::new(),
            primitive_storage: Vec::new(),
            structural_parameters: Vec::new(),
            structural_locals: Vec::new(),
            local_cases: Vec::new(),
            structural_fields: Vec::new(),
            structural_cases: Vec::new(),
            element_views: std::collections::BTreeMap::new(),
        }
    }

    pub(crate) fn new(parameters: usize) -> Self {
        Self {
            immutable: (0..parameters).map(Some).collect(),
            storage: Vec::new(),
            primitive_storage: Vec::new(),
            structural_parameters: Vec::new(),
            structural_locals: Vec::new(),
            local_cases: Vec::new(),
            structural_fields: Vec::new(),
            structural_cases: Vec::new(),
            element_views: std::collections::BTreeMap::new(),
        }
    }

    pub(crate) fn with_structural_parameters(
        mut self,
        parameters: &[(u32, StructuralParameterDeclaration)],
    ) -> Self {
        self.structural_parameters = parameters.to_vec();
        self.structural_fields.clear();
        self.structural_cases.clear();
        self.element_views.clear();
        self
    }

    /// Register initialized primitive places supplied by the enclosing Unit producer.
    /// Reads use these places even if an earlier SSA storage row still exists.
    pub(crate) fn with_primitive_storage(
        mut self,
        storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    ) -> Self {
        self.primitive_storage = storage.to_vec();
        self
    }

    /// Retain places established by the ordinary structural operation sequence.
    pub(crate) fn with_structural_locals(
        mut self,
        locals: &[(symbols::SymbolHandle, StructuralArgument)],
    ) -> Self {
        self.structural_locals = locals.to_vec();
        self
    }

    /// Resolve an already source-validated whole primitive borrow without
    /// materializing its contents as an immutable scalar argument.
    pub(crate) fn primitive_borrow(
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
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                // A `&T` local is itself a shared-borrow join result: the call
                // loans the referent's exact established place onward under
                // the same custody rather than copying the scalar out of it.
                let mut locals = self.structural_locals.iter().filter(|row| row.0 == symbol);
                let (_, source) = locals.next().ok_or(LoweringError::Unsupported(
                    "computed primitive borrow lost its established local",
                ))?;
                if !symbol.is_valid()
                    || locals.next().is_some()
                    || source.access != StructuralAccess::SharedBorrow
                    || !source.path.is_empty()
                    || access != StructuralAccess::SharedBorrow
                {
                    return unsupported("computed primitive borrow changes its local custody");
                }
                source.place
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
    pub(crate) fn owned_argument(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            argument.source
        {
            let mut locals = self.structural_locals.iter().filter(|row| row.0 == symbol);
            let (_, source) = locals.next().ok_or(LoweringError::Unsupported(
                "computed owned structural operand lost its established local",
            ))?;
            if !symbol.is_valid()
                || locals.next().is_some()
                || !argument.path.is_empty()
                || argument.access != checked_trees::CheckedStructuralAccess::Owned
                || !source.path.is_empty()
                || source.access != StructuralAccess::Owned
            {
                return unsupported("computed owned structural operand changes its source custody");
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

    pub(crate) fn initialize_parameter(
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

    pub(crate) fn append(
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
            | CheckedScalarExpression::IntegerSaturatingCast { operand, .. }
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
            | CheckedScalarExpression::ErasedParameter { .. }
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
            CheckedBooleanExpression::IntegerComparison { left, right, .. }
            | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
                self.scalar(left)?;
                self.scalar(right)?;
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::PayloadlessSumEqual { .. }
            | CheckedBooleanExpression::ErasedParameter { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => {}
        }
        Ok(())
    }

    pub(crate) fn expression_at(
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
            .ok_or_else(|| {
                LoweringError::Unsupported(
                    "scalar computation needs one checked expression and one source binding",
                )
            })?;
        let expression = self.expression(expression)?;
        source_custody::validate_pure(checked, binding, expression.scalar_type())?;
        Ok(expression)
    }

    pub(crate) fn expression(
        &self,
        expression: &CheckedScalarExpression,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let mut expression = expression.clone();
        self.scalar(&mut expression)?;
        crate::expression_preparation::prepare_expression::lower_checked_scalar_expression_with_parameters(
            &expression,
            &self.structural_parameters,
            &self.structural_fields,
            &self.structural_cases,
            &self.primitive_storage,
            &self.element_views,
        )
    }
}

pub(crate) fn primitive_storage_place(
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
